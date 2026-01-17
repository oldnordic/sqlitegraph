# CLI WAL Commands Documentation

**Date**: 2025-12-23
**Status**: ✅ Complete

## Overview

Enhanced the SQLiteGraph CLI with WAL-specific commands for monitoring and managing the Write-Ahead Logging system. These commands are available when the `native-v2` feature is enabled.

## New CLI Commands

### 1. `wal-checkpoint` (Existing)

Triggers a WAL checkpoint operation to flush WAL data to the main graph file.

```bash
sqlitegraph --backend native --db /path/to/graph.db wal-checkpoint
```

**Output**:
```json
{
  "command": "wal-checkpoint",
  "status": "completed"
}
```

### 2. `wal-metrics` (NEW)

Shows WAL file metrics including file sizes and existence status.

```bash
sqlitegraph --backend native --db /path/to/graph.db wal-metrics
```

**Output**:
```json
{
  "command": "wal-metrics",
  "database_path": "/path/to/graph.db",
  "wal_file": "/path/to/graph.wal",
  "checkpoint_file": "/path/to/graph.checkpoint",
  "wal_exists": false,
  "wal_size_bytes": 1048576,
  "wal_size_mb": 1.0,
  "checkpoint_size_bytes": 4096
}
```

**Fields**:
- `wal_exists`: Boolean indicating if WAL file exists
- `wal_size_bytes`: WAL file size in bytes (if exists)
- `wal_size_mb`: WAL file size in megabytes (if exists)
- `checkpoint_size_bytes`: Checkpoint file size in bytes (if exists)

### 3. `wal-config` (NEW)

Displays the WAL configuration settings for a graph file.

```bash
sqlitegraph --backend native --db /path/to/graph.db wal-config
```

**Output**:
```json
{
  "command": "wal-config",
  "database_path": "/path/to/graph.db",
  "graph_path": "/path/to/graph.db",
  "wal_path": "/path/to/graph.wal",
  "checkpoint_path": "/path/to/graph.checkpoint",
  "max_wal_size": 1073741824,
  "max_wal_size_mb": 1024,
  "buffer_size": 1048576,
  "buffer_size_kb": 1024,
  "checkpoint_interval": 1000,
  "group_commit_timeout_ms": 10,
  "max_group_commit_size": 100,
  "enable_compression": false,
  "compression_level": 3
}
```

**Fields Explained**:
- `max_wal_size_mb`: Maximum WAL file size before forced checkpoint (default: 1GB)
- `buffer_size_kb`: Write buffer size for optimal I/O alignment (default: 1MB)
- `checkpoint_interval`: Number of transactions before automatic checkpoint (default: 1000)
- `group_commit_timeout_ms`: Timeout in milliseconds for group commit batching (default: 10ms)
- `max_group_commit_size`: Maximum number of transactions per group commit batch (default: 100)
- `enable_compression`: Whether WAL record compression is enabled (default: false)
- `compression_level`: Compression level 1-9 if compression is enabled (default: 3)

## Implementation Details

### Feature Gating

The new WAL commands are feature-gated behind `native-v2`:

```rust
#[cfg(feature = "native-v2")]
fn run_wal_metrics(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    // Implementation
}

#[cfg(not(feature = "native-v2"))]
fn run_wal_metrics(_client: &BackendClient, _args: &[String]) -> Result<(), SqliteGraphError> {
    // Returns error message about feature requirement
    let payload = json!({
        "command": "wal-metrics",
        "error": "WAL metrics require native-v2 feature",
        "status": "unsupported"
    });
    println!("{payload}");
    Ok(())
}
```

### Path Resolution

The WAL commands use the `--db` parameter to determine the graph file path and construct WAL file paths:

```rust
// Graph file: /tmp/mygraph.db
let db_path = Path::new("/tmp/mygraph.db");

// WAL file: /tmp/mygraph.wal
let wal_path = db_path.with_extension("wal");

// Checkpoint file: /tmp/mygraph.checkpoint
let checkpoint_path = db_path.with_extension("checkpoint");
```

### File Existence Checking

The `wal-metrics` command checks for file existence and provides metadata:

```rust
if wal_path.exists() {
    if let Ok(metadata) = fs::metadata(&wal_path) {
        metrics["wal_size_bytes"] = json!(metadata.len());
        metrics["wal_size_mb"] = json!(metadata.len() as f64 / 1_048_576.0);
        metrics["wal_exists"] = json!(true);
    }
} else {
    metrics["wal_exists"] = json!(false);
}
```

## CLI Help Updates

Updated help text in `sqlitegraph-cli/src/cli.rs`:

```
Commands:
  ...
  wal-checkpoint            Trigger WAL checkpoint operation
  wal-metrics                Show WAL performance metrics and file sizes
  wal-config                 Show WAL configuration settings
  snapshot-create --dir DIR  Create database snapshot
  snapshot-load --dir DIR     Load database snapshot
```

## Usage Examples

### Example 1: Check WAL Configuration

```bash
$ sqlitegraph --backend native --db /tmp/mygraph.db wal-config
```

### Example 2: Monitor WAL File Growth

```bash
# Before operations
$ sqlitegraph --backend native --db /tmp/mygraph.db wal-metrics

# Perform graph operations...
$ sqlitegraph --backend native --db /tmp/mygraph.db node-create ...

# Check WAL size again
$ sqlitegraph --backend native --db /tmp/mygraph.db wal-metrics
```

### Example 3: Checkpoint and Verify

```bash
# Trigger checkpoint
$ sqlitegraph --backend native --db /tmp/mygraph.db wal-checkpoint

# Verify WAL still exists (checkpoint doesn't remove WAL)
$ sqlitegraph --backend native --db /tmp/mygraph.db wal-metrics
```

## Files Modified

1. **`sqlitegraph-cli/src/main.rs`**
   - Added `wal-metrics` command handler
   - Added `wal-config` command handler
   - Feature-gated implementations for native-v2

2. **`sqlitegraph-cli/src/cli.rs`**
   - Updated help text with new commands

## Compilation

Built successfully with `native-v2` feature:

```bash
cargo build --features native-v2 -p sqlitegraph-cli
# Result: ✅ Compiled successfully
```

## Testing

```bash
# Test wal-config
$ cargo run -p sqlitegraph-cli -- --backend native --db /tmp/test.db wal-config
{"command":"wal-config",...,"status":"completed"}

# Test wal-metrics
$ cargo run -p sqlitegraph-cli -- --backend native --db /tmp/test.db wal-metrics
{"command":"wal-metrics",...,"wal_exists":false}

# Test wal-checkpoint (existing)
$ cargo run -p sqlitegraph-cli -- --backend native --db /tmp/test.db wal-checkpoint
{"command":"wal-checkpoint","status":"completed"}
```

## Future Enhancements

Potential improvements to WAL CLI commands:

1. **Real-time WAL Statistics**
   - Transaction throughput (txns/sec)
   - Average commit latency
   - Checkpoint frequency

2. **WAL Management**
   - `wal-truncate` - Manually truncate WAL file
   - `wal-rotate` - Force WAL rotation
   - `wal-backup` - Backup WAL file

3. **Configuration**
   - `wal-set-config` - Modify WAL settings
   - `wal-reconfigure` - Reconfigure WAL with new parameters

4. **Monitoring**
   - `wal-stats` - Detailed performance statistics
   - `wal-history` - Checkpoint history
   - `wal-analyze` - WAL usage analysis

## Conclusion

The CLI now provides convenient access to WAL functionality:
- ✅ `wal-checkpoint` - Trigger checkpoints
- ✅ `wal-metrics` - Monitor WAL file sizes
- ✅ `wal-config` - View WAL configuration

All commands are feature-gated and only available with `native-v2` feature, ensuring backward compatibility for users who don't need WAL functionality.
