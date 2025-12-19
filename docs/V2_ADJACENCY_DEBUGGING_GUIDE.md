# V2 Adjacency System - Debugging Guide

## Overview

This guide provides comprehensive debugging techniques and troubleshooting strategies for the SQLiteGraph V2 adjacency system. It covers common issues, debugging tools, and step-by-step problem resolution procedures.

## Debug Output Analysis

### Key Debug Messages

The V2 adjacency system provides extensive debug output when compiled with `debug_assertions`. Look for these critical messages:

#### 1. Edge Writing and Header Updates
```
DEBUG: Before writing edge 1 - header.edge_count = 0
DEBUG: Updating header.edge_count from 0 to 1 to accommodate edge 1
DEBUG: After writing edge 1 - header.edge_count = 1
```

**Problem Indicators**:
- ❌ Header count doesn't change after edge writing
- ❌ Missing "Updating header.edge_count" messages
- ❌ Edge count remains at 0 after multiple edge writes

#### 2. V2 Cluster Reading Attempts
```
DEBUG: Reading V2 cluster for node 1 at offset 1536, size 4096
DEBUG: Failed to deserialize V2 cluster for node 1: [error message]
DEBUG: V2 cluster read failed for node 1: [error], falling back to edge store traversal
```

**Problem Indicators**:
- ⚠️ V2 cluster deserialization failures
- ⚠️ SIZE_MISMATCH errors indicating empty/unwritten clusters
- ✅ Graceful fallback to edge store is expected behavior

#### 3. Edge Scanning Results
```
DEBUG: Edge scanning - header.edge_count = 2, scanning edges 1..=2
DEBUG: Attempting to read edge 1
DEBUG: Successfully read edge 1 -> 1 (from_id=1, to_id=2)
DEBUG: Edge 1 matches direction for node 1 - neighbor 2
DEBUG: Direct edge iteration found 1 neighbors for node 1 (direction: Outgoing)
```

**Problem Indicators**:
- ❌ "header.edge_count = 0" when edges should exist
- ❌ "Failed to read edge X" messages
- ❌ "Found 0 neighbors" when neighbors should exist

#### 4. Infinite Loop Prevention
```
DEBUG: Terminating iteration early - no neighbor found at index 0 for node 1 (total_count: 1)
DEBUG: Completed collect operation for node 1 - 0 raw neighbors, 0 unique neighbors
DEBUG: Final collect metrics - iterations: 1, v2_reads: 1, loop_detections: 0
```

**Success Indicators**:
- ✅ Proper early termination when no neighbors found
- ✅ No excessive iteration counts
- ✅ Zero infinite loop detections

## Common Issues and Solutions

### Issue 1: Header Edge Count Not Updated

**Symptoms**:
```
DEBUG: Before writing edge 1 - header.edge_count = 0
DEBUG: After writing edge 1 - header.edge_count = 0  ❌
DEBUG: Edge scanning - header.edge_count = 0, scanning edges 1..=0  ❌
```

**Root Cause**: `EdgeStore::write_edge()` not updating header edge_count for manually assigned edge IDs.

**Debug Steps**:
1. Check if edge IDs are manually assigned in test code:
   ```rust
   let edge1 = EdgeRecord::new(1, 1, 2, "test".to_string(), serde_json::json!({}));
   ```

2. Verify header update logic in `write_edge_with_cluster_metadata()`:
   ```rust
   if edge.id > current_edge_count as i64 {
       println!("DEBUG: Updating header.edge_count from {} to {}", current_edge_count, edge.id);
       self.graph_file.persistent_header_mut().edge_count = edge.id as u64;
   }
   ```

3. Test fix with debug output showing proper updates.

### Issue 2: V2 Cluster Reading Fails

**Symptoms**:
```
DEBUG: Reading V2 cluster for node 1 at offset 1536, size 4096
DEBUG: Failed to deserialize V2 cluster for node 1: SIZE_MISMATCH file=... actual=4096, expected=8
DEBUG: V2 cluster read failed for node 1: ..., falling back to edge store traversal
```

**Root Cause**: V2 clusters not written or corrupted cluster data.

**Debug Steps**:
1. **Check if V2 clusters are actually written**:
   - Look for cluster writing debug messages
   - Verify edge creation includes cluster metadata updates

2. **Examine cluster data content**:
   ```bash
   # Add hex dump of cluster data in read_v2_edge_cluster_directly()
   println!("DEBUG: Cluster data (first 32 bytes): {:?}", &cluster_data[..32.min(cluster_data.len())]);
   ```

3. **Verify cluster metadata on nodes**:
   ```rust
   println!("DEBUG: Node V2 metadata - outgoing_cluster_offset: {}, outgoing_cluster_size: {}",
            node_v2.outgoing_cluster_offset, node_v2.outgoing_cluster_size);
   ```

**Expected Behavior**: V2 cluster failure should gracefully fall back to edge scanning with correct results.

### Issue 3: Edge Scanning Finds Zero Neighbors

**Symptoms**:
```
DEBUG: Edge scanning - header.edge_count = 0, scanning edges 1..=0  ❌
DEBUG: Direct edge iteration found 0 neighbors for node 1 (direction: Outgoing)  ❌
```

**Root Cause**: Header edge_count is 0, so no edges are scanned.

**Debug Steps**:
1. **Verify header edge_count is updated**:
   ```bash
   cargo test -- --nocapture 2>&1 | grep "header.edge_count"
   ```

2. **Check if edge records are actually written**:
   ```bash
   cargo test -- --nocapture 2>&1 | grep "Successfully read edge"
   ```

3. **Validate edge creation process**:
   - Ensure `EdgeStore::write_edge()` is called
   - Verify `EdgeRecordOperations::write_edge()` succeeds
   - Check header update logic executes

### Issue 4: Infinite Loop or Excessive Iterations

**Symptoms**:
```
DEBUG: Final collect metrics - iterations: 5000, v2_reads: 1, loop_detections: 5  ❌
WARNING: Collect operation shows potential infinite loop pattern for node 1
```

**Root Cause**: Inconsistent `total_count` vs actual available neighbors.

**Debug Steps**:
1. **Check iteration state consistency**:
   ```rust
   println!("DEBUG: Node {} - current_index: {}, total_count: {}, cached_len: {:?}",
            node_id, current_index, total_count, cached_neighbors_len);
   ```

2. **Verify V2 cluster initialization logic**:
   - Check if `cached_clustered_neighbors` is properly set
   - Validate `total_count` matches cached neighbors length
   - Ensure early termination conditions are working

3. **Add more detailed iteration tracking**:
   ```rust
   println!("DEBUG: Iteration {} - get_current_neighbor() returned: {:?}", iteration_count, neighbor_result);
   ```

## Debug Tools and Techniques

### 1. Environment Variables for Debug Output

```bash
# Enable maximum debug output
RUST_LOG=debug cargo test -p sqlitegraph --lib

# V2 specific debugging
V2_SLOT_DEBUG=1 cargo test -p sqlitegraph --lib

# Enable V2 instrumentation (if available)
PHASE75_INSTRUMENTATION=1 cargo test -p sqlitegraph --lib
```

### 2. Custom Debug Macros

Add to your test code for enhanced debugging:

```rust
#[macro_export]
macro_rules! debug_edge_state {
    ($graph_file:expr) => {
        println!("DEBUG: File header state:");
        println!("  - edge_count: {}", $graph_file.header().edge_count);
        println!("  - node_count: {}", $graph_file.header().node_count);
        println!("  - file_size: {}", $graph_file.file_size().unwrap_or(0));
    };
}

#[macro_export]
macro_rules! debug_iterator_state {
    ($iterator:expr) => {
        println!("DEBUG: Iterator state for node {}:");
        println!("  - current_index: {}", $iterator.current_index);
        println!("  - total_count: {}", $iterator.total_count);
        println!("  - direction: {:?}", $iterator.direction);
        println!("  - cached_neighbors: {:?}", $iterator.cached_clustered_neighbors.as_ref().map(|n| n.len()));
        println!("  - is_complete: {}", $iterator.is_complete());
    };
}
```

### 3. Step-by-Step Test Execution

Create a debug test to isolate the issue:

```rust
#[test]
fn debug_v2_adjacency_step_by_step() {
    let (mut graph_file, _temp_file) = create_test_graph_file();

    // Step 1: Create nodes with debug
    println!("=== STEP 1: Creating nodes ===");
    let node1 = NodeRecord::new(1, "Test".to_string(), "node1".to_string(), serde_json::json!({}));
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        node_store.write_node(&node1).unwrap();
        debug_edge_state!(graph_file);
    }

    // Step 2: Create edges with debug
    println!("=== STEP 2: Creating edge ===");
    let edge1 = EdgeRecord::new(1, 1, 2, "test".to_string(), serde_json::json!({}));
    {
        let mut edge_store = EdgeStore::new(&mut graph_file);
        edge_store.write_edge(&edge1).unwrap();
        debug_edge_state!(graph_file);
    }

    // Step 3: Test adjacency iteration
    println!("=== STEP 3: Testing adjacency ===");
    {
        let mut iterator = AdjacencyIterator::new_outgoing(&mut graph_file, 1).unwrap();
        debug_iterator_state!(iterator);

        let neighbors = iterator.collect().unwrap();
        println!("Neighbors found: {:?}", neighbors);
    }
}
```

### 4. Memory and File Inspection

For advanced debugging, inspect the actual file contents:

```rust
fn debug_graph_file_contents(graph_file: &GraphFile) {
    println!("=== Graph File Debug ===");

    // Header information
    let header = graph_file.header();
    println!("Header: {:?}", header);

    // Edge data area inspection
    let edge_data_offset = graph_file.persistent_header().edge_data_offset;
    let edge_count = header.edge_count;

    println!("Edge data area:");
    println!("  - offset: {}", edge_data_offset);
    println!("  - count: {}", edge_count);

    // Sample edge records
    for i in 1..=edge_count.min(3) {
        let edge_offset = edge_data_offset + ((i - 1) as u64 * 256);
        let mut edge_buffer = vec![0u8; 256];
        if graph_file.read_bytes(edge_offset, &mut edge_buffer).is_ok() {
            println!("  - Edge {} raw data: {:?}", i, &edge_buffer[..64.min(edge_buffer.len())]);
        }
    }
}
```

## Performance Debugging

### Measuring Adjacency Performance

```rust
use std::time::Instant;

fn benchmark_adjacency_operations() {
    let (mut graph_file, _temp_file) = create_large_test_graph();

    // Benchmark V2 cluster reading
    let start = Instant::now();
    let v2_neighbors = test_v2_cluster_reading(&mut graph_file);
    let v2_duration = start.elapsed();

    // Benchmark legacy edge scanning
    let start = Instant::now();
    let legacy_neighbors = test_legacy_edge_scanning(&mut graph_file);
    let legacy_duration = start.elapsed();

    println!("Performance comparison:");
    println!("  V2 cluster: {:?} ({} neighbors)", v2_duration, v2_neighbors.len());
    println!("  Legacy scan: {:?} ({} neighbors)", legacy_duration, legacy_neighbors.len());

    if v2_neighbors.len() == legacy_neighbors.len() {
        println!("  ✅ Results match");
    } else {
        println!("  ❌ Results don't match!");
    }
}
```

### Memory Usage Analysis

```rust
fn analyze_memory_usage() {
    use std::mem;

    let iterator_size = mem::size_of::<AdjacencyIterator>();
    let edge_record_size = mem::size_of::<EdgeRecord>();
    let node_record_size = mem::size_of::<NodeRecord>();

    println!("Memory usage analysis:");
    println!("  AdjacencyIterator: {} bytes", iterator_size);
    println!("  EdgeRecord: {} bytes", edge_record_size);
    println!("  NodeRecord: {} bytes", node_record_size);

    // Estimate memory usage for large graphs
    let estimated_memory = iterator_size + (4096 * 2); // Iterator + V2 clusters
    println!("  Estimated per-node usage: {} bytes", estimated_memory);
}
```

## Automated Debugging Scripts

### Health Check Script

```bash
#!/bin/bash
# v2_adjacency_health_check.sh

echo "=== V2 Adjacency System Health Check ==="

# Run tests with maximum debug output
echo "1. Running adjacency tests with debug output..."
RUST_LOG=debug V2_SLOT_DEBUG=1 cargo test -p sqlitegraph --lib test_native_bfs_simple -- --nocapture

# Check for common error patterns
echo "2. Checking for error patterns..."
cargo test -p sqlitegraph --lib 2>&1 | grep -E "(ERROR|Failed|panic)" | head -10

# Verify header consistency
echo "3. Checking header consistency updates..."
cargo test -p sqlitegraph --lib 2>&1 | grep "header.edge_count" | tail -5

# Check V2 cluster behavior
echo "4. Analyzing V2 cluster behavior..."
cargo test -p sqlitegraph --lib 2>&1 | grep -E "(V2 cluster|falling back)" | head -5

echo "Health check complete!"
```

### Debug Output Analyzer

```python
#!/usr/bin/env python3
# analyze_v2_debug_output.py

import re
import sys
from collections import defaultdict

def analyze_debug_output(output_text):
    results = {
        'header_edge_count_issues': [],
        'v2_cluster_failures': [],
        'edge_scanning_issues': [],
        'infinite_loop_detections': [],
        'success_indicators': []
    }

    lines = output_text.split('\n')

    for line in lines:
        # Header edge count issues
        if 'header.edge_count = 0' in line and 'After writing edge' in line:
            results['header_edge_count_issues'].append(line)

        # V2 cluster failures
        if 'Failed to deserialize V2 cluster' in line:
            results['v2_cluster_failures'].append(line)

        # Edge scanning issues
        if 'Direct edge iteration found 0 neighbors' in line:
            results['edge_scanning_issues'].append(line)

        # Infinite loop detections
        if 'infinite_loop_detections' in line and 'loop_detections: 0' not in line:
            results['infinite_loop_detections'].append(line)

        # Success indicators
        if 'Successfully read edge' in line or 'V2 clustered adjacency SUCCESS' in line:
            results['success_indicators'].append(line)

    return results

def print_analysis(results):
    print("=== V2 Adjacency Debug Analysis ===")

    if results['header_edge_count_issues']:
        print("\n❌ Header Edge Count Issues:")
        for issue in results['header_edge_count_issues']:
            print(f"  {issue}")

    if results['v2_cluster_failures']:
        print("\n⚠️  V2 Cluster Failures:")
        for failure in results['v2_cluster_failures']:
            print(f"  {failure}")

    if results['edge_scanning_issues']:
        print("\n❌ Edge Scanning Issues:")
        for issue in results['edge_scanning_issues']:
            print(f"  {issue}")

    if results['infinite_loop_detections']:
        print("\n🚨 Infinite Loop Detections:")
        for detection in results['infinite_loop_detections']:
            print(f"  {detection}")

    if results['success_indicators']:
        print("\n✅ Success Indicators:")
        for success in results['success_indicators'][:5]:  # Limit output
            print(f"  {success}")

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python3 analyze_v2_debug_output.py <debug_output_file>")
        sys.exit(1)

    with open(sys.argv[1], 'r') as f:
        output_text = f.read()

    results = analyze_debug_output(output_text)
    print_analysis(results)
```

## Integration with Development Workflow

### Pre-commit Validation

Add to your `.git/hooks/pre-commit`:

```bash
#!/bin/bash
# V2 adjacency validation before commits

echo "Running V2 adjacency system validation..."

# Run tests with required output
if ! RUST_LOG=debug cargo test -p sqlitegraph --lib test_native_bfs_simple > /tmp/v2_test_output 2>&1; then
    echo "❌ V2 adjacency tests failed!"
    cat /tmp/v2_test_output
    exit 1
fi

# Check for critical issues
if grep -q "header.edge_count = 0" /tmp/v2_test_output; then
    echo "❌ Header edge count not being updated!"
    exit 1
fi

if grep -q "infinite_loop_detections: [1-9]" /tmp/v2_test_output; then
    echo "❌ Infinite loops detected!"
    exit 1
fi

echo "✅ V2 adjacency system validation passed"
```

### Continuous Integration Integration

Add to your CI pipeline:

```yaml
# .github/workflows/v2-adjacency-validation.yml
name: V2 Adjacency System Validation

on: [push, pull_request]

jobs:
  validate-v2-adjacency:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v3
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable

    - name: Run V2 adjacency tests
      run: |
        RUST_LOG=debug V2_SLOT_DEBUG=1 cargo test -p sqlitegraph --lib test_native_bfs_simple -- --nocapture > v2_test_output.txt 2>&1

    - name: Analyze debug output
      run: |
        python3 scripts/analyze_v2_debug_output.py v2_test_output.txt

    - name: Check for regressions
      run: |
        if grep -q "header.edge_count = 0" v2_test_output.txt; then
          echo "❌ Header edge count regression detected"
          exit 1
        fi

        if grep -q "infinite_loop_detections: [1-9]" v2_test_output.txt; then
          echo "❌ Infinite loop regression detected"
          exit 1
        fi

        echo "✅ V2 adjacency system validation passed"
```

---

*Document created: 2025-01-19*
*Version: 1.0*
*Status: Production Ready*