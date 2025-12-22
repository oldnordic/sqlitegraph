# V2 WAL Record Structure Analysis

## Overview

This document provides a comprehensive analysis of the current V2WALRecord structure to ensure proper alignment between the WAL record format and the V2GraphIntegrator during modularization.

## Current V2WALRecord Structure

Based on analysis of `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/v2/wal/record.rs`.

### Edge Records

#### EdgeInsert
```rust
EdgeInsert {
    cluster_key: (i64, Direction), // (node_id, direction)
    edge_record: CompactEdgeRecord,
    insertion_point: u32,
}
```

**Key Changes from Expected Format:**
- Uses `cluster_key` tuple instead of separate `source_node`, `target_node` fields
- Uses `edge_record` (CompactEdgeRecord) instead of raw `edge_data` bytes
- Includes `insertion_point` for positioning within cluster

#### EdgeUpdate
```rust
EdgeUpdate {
    cluster_key: (i64, Direction), // (node_id, direction)
    old_edge: CompactEdgeRecord,
    new_edge: CompactEdgeRecord,
    position: u32,
}
```

**Key Changes from Expected Format:**
- Uses `cluster_key` tuple instead of separate `source_node`, `target_node` fields
- Uses `old_edge` and `new_edge` (CompactEdgeRecord) instead of raw data bytes
- Includes `position` for location within cluster

#### EdgeDelete
```rust
EdgeDelete {
    cluster_key: (i64, Direction), // (node_id, direction)
    old_edge: CompactEdgeRecord,
    position: u32,
}
```

**Key Changes from Expected Format:**
- Uses `cluster_key` tuple instead of separate `source_node`, `target_node` fields
- Uses `old_edge` (CompactEdgeRecord) instead of raw `edge_data` bytes
- Includes `position` for location within cluster

### String Table Records

#### StringInsert (not StringTableInsert)
```rust
StringInsert {
    string_id: u32,
    string_value: String,
}
```

**Key Changes from Expected Format:**
- Named `StringInsert` not `StringTableInsert`
- Uses `string_id: u32` not `string_id: u64`

### Free Space Records

#### FreeSpaceAllocate
```rust
FreeSpaceAllocate {
    block_offset: u64,
    block_size: u32,
    block_type: u8,
}
```

**Key Changes from Expected Format:**
- Uses `block_offset` not `region_offset`
- Uses `block_size` not `region_size`
- Includes additional `block_type: u8` field

#### FreeSpaceDeallocate
```rust
FreeSpaceDeallocate {
    block_offset: u64,
    block_size: u32,
    block_type: u8,
}
```

### Node Records (Unchanged)

#### NodeInsert
```rust
NodeInsert {
    node_id: i64,
    slot_offset: u64,
    node_data: Vec<u8>,
}
```

#### NodeUpdate
```rust
NodeUpdate {
    node_id: i64,
    slot_offset: u64,
    old_data: Vec<u8>,
    new_data: Vec<u8>,
}
```

#### NodeDelete
```rust
NodeDelete {
    node_id: i64,
    slot_offset: u64,
    old_data: Vec<u8>,
}
```

### Cluster Records

#### ClusterCreate
```rust
ClusterCreate {
    node_id: i64,
    direction: Direction,
    cluster_offset: u64,
    cluster_size: u32,
    edge_data: Vec<u8>,
}
```

## Required V2GraphIntegrator Updates

### Method Signature Changes

1. **Edge Insert Handler**:
   - Current: `apply_edge_insert(source_node, target_node, edge_data, direction, lsn)`
   - Required: `apply_edge_insert_v2(node_id, direction, edge_record, lsn)`

2. **Edge Update Handler**:
   - Current: `apply_edge_update(source_node, target_node, new_data, direction, lsn)`
   - Required: `apply_edge_update_v2(node_id, direction, old_edge, new_edge, lsn)`

3. **Edge Delete Handler**:
   - Current: `apply_edge_delete(source_node, target_node, direction, lsn)`
   - Required: `apply_edge_delete_v2(node_id, direction, old_edge, lsn)`

4. **String Table Handler**:
   - Current: `apply_string_table_insert(string_id, string_value, lsn)`
   - Required: `apply_string_insert(string_id, string_value, lsn)`

5. **Free Space Handler**:
   - Current: `apply_free_space_allocate(region_offset, region_size, lsn)`
   - Required: `apply_free_space_allocate(block_offset, block_size, block_type, lsn)`

### Implementation Strategy

1. **Preserve Existing Methods**: Keep current methods for backward compatibility during transition
2. **Add New V2 Methods**: Create new methods with proper V2 clustered edge handling
3. **Update Pattern Matching**: Modify match arms to call appropriate V2 methods
4. **Test Integration**: Ensure all new methods properly integrate with V2 backend components

## Dependencies

### Required Imports
```rust
use crate::backend::native::v2::edge_cluster::{CompactEdgeRecord, Direction};
```

### CompactEdgeRecord Structure
```rust
pub struct CompactEdgeRecord {
    /// Neighbor node ID (target for outgoing, source for incoming).
    pub neighbor_id: i64,
    /// Edge type offset inside the shared string table.
    pub edge_type_offset: u16,
    /// Serialized JSON payload for the edge.
    pub edge_data: Vec<u8>,
}
```

**Key Implications:**
- `neighbor_id` provides the actual target/source node connection
- `edge_type_offset` is a u16 offset into string table, not direct string
- `edge_data` contains serialized JSON payload
- Cluster key `(node_id, direction)` combined with `neighbor_id` gives full edge connection

### Backend Integration
- Methods must work with `EdgeStore` that understands `CompactEdgeRecord`
- String table must handle `u32` string IDs
- Free space manager must handle `block_type` classification

## Next Steps

1. Update V2GraphIntegrator pattern matching to use correct field names
2. Implement new V2-specific handler methods
3. Test integration with existing V2 backend components
4. Ensure all WAL record types are properly handled during checkpoint operations

---
*Analysis Date: 2025-12-21*
*Source File: `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/v2/wal/record.rs`*