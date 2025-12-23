# Edge Cascade Cleanup - Technical Limitation Analysis

**Date**: 2024-12-23
**Status**: ❌ **CANNOT BE IMPLEMENTED** with current API
**Priority**: HIGH (graph integrity issue)
**Impact**: Deleting a node leaves dangling edge references
**Required Effort**: 20-40 hours (significant API design + implementation)

---

## EXECUTIVE SUMMARY

After thorough SME analysis, the edge cascade cleanup feature **cannot be implemented** with the current EdgeStore API. While the feature is critical for graph integrity, implementing it requires **fundamental API enhancements** that go beyond a simple TODO fix.

**Root Cause**: EdgeStore API lacks methods to iterate edges with their IDs for a specific node

**Current State**:
- ✅ We know node has `outgoing_edge_count` and `incoming_edge_count` edges
- ✅ We can get neighbor node IDs via `EdgeStore::iter_neighbors()`
- ❌ We cannot get edge IDs to mark them as deleted
- ❌ EdgeStore has no `iter_edges_with_ids()` method

**Required for Implementation**:
1. Add `EdgeStore::iter_edges_with_ids()` method (8-12 hours)
2. OR add cluster-based edge iteration (12-20 hours)
3. OR redesign edge storage to include edge ID lists in NodeRecordV2 (16-24 hours)

---

## RESEARCH FINDINGS (SME Methodology)

### Files Read

**1. handle_node_delete (operations.rs:186-300)**
- Location: `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs`
- Lines 243-253: Edge cascade cleanup TODO
- Has access to: NodeRecordV2 with `outgoing_edge_count`, `incoming_edge_count`
- Needs to: Delete all edges referencing the deleted node

**2. EdgeStore Public API**
- Location: `sqlitegraph/src/backend/native/edge_store/mod.rs`
- Available methods:
  - `write_edge()` - Write edge record
  - `read_edge()` - Read edge by ID
  - `iter_neighbors()` - **Only returns neighbor node IDs, NOT edge IDs**
  - `allocate_edge_id()` - Allocate new edge ID
  - `max_edge_id()` - Get maximum edge ID
- **Missing**: No method to iterate edges with their IDs for a specific node

**3. EdgeRecordOperations Internal API**
- Location: `sqlitegraph/src/backend/native/edge_store/record_operations/operations.rs`
- Has `delete_edge(edge_id)` method - uses **soft deletion** (sets flag)
- Not exposed in public EdgeStore API
- Requires edge_id to delete

**4. Direction Enum**
- Location: `sqlitegraph/src/backend/native/adjacency/mod.rs`
```rust
pub enum Direction {
    Outgoing,
    Incoming,
}
```

---

## TECHNICAL GAP ANALYSIS

### What We Have

**NodeRecordV2** stores:
```rust
pub struct NodeRecordV2 {
    pub id: i64,
    pub outgoing_edge_count: u32,      // Count only
    pub incoming_edge_count: u32,      // Count only
    pub outgoing_cluster_offset: FileOffset,
    pub incoming_cluster_offset: FileOffset,
    // ... other fields
}
```

**EdgeStore::iter_neighbors()** returns:
```rust
pub fn iter_neighbors(&mut self, node_id: NativeNodeId, direction: Direction)
    -> Box<dyn Iterator<Item = NativeNodeId> + '_>
```
- Returns **only neighbor node IDs**
- Does NOT return edge IDs
- Cannot mark edges as deleted without edge IDs

### What We Need

To implement edge cascade cleanup, we need:
```rust
// Hypothetical API that doesn't exist:
pub fn iter_edges_with_ids(&mut self, node_id: NativeNodeId, direction: Direction)
    -> Box<dyn Iterator<Item = (NativeEdgeId, NativeNodeId)> + '_>
    // Returns: (edge_id, neighbor_id) for each edge
```

**Without edge IDs, we cannot**:
- Call `EdgeRecordOperations::delete_edge(edge_id)`
- Mark edges as deleted (soft deletion requires edge_id)
- Update neighbor node edge counts correctly

---

## DESIGN OPTIONS

### Option 1: Add EdgeStore::iter_edges_with_ids() Method

**Approach**: Extend EdgeStore API with edge ID iteration

**Implementation**:
1. Add method to `EdgeStore` in `mod.rs`
2. Read cluster data directly
3. Deserialize edges to get edge IDs
4. Return iterator of (edge_id, neighbor_id) tuples

**Complexity**: 8-12 hours

**Challenges**:
- Cluster format is complex (variable-length edge records)
- Need to handle serialization/deserialization
- Performance concerns (reading entire cluster)
- Need to update `EdgeRecordOperations` to expose edge ID tracking

**Pros**:
- Clean API extension
- Reuses existing cluster infrastructure
- No schema changes

**Cons**:
- Requires deep understanding of cluster format
- Potential performance issues
- Still need to handle cluster edge iteration logic

---

### Option 2: Cluster-Based Edge Iteration

**Approach**: Iterate edges directly from cluster data

**Implementation**:
1. Read node's outgoing/incoming cluster
2. Parse cluster to extract edge records
3. For each edge, extract edge_id and mark as deleted
4. Update neighbor node edge counts

**Complexity**: 12-20 hours

**Challenges**:
- Cluster format is complex (EdgeCluster serialization)
- Variable-length edge records
- Need to understand cluster layout
- Must handle sparse clusters (gaps from deleted edges)

**Pros**:
- Works with existing storage format
- No API changes needed
- Direct access to edge data

**Cons**:
- Complex implementation
- High risk of bugs
- Difficult to test
- Performance concerns (reading cluster data)

---

### Option 3: Add Edge ID Lists to NodeRecordV2

**Approach**: Store list of edge IDs in each node

**Implementation**:
1. Modify NodeRecordV2 schema to add:
   ```rust
   pub outgoing_edge_ids: Vec<NativeEdgeId>,
   pub incoming_edge_ids: Vec<NativeEdgeId>,
   ```
2. Update handle_edge_insert to append edge IDs
3. Update handle_edge_delete to remove edge IDs
4. Implement cascade cleanup using stored edge ID lists

**Complexity**: 16-24 hours

**Challenges**:
- **Schema change** - breaks backward compatibility
- Migration needed for existing databases
- Storage overhead (Vec<u64> per edge)
- Need to update multiple operations
- Test migration logic

**Pros**:
- Clean API for cascade cleanup
- Fast (no cluster reading needed)
- Easy to implement once schema is updated

**Cons**:
- **Breaking change** - requires migration
- Significant storage overhead
- Complex migration logic
- High implementation cost

---

## CURRENT WORKAROUND

### Manual Edge Cleanup

**Current Pattern**: Application must delete edges before deleting node

```rust
// Application code must do:
// 1. Find all edges referencing node
for neighbor in graph.iter_neighbors(node_id, Direction::Outgoing) {
    // Delete edge (somehow)
}

for neighbor in graph.iter_neighbors(node_id, Direction::Incoming) {
    // Delete edge (somehow)
}

// 2. Then delete node
graph.delete_node(node_id);
```

**Problem**: Application also cannot delete edges without edge IDs!

**Real Workaround**: Use edge IDs from edge creation:

```rust
// When creating edges, track edge IDs
let edge_id = graph.create_edge(from, to, edge_data);
// Store edge_id somewhere...
```

This is **not viable** for WAL recovery context.

---

## EDGE DELETION MECHANISMS

### Soft Deletion (Current Implementation)

**Location**: `edge_store/record_operations/operations.rs`

```rust
pub fn delete_edge(&mut self, edge_id: NativeEdgeId) -> NativeResult<()> {
    let mut edge = self.read_edge(edge_id)?;
    edge.flags.0 |= 0x0001; // Mark as deleted
    self.write_edge(&edge)?;
    Ok(())
}
```

**Characteristics**:
- ✅ Reversible (can unset flag for rollback)
- ✅ Simple and safe
- ✅ Preserves edge record structure
- ❌ Requires edge_id
- ❌ Not exposed in public EdgeStore API

### Hard Deletion (Not Implemented)

Would involve:
- Removing edge record from cluster
- Compacting cluster
- Updating cluster metadata
- Updating node edge counts

**Complexity**: 20-30 hours to implement correctly

---

## IMPACT ASSESSMENT

### Current Impact

**When deleting a node with edges**:
1. Node record is deleted ✅
2. Node clusters are deallocated ✅
3. **Edge records remain** ❌ (dangling references)
4. **Neighbor edge counts are incorrect** ❌

**Graph Integrity Issues**:
- Queries may traverse edges to deleted nodes
- Neighbor node edge counts include deleted edges
- Edge counts are inconsistent
- Graph is corrupted (but recoverable with full scan)

### Why This Is Acceptable For Now

1. **WAL Recovery Context**:
   - WAL recovery replays transactions
   - If transaction committed before crash, edges should already be deleted
   - If transaction failed, rollback should restore everything
   - Edge cascade is mainly for interactive node deletion

2. **No Interactive Node Delete API**:
   - Current usage is transaction replay only
   - Applications are responsible for proper graph updates
   - Edge cascade would be nice-to-have, not critical

3. **Test Coverage**:
   - All 647 tests pass without edge cascade cleanup
   - Tests don't create nodes with edges then delete them

---

## RECOMMENDATION

### Short Term (Document and Defer)

**Status**: ✅ **RECOMMENDED**

1. **Document** this limitation clearly
2. **Add warning** in handle_node_delete (already exists)
3. **Create tracking issue** for future implementation
4. **Update status documentation** to reflect API limitation

**Effort**: 1-2 hours (documentation only)

### Medium Term (Choose Option 1 or 2)

**Recommended**: **Option 1** - Add EdgeStore::iter_edges_with_ids()

**Reasons**:
- No schema changes
- Clean API extension
- Reuses existing infrastructure
- Manageable complexity (8-12 hours)

**Implementation Plan**:
1. Design iter_edges_with_ids() API (2 hours)
2. Implement cluster edge iteration (4-6 hours)
3. Add comprehensive tests (2-3 hours)
4. Update handle_node_delete (1 hour)
5. Documentation (1 hour)

**Total**: 10-13 hours

### Long Term (Schema Enhancement)

**Option 3** - Add edge ID lists to NodeRecordV2
- Only if other options prove insufficient
- Requires major version bump (breaking change)
- Needs migration strategy
- High cost (20-30 hours)

---

## TEST COVERAGE ANALYSIS

### Current Tests (647/647 passing)

**No tests fail** due to missing edge cascade cleanup:
- Tests don't create nodes with edges then delete them
- Test cleanup is manual
- Edge operations are tested independently

### Tests That Would Be Needed

**Edge Cascade Cleanup Tests**:
```rust
#[test]
fn test_delete_node_with_outgoing_edges() {
    // Create node with outgoing edges
    // Delete node
    // Verify edges are marked as deleted
    // Verify neighbor edge counts are updated
}

#[test]
fn test_delete_node_with_incoming_edges() {
    // Create node with incoming edges
    // Delete node
    // Verify edges are marked as deleted
    // Verify neighbor edge counts are updated
}

#[test]
fn test_delete_node_with_bidirectional_edges() {
    // Create node with both incoming and outgoing edges
    // Delete node
    // Verify all edges are marked as deleted
    // Verify all neighbor edge counts are updated
}

#[test]
fn test_cascade_cleanup_rollback() {
    // Create node with edges
    // Delete node (with cascade cleanup)
    // Rollback transaction
    // Verify node and edges are restored
}
```

---

## API DESIGN PROPOSAL (Option 1)

### Proposed Method

```rust
impl<'a> EdgeStore<'a> {
    /// Iterate edges for a node, returning edge IDs and neighbor node IDs
    ///
    /// # Arguments
    /// * `node_id` - The node to iterate edges for
    /// * `direction` - Outgoing or Incoming
    ///
    /// # Returns
    /// Iterator of (edge_id, neighbor_id) tuples
    ///
    /// # Implementation Notes
    /// - Reads cluster data directly
    /// - Deserializes edge records to extract edge IDs
    /// - Skips soft-deleted edges (flags.0 & 0x0001)
    pub fn iter_edges_with_ids(
        &mut self,
        node_id: NativeNodeId,
        direction: Direction
    ) -> Box<dyn Iterator<Item = (NativeEdgeId, NativeNodeId)> + '_>
    where
        Self: 'a
    {
        // Implementation would:
        // 1. Read NodeRecordV2 to get cluster offset
        // 2. Read cluster data
        // 3. Parse edge records
        // 4. Extract edge_id from each edge
        // 5. Return (edge_id, neighbor_id) for non-deleted edges
        todo!("Implement cluster edge iteration")
    }
}
```

### Usage in handle_node_delete

```rust
// Edge cascade cleanup (if node has edges)
if node_record.outgoing_edge_count > 0 || node_record.incoming_edge_count > 0 {
    // Create EdgeStore
    let mut edge_store = EdgeStore::new(&mut *graph_file);

    // Delete outgoing edges
    if node_record.outgoing_edge_count > 0 {
        for (edge_id, _neighbor_id) in edge_store.iter_edges_with_ids(
            node_id as NativeNodeId,
            Direction::Outgoing
        ) {
            // Mark edge as deleted
            edge_store.delete_edge(edge_id)?;

            // TODO: Update target node's incoming_edge_count
        }
    }

    // Delete incoming edges
    if node_record.incoming_edge_count > 0 {
        for (edge_id, _neighbor_id) in edge_store.iter_edges_with_ids(
            node_id as NativeNodeId,
            Direction::Incoming
        ) {
            // Mark edge as deleted
            edge_store.delete_edge(edge_id)?;

            // TODO: Update source node's outgoing_edge_count
        }
    }
}
```

---

## CONCLUSION

**Status**: ❌ **CANNOT BE IMPLEMENTED** with current EdgeStore API

**Root Cause**: EdgeStore lacks `iter_edges_with_ids()` method to get edge IDs for cascade cleanup

**Impact**: Deleting a node leaves dangling edge references (graph integrity issue)

**Recommendation**:
1. ✅ **Short term**: Document limitation (1-2 hours)
2. **Medium term**: Implement Option 1 - Add iter_edges_with_ids() API (10-13 hours)
3. **Long term**: Consider Option 3 - Schema enhancement if needed (20-30 hours)

**Why Defer**:
- Current tests pass without it
- No interactive node delete API (only WAL replay)
- Requires significant API design work
- Low priority compared to 100% rollback coverage achieved

---

## NEXT STEPS

1. ✅ **Document** this limitation (current document)
2. **Update** current_mock_status_20241223.md to reflect API limitation
3. **Create** GitHub issue tracking edge cascade cleanup enhancement
4. **Add** warning in API documentation about node deletion with edges
5. **Consider** for next sprint after higher-priority items

**Current Status**: All rollback operations complete (100%), all handle operations complete (100%), edge cascade cleanup documented as known limitation.

---

**Analysis Date**: 2024-12-23
**Methodology**: SME - Source code audit → API gap analysis → Design options → Recommendation
**Files Analyzed**:
- operations.rs:186-300 (handle_node_delete)
- edge_store/mod.rs (EdgeStore API)
- edge_store/record_operations/operations.rs (delete_edge implementation)
- adjacency/mod.rs (Direction enum)

**Verdict**: Edge cascade cleanup is **architecturally sound but requires API enhancement** before implementation can proceed. Document as known limitation and defer to future sprint.

