# WAL Public API Documentation

**Date**: 2025-12-23
**Status**: ✅ Complete
**Feature**: Native backend WAL functionality exposed to public API

## Overview

The V2 WAL (Write-Ahead Logging) system is now fully exposed through the public API when the `native-v2` feature is enabled. Users can:

1. **Configure WAL settings** - buffer sizes, checkpoint intervals, compression
2. **Monitor WAL metrics** - transaction counts, WAL size, performance data
3. **Force checkpoints** - manual checkpoint triggers
4. **Advanced integration** - direct WAL manager access for custom workflows

## Feature Flag

WAL API requires the `native-v2` feature:

```toml
# Cargo.toml
[dependencies]
sqlitegraph = { version = "0.2", features = ["native-v2"] }
```

## Public API Types

### Core WAL Configuration

```rust
use sqlitegraph::V2WALConfig;

// Create WAL config for a graph file
let config = V2WALConfig::for_graph_file("/path/to/graph.db");

// Or customize settings
let config = V2WALConfig {
    graph_path: "/path/to/graph.db".into(),
    wal_path: "/path/to/graph.wal".into(),
    checkpoint_path: "/path/to/graph.checkpoint".into(),
    max_wal_size: 1024 * 1024 * 1024, // 1GB
    buffer_size: 1024 * 1024,          // 1MB
    checkpoint_interval: 1000,
    group_commit_timeout_ms: 10,
    max_group_commit_size: 100,
    enable_compression: false,
    compression_level: 3,
};
```

### WAL Manager

```rust
use sqlitegraph::{V2WALManager, V2WALConfig};

let config = V2WALConfig::for_graph_file("/path/to/graph.db");
let wal_manager = V2WALManager::create(config)?;

// Force checkpoint
wal_manager.force_checkpoint()?;

// Get metrics
let metrics = wal_manager.get_metrics();
println!("Transactions: {}", metrics.committed_transactions);
println!("WAL size: {} bytes", metrics.wal_size_bytes);
```

### Transaction Isolation Levels

```rust
use sqlitegraph::TransactionIsolation;

// Read committed isolation
let isolation = TransactionIsolation::ReadCommitted;

// Serializable isolation
let isolation = TransactionIsolation::Serializable;

// Snapshot isolation
let isolation = TransactionIsolation::Snapshot;

// Use with WAL manager
let tx_id = wal_manager.begin_transaction(isolation)?;
```

### WAL Metrics

```rust
use sqlitegraph::WALManagerMetrics;

let metrics = WALManagerMetrics {
    total_transactions: 1000,
    committed_transactions: 950,
    rolled_back_transactions: 50,
    avg_transaction_duration_us: 1500,
    total_records_written: 50000,
    wal_size_bytes: 10 * 1024 * 1024, // 10MB
    checkpoint_count: 5,
    recovery_count: 0,
    group_commit_batches: 20,
    avg_group_commit_size: 2.5,
    compression_ratio: 0.0, // Not used if compression disabled
};
```

### Advanced WAL Integration

```rust
use sqlitegraph::{
    V2GraphWALIntegrator, GraphWALIntegrationConfig,
    V2WALConfig, TransactionIsolation,
};

let wal_config = V2WALConfig::for_graph_file("/path/to/graph.db");
let integration_config = GraphWALIntegrationConfig {
    auto_checkpoint: true,
    checkpoint_interval: 1000,
    cluster_affinity: true,
    enable_compression: false,
    max_batch_size: 50,
    sync_writes: true,
};

let integrator = V2GraphWALIntegrator::create(
    wal_config,
    integration_config
)?;

// Begin transaction
let tx_id = integrator.begin_transaction(TransactionIsolation::Serializable)?;

// ... perform operations ...

// Transaction will be auto-managed by WAL integrator
```

### Operation Metrics

```rust
use sqlitegraph::OperationMetrics;

let metrics = OperationMetrics {
    duration_us: 250,
    wal_records_written: 5,
    bytes_written: 1024,
    nodes_affected: 2,
    clusters_affected: 1,
    edges_affected: 3,
};
```

## Usage Examples

### Example 1: Basic WAL Checkpoint

```rust
use sqlitegraph::{GraphBackend, NativeGraphBackend};

// Create backend
let backend = NativeGraphBackend::new("/path/to/graph.db")?;

// Perform operations
backend.insert_node(/* ... */)?;
backend.insert_edge(/* ... */)?;

// Trigger checkpoint
backend.checkpoint()?;
```

### Example 2: Custom WAL Configuration

```rust
use sqlitegraph::{V2WALManager, V2WALConfig};

// Configure WAL with custom settings
let config = V2WALConfig {
    graph_path: "/path/to/graph.db".into(),
    wal_path: "/path/to/graph.wal".into(),
    checkpoint_path: "/path/to/graph.checkpoint".into(),
    max_wal_size: 2 * 1024 * 1024 * 1024, // 2GB WAL limit
    buffer_size: 2 * 1024 * 1024,          // 2MB buffer
    checkpoint_interval: 500,              // Checkpoint every 500 txns
    group_commit_timeout_ms: 5,             // 5ms group commit timeout
    max_group_commit_size: 200,             // Max 200 txns per group
    enable_compression: true,
    compression_level: 6,
    ..Default::default()
};

let wal_manager = V2WALManager::create(config)?;
```

### Example 3: Monitoring WAL Performance

```rust
use sqlitegraph::V2WALManager;

let wal_manager = V2WALManager::create(config)?;

// Perform operations
// ...

// Get performance metrics
let metrics = wal_manager.get_metrics();

println!("=== WAL Performance Metrics ===");
println!("Total transactions: {}", metrics.total_transactions);
println!("Committed: {}", metrics.committed_transactions);
println!("Rolled back: {}", metrics.rolled_back_transactions);
println!("Avg duration: {} μs", metrics.avg_transaction_duration_us);
println!("Records written: {}", metrics.total_records_written);
println!("WAL size: {} MB", metrics.wal_size_bytes / 1_048_576);
println!("Checkpoints: {}", metrics.checkpoint_count);

if metrics.avg_group_commit_size > 1.0 {
    println!("Avg group commit size: {:.1}", metrics.avg_group_commit_size);
}

if metrics.compression_ratio > 0.0 {
    println!("Compression ratio: {:.2}", metrics.compression_ratio);
}
```

### Example 4: Manual Transaction Management

```rust
use sqlitegraph::{V2WALManager, TransactionIsolation};

let wal_manager = V2WALManager::create(config)?;

// Begin transaction with serializable isolation
let tx_id = wal_manager.begin_transaction(TransactionIsolation::Serializable)?;

// Write WAL records
// ...

// Commit transaction
wal_manager.commit_transaction(tx_id)?;

// Or rollback if needed
// wal_manager.rollback_transaction(tx_id)?;
```

## File Layout

When using WAL with a graph file at `/path/to/mygraph.db`:

```
/path/to/mygraph.db           - Graph data (V2 format)
/path/to/mygraph.wal          - WAL transaction log
/path/to/mygraph.checkpoint   - Checkpoint metadata
```

The WAL files are automatically created based on the graph file base name (everything before the last `.`).

## WAL vs No-WAL

### With WAL (native-v2 feature)

✅ **Pros**:
- Durability: Transactions logged before being applied
- Performance: Sequential I/O patterns for writes
- Concurrency: Better read/write concurrent access
- Recovery: Crash recovery from WAL

⚠️ **Cons**:
- Additional disk space for WAL files
- Slight write latency (WAL writes + main file writes)
- Requires periodic checkpointing

### Without WAL (default)

✅ **Pros**:
- Simpler file layout (single .db file)
- Faster writes (no WAL overhead)
- Less disk space

⚠️ **Cons**:
- No crash recovery beyond last sync
- Potential data loss on crash
- No concurrent read/write optimization

## API Export Location

All WAL types are re-exported in `sqlitegraph/src/lib.rs:102-113`:

```rust
// Re-export WAL functionality for native backend
#[cfg(feature = "native-v2")]
pub use backend::native::v2::wal::{
    V2WALConfig, V2WALManager,
    TransactionIsolation, WALManagerMetrics,
};

// Re-export WAL integration for advanced usage
#[cfg(feature = "native-v2")]
pub use backend::native::v2::wal::{
    V2GraphWALIntegrator, GraphWALIntegrationConfig,
    GraphOperationResult, OperationMetrics,
};
```

## Backward Compatibility

✅ **Fully backward compatible**

- Existing code without `native-v2` feature is unaffected
- WAL types are feature-gated and only available when enabled
- No breaking changes to existing public API

## Testing

### Compilation Test

```bash
# Build with native-v2 feature
cargo build --features native-v2 -p sqlitegraph

# Test that WAL API is accessible
cat > test_wal_api.rs << 'EOF'
use sqlitegraph::{
    V2WALConfig, V2WALManager,
    TransactionIsolation, WALManagerMetrics,
};

fn main() {
    let config = V2WALConfig::for_graph_file(std::path::Path::new("/tmp/test.db"));
    assert_eq!(config.wal_path.to_str().unwrap(), "/tmp/test.wal");
    println!("✅ WAL API accessible!");
}
EOF

rustc --edition 2021 --cfg 'feature="native-v2"' \
    test_wal_api.rs \
    --extern sqlitegraph=target/debug/libsqlitegraph.rlib \
    -L target/debug/deps

./test_wal_api
# Output: ✅ WAL API accessible!
```

## Future Enhancements

1. **Async WAL API** - Non-blocking checkpoint operations
2. **WAL Rotation** - Automatic WAL file rotation
3. **Compression Options** - Different compression algorithms
4. **Replication Support** - WAL-based replication
5. **WAL Analytics** - Built-in WAL performance analysis

## Conclusion

The V2 WAL system is now fully accessible through the public API, enabling users to:

- ✅ Configure WAL behavior
- ✅ Monitor WAL performance
- ✅ Integrate WAL into custom workflows
- ✅ Build advanced transactional applications

All WAL functionality is properly feature-gated behind `native-v2` and does not affect users who don't need WAL capabilities.
