# V2 Adjacency System - Technical Specification

## Overview

This document provides the complete technical specification for SQLiteGraph's V2 adjacency system implementation. The system provides high-performance neighbor discovery through V2 clustered adjacency while maintaining backward compatibility through a hybrid fallback mechanism.

## System Architecture

### High-Level Design

```
┌─────────────────────────────────────────────────────────────────┐
│                    V2 Adjacency System                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────┐    ┌─────────────────┐    ┌──────────────┐ │
│  │   Input Node    │───▶│   V2 Cluster     │───▶│   Primary    │ │
│  │   & Direction   │    │   Reader        │    │   Result     │ │
│  └─────────────────┘    └─────────────────┘    └──────────────┘ │
│           │                       │                       │      │
│           ▼                       ▼                       ▼      │
│  ┌─────────────────┐    ┌─────────────────┐    ┌──────────────┐ │
│  │   Legacy Edge   │───▶│   Edge Scanner   │───▶│  Fallback    │ │
│  │   Storage       │    │   (Direct)       │    │   Result     │ │
│  └─────────────────┘    └─────────────────┘    └──────────────┘ │
│                                                           │      │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │           Unified Neighbor Discovery Results            │ │
│  └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                     ▲
                     │
┌─────────────────────────────────────────────────────────────────┐
│                Header Consistency Layer                         │
├─────────────────────────────────────────────────────────────────┤
│  • edge_count synchronized with actual stored edge data        │
│  • V2 cluster metadata properly maintained                    │
│  • Node adjacency counts kept consistent                       │
└─────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. AdjacencyIterator (`core_iterator.rs`)

The main entry point for neighbor discovery operations.

#### Key Methods

```rust
impl<'a> AdjacencyIterator<'a> {
    /// Create adjacency iterator for outgoing neighbors
    pub fn new_outgoing(
        graph_file: &'a mut GraphFile,
        node_id: NativeNodeId,
    ) -> NativeResult<Self>;

    /// Create adjacency iterator for incoming neighbors
    pub fn new_incoming(
        graph_file: &'a mut GraphFile,
        node_id: NativeNodeId,
    ) -> NativeResult<Self>;

    /// Get neighbor at current position with V2 cluster support
    pub fn get_current_neighbor(&mut self) -> NativeResult<Option<NativeNodeId>>;

    /// Collect all neighbors with infinite loop protection
    pub fn collect(mut self) -> NativeResult<Vec<NativeNodeId>>;

    /// Reset iterator to beginning
    pub fn reset(&mut self);
}
```

#### Infinite Loop Protection

```rust
pub fn collect(mut self) -> NativeResult<Vec<NativeNodeId>> {
    let mut neighbors = Vec::new();

    while !self.is_complete() {
        match self.get_current_neighbor()? {
            Some(neighbor) => {
                neighbors.push(neighbor);
                self.current_index += 1;
            }
            None => {
                // Critical: Terminate when no neighbor found
                #[cfg(debug_assertions)]
                eprintln!("DEBUG: Terminating iteration early - no neighbor found at index {} for node {} (total_count: {})",
                         self.current_index, self.node_id, self.total_count);
                break;
            }
        }
    }

    // Deduplicate and return unique neighbors
    let mut seen_neighbors = std::collections::HashSet::new();
    let mut unique_neighbors = Vec::new();

    for neighbor in neighbors {
        if seen_neighbors.insert(neighbor) {
            unique_neighbors.push(neighbor);
        }
    }

    Ok(unique_neighbors)
}
```

### 2. V2 Cluster Manager (`v2_clustered.rs`)

Handles V2 clustered adjacency reading with graceful fallback.

#### Primary V2 Cluster Reading

```rust
impl super::AdjacencyIterator<'_> {
    /// Initialize V2 clustered adjacency with comprehensive error handling
    pub fn try_initialize_clustered_adjacency(&mut self) -> NativeResult<()> {
        // Return early if already attempted (prevent infinite loops)
        if self.cached_clustered_neighbors.is_some() {
            return Ok(());
        }

        // Check if node is V2 format with cluster metadata
        let node_data_offset = self.graph_file.persistent_header().node_data_offset;
        let slot_offset = node_data_offset + ((self.node_id - 1) as u64 * 4096);
        let mut version = [0u8; 1];

        match self.graph_file.read_bytes(slot_offset, &mut version) {
            Ok(()) => {
                if version[0] == 2 {
                    // V2 node detected - attempt cluster reading
                    match self.initialize_v2_cluster_reading() {
                        Ok(neighbors) => {
                            self.cached_clustered_neighbors = Some(neighbors);
                            return Ok(());
                        }
                        Err(e) => {
                            #[cfg(debug_assertions)]
                            println!("DEBUG: V2 cluster read failed for node {}: {}, falling back to edge store traversal",
                                     self.node_id, e);
                        }
                    }
                }
            }
            Err(e) => {
                // Handle read errors with proper error propagation
                self.cached_clustered_neighbors = Some(Vec::new());
                self.total_count = 0;
                return Err(e);
            }
        }

        // V2 cluster not available - cache empty result
        self.cached_clustered_neighbors = Some(Vec::new());
        self.total_count = 0;
        Err(NativeBackendError::CorruptNodeRecord {
            node_id: self.node_id as i64,
            reason: "V2 cluster metadata not found".to_string(),
        })
    }
}
```

#### Direct V2 Cluster Reading

```rust
/// Read V2 edge cluster directly without circular dependencies
fn read_v2_edge_cluster_directly(&mut self, node_v2: &crate::backend::native::v2::node_record_v2::NodeRecordV2) -> NativeResult<Vec<NativeNodeId>> {
    use crate::backend::native::v2::edge_cluster::EdgeCluster;

    let (cluster_offset, cluster_size) = match self.direction {
        Direction::Outgoing => (node_v2.outgoing_cluster_offset, node_v2.outgoing_cluster_size),
        Direction::Incoming => (node_v2.incoming_cluster_offset, node_v2.incoming_cluster_size),
    };

    // Validate cluster metadata
    if cluster_offset == 0 || cluster_size == 0 {
        return Ok(Vec::new());
    }

    // Read cluster data directly from file
    let mut cluster_data = vec![0u8; cluster_size as usize];
    self.graph_file.read_bytes(cluster_offset, &mut cluster_data)?;

    // Check for empty cluster data
    if cluster_data.iter().all(|&byte| byte == 0) {
        #[cfg(debug_assertions)]
        println!("DEBUG: V2 cluster data is all zeros - no edge cluster was written");
        return Ok(Vec::new());
    }

    // Deserialize and extract neighbors
    match EdgeCluster::deserialize(&cluster_data) {
        Ok(cluster) => {
            let neighbors: Vec<NativeNodeId> = cluster.iter_neighbors()
                .map(|id| id as NativeNodeId)
                .collect();

            #[cfg(debug_assertions)]
            println!("DEBUG: Direct V2 cluster read for node {} (direction: {:?}) - found {} neighbors",
                     self.node_id, self.direction, neighbors.len());

            Ok(neighbors)
        }
        Err(e) => {
            #[cfg(debug_assertions)]
            println!("DEBUG: Failed to deserialize V2 cluster for node {}: {}", self.node_id, e);
            Err(e)
        }
    }
}
```

### 3. Edge Store with Hybrid Scanning (`edge_store/mod.rs`)

Provides edge storage operations with header consistency and circular dependency prevention.

#### Edge Writing with Header Consistency

```rust
impl<'a> EdgeStore<'a> {
    /// Write edge record with V2 cluster metadata integration
    pub fn write_edge(&mut self, edge: &crate::backend::native::types::EdgeRecord) -> crate::backend::native::types::NativeResult<()> {
        self.write_edge_with_cluster_metadata(edge)
    }

    /// Write edge record and ensure header consistency
    fn write_edge_with_cluster_metadata(&mut self, edge: &crate::backend::native::types::EdgeRecord) -> crate::backend::native::types::NativeResult<()> {
        let edge_count_before = self.graph_file.header().edge_count;

        #[cfg(debug_assertions)]
        println!("DEBUG: Before writing edge {} - header.edge_count = {}", edge.id, edge_count_before);

        // Write the edge record
        let mut operations = record_operations::EdgeRecordOperations::new(self.graph_file);
        operations.write_edge(edge)?;

        // CRITICAL: Update header edge_count for manually assigned IDs
        let current_edge_count = self.graph_file.header().edge_count;
        if edge.id > current_edge_count as i64 {
            #[cfg(debug_assertions)]
            println!("DEBUG: Updating header.edge_count from {} to {} to accommodate edge {}",
                     current_edge_count, edge.id, edge.id);
            self.graph_file.persistent_header_mut().edge_count = edge.id as u64;
        }

        let edge_count_after = self.graph_file.header().edge_count;

        #[cfg(debug_assertions)]
        println!("DEBUG: After writing edge {} - header.edge_count = {}", edge.id, edge_count_after);

        // Update cluster metadata on source and target nodes
        self.update_node_cluster_metadata(edge.from_id, edge.to_id)
    }
}
```

#### Direct Edge Scanning (No Circular Dependencies)

```rust
/// Iterate neighbors using direct edge scanning (prevents circular dependencies)
pub fn iter_neighbors(&mut self, node_id: crate::backend::native::types::NativeNodeId, direction: crate::backend::native::adjacency::Direction) -> Box<dyn Iterator<Item = crate::backend::native::types::NativeNodeId> + '_> {
    match self.iter_neighbors_direct(node_id, direction) {
        Ok(neighbors) => Box::new(neighbors.into_iter()),
        Err(_) => Box::new(std::iter::empty()),
    }
}

/// Direct edge neighbor iteration without creating AdjacencyIterator instances
fn iter_neighbors_direct(&mut self, node_id: crate::backend::native::types::NativeNodeId, direction: crate::backend::native::adjacency::Direction) -> crate::backend::native::types::NativeResult<Vec<crate::backend::native::types::NativeNodeId>> {
    let header = self.graph_file.header();
    let mut neighbors = Vec::new();

    #[cfg(debug_assertions)]
    println!("DEBUG: Edge scanning - header.edge_count = {}, scanning edges 1..={}", header.edge_count, header.edge_count);

    // Scan all edges directly (prevents circular dependency)
    for edge_id in 1..=header.edge_count as i64 {
        #[cfg(debug_assertions)]
        println!("DEBUG: Attempting to read edge {}", edge_id);

        let mut operations = record_operations::EdgeRecordOperations::new(self.graph_file);
        if let Ok(edge) = operations.read_edge(edge_id) {
            #[cfg(debug_assertions)]
            println!("DEBUG: Successfully read edge {} -> {} (from_id={}, to_id={})",
                     edge.id, edge_id, edge.from_id, edge.to_id);

            let matches_direction = match direction {
                crate::backend::native::adjacency::Direction::Outgoing => edge.from_id == node_id,
                crate::backend::native::adjacency::Direction::Incoming => edge.to_id == node_id,
            };

            if matches_direction {
                let neighbor_id = match direction {
                    crate::backend::native::adjacency::Direction::Outgoing => edge.to_id,
                    crate::backend::native::adjacency::Direction::Incoming => edge.from_id,
                };

                #[cfg(debug_assertions)]
                println!("DEBUG: Edge {} matches direction for node {} - neighbor {}",
                         edge_id, node_id, neighbor_id);
                neighbors.push(neighbor_id);
            }
        } else {
            #[cfg(debug_assertions)]
            println!("DEBUG: Failed to read edge {}", edge_id);
        }
    }

    #[cfg(debug_assertions)]
    println!("DEBUG: Direct edge iteration found {} neighbors for node {} (direction: {:?})",
             neighbors.len(), node_id, direction);

    Ok(neighbors)
}
```

### 4. Debug Instrumentation (`instrumentation.rs`)

Comprehensive instrumentation for debugging and performance monitoring.

#### Atomic Counter Tracking

```rust
pub struct IterationMetrics {
    pub total_iterations: AtomicUsize,
    pub total_v2_reads: AtomicUsize,
    pub infinite_loop_detections: AtomicUsize,
}

impl IterationMetrics {
    pub fn record_iteration(&self) -> bool {
        let count = self.total_iterations.fetch_add(1, Ordering::SeqCst);
        const INFINITE_LOOP_THRESHOLD: usize = 1000;

        if count > INFINITE_LOOP_THRESHOLD {
            self.infinite_loop_detections.fetch_add(1, Ordering::SeqCst);
            error!("POTENTIAL INFINITE LOOP DETECTED: {} iterations logged", count);
            return false;
        }
        true
    }

    pub fn record_v2_read(&self) {
        self.total_v2_reads.fetch_add(1, Ordering::SeqCst);
    }

    pub fn get_metrics(&self) -> IterationStats {
        IterationStats {
            total_iterations: self.total_iterations.load(Ordering::SeqCst),
            total_v2_reads: self.total_v2_reads.load(Ordering::SeqCst),
            infinite_loop_detections: self.infinite_loop_detections.load(Ordering::SeqCst),
        }
    }
}

pub struct IterationStats {
    pub total_iterations: usize,
    pub total_v2_reads: usize,
    pub infinite_loop_detections: usize,
}

impl IterationStats {
    pub fn iteration_efficiency(&self) -> f64 {
        if self.total_iterations == 0 {
            1.0
        } else {
            self.total_v2_reads as f64 / self.total_iterations as f64
        }
    }

    pub fn suggests_infinite_loop(&self) -> bool {
        self.total_iterations > 1000 && self.infinite_loop_detections > 0
    }
}
```

#### Convenience Functions

```rust
pub fn track_iteration(node_id: u32) -> bool {
    GLOBAL_METRICS.record_iteration()
}

pub fn track_v2_read(node_id: u32) {
    GLOBAL_METRICS.record_v2_read();
}

pub fn start_timing(operation: &str) -> TimingGuard<'_> {
    TimingGuard::new(operation)
}

pub fn get_metrics() -> IterationStats {
    GLOBAL_METRICS.get_metrics()
}

pub fn validate_state(node_id: u32, current_index: u32, total_count: u32, cached_neighbors_len: Option<usize>) -> ValidationReport {
    let mut report = ValidationReport::new();

    // Check for infinite loop indicators
    if current_index > total_count {
        report.add_issue(format!("Current index ({}) exceeds total count ({})", current_index, total_count));
    }

    if current_index > 1000 {
        report.add_warning("High iteration count detected - potential infinite loop".to_string());
    }

    // Check cache consistency
    if let Some(cached_len) = cached_neighbors_len {
        if cached_len != total_count as usize {
            report.add_issue(format!("Cached neighbors length ({}) doesn't match total count ({})", cached_len, total_count));
        }
    }

    report
}

pub struct TimingGuard<'a> {
    operation: &'a str,
    start_time: std::time::Instant,
}

impl<'a> TimingGuard<'a> {
    pub fn new(operation: &'a str) -> Self {
        Self {
            operation,
            start_time: std::time::Instant::now(),
        }
    }
}

impl<'a> Drop for TimingGuard<'a> {
    fn drop(&mut self) {
        let duration = self.start_time.elapsed();
        #[cfg(debug_assertions)]
        println!("DEBUG: {} completed in {:?}", self.operation, duration);
    }
}
```

## Data Structures

### AdjacencyIterator State

```rust
pub struct AdjacencyIterator<'a> {
    /// Graph file reference for I/O operations
    pub(crate) graph_file: &'a mut GraphFile,

    /// Target node identifier for adjacency traversal
    pub(crate) node_id: NativeNodeId,

    /// Traversal direction (outgoing or incoming edges)
    pub(crate) direction: Direction,

    /// Optional edge type filter for iteration
    pub(crate) edge_filter: Option<Vec<String>>,

    /// Current iteration position index
    pub(crate) current_index: u32,

    /// Total number of neighbors available
    pub(crate) total_count: u32,

    /// Cached node metadata to avoid repeated deserialization
    pub(crate) cached_node: Option<NodeRecord>,

    /// Pre-computed edge offsets from neighbor pointer table (fast path)
    pub(crate) edge_offsets: Option<Vec<FileOffset>>,

    /// Hot node metadata for fast adjacency operations
    pub(crate) node_hot: Option<NodeHot>,

    /// V2 Clustered adjacency: cached neighbors for sequential I/O
    pub(crate) cached_clustered_neighbors: Option<Vec<NativeNodeId>>,
}
```

### Edge Record Structure

```rust
pub struct EdgeRecord {
    /// Edge identifier (globally unique)
    pub id: NativeEdgeId,

    /// Source node identifier
    pub from_id: NativeNodeId,

    /// Target node identifier
    pub to_id: NativeNodeId,

    /// Edge type label
    pub edge_type: String,

    /// Edge flags (deleted, transaction state, etc.)
    pub flags: EdgeFlags,

    /// Optional edge metadata as JSON
    pub data: serde_json::Value,
}
```

### V2 Node Record Extension

```rust
// V2-specific node metadata extensions
pub trait NodeRecordV2Ext {
    /// Get outgoing edge cluster offset
    fn get_outgoing_cluster_offset(&self) -> FileOffset;

    /// Get outgoing edge cluster size
    fn get_outgoing_cluster_size(&) -> u32;

    /// Get incoming edge cluster offset
    fn get_incoming_cluster_offset(&self) -> FileOffset;

    /// Get incoming edge cluster size
    fn get_incoming_cluster_size(&self) -> u32;

    /// Get outgoing edge count
    fn get_outgoing_edge_count(&self) -> u32;

    /// Get incoming edge count
    fn get_incoming_edge_count(&self) -> u32;
}
```

## Error Handling

### Custom Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum AdjacencyError {
    #[error("V2 cluster initialization failed for node {node_id}: {reason}")]
    V2ClusterInitFailed { node_id: NativeNodeId, reason: String },

    #[error("Infinite loop detected in adjacency iteration for node {node_id}: {iterations} iterations")]
    InfiniteLoopDetected { node_id: NativeNodeId, iterations: usize },

    #[error("Inconsistent adjacency state for node {node_id}: current_index={current_index}, total_count={total_count}")]
    InconsistentState {
        node_id: NativeNodeId,
        current_index: u32,
        total_count: u32
    },

    #[error("Edge count mismatch: header={header_count}, actual={actual_count}")]
    EdgeCountMismatch { header_count: u64, actual_count: u64 },

    #[error("Circular dependency detected in adjacency operations")]
    CircularDependency,

    #[error("Header consistency violation: {details}")]
    HeaderConsistencyViolation { details: String },
}
```

### Error Recovery Strategies

1. **Graceful Degradation**: Fall back to legacy edge scanning when V2 clusters fail
2. **State Consistency**: Reset iterator state on critical errors
3. **Header Validation**: Detect and report header inconsistencies
4. **Infinite Loop Prevention**: Force termination when iteration anomalies detected

## Performance Characteristics

### Time Complexity

- **V2 Cluster Reading**: O(1) - direct memory access to cluster data
- **Legacy Edge Scanning**: O(n) where n = total number of edges
- **Header Update**: O(1) - constant time metadata update
- **Neighbor Deduplication**: O(k) where k = number of found neighbors

### Space Complexity

- **AdjacencyIterator**: O(1) + cached V2 cluster size (typically 4KB)
- **Neighbor Collection**: O(k) where k = number of neighbors
- **Debug Instrumentation**: O(1) - atomic counters and fixed-size structures

### Memory Usage

- **Base Iterator**: ~200 bytes
- **Cached V2 Cluster**: Up to 4096 bytes (configurable)
- **Debug Metrics**: ~48 bytes
- **Temporary Buffers**: Up to 256 bytes per edge record

## Configuration Options

### Debug Configuration

```rust
// Compile-time debug features
#[cfg(debug_assertions)]
const DEBUG_ADJACENCY: bool = true;

const INFINITE_LOOP_THRESHOLD: usize = 1000;
const MAX_ITERATION_WARNINGS: usize = 10;
const CLUSTER_CACHE_SIZE_LIMIT: usize = 4096;
```

### Runtime Configuration

```rust
#[derive(Debug, Clone)]
pub struct AdjacencyConfig {
    /// Enable V2 cluster reading
    pub enable_v2_clusters: bool,

    /// Maximum cluster size to cache
    pub max_cluster_cache_size: usize,

    /// Enable debug instrumentation
    pub enable_instrumentation: bool,

    /// Infinite loop detection threshold
    pub infinite_loop_threshold: usize,

    /// Enable legacy edge scanning fallback
    pub enable_legacy_fallback: bool,
}

impl Default for AdjacencyConfig {
    fn default() -> Self {
        Self {
            enable_v2_clusters: true,
            max_cluster_cache_size: 4096,
            enable_instrumentation: cfg!(debug_assertions),
            infinite_loop_threshold: 1000,
            enable_legacy_fallback: true,
        }
    }
}
```

## Testing Strategy

### Unit Tests

- **Iterator Behavior**: Test all iterator states and transitions
- **Error Handling**: Validate error propagation and recovery
- **Header Consistency**: Ensure edge count synchronization
- **Infinite Loop Prevention**: Verify termination conditions

### Integration Tests

- **End-to-End Neighbor Discovery**: Full graph traversal scenarios
- **V2 Cluster Reading**: Direct cluster data access
- **Legacy Fallback**: Graceful degradation when V2 clusters unavailable
- **Performance Benchmarks**: Verify O(1) V2 vs O(n) legacy performance

### Stress Tests

- **Large Graph Traversal**: Test with 10,000+ nodes and edges
- **Memory Pressure**: Validate behavior under memory constraints
- **Concurrent Access**: Test thread safety (if applicable)
- **Long-Running Operations**: Verify no memory leaks or resource exhaustion

### Regression Tests

- **Infinite Loop Prevention**: Ensure no regression of stack overflow issues
- **Header Consistency**: Verify edge count synchronization remains intact
- **V2 Cluster Fallback**: Test fallback mechanism under various failure scenarios

## Monitoring and Observability

### Key Metrics

```rust
#[derive(Debug, Clone)]
pub struct AdjacencyMetrics {
    /// Total adjacency operations performed
    pub total_operations: u64,

    /// Operations using V2 clusters
    pub v2_cluster_operations: u64,

    /// Operations using legacy fallback
    pub legacy_fallback_operations: u64,

    /// Average operation duration (microseconds)
    pub avg_operation_duration_us: f64,

    /// Maximum operation duration (microseconds)
    pub max_operation_duration_us: u64,

    /// Infinite loop detections prevented
    pub infinite_loop_preventions: u64,

    /// Header consistency violations detected
    pub header_violations: u64,
}
```

### Performance Monitoring

```rust
impl AdjacencyMetrics {
    pub fn record_operation(&mut self, duration: std::time::Duration, used_v2_cluster: bool, used_legacy_fallback: bool) {
        self.total_operations += 1;

        if used_v2_cluster {
            self.v2_cluster_operations += 1;
        }

        if used_legacy_fallback {
            self.legacy_fallback_operations += 1;
        }

        let duration_us = duration.as_micros() as f64;
        self.avg_operation_duration_us =
            (self.avg_operation_duration_us * (self.total_operations - 1) as f64 + duration_us) / self.total_operations as f64;

        if duration_us > self.max_operation_duration_us as f64 {
            self.max_operation_duration_us = duration_us as u64;
        }
    }

    pub fn v2_cluster_success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            0.0
        } else {
            self.v2_cluster_operations as f64 / self.total_operations as f64
        }
    }

    pub fn legacy_fallback_rate(&self) -> f64 {
        if self.total_operations == 0 {
            0.0
        } else {
            self.legacy_fallback_operations as f64 / self.total_operations as f64
        }
    }
}
```

## Security Considerations

### Input Validation

1. **Node ID Validation**: Ensure node IDs are within valid ranges
2. **Edge Count Limits**: Prevent integer overflow in edge counting
3. **Memory Bounds**: Validate cluster sizes and buffer accesses
4. **Type Safety**: Strong typing throughout prevents injection attacks

### Resource Protection

1. **Iteration Limits**: Prevent infinite loops with configurable thresholds
2. **Memory Usage**: Bound cluster caching and temporary buffer allocation
3. **File Access**: Validate file offsets and sizes before I/O operations
4. **Error Propagation**: Prevent information leakage through error messages

### Data Integrity

1. **Header Consistency**: Regular validation of file header metadata
2. **Checksum Validation**: Optional checksums for critical data structures
3. **Transaction Atomicity**: Ensure consistent state during edge operations
4. **Rollback Support**: Ability to recover from partial failures

---

*Document created: 2025-01-19*
*Version: 1.0*
*Status: Production Ready*