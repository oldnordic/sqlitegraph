# SQLiteGraph Scaling Limits

This document describes known scaling limits and their mitigation strategies for SQLiteGraph.

## Checkpoint Scaling

### Current Limit: 1GB per checkpoint file

- **Constant**: `MAX_CHECKPOINT_SIZE = 1024 * 1024 * 1024` (constants.rs:22)
- **Impact**: Databases larger than 1GB require multi-file checkpoint
- **Mitigation**: Multi-file checkpoint implemented in Phase 22-01
- **Status**: RESOLVED - Multi-file checkpoint supports unlimited size

### Multi-File Checkpoint

- **Segment Size**: 512MB default (configurable)
- **Max Segments**: 16 default = 8GB max (configurable)
- **Recovery**: Atomic via manifest file validation
- **Reference**: `checkpoint/io/multi_file.rs`

#### Multi-File Checkpoint Details

The multi-file checkpoint system addresses the 1GB single-file limit through:

- **Segment rotation**: Automatic rotation when segment size exceeds threshold
- **Manifest file**: Atomic write pattern (temp file + fsync + atomic rename)
- **LSN continuity**: Validation across segments to prevent data loss
- **Checksum per segment**: Rolling hash (multiply-by-31) for faster validation

Configuration via `MultiFileCheckpointConfig`:
```rust
use sqlitegraph::backend::native::v2::wal::checkpoint::io::multi_file::MultiFileCheckpointConfig;

let config = MultiFileCheckpointConfig::new()
    .with_max_segment_size(512 * 1024 * 1024) // 512MB
    .with_max_segments(16); // 8GB total
```

## Dirty Block Tracking

### Current Limit: 50,000 global dirty blocks

- **Constant**: `MAX_GLOBAL_DIRTY_BLOCKS = 50_000` (constants.rs:34)
- **Per-Cluster Limit**: `MAX_DIRTY_BLOCKS_PER_CLUSTER = 10_000`
- **Impact**: High write workloads may hit limit
- **Mitigation**: Overflow strategies implemented in Phase 22-02
- **Status**: RESOLVED - 4 overflow strategies available

### Overflow Strategies

| Strategy | Behavior | Use Case |
|----------|----------|----------|
| **Reject** | Returns error (default) | Backward compatible, requires manual checkpoint |
| **ForceCheckpoint** | Triggers checkpoint automatically | High-throughput workloads |
| **SpillToDisk** | Spills oldest blocks to disk | Memory-constrained environments |
| **HierarchicalPromotion** | Promotes to cluster-affinity tracking | Cluster-local workloads |

#### Overflow Strategy Configuration

```rust
use sqlitegraph::backend::native::v2::wal::checkpoint::core::DirtyBlockOverflowStrategy;

// Default: Reject on overflow
let strategy = DirtyBlockOverflowStrategy::Reject;

// Auto-trigger checkpoint
let strategy = DirtyBlockOverflowStrategy::ForceCheckpoint;

// Spill to disk (requires overflow store)
let strategy = DirtyBlockOverflowStrategy::SpillToDisk;

// Hierarchical promotion
let strategy = DirtyBlockOverflowStrategy::HierarchicalPromotion;
```

## Transaction ID Management

### Current Limit: u64::MAX theoretical

- **Type**: `TransactionId = u64` (transaction_coordinator.rs:17)
- **Wraparound Protection**: 1M transaction safety margin
- **Warning Threshold**: u64::MAX - 11M
- **Impact**: No practical limit for production workloads
- **Mitigation**: Wraparound protection implemented in Phase 22-03
- **Status**: RESOLVED - TransactionIdManager enforces bounds

### Transaction ID Bounds

The TransactionIdManager implements PostgreSQL-style wraparound protection:

```rust
const SAFETY_MARGIN: u64 = 1_000_000;        // Stop 1M transactions before wraparound
const WRAP_WARNING_THRESHOLD: u64 = u64::MAX - 11_000_000;  // Warn at 11M before limit
```

**Monitoring metrics**:
- `tx_id_manager.remaining_ids()` - Transactions until hard limit
- `tx_id_manager.approaching_wraparound()` - True if within warning threshold

### Deadlock Detector Growth

- **Unbounded**: Wait-for graph grows with concurrent transactions
- **Cleanup Threshold**: 1000 entries triggers cleanup
- **Mitigation**: Periodic cleanup of completed transactions
- **Status**: RESOLVED - Cleanup implemented in Phase 22-03

Deadlock detector cleanup methods:
- `detector.cleanup_stale_transactions()` - Manual cleanup
- `detector.graph_size()` - Current graph entry count
- `detector.needs_cleanup()` - True if > 1000 entries

## HNSW Vector Index

### Current Limit: In-memory only

- **Storage**: `InMemoryVectorStorage` (hnsw/storage.rs)
- **Overhead**: ~30% beyond raw vector data
- **Impact**: Index size limited by available RAM
- **Mitigation**: DEFERRED to v2 - DiskANN or HNSW disk spill

### Disk-Based HNSW Options (Research)

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

## Database File Size

### Native V2 Backend

- **Theoretical Limit**: 16 EB (u64 file offsets)
- **Practical Limit**: Disk space and checkpoint duration
- **Recommendation**: Monitor checkpoint duration, use multi-file for >1GB

### SQLite Backend

- **Limit**: 281 TB (SQLite limits)
- **Practical Limit**: Host filesystem

## Monitoring Recommendations

### Metrics to Track

1. **Dirty block count**
   ```rust
   let count = dirty_blocks.global_dirty_blocks().len();
   if count > 40_000 { warn("Approaching dirty block limit"); }
   ```

2. **Checkpoint file size**
   ```rust
   let size = manager.get_wal_size();
   if size > 512 * 1024 * 1024 { warn("Checkpoint > 512MB"); }
   ```

3. **Transaction ID consumption**
   ```rust
   let remaining = tx_id_manager.remaining_ids();
   if remaining < 100_000_000 { warn("Low transaction IDs remaining"); }
   ```

4. **Deadlock graph size**
   ```rust
   let size = detector.graph_size();
   if size > 1000 { detector.cleanup_stale_transactions(); }
   ```

### Alerts

| Metric | Threshold | Severity |
|--------|-----------|----------|
| Dirty blocks | > 40,000 (80% of limit) | Warning |
| Transaction IDs | < 100M remaining | Warning |
| Checkpoint duration | > 10 minutes | Warning |
| Deadlock graph size | > 1000 | Info (auto-cleanup) |

### Monitoring Integration

```rust
use sqlitegraph::backend::native::v2::wal::V2WALManager;

// Get metrics from WAL manager
let metrics = manager.get_metrics();

println!("Dirty blocks: {}", metrics.dirty_block_count());
println!("WAL size: {} bytes", manager.get_wal_size());
println!("Transactions since checkpoint: {}",
    metrics.get_transactions_since_checkpoint());
```

## Future Work (v2+)

1. **HNSW Disk-Based Index**: Evaluate DiskANN vs HNSW disk spill
2. **Incremental Checkpoint**: Reduce checkpoint duration for large files
3. **Parallel Dirty Block Tracking**: Sharded tracking for higher throughput

## Code References

### Constants

All checkpoint-related constants defined in:
- `sqlitegraph/src/backend/native/v2/wal/checkpoint/constants.rs`

Key constants:
- `MAX_CHECKPOINT_SIZE` - 1GB single-file limit
- `MAX_GLOBAL_DIRTY_BLOCKS` - 50,000 dirty block limit
- `MAX_DIRTY_BLOCKS_PER_CLUSTER` - 10,000 per-cluster limit

### Overflow Strategy

- `sqlitegraph/src/backend/native/v2/wal/checkpoint/core.rs` - DirtyBlockTracker
- `sqlitegraph/src/backend/native/v2/wal/checkpoint/overflow/` - Overflow handling

### Transaction ID Management

- `sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs` - TransactionIdManager

### Multi-File Checkpoint

- `sqlitegraph/src/backend/native/v2/wal/checkpoint/io/multi_file.rs` - Segment writer/reader
