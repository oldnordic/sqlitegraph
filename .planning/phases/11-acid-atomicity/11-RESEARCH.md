# Phase 11: ACID Atomicity - Research

**Researched:** 2026-01-20
**Domain:** V2 WAL Recovery - Node Deletion Rollback & Transaction State Management
**Confidence:** HIGH

## Summary

This research covers the implementation of complete ACID atomicity for node deletion operations in SQLiteGraph's V2 WAL system. The current implementation has partial rollback support but lacks complete before-image capture for node deletion (edges are not captured in the WAL record).

**Primary finding:** The `V2WALRecord::NodeDelete` record (lines 190-195 of `record.rs`) only stores `node_id`, `slot_offset`, and `old_data` (serialized node). It does NOT capture the edge data that gets deleted during the cascade. The recovery replay code in `node_ops.rs` (lines 186-242) DOES delete edges, but this data isn't captured for rollback.

**Key gap:** Edge rollback during node deletion requires capturing all incoming/outgoing edges BEFORE deletion. The current `RollbackOperation::NodeDelete` (line 232-236 of `types.rs`) only has `node_id`, `slot_offset`, and `old_data` - insufficient for edge restoration.

## Standard Stack

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `NodeRecordV2` | V2-only | Core node storage with cluster references | Current architecture |
| `EdgeCluster` | V2 | Edge storage with compact serialization | Cluster-based edge storage |
| `RollbackOperation` | existing | Rollback operation definitions | Already defined types |
| `RecoveryError` | existing | Recovery error handling | Consistent error types |

### Supporting Types
| Type | Purpose | When to Use |
|------|---------|-------------|
| `CompactEdgeRecord` | Serialized edge data | For capturing edge before-images |
| `TransactionState` | Transaction status tracking | For IN_PROGRESS detection |
| `Direction` enum | Edge direction (Outgoing/Incoming) | For edge iteration |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Single NodeDelete record | Multiple records (NodeDelete + EdgeDelete per edge) | Single record is atomic but larger; multiple records require transaction coordination |

**Installation:** No new dependencies required.

## Architecture Patterns

### Recommended Project Structure
```
sqlitegraph/src/backend/native/v2/wal/
├── record.rs                    # V2WALRecord::NodeDelete (modify)
├── recovery/
│   ├── replayer/
│   │   ├── types.rs             # RollbackOperation::NodeDelete (modify)
│   │   ├── operations/
│   │   │   ├── node_ops.rs       # handle_node_delete (modify)
│   │   │   └── edge_ops.rs       # Edge iteration patterns (reference)
│   │   └── rollback.rs          # rollback_node_delete (modify)
│   └── scanner.rs               # IN_PROGRESS handling (verify)
└── transaction_coordinator.rs  # TransactionState enum
```

### Pattern 1: Before-Image Capture for Node Deletion

**What:** Capture complete node state including all edges BEFORE any deletion.

**When to use:** During node deletion WAL record creation.

**Current implementation (INCOMPLETE):**
```rust
// sqlitegraph/src/backend/native/v2/wal/record.rs:190-195
V2WALRecord::NodeDelete {
    node_id: i64,
    slot_offset: u64,
    old_data: Vec<u8>,  // Only node data, NO edges
}
```

**Required enhancement:**
```rust
// Expanded NodeDelete record with edge capture
V2WALRecord::NodeDelete {
    node_id: i64,
    slot_offset: u64,
    old_data: Vec<u8>,           // NodeRecordV2 serialized
    outgoing_edges: Vec<CompactEdgeRecord>,  // NEW: All outgoing edges
    incoming_edges: Vec<CompactEdgeRecord>,  // NEW: All incoming edges
}
```

### Pattern 2: Edge Collection Before Deletion

**What:** Use `EdgeStore::iter_edges_with_ids()` to collect edges before deletion.

**Current reference (from node_ops.rs:196-215):**
```rust
// Collect and delete outgoing edges (edges where from_id = node_id)
if node_record.outgoing_edge_count > 0 {
    let outgoing_edges: Vec<(NativeNodeId, NativeNodeId)> = edge_store
        .iter_edges_with_ids(
            node_id as NativeNodeId,
            Direction::Outgoing
        )
        .collect();

    for (edge_id, neighbor_id) in outgoing_edges {
        if let Err(e) = edge_store.delete_edge(edge_id) {
            warn_log!("Failed to delete outgoing edge...");
        }
    }
}
```

**Enhancement needed:** Capture `CompactEdgeRecord` data before deletion:
```rust
// Read and serialize edge data BEFORE deletion
let outgoing_edges_data: Vec<CompactEdgeRecord> = edge_store
    .iter_edges_with_ids(node_id as NativeNodeId, Direction::Outgoing)
    .map(|(edge_id, _neighbor_id)| {
        // Read edge record data
        edge_store.read_edge(edge_id).unwrap()
    })
    .collect();
```

### Pattern 3: RollbackOperation for Complete Node Restoration

**What:** Expand `RollbackOperation::NodeDelete` to include edge data.

**Current (types.rs:232-236):**
```rust
NodeDelete {
    node_id: NativeNodeId,
    slot_offset: u64,
    old_data: Vec<u8>,  // Only node data
}
```

**Required:**
```rust
NodeDelete {
    node_id: NativeNodeId,
    slot_offset: u64,
    old_data: Vec<u8>,
    outgoing_edges: Vec<CompactEdgeRecord>,  // NEW
    incoming_edges: Vec<CompactEdgeRecord>,  // NEW
}
```

### Pattern 4: Rollback Edge Restoration

**What:** Restore deleted node AND all its edges during rollback.

**Current rollback_node_delete (rollback.rs:201-255):**
```rust
fn rollback_node_delete(&self, node_id: NativeNodeId, _slot_offset: u64, old_data: Vec<u8>) {
    // Step 1: Deserialize old node data
    let node_record = NodeRecordV2::deserialize(&old_data)?;

    // Step 2: Write node back
    node_store.write_node_v2(&node_record)?;

    // Missing: No edge restoration!
}
```

**Required enhancement:**
```rust
fn rollback_node_delete(&self,
    node_id: NativeNodeId,
    slot_offset: u64,
    old_data: Vec<u8>,
    outgoing_edges: Vec<CompactEdgeRecord>,  // NEW
    incoming_edges: Vec<CompactEdgeRecord>   // NEW
) {
    // Step 1: Restore node record
    let node_record = NodeRecordV2::deserialize(&old_data)?;
    node_store.write_node_v2(&node_record)?;

    // Step 2: Restore outgoing edges
    if !outgoing_edges.is_empty() {
        // Re-create outgoing cluster
        let cluster = EdgeCluster::create_from_compact_edges(
            outgoing_edges, node_id, Direction::Outgoing
        )?;
        // Allocate space and write cluster
        // Update node_record.outgoing_cluster_offset
    }

    // Step 3: Restore incoming edges
    if !incoming_edges.is_empty() {
        // Re-create incoming cluster
        let cluster = EdgeCluster::create_from_compact_edges(
            incoming_edges, node_id, Direction::Incoming
        )?;
        // Allocate space and write cluster
        // Update node_record.incoming_cluster_offset
    }

    // Step 4: Reclaim allocated slot (if needed)
    // Slot was deallocated during deletion, may need to mark as used
}
```

### Pattern 5: IN_PROGRESS Transaction Handling

**What:** WAL recovery treats IN_PROGRESS transactions as ABORTED.

**Current handling (scanner.rs:538-553):**
```rust
fn finalize_incomplete_transactions(&mut self, transactions: &mut Vec<TransactionState>, warnings: &mut Vec<String>) {
    let mut active_tx = self.active_transactions.lock();

    for (_, tx_state) in active_tx.drain() {
        warnings.push(format!("Incomplete transaction TX {} recovered", tx_state.tx_id));
        transactions.push(tx_state);  // Added with committed=false
    }
}
```

**Status:** ALREADY IMPLEMENTED - IN_PROGRESS transactions are marked as incomplete (committed=false) and included in recovery results. They will NOT be replayed (only committed transactions are replayed per core.rs:136-143).

**Verification needed:** Confirm that replay skips uncommitted transactions (core.rs:136-143 filters `tx.committed && tx.commit_lsn.is_some()`).

### Anti-Patterns to Avoid
- **Partial before-image capture:** Capturing node data but not edges
- **Edge iteration during rollback:** Reading edges from storage after deletion (they're gone!)
- **Inconsistent slot reclamation:** Not restoring slot to allocation map after rollback
- ** serde for binary data:** Using `serde_json::from_slice` for binary data (node_ops.rs:151) - should use `NodeRecordV2::deserialize`

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Edge iteration | Manual edge scanning | `EdgeStore::iter_edges_with_ids()` | Already handles direction filtering |
| Cluster serialization | Manual byte construction | `EdgeCluster::create_from_compact_edges().serialize()` | Handles V2 cluster format correctly |
| Edge data reading | Direct file reads | `EdgeStore::read_edge()` or `EdgeCluster::deserialize()` | Proper deserialization |
| Slot allocation | Manual offset tracking | `FreeSpaceManager::allocate()` | Consistent space management |

**Key insight:** The V2 clustered edge format is complex. Manual byte manipulation leads to corruption. Use the `EdgeCluster` API which handles the V2 format correctly.

## Common Pitfalls

### Pitfall 1: Incomplete Before-Image Capture
**What goes wrong:** Only node data is captured, edges are lost
**Why it happens:** `V2WALRecord::NodeDelete` doesn't include edge vectors
**How to avoid:** Expand `NodeDelete` to include `outgoing_edges` and `incoming_edges`
**Warning signs:** Rollback succeeds but graph is missing edges

### Pitfall 2: Wrong Deserialization Format
**What goes wrong:** Using `serde_json::from_slice` for binary node data (node_ops.rs:151)
**Why it happens:** Confusion between JSON and binary serialization formats
**How to avoid:** Use `NodeRecordV2::deserialize()` for binary data
**Warning signs:** Deserialization fails with "expected value" error

### Pitfall 3: Slot Reclamation Without Restoration
**What goes wrong:** Rollback restores node but slot remains marked free
**Why it happens:** `FreeSpaceManager::add_free_block()` called during deletion, not reversed during rollback
**How to avoid:** Implement slot reclamation or track deallocated slots separately
**Warning signs:** Node restored but subsequent writes may overwrite it

### Pitfall 4: Missing Cluster Floor Validation
**What goes wrong:** Allocated cluster offset is below node region
**Why it happens:** `cluster_floor` not checked during rollback
**How to avoid:** Always validate `allocated_offset >= graph_file.cluster_floor()`
**Warning signs:** `NodeRecordV2` validation fails after rollback

### Pitfall 5: Direction Enum Mismatch
**What goes wrong:** Using wrong direction value (0 vs 1, or enum vs u32)
**Why it happens:** Direction represented as both `Direction` enum and `u32`
**How to avoid:** Consistent conversion: `0 = Outgoing, 1 = Incoming`
**Warning signs:** Edges restored in wrong direction

## Code Examples

### Example 1: Complete Node Delete with Edge Capture

```rust
// Source: node_ops.rs handle_node_delete pattern (ENHANCED)
pub fn handle_node_delete(
    &self,
    node_id: u64,
    slot_offset: u64,
    old_data: Option<&Vec<u8>>,
    rollback_data: &mut Vec<RollbackOperation>,
) -> Result<(), RecoveryError> {
    // Step 1: Get node record
    let node_record = if let Some(data) = old_data {
        NodeRecordV2::deserialize(data)?
    } else {
        // Fallback: read from storage
        let mut node_store = /* ... */;
        node_store.read_node_v2(node_id as NativeNodeId)?
    };

    // Step 2: CAPTURE EDGES BEFORE DELETION (NEW)
    let mut outgoing_edges = Vec::new();
    let mut incoming_edges = Vec::new();

    {
        let mut graph_file = self.graph_file.write()?;
        let mut edge_store = EdgeStore::new(&mut *graph_file);

        // Capture outgoing edges
        if node_record.outgoing_edge_count > 0 {
            let cluster_data = Self::read_cluster_data(
                &graph_file,
                node_record.outgoing_cluster_offset,
                node_record.outgoing_cluster_size
            )?;
            let cluster = EdgeCluster::deserialize(&cluster_data)?;
            outgoing_edges = cluster.edges().to_vec();
        }

        // Capture incoming edges
        if node_record.incoming_edge_count > 0 {
            let cluster_data = Self::read_cluster_data(
                &graph_file,
                node_record.incoming_cluster_offset,
                node_record.incoming_cluster_size
            )?;
            let cluster = EdgeCluster::deserialize(&cluster_data)?;
            incoming_edges = cluster.edges().to_vec();
        }

        // Step 3: Delete edges (cascade)
        // ... existing edge deletion code ...
    }

    // Step 4: Add rollback with COMPLETE before-image
    let old_data = NodeRecordV2::serialize(&node_record)?;
    rollback_data.push(RollbackOperation::NodeDelete {
        node_id: node_id as NativeNodeId,
        slot_offset,
        old_data,
        outgoing_edges,  // NEW
        incoming_edges,  // NEW
    });

    // Step 5: Deallocate node and clusters
    // ... existing deallocation code ...

    Ok(())
}
```

### Example 2: Complete Rollback with Edge Restoration

```rust
// Source: rollback.rs rollback_node_delete (ENHANCED)
fn rollback_node_delete(&self,
    node_id: NativeNodeId,
    slot_offset: u64,
    old_data: Vec<u8>,
    outgoing_edges: Vec<CompactEdgeRecord>,  // NEW
    incoming_edges: Vec<CompactEdgeRecord>,   // NEW
) -> Result<(), RecoveryError> {
    // Step 1: Restore node record
    let node_record = NodeRecordV2::deserialize(&old_data)?;

    let mut node_store_guard = self.node_store.lock()?;
    // Initialize NodeStore if needed...

    {
        let node_store = node_store_guard.as_mut().unwrap();
        node_store.write_node_v2(&node_record)?;
    }

    // Step 2: Restore outgoing cluster (NEW)
    if !outgoing_edges.is_empty() {
        let cluster_offset = {
            let mut free_space_guard = self.free_space_manager.lock()?;
            let free_space_manager = free_space_guard.as_mut().unwrap();
            free_space_manager.allocate(/* calculate size */)?
        };

        let cluster = EdgeCluster::create_from_compact_edges(
            outgoing_edges,
            node_id as i64,
            Direction::Outgoing
        )?;
        let cluster_data = cluster.serialize();

        {
            let mut graph_file = self.graph_file.write()?;
            graph_file.write_bytes(cluster_offset, &cluster_data)?;
        }

        // Update node record with cluster reference
        let mut node_store_guard = self.node_store.lock()?;
        let node_store = node_store_guard.as_mut().unwrap();
        let mut updated_node = node_store.read_node_v2(node_id)?;
        updated_node.outgoing_cluster_offset = cluster_offset;
        updated_node.outgoing_cluster_size = cluster_data.len() as u32;
        updated_node.outgoing_edge_count = outgoing_edges.len() as u32;
        node_store.write_node_v2(&updated_node)?;
    }

    // Step 3: Restore incoming cluster (NEW)
    if !incoming_edges.is_empty() {
        // Same pattern as outgoing...
    }

    debug_log!("Successfully rolled back node delete: node_id={}, restored {} outgoing, {} incoming edges",
               node_id, outgoing_edges.len(), incoming_edges.len());

    Ok(())
}
```

### Example 3: Reading Cluster Data

```rust
// Source: edge_ops.rs pattern for cluster reading
fn read_cluster_data(
    graph_file: &GraphFile,
    cluster_offset: u64,
    cluster_size: u32,
) -> Result<Vec<u8>, RecoveryError> {
    let mut cluster_buffer = vec![0u8; cluster_size as usize];
    graph_file.read_bytes(cluster_offset, &mut cluster_buffer)?;

    // Verify format
    EdgeCluster::verify_serialized_layout(&cluster_buffer)?;

    Ok(cluster_buffer)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No rollback | Basic rollback (node only) | Phase 10 | Partial atomicity |
| Partial rollback | Complete rollback (node + edges) | Phase 11 (this) | Full atomicity |

**Deprecated/outdated:**
- `serde_json::from_slice::<NodeRecordV2>()` - Use `NodeRecordV2::deserialize()` instead (node_ops.rs:151)
- Minimal rollback records - Must capture complete before-image

**Transaction state handling (ALREADY IMPLEMENTED):**
- `TransactionState` enum (transaction_coordinator.rs:38-47): `Active`, `Preparing`, `Prepared`, `Committing`, `Committed`, `Aborting`, `Aborted`
- IN_PROGRESS detection: `finalize_incomplete_transactions()` (scanner.rs:538-553)
- Recovery filters: Only `committed && commit_lsn.is_some()` are replayed (core.rs:136-143)

## Open Questions

1. **Slot reclamation during rollback**
   - What we know: `FreeSpaceManager::add_free_block()` is called during deletion
   - What's unclear: Should rollback remove the block from free list?
   - Recommendation: Track deallocated slots per transaction, restore on rollback

2. **WAL record size limits**
   - What we know: Adding edges to `NodeDelete` increases record size significantly
   - What's unclear: Maximum WAL record size? Impact on performance?
   - Recommendation: Monitor WAL record sizes, consider compression if needed

3. **Cluster allocation during rollback**
   - What we know: Need to allocate new space for restored clusters
   - What's unclear: Should we try to reuse original offsets?
   - Recommendation: Use `FreeSpaceManager::allocate()` - original offset may be reused

4. **Cross-node edge references**
   - What we know: Edges have `neighbor_id` pointing to other nodes
   - What's unclear: What if neighbor node was also deleted?
   - Recommendation: Document that edge restoration to deleted nodes is undefined

## Sources

### Primary (HIGH confidence)
- `sqlitegraph/src/backend/native/v2/wal/record.rs:190-195` - NodeDelete WAL record definition
- `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/types.rs:232-236` - RollbackOperation::NodeDelete
- `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations/node_ops.rs:132-299` - handle_node_delete implementation
- `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/rollback.rs:201-255` - rollback_node_delete
- `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations/edge_ops.rs` - Edge operation patterns
- `sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs:38-47` - TransactionState enum
- `sqlitegraph/src/backend/native/v2/wal/recovery/scanner.rs:538-553` - IN_PROGRESS handling
- `sqlitegraph/src/backend/native/v2/node_record_v2/core.rs:6-19` - NodeRecordV2 structure
- `sqlitegraph/src/backend/native/node_store.rs:228-390` - NodeStore read/write operations

### Secondary (MEDIUM confidence)
- `sqlitegraph/src/backend/native/v2/wal/recovery/core.rs:136-143` - Committed transaction filtering
- `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/mod.rs:129-215` - Transaction replay orchestration

### Tertiary (LOW confidence)
- None - all findings verified with source code

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - verified all types exist in codebase
- Architecture: HIGH - patterns extracted from actual implementation
- Pitfalls: HIGH - based on code review and documented issues

**Research date:** 2026-01-20
**Valid until:** 30 days (stable WAL architecture)
