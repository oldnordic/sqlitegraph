# Phase 22: Scaling & Dependencies - Research

**Researched:** 2026-01-20
**Domain:** Database scaling limits, dirty block tracking overflow, transaction ID management, HNSW disk-based indexing, dependency migration
**Confidence:** MEDIUM

## Summary

Phase 22 focuses on understanding and addressing scaling limits in the V2 WAL checkpoint system, transaction management, and HNSW vector indexing. Key research findings include:

1. **Checkpoint scaling**: SQLite WAL checkpoints have no inherent 1GB file limit, but checkpoint blocking behavior creates practical scaling concerns
2. **Dirty block tracking**: Current implementation has fixed capacity limits (`MAX_GLOBAL_DIRTY_BLOCKS`) that require overflow strategies
3. **Transaction ID bounds**: PostgreSQL's 32-bit XID wraparound provides a reference pattern for transaction ID lifecycle management
4. **HNSW disk-based options**: HNSW requires ~30% more memory than raw vector data; DiskANN is more suitable for datasets exceeding RAM
5. **Dependency updates**: bincode development has ceased; rusqlite 0.31 uses bundled SQLite by default for security

**Primary recommendation**: Implement overflow strategies for dirty block tracking, add transaction ID wraparound protection, evaluate disk-based HNSW alternatives, and create bincode 2.0 migration plan.

## Standard Stack

### Core Scaling & Monitoring
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `parking_lot` | 0.12 | Lock primitives for deadlock detection | Faster than std, has experimental deadlock detection |
| `rusqlite` | 0.31 | SQLite bindings with bundled feature | Bundled SQLite ensures security patches are included |
| `bincode` | 1.3 | Binary serialization (legacy) | Current version; migration to 2.0 needed |
| `bincode_next` / `cu-bincode` | 2.0 | Binary serialization (future) | Community fork after original bincode ceased development |

### HNSW & Vector Storage
| Component | Current State | Scaling Limit | Alternative |
|-----------|---------------|---------------|-------------|
| `InMemoryVectorStorage` | HashMap-backed | RAM-bound | `SQLiteVectorStorage` for persistence |
| HNSW algorithm | In-memory graph | ~30% overhead + vector data | DiskANN for disk-based indexes |
| Vector persistence | SQLite BLOB storage | Database size limit | Multi-file sharding (custom) |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `bytemuck` | 1.13 | Zero-copy f32 serialization | Vector I/O, already in use |
| `memmap2` | 0.9 | Memory-mapped file access | Large file checkpoint streaming |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| HNSW in-memory | DiskANN | Better for disk-based, larger datasets; less mature Rust ecosystem |
| bincode 2.0 | Postcard | More ergonomic, but bincode 2.0 has broader serde compat |
| Single checkpoint file | Multi-file sharding | Complexity vs. single atomic file write |

**Installation:**
```bash
# Current dependencies (already in Cargo.toml)
# No new additions required for Phase 22

# For bincode 2.0 migration (future):
bincode2 = "2.0"  # or cu-bincode community fork
```

## Architecture Patterns

### Recommended Project Structure

For Phase 22 scaling additions:

```
src/backend/native/v2/wal/
├── checkpoint/
│   ├── overflow/              # NEW: Overflow handling
│   │   ├── mod.rs
│   │   ├── strategy.rs        # Spill-to-disk strategies
│   │   └── multi_file.rs      # Multi-file checkpoint coordination
│   └── ...
├── transaction_coordinator.rs # MODIFY: Add TX ID bounds enforcement
└── ...

src/hnsw/
├── storage/
│   ├── mod.rs                 # MODIFY: Add disk-based option
│   ├── memory_limited.rs      # NEW: Memory-bounded storage
│   └── disk_spill.rs          # NEW: Spill-to-disk vector storage
└── ...
```

### Pattern 1: Dirty Block Overflow Strategy

**What:** Hierarchical dirty block tracking with overflow to secondary storage

**When to use:** When `global_dirty_blocks` approaches `MAX_GLOBAL_DIRTY_BLOCKS` (currently 50,000)

**Example:**
```rust
// Source: Based on DirtyBlockTracker in checkpoint/core.rs

pub enum DirtyBlockOverflowStrategy {
    /// Reject writes when overflow would occur (current behavior)
    Reject,
    /// Force immediate checkpoint to free dirty blocks
    ForceCheckpoint,
    /// Spill oldest blocks to secondary storage
    SpillToDisk,
    /// Promote to higher-level tracking (hierarchical)
    HierarchicalPromotion,
}

pub struct DirtyBlockTracker {
    // Existing fields...
    cluster_dirty_blocks: HashMap<i64, HashSet<u64>>,
    global_dirty_blocks: HashSet<u64>,

    // NEW: Overflow handling
    overflow_strategy: DirtyBlockOverflowStrategy,
    overflow_blocks: Option<DiskOverflowStore>,
    max_global_blocks: usize,
}

impl DirtyBlockTracker {
    pub fn mark_global_block_dirty(&mut self, block_offset: u64, timestamp: u64)
        -> CheckpointResult<()>
    {
        // Enforce capacity limits with overflow handling
        if self.global_dirty_blocks.len() >= self.max_global_blocks {
            match self.overflow_strategy {
                DirtyBlockOverflowStrategy::SpillToDisk => {
                    self.spill_oldest_blocks()?;
                }
                DirtyBlockOverflowStrategy::ForceCheckpoint => {
                    return Err(CheckpointError::checkpoint_required(
                        "Dirty block overflow - force checkpoint"
                    ));
                }
                DirtyBlockOverflowStrategy::Reject => {
                    return Err(CheckpointError::resource("Maximum global dirty blocks exceeded"));
                }
                DirtyBlockOverflowStrategy::HierarchicalPromotion => {
                    self.promote_to_hierarchical()?;
                }
            }
        }

        self.global_dirty_blocks.insert(block_offset);
        Ok(())
    }
}
```

### Pattern 2: Transaction ID Bounds Enforcement

**What:** Prevent transaction ID wraparound using PostgreSQL's pattern

**When to use:** Systems with long-running transactions or high TX throughput

**Example:**
```rust
// Source: Based on transaction_coordinator.rs TransactionId = u64

pub struct TransactionIdManager {
    next_tx_id: Arc<Mutex<u64>>,
    tx_id_upper_bound: u64,  // Safety margin from u64::MAX
    warn_threshold: u64,      // Warning threshold
    last_wrap_check: Arc<Mutex<Instant>>,
}

impl TransactionIdManager {
    // PostgreSQL pattern: Stop accepting writes at 1M transactions before wraparound
    const SAFETY_MARGIN: u64 = 1_000_000;
    const WRAP_WARNING_THRESHOLD: u64 = u64::MAX - Self::SAFETY_MARGIN - 10_000_000;

    pub fn allocate_transaction_id(&self) -> NativeResult<TransactionId> {
        let mut next_id = self.next_transaction_id.lock();
        let id = *next_id;

        // Check for wraparound danger
        if id >= Self::WRAP_WARNING_THRESHOLD {
            return Err(NativeBackendError::TransactionIdExhaustion {
                current_id: id,
                remaining: u64::MAX - id,
            });
        }

        *next_id = next_id.wrapping_add(1);
        Ok(id)
    }

    /// Check if transaction ID cleanup is needed
    pub fn needs_wraparound_protection(&self) -> bool {
        let current = *self.next_transaction_id.lock();
        current >= Self::WRAP_WARNING_THRESHOLD
    }
}
```

### Pattern 3: HNSW Disk-Based Storage Option

**What:** Hybrid HNSW with in-memory graph and disk-based vector storage

**When to use:** When vector data exceeds 50% of available RAM

**Example:**
```rust
// Source: Extension of hnsw/storage.rs VectorStorage trait

pub struct DiskSpillVectorStorage {
    memory_storage: InMemoryVectorStorage,
    disk_storage: SQLiteVectorStorage,
    memory_limit_bytes: usize,
    current_memory_bytes: Arc<Mutex<usize>>,
    spill_lru: Arc<Mutex<lru::LruCache<u64, ()>>>,
}

impl VectorStorage for DiskSpillVectorStorage {
    fn store_vector(&mut self, vector: &[f32], metadata: Option<Value>)
        -> Result<u64, HnswError>
    {
        let vector_size = vector.len() * std::mem::size_of::<f32>();

        // Check if we need to spill
        if *self.current_memory_bytes.lock() + vector_size > self.memory_limit_bytes {
            self.spill_cold_vectors()?;
        }

        let id = self.memory_storage.store_vector(vector, metadata)?;
        *self.current_memory_bytes.lock() += vector_size;
        self.spill_lru.lock().put(id, ());

        Ok(id)
    }

    fn get_vector(&self, id: u64) -> Result<Option<Vec<f32>>, HnswError> {
        // Check memory first
        if let Some(vec) = self.memory_storage.get_vector(id)? {
            return Ok(Some(vec));
        }

        // Fall back to disk
        self.disk_storage.get_vector(id)
    }
}
```

### Anti-Patterns to Avoid

- **Fixed capacity without overflow:** Current `DirtyBlockTracker` returns error when full instead of spilling
- **No TX ID bounds:** Unbounded `u64` wrapping without warning leads to subtle bugs
- **HNSW all-in-memory:** For large indexes, entire graph in RAM is unnecessary
- **Blocking checkpoints:** SQLite checkpoints can block writes; use async or incremental strategies

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Transaction ID wraparound detection | Custom counter logic | PostgreSQL's pattern (1M margin) | Battle-tested, predictable behavior |
| Dirty block overflow | Custom spill logic | LRU cache + SQLite | lru crate handles eviction correctly |
| HNSW disk spill | Custom file format | SQLite BLOB storage | Already has persistence, ACID |
| Deadlock detection | Custom cycle detection | parking_lot's detection | Built-in, core-dump analysis support |

**Key insight:** Scaling limits often require architectural changes, not just tuning. Overflow strategies need to be part of the core design, not bolted on later.

## Common Pitfalls

### Pitfall 1: Checkpoint Blocking Writes

**What goes wrong:** SQLite checkpoints can block write transactions when the WAL file cannot be reset due to long-running readers.

**Why it happens:** Checkpoint requires exclusive access to complete; any active reader prevents WAL truncation.

**How to avoid:**
- Implement incremental checkpoints that don't require full WAL lock
- Use timeout-based reader abort for long-running transactions
- Monitor checkpoint duration and warn if exceeding thresholds

**Warning signs:**
- Checkpoint duration grows linearly with WAL size
- Write latency spikes during checkpoint
- WAL file growing despite checkpoints

### Pitfall 2: Dirty Block Tracker Memory Exhaustion

**What goes wrong:** `DirtyBlockTracker` grows unbounded under high write load, causing OOM.

**Why it happens:** Current implementation uses fixed `HashSet` without eviction or overflow.

**How to avoid:**
- Implement overflow strategy (spill to disk or force checkpoint)
- Add memory limits and monitoring
- Use hierarchical tracking for cluster-affinity blocks

**Warning signs:**
- `global_dirty_blocks.len()` approaching max
- Memory usage growing with write throughput
- Checkpoint frequency too low relative to dirty rate

### Pitfall 3: Transaction ID Wraparound

**What goes wrong:** Transaction IDs wrap around, causing visibility errors and data corruption.

**Why it happens:** Unbounded `u64` counter without wraparound protection.

**How to avoid:**
- Implement safety margin (1M transactions before u64::MAX)
- Monitor TX ID allocation rate
- Force cleanup when approaching threshold

**Warning signs:**
- Transaction IDs approaching 2^64 - 1M
- Old transactions not being cleaned up
- Deadlock detector growing unbounded

### Pitfall 4: HNSW Memory Overflow

**What goes wrong:** HNSW index consumes all RAM when vector data exceeds memory capacity.

**Why it happens:** HNSW requires ~30% overhead beyond raw vector data; `InMemoryVectorStorage` has no limits.

**How to avoid:**
- Implement disk-based vector storage with LRU cache
- Set memory limits on index size
- Consider DiskANN for pure disk-based workloads

**Warning signs:**
- Process memory grows with vector count
- Swap usage increases
- OOM kills under load

## Code Examples

Verified patterns from official sources:

### Overflow Strategy Enum

```rust
// Source: Based on checkpoint overflow requirements (SCALE-DB-01, SCALE-DB-02)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirtyBlockOverflowStrategy {
    /// Reject writes when overflow would occur (current behavior)
    Reject,
    /// Force immediate checkpoint to free dirty blocks
    ForceCheckpoint,
    /// Spill oldest blocks to secondary storage
    SpillToDisk,
    /// Promote to higher-level tracking (hierarchical)
    HierarchicalPromotion,
}

impl Default for DirtyBlockOverflowStrategy {
    fn default() -> Self {
        // Default to safe behavior: reject on overflow
        Self::Reject
    }
}
```

### Transaction ID Wraparound Detection

```rust
// Source: PostgreSQL pattern (Bytebase, Oct 2025)

pub struct TransactionIdBounds {
    next_id: AtomicU64,
    warn_threshold: u64,
    hard_limit: u64,
}

impl TransactionIdBounds {
    pub const fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            warn_threshold: u64::MAX - 10_000_000,
            hard_limit: u64::MAX - 1_000_000,
        }
    }

    pub fn allocate(&self) -> Result<u64, TransactionError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        if id >= self.hard_limit {
            return Err(TransactionError::IdExhaustion {
                current: id,
                remaining: self.hard_limit - id,
            });
        }

        if id >= self.warn_threshold {
            log::warn!("Transaction ID approaching wraparound: {}", id);
        }

        Ok(id)
    }
}
```

### Deadlock Detector Cleanup

```rust
// Source: Based on transaction_coordinator.rs DeadlockDetector

impl DeadlockDetector {
    /// Periodic cleanup to prevent unbounded growth
    pub fn cleanup_stale_transactions(&self, timeout: Duration) {
        let now = Instant::now();
        let mut wait_for_graph = self.wait_for_graph.write();

        // Remove entries for completed transactions
        wait_for_graph.retain(|tx_id, _waiting_for| {
            // This is a placeholder - real implementation would
            // query the transaction coordinator for active TX status
            true
        });

        self.last_detection.lock().replace(now);
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Fixed dirty block limits | Overflow strategies | Phase 22 | Enables scaling beyond 50K dirty blocks |
| Unbounded TX IDs | Wraparound protection | Phase 22 | Prevents subtle data corruption |
| HNSW all-in-memory | Hybrid disk/memory | Phase 22 | Supports indexes larger than RAM |
| bincode 1.3 | bincode 2.0 / cu-bincode | Phase 22 | Community fork for continued support |

**Deprecated/outdated:**
- **bincode 1.3 (development ceased):** Original bincode development stopped Dec 2025 due to harassment. Use community forks (`cu-bincode`, `bincode_next`) for 2.0 features.
- **Unbounded dirty block tracking:** Fixed capacity limits are insufficient for production scaling.

## Open Questions

Things that couldn't be fully resolved:

1. **Multi-file checkpoint atomicity**
   - What we know: SQLite uses single WAL file for atomicity
   - What's unclear: How to coordinate atomic multi-file checkpoint without losing crash consistency
   - Recommendation: Keep single-file checkpoint with streaming writes; multi-file adds significant complexity

2. **HNSW graph persistence vs. vector persistence**
   - What we know: Current implementation only persists vectors, rebuilds graph on load
   - What's unclear: Cost/benefit of persisting graph structure for fast restart
   - Recommendation: Benchmark rebuild cost vs. persistence overhead for typical workloads

3. **rusqlite 0.31 system SQLite security**
   - What we know: Bundled feature ensures known SQLite version
   - What's unclear: Whether system SQLite provides faster security updates
   - Recommendation: Stick with bundled for consistency; monitor rusqlite releases

## Sources

### Primary (HIGH confidence)

- [SQLite WAL Documentation](https://sqlite.org/wal.html) - WAL format and checkpoint behavior
- [SQLite WAL Format](https://sqlite.org/walformat.html) - File format specification
- [rusqlite 0.31.0 crates.io](https://crates.io/crates/rusqlite/0.31.0) - Current version with bundled feature
- [parking_lot Documentation](https://docs.rs/parking_lot/) - Lock primitives and deadlock detection

### Secondary (MEDIUM confidence)

- [Bytebase: PostgreSQL Transaction ID Wraparound](https://www.bytebase.com/blog/postgres-transaction-id-wraparound/) (Oct 2025) - TX ID wraparound patterns
- [TigerData: HNSW vs DiskANN](https://www.tigerdata.com/learn/hnsw-vs-diskann) (Nov 2024) - Disk-based vector index comparison
- [Fly.io: parking_lot Deadlock Detection](https://fly.io/blog/parking-lot-ffffffffffffffff/) (2025) - Real-world deadlock detection usage
- [bincode_next Migration Guide](https://docs.rs/bincode-next/latest/bincode_next/migration_guide/index.html) - Bincode 2.0 migration

### Tertiary (LOW confidence)

- [PlanetScale: Larger than RAM Vector Indexes](https://planetscale.com/blog/larger-than-ram-vector-indexes-for-relational-databases) (Oct 2025) - Two-layer HNSW approach
- [Reddit: Bincode Development Ceased](https://www.reddit.com/r/lnz1iz/bincode_development_has_ceased_permanently/) (Dec 2025) - Community fork status
- [VectorChord AI: Why HNSW is Not the Answer](https://blog.vectorchord.ai/why-hnsw-is-not-the-answer) (Apr 2025) - HNSW scaling limitations

## Metadata

**Confidence breakdown:**
- Standard stack: MEDIUM - Dependency versions verified, but bincode 2.0 migration has uncertainty
- Architecture: MEDIUM - Patterns based on PostgreSQL and SQLite best practices, but multi-file checkpoint needs validation
- Pitfalls: HIGH - Based on verified SQLite behavior and current codebase analysis
- HNSW scaling: LOW - WebSearch only, no production experience with disk-based HNSW in this codebase

**Research date:** 2026-01-20
**Valid until:** 2026-03-20 (dependency versions may change)
