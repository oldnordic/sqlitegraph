# Dependency Migration Guide

This document tracks dependency health and migration plans for SQLiteGraph.

## bincode 1.3 -> 2.0 Migration

### Status: PLANNED (Deferred to v1.2+)

### Background

- **Current**: bincode 1.3 (development ceased Dec 2025)
- **Issue**: Original author halted development due to harassment
- **Community**: Forks available (cu-bincode, bincode_next)

**Current version in Cargo.toml**:
```toml
bincode = "1.3"
```

### Breaking Changes (bincode 2.0)

#### 1. API Changes

- `serialize()` returns `Result<Vec<u8>>` instead of `Vec<u8>`
- `deserialize()` takes `&[u8]` instead of `&[u8; N]` for fixed-size
- Configuration-based features (e.g., variable integer encoding)

**Current usage (bincode 1.3)**:
```rust
use bincode::{serialize, deserialize};

// Returns Vec<u8> directly
let encoded = serialize(&my_struct)?;

// Deserializes from &[u8]
let decoded: MyStruct = deserialize(&encoded[..])?;
```

**New usage (bincode 2.0)**:
```rust
use bincode::{encode_to_vec, decode_from_slice};
use bincode::config::standard();

// Returns Result<Vec<u8>>
let encoded = encode_to_vec(&my_struct, standard())?;

// Returns Result<(T, usize)> - tuple with decoded value and bytes consumed
let (decoded, _len): (MyStruct, _) = decode_from_slice(&encoded[..], standard())?;
```

#### 2. Format Changes

- Different binary format (NOT backward compatible)
- Requires file format version bump (v3 -> v4)

### Migration Plan

#### Phase 1: Add feature flag

Keep bincode 1.3 as default, add conditional compilation for 2.0:

```toml
# Cargo.toml
[dependencies]
bincode = "1.3"
bincode2 = { version = "2.0", package = "bincode", optional = true }

[features]
default = []
bincode2 = ["dep:bincode2"]  # Opt-in for bincode 2.0
```

#### Phase 2: Implement format migration

Auto-detect v3 files (bincode 1.3) and convert to v4 format (bincode 2.0):

```rust
#[cfg(feature = "bincode2")]
pub fn migrate_v3_to_v4(data: &[u8]) -> Result<Vec<u8>, MigrationError> {
    // Try bincode 1.3 first
    let v3_value: MyStruct = bincode::deserialize(data)
        .map_err(|_| MigrationError::InvalidFormat)?;

    // Re-serialize with bincode 2.0
    bincode2::encode_to_vec(&v3_value, bincode2::config::standard())
        .map_err(Into::into)
}
```

#### Phase 3: Switch default

Make bincode 2.0 default after stabilization period:

```toml
[dependencies]
bincode2 = { version = "2.0", package = "bincode" }
bincode = { version = "1.3", optional = true }

[features]
default = ["bincode2"]
legacy-bincode = ["bincode"]  # Opt-in for legacy
```

### Estimated Effort

- 2-3 days for implementation (conditional compilation, migration)
- 1 day for testing (format conversion, backward compatibility)
- 1 week soak period before switching default

### Files Requiring Changes

The following files use bincode serialization:

1. **V2 WAL checkpoint format**
   - `src/backend/native/v2/wal/checkpoint/record/ops.rs`
   - Uses bincode for WAL record serialization

2. **HNSW persistence**
   - `src/hnsw/storage.rs`
   - Uses bincode for vector metadata serialization

3. **Graph file format**
   - `src/backend/native/v2/graph_file.rs`
   - Uses bincode for node/edge serialization

## rusqlite Dependency

### Status: HEALTHY

### Current Version: 0.31

**Current version in Cargo.toml**:
```toml
rusqlite = { version = "0.31", features = ["bundled"] }
```

- **Features**: bundled (SQLite 3.x compiled in)
- **Rationale**: Bundled SQLite ensures security patches are included
- **System SQLite**: Available but NOT recommended (version uncertainty)

### Monitoring

Track rusqlite releases for security updates:
- Current releases: https://crates.io/crates/rusqlite
- SQLite releases: https://sqlite.org/releaselog/index.html

### Future Versions

- **rusqlite 0.32+** requires r2d2_sqlite 0.38+ (breaking change)
- Current: r2d2_sqlite 0.24 is compatible with rusqlite 0.31
- **Action**: Monitor r2d2_sqlite compatibility before upgrading

**Current dependency chain**:
```toml
[dependencies]
rusqlite = { version = "0.31", features = ["bundled"] }
r2d2 = "0.8"
r2d2_sqlite = "0.24"  # Compatible with rusqlite 0.31
```

**Future upgrade path** (when rusqlite 0.32+ is released):
```toml
[dependencies]
rusqlite = { version = "0.32", features = ["bundled"] }
r2d2 = "0.8"
r2d2_sqlite = "0.38"  # Requires rusqlite 0.38+
```

## HNSW Disk-Based Migration (v2+)

### Current State (v1.1)

- **In-Memory Graph**: Full HNSW structure in RAM
- **Vector Persistence**: Vectors stored in SQLite BLOB
- **Rebuild on Load**: Graph structure rebuilt from vectors
- **Limitation**: ~30% overhead + vector data must fit in RAM

### Disk-Based Options

#### Option 1: Hybrid HNSW with Disk Spill

**Architecture**:
- Hot nodes (recently accessed) in memory
- Cold nodes spilled to disk (SQLite BLOB)
- LRU cache for hot node promotion

**Pros**:
- Maintains current HNSW algorithm
- Gradual migration path
- Familiar SQLite storage

**Cons**:
- Complex cache management
- Disk I/O during search
- Cache miss performance penalty

**Implementation Estimate**: 3-5 days

#### Option 2: DiskANN Integration

**Architecture**:
- Replace HNSW with DiskANN entirely
- Separate disk-optimized index

**Pros**:
- Designed for disk-based indexes
- Better large-scale performance

**Cons**:
- Less mature Rust ecosystem
- Breaking API change
- Different algorithm semantics

**Implementation Estimate**: 5-7 days

#### Option 3: Separate Vector Database

**Architecture**:
- Use separate vector DB (e.g., sqlite-vec extension)
- SQLiteGraph handles graph structure only

**Pros**:
- Leverages optimized vector storage
- Separation of concerns

**Cons**:
- Additional dependency
- Coordination complexity

**Implementation Estimate**: 2-3 days

### Recommended Decision Matrix

| Use Case | Recommendation |
|----------|----------------|
| < 1M vectors, RAM available | Current in-memory HNSW |
| 1M-10M vectors | Hybrid HNSW with disk spill |
| > 10M vectors | DiskANN or separate vector DB |

### v2 Decision Criteria

- User workload data (vector count, RAM availability)
- Benchmark results for each option
- Community feedback on scaling needs

### Code References

Current HNSW implementation:
- `sqlitegraph/src/hnsw/storage.rs` - InMemoryVectorStorage
- `sqlitegraph/src/hnsw/index.rs` - HnswIndex implementation

## Dependency Health Checklist

| Dependency | Version | Status | Action Needed |
|------------|---------|--------|---------------|
| rusqlite | 0.31 | Healthy | Monitor for updates |
| bincode | 1.3 | Deprecated | Plan 2.0 migration |
| r2d2_sqlite | 0.24 | Healthy | Compatible with rusqlite 0.31 |
| r2d2 | 0.8 | Healthy | Stable API |
| parking_lot | 0.12 | Healthy | Stable API |
| serde | 1 | Healthy | Stable API |
| serde_json | 1 | Healthy | Stable API |
| ahash | 0.8 | Healthy | Stable API |
| rand | 0.8 | Healthy | Stable API |
| arc-swap | 1 | Healthy | Stable API |
| bytemuck | 1.13 | Healthy | Stable API |
| binrw | 0.13 | Healthy | Stable API |
| memmap2 | 0.9 | Healthy | Stable API |
| log | 0.4 | Healthy | Stable API |
| rayon | 1.10 | Healthy | Stable API |

## Monitoring Strategy

### Weekly

Check crates.io for dependency updates:
```bash
# Check for outdated dependencies
cargo outdated
```

### Monthly

Review security advisories:
- RustSec advisories: https://rustsec.org/advisories
- GitHub Advisory Database: https://github.com/advisories

### Quarterly

Evaluate dependency updates for breaking changes:
```bash
# Check for security vulnerabilities
cargo install cargo-audit
cargo audit
```

### Per Release

Run security audit before releases:
```bash
cargo audit
```

## cargo-audit Integration

### Installation

```bash
cargo install cargo-audit
```

### Usage

```bash
# Run security audit
cargo audit

# Check for specific advisory database
cargo audit --db https://github.com/RustSec/advisory-db
```

### CI/CD Integration

Add to `.github/workflows/ci.yml` or equivalent:

```yaml
name: Security Audit
on: [push, pull_request]

jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/audit-check@v1
        with:
          token: ${{ secrets.GITHUB_TOKEN }}
```

## Version Compatibility Matrix

| Component | Current | Minimum Compatible | Notes |
|-----------|---------|-------------------|-------|
| SQLite (via rusqlite 0.31 bundled) | 3.x | 3.40+ | Bundled with rusqlite |
| r2d2_sqlite | 0.24 | 0.22+ | Requires rusqlite 0.30+ |
| Rust edition | 2024 | 2021 | Edition 2024 stable |

## Breaking Change Policy

When evaluating dependency updates:

1. **Security patches**: Always apply immediately
2. **Minor updates**: Apply after testing
3. **Major updates**: Require migration plan and backward compatibility

### Update Criteria

- **Low risk**: Patch updates (0.31.0 -> 0.31.1)
- **Medium risk**: Minor updates (0.31 -> 0.32)
- **High risk**: Major updates (1.x -> 2.0) - requires migration plan

## References

- **bincode migration**: https://docs.rs/bincode/latest/bincode/migrate/index.html
- **bincode_next**: https://docs.rs/bincode-next/latest/bincode_next/
- **rusqlite releases**: https://crates.io/crates/rusqlite
- **RustSec advisories**: https://rustsec.org/
