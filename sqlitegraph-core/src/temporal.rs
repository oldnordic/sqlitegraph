//! Temporal topology analysis over the MVCC version chain.
//!
//! Sweeps across retained [`VersionedSnapshot`](crate::mvcc::VersionedSnapshot)s
//! and tracks how the strongly-connected-component (SCC) landscape evolves. The
//! result is a persistence barcode: each component has a `(birth_version,
//! death_version)` lifetime, forming the discrete analogue of 0-dimensional
//! persistent homology with version number as the filtration parameter.
//!
//! This is the sqlitegraph-native port of geographdb's
//! `temporal_persistence_sweep` + `compute_temporal_barcode` (originally in
//! `geographdb-core/src/algorithms/persistence.rs`). The geographdb version
//! operates on 4D graphs with spatial (sphere) and temporal-window filters;
//! this version operates on the in-memory adjacency maps stored in the MVCC
//! version chain — no spatial axis, no SQLite I/O.
//!
//! # Background: what is an SCC?
//!
//! A **strongly connected component** (SCC) is a maximal group of nodes where
//! every node can reach every other node following directed edges. "Maximal"
//! means you can't add another node and keep the property. Tarjan's algorithm
//! finds all SCCs in one pass (O(V + E)).
//!
//! As the graph evolves across versions, components form (birth), grow, shrink,
//! merge, and disappear (death). The barcode records these lifetimes — a
//! stable component that survives many versions is a long bar; a transient one
//! that flickers in and out is a short bar.

use std::collections::{HashMap, HashSet};

use ahash::AHashSet;

use crate::mvcc::{SnapshotState, VersionedSnapshot};

/// One measurement point in the temporal persistence sweep.
#[derive(Debug, Clone)]
pub struct TemporalPersistencePoint {
    /// Version number at this measurement.
    pub version: u64,
    /// Total nodes in this snapshot.
    pub active: usize,
    /// Number of strongly connected components.
    pub n_components: usize,
    /// Size of the largest SCC.
    pub largest_size: usize,
    /// `largest_size / active`, or 0.0 when `active == 0`.
    pub fraction_largest: f32,
}

/// One bar in the persistence barcode.
///
/// Represents a connected component that was first observed at `birth_version`
/// and last observed at `death_version` (or still alive at sweep end when
/// `death_version` is `None`).
#[derive(Debug, Clone, PartialEq)]
pub struct TemporalBarcode {
    /// Version where this component first appeared.
    pub birth_version: u64,
    /// Version where this component was last seen, or `None` if it survived to
    /// the end of the sweep.
    pub death_version: Option<u64>,
    /// Largest size this component reached across its lifetime.
    pub peak_size: usize,
}

/// Compute the strongly-connected components of a [`SnapshotState`].
///
/// Tarjan's single-pass algorithm operating directly on the in-memory
/// adjacency map (no SQLite I/O). Returns a vector of components, each a set
/// of node IDs.
///
/// This is the in-memory analogue of
/// [`strongly_connected_components`](crate::algo::strongly_connected_components)
/// (which operates on the live `SqliteGraph` + SQLite). Used here so the sweep
/// reads each version's topology without touching the database.
pub fn strongly_connected_components_snapshot(state: &SnapshotState) -> Vec<HashSet<i64>> {
    let nodes: Vec<i64> = state.outgoing.keys().copied().collect();
    if nodes.is_empty() {
        return Vec::new();
    }

    let mut index_counter: i64 = 0;
    let mut stack: Vec<i64> = Vec::new();
    let mut on_stack: AHashSet<i64> = AHashSet::new();
    let mut indices: HashMap<i64, i64> = HashMap::new();
    let mut lowlink: HashMap<i64, i64> = HashMap::new();
    let mut components: Vec<HashSet<i64>> = Vec::new();

    fn strongconnect(
        v: i64,
        state: &SnapshotState,
        index_counter: &mut i64,
        stack: &mut Vec<i64>,
        on_stack: &mut AHashSet<i64>,
        indices: &mut HashMap<i64, i64>,
        lowlink: &mut HashMap<i64, i64>,
        components: &mut Vec<HashSet<i64>>,
    ) {
        indices.insert(v, *index_counter);
        lowlink.insert(v, *index_counter);
        *index_counter += 1;
        stack.push(v);
        on_stack.insert(v);

        if let Some(neighbors) = state.get_outgoing(v) {
            for &w in neighbors {
                if !indices.contains_key(&w) {
                    strongconnect(
                        w,
                        state,
                        index_counter,
                        stack,
                        on_stack,
                        indices,
                        lowlink,
                        components,
                    );
                    let wl = *lowlink.get(&w).unwrap();
                    let vl = *lowlink.get(&v).unwrap();
                    lowlink.insert(v, vl.min(wl));
                } else if on_stack.contains(&w) {
                    let wi = *indices.get(&w).unwrap();
                    let vl = *lowlink.get(&v).unwrap();
                    lowlink.insert(v, vl.min(wi));
                }
            }
        }

        // Root of an SCC?
        if lowlink.get(&v) == indices.get(&v) {
            let mut component = HashSet::new();
            loop {
                let w = stack.pop().expect("stack non-empty for SCC root");
                on_stack.remove(&w);
                component.insert(w);
                if w == v {
                    break;
                }
            }
            components.push(component);
        }
    }

    for &node in &nodes {
        if !indices.contains_key(&node) {
            strongconnect(
                node,
                state,
                &mut index_counter,
                &mut stack,
                &mut on_stack,
                &mut indices,
                &mut lowlink,
                &mut components,
            );
        }
    }

    components
}

/// Sweep across a sequence of [`VersionedSnapshot`]s, computing the SCC
/// landscape at each version.
///
/// The `versions` must be ordered by increasing version number (as produced by
/// [`SnapshotManager::versions`](crate::mvcc::SnapshotManager::versions)).
///
/// Returns one [`TemporalPersistencePoint`] per version.
pub fn temporal_persistence_sweep(versions: &[VersionedSnapshot]) -> Vec<TemporalPersistencePoint> {
    versions
        .iter()
        .map(|vs| {
            let sccs = strongly_connected_components_snapshot(&vs.state);
            let active: usize = sccs.iter().map(|c| c.len()).sum();
            let n_components = sccs.len();
            let largest_size = sccs.iter().map(|c| c.len()).max().unwrap_or(0);
            let fraction_largest = if active == 0 {
                0.0
            } else {
                largest_size as f32 / active as f32
            };
            TemporalPersistencePoint {
                version: vs.version,
                active,
                n_components,
                largest_size,
                fraction_largest,
            }
        })
        .collect()
}

/// Derive a persistence barcode from a sweep.
///
/// Tracks the component count across versions. Each time the count rises, new
/// components are "born"; each time it falls, existing ones "die" (merge or
/// disappear). Returns one [`TemporalBarcode`] per birth event.
///
/// This is the ported barcode kernel from geographdb
/// (`compute_temporal_barcode`), unchanged in logic — only the field names
/// reflect `version` instead of `t`. The `points` must be ordered by increasing
/// `version`.
pub fn compute_temporal_barcode(points: &[TemporalPersistencePoint]) -> Vec<TemporalBarcode> {
    let mut open: Vec<(u64, usize)> = Vec::new(); // (birth_version, peak_size)
    let mut bars: Vec<TemporalBarcode> = Vec::new();

    let mut prev_count = 0usize;

    for pt in points {
        let curr = pt.n_components;

        if curr > prev_count {
            for _ in 0..(curr - prev_count) {
                open.push((pt.version, pt.largest_size));
            }
        }

        for ob in &mut open {
            ob.1 = ob.1.max(pt.largest_size);
        }

        if curr < prev_count {
            let died = prev_count - curr;
            let close_from = open.len().saturating_sub(died);
            for (birth, peak) in open.drain(close_from..) {
                bars.push(TemporalBarcode {
                    birth_version: birth,
                    death_version: Some(pt.version),
                    peak_size: peak,
                });
            }
        }

        prev_count = curr;
    }

    for (birth, peak) in open {
        bars.push(TemporalBarcode {
            birth_version: birth,
            death_version: None,
            peak_size: peak,
        });
    }

    bars.sort_by_key(|b| b.birth_version);
    bars
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mvcc::{SnapshotManager, SnapshotState};
    use std::collections::HashMap;

    fn state_from(outgoing: &[(&i64, &[i64])]) -> SnapshotState {
        let mut out: HashMap<i64, Vec<i64>> = HashMap::new();
        let mut inc: HashMap<i64, Vec<i64>> = HashMap::new();
        for &(src, dsts) in outgoing {
            for &d in dsts {
                out.entry(*src).or_default().push(d);
                inc.entry(d).or_default().push(*src);
            }
        }
        // Ensure every node is a key in outgoing (even if no out-edges).
        for &(src, _) in outgoing {
            out.entry(*src).or_default();
        }
        SnapshotState::new(&out, &inc)
    }

    // ── strongly_connected_components_snapshot ─────────────────────────────

    #[test]
    fn scc_empty_state_has_zero_components() {
        let state = SnapshotState::new(&HashMap::new(), &HashMap::new());
        assert!(strongly_connected_components_snapshot(&state).is_empty());
    }

    #[test]
    fn scc_isolated_nodes_are_all_trivial() {
        let state = state_from(&[(&1, &[]), (&2, &[]), (&3, &[])]);
        let sccs = strongly_connected_components_snapshot(&state);
        assert_eq!(sccs.len(), 3);
        for c in &sccs {
            assert_eq!(c.len(), 1);
        }
    }

    #[test]
    fn scc_directed_cycle_is_one_component() {
        // 1 → 2 → 3 → 1 forms a single SCC.
        let state = state_from(&[(&1, &[2]), (&2, &[3]), (&3, &[1])]);
        let sccs = strongly_connected_components_snapshot(&state);
        assert_eq!(sccs.len(), 1);
        assert_eq!(sccs[0].len(), 3);
    }

    #[test]
    fn scc_dag_all_trivial() {
        // 1 → 2 → 3 (no cycles) → 3 trivial SCCs.
        let state = state_from(&[(&1, &[2]), (&2, &[3])]);
        let sccs = strongly_connected_components_snapshot(&state);
        assert_eq!(sccs.len(), 3);
    }

    // ── temporal_persistence_sweep ─────────────────────────────────────────

    #[test]
    fn sweep_empty_versions_gives_empty_points() {
        let pts = temporal_persistence_sweep(&[]);
        assert!(pts.is_empty());
    }

    #[test]
    fn sweep_single_cycle_version() {
        let mgr = SnapshotManager::new();
        let state = state_from(&[(&1, &[2]), (&2, &[3]), (&3, &[1])]);
        let vs = VersionedSnapshot {
            version: 1,
            created_at: std::time::SystemTime::now(),
            state: std::sync::Arc::new(state),
        };
        let _ = mgr; // just to use the type
        let pts = temporal_persistence_sweep(&[vs]);
        assert_eq!(pts.len(), 1);
        assert_eq!(pts[0].n_components, 1);
        assert_eq!(pts[0].largest_size, 3);
        assert!((pts[0].fraction_largest - 1.0).abs() < 1e-6);
    }

    #[test]
    fn sweep_tracks_component_evolution() {
        // v1: 3 isolated nodes → 3 components
        let v1 = VersionedSnapshot {
            version: 1,
            created_at: std::time::SystemTime::now(),
            state: std::sync::Arc::new(state_from(&[(&1, &[]), (&2, &[]), (&3, &[])])),
        };
        // v2: form a cycle → 1 component
        let v2 = VersionedSnapshot {
            version: 2,
            created_at: std::time::SystemTime::now(),
            state: std::sync::Arc::new(state_from(&[(&1, &[2]), (&2, &[3]), (&3, &[1])])),
        };
        let pts = temporal_persistence_sweep(&[v1, v2]);
        assert_eq!(pts.len(), 2);
        assert_eq!(pts[0].n_components, 3);
        assert_eq!(pts[1].n_components, 1);
    }

    // ── compute_temporal_barcode ───────────────────────────────────────────

    #[test]
    fn barcode_birth_and_death() {
        let points = vec![
            TemporalPersistencePoint {
                version: 0,
                active: 0,
                n_components: 0,
                largest_size: 0,
                fraction_largest: 0.0,
            },
            TemporalPersistencePoint {
                version: 1,
                active: 3,
                n_components: 1,
                largest_size: 3,
                fraction_largest: 1.0,
            },
            TemporalPersistencePoint {
                version: 2,
                active: 3,
                n_components: 1,
                largest_size: 3,
                fraction_largest: 1.0,
            },
            TemporalPersistencePoint {
                version: 3,
                active: 0,
                n_components: 0,
                largest_size: 0,
                fraction_largest: 0.0,
            },
        ];
        let bars = compute_temporal_barcode(&points);
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].birth_version, 1);
        assert_eq!(bars[0].death_version, Some(3));
        assert_eq!(bars[0].peak_size, 3);
    }

    #[test]
    fn barcode_surviving_component_no_death() {
        let points = vec![
            TemporalPersistencePoint {
                version: 0,
                active: 0,
                n_components: 0,
                largest_size: 0,
                fraction_largest: 0.0,
            },
            TemporalPersistencePoint {
                version: 1,
                active: 2,
                n_components: 1,
                largest_size: 2,
                fraction_largest: 1.0,
            },
            TemporalPersistencePoint {
                version: 2,
                active: 2,
                n_components: 1,
                largest_size: 2,
                fraction_largest: 1.0,
            },
        ];
        let bars = compute_temporal_barcode(&points);
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].birth_version, 1);
        assert_eq!(bars[0].death_version, None);
    }

    #[test]
    fn barcode_end_to_end() {
        // End-to-end: sweep a version chain then barcode it.
        let v1 = VersionedSnapshot {
            version: 1,
            created_at: std::time::SystemTime::now(),
            state: std::sync::Arc::new(state_from(&[(&1, &[]), (&2, &[]), (&3, &[])])),
        };
        let v2 = VersionedSnapshot {
            version: 2,
            created_at: std::time::SystemTime::now(),
            state: std::sync::Arc::new(state_from(&[(&1, &[2]), (&2, &[3]), (&3, &[1])])),
        };
        let v3 = VersionedSnapshot {
            version: 3,
            created_at: std::time::SystemTime::now(),
            state: std::sync::Arc::new(state_from(&[(&1, &[]), (&2, &[]), (&3, &[])])),
        };
        let pts = temporal_persistence_sweep(&[v1, v2, v3]);
        let bars = compute_temporal_barcode(&pts);
        // Two die at v2 (3→1 merge), two are born at v3 (1→3 split),
        // one from v1 survives through v3. Alive = 3, dead = 2, total = 5.
        assert_eq!(bars.len(), 5);
        let alive = bars.iter().filter(|b| b.death_version.is_none()).count();
        assert_eq!(alive, 3);
    }
}
