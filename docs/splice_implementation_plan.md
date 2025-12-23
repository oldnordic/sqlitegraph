# Splice Implementation Plan for Remaining Mocks

**Date**: 2024-12-23
**Status**: Ready for implementation
**Test Status**: ✅ 647/647 tests passing (100%)
**Splice Version**: 0.1.0

---

## EXECUTIVE SUMMARY

Now that Splice is available and tested, we can systematically eliminate the remaining mocks and placeholders in the V2 WAL Recovery system.

**Current State**:
- ✅ 91% operation implementation (10/11)
- ⚠️ 55% rollback implementation (6/11)
- ✅ 100% test coverage

**Goal**: Complete all placeholders and reach 100% implementation

---

## PRIORITY 1: TRANSACTION INTEGRITY (HIGH)

### Target: Edge Operation Rollbacks

**Files**:
- `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/rollback.rs`

**Impact**: Cannot roll back edge operations during failed transactions

#### 1.1 `rollback_edge_insert` (Line 390-394)

**Current Code**:
```rust
fn rollback_edge_insert(&self, _cluster_key: (u64, u64), _insertion_point: u32, _edge_record: &[u8])
    -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError>
{
    // TODO: Implement rollback_edge_insert with cluster modification
    debug!("Rolling back edge insert (placeholder)");
    Ok(())
}
```

**Implementation Plan with Splice**:

1. **Research Phase**:
   ```bash
   # Read edge insert implementation to understand what needs to be rolled back
   rg "handle_edge_insert" sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs -A 50

   # Read cluster modification logic
   rg "modify_cluster" sqlitegraph/src/backend/native/v2/ -A 20
   ```

2. **Create Implementation File** (`/tmp/rollback_edge_insert.rs`):
   - Implement edge removal from cluster
   - Update cluster metadata (decrement edge count)
   - Handle cluster compaction if needed
   - Write changes back to GraphFile

3. **Apply with Splice**:
   ```bash
   splice patch \
     --file sqlitegraph/src/backend/native/v2/wal/recovery/replayer/rollback.rs \
     --symbol rollback_edge_insert \
     --kind function \
     --with /tmp/rollback_edge_insert.rs \
     --verbose
   ```

4. **Test**:
   ```bash
   cargo test rollback_edge_insert
   cargo test edge_operations
   ```

#### 1.2 `rollback_edge_update` (Line 401-407)

**Current Code**:
```rust
// TODO: Implement comprehensive edge update rollback
// This would involve:
// 1. Locating the edge cluster identified by cluster_key
// 2. Finding the edge at the specified position
// 3. Restoring the old edge data
// 4. Updating cluster if size changed
// 5. Writing back to GraphFile
```

**Implementation Plan with Splice**:

Same workflow as 1.1:
1. Research edge update implementation
2. Create implementation file
3. Apply with Splice
4. Test

#### 1.3 `rollback_edge_delete` (Line 441-447)

**Current Code**:
```rust
// TODO: Implement comprehensive edge delete rollback
// This would involve:
// 1. Locating the edge cluster identified by cluster_key
// 2. Re-inserting the old edge at the specified position
// 3. Updating cluster metadata
// 4. Writing back to GraphFile
```

**Implementation Plan with Splice**:

Same workflow as 1.1-1.2

---

## PRIORITY 2: DATA INTEGRITY (HIGH)

### Target: Node Delete Cleanup

**File**: `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs`

#### 2.1 Edge Cascade Cleanup (Line 239-244)

**Current Code**:
```rust
// TODO: Implement edge cascade deletion
// This is a placeholder for edge cleanup - would integrate with EdgeStore
// For now, we log the requirement and proceed with node deletion
warn!("Edge cascade cleanup not yet implemented - node {} had {} outgoing, {} incoming edges",
      node_id, outgoing_count, incoming_count);
```

**Implementation Plan with Splice**:

1. **Research Edge Deletion**:
   ```bash
   # Read edge delete implementation
   rg "handle_edge_delete" sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs -A 50

   # Understand edge iteration logic
   rg "iter_edges" sqlitegraph/src/backend/native/v2/ -A 10
   ```

2. **Create Implementation File** (`/tmp/edge_cascade_cleanup.rs`):
   - Iterate through all edges pointing to/from deleted node
   - Delete each edge using EdgeStore
   - Update source/target node edge counts
   - Log cleanup progress

3. **Apply with Splice**:
   ```bash
   # Need to update handle_node_delete function
   splice patch \
     --file sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs \
     --symbol handle_node_delete \
     --kind function \
     --with /tmp/handle_node_delete_with_cleanup.rs \
     --verbose
   ```

4. **Test**:
   ```bash
   cargo test handle_node_delete
   cargo test node_delete_with_edges
   ```

#### 2.2 Cluster Reference Cleanup (Line 251-255)

**Current Code**:
```rust
// TODO: Implement cluster reference cleanup
// This would involve updating cluster metadata and potentially deallocating cluster storage
// For now, we log the requirement
debug!("Cluster reference cleanup not yet implemented for node {}", node_id);
```

**Implementation Plan with Splice**:

Same file as 2.1 (handle_node_delete function), just add cleanup logic:

1. **Research Cluster Management**:
   ```bash
   rg "FreeSpaceManager" sqlitegraph/src/backend/native/v2/ -A 10
   rg "deallocate_cluster" sqlitegraph/src/backend/native/v2/ -A 10
   ```

2. **Update Implementation**:
   - Call FreeSpaceManager to deallocate cluster storage
   - Update cluster metadata

---

## PRIORITY 3: COMPLETENESS (MEDIUM)

### Target: Header Update and Cluster Rollback

#### 3.1 `handle_header_update` (Line 1486-1497)

**Status**: ✅ Already enhanced with documentation via Splice!
**Next Step**: Replace documentation with actual implementation

**Implementation Plan**:

1. **Research Header Management**:
   ```bash
   rg "GraphFile.*header" sqlitegraph/src/backend/native/ -A 10
   rg "HEADER_SIZE" sqlitegraph/src/backend/native/graph_file/ -B 2 -A 2
   ```

2. **Create Implementation File** (`/tmp/handle_header_update_impl.rs`):
   - Validate header_offset is within valid header region
   - Verify new_data size doesn't exceed header bounds
   - Perform atomic write to GraphFile header
   - Store rollback operation with old_data if provided

3. **Apply with Splice**:
   ```bash
   splice patch \
     --file sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs \
     --symbol handle_header_update \
     --kind function \
     --with /tmp/handle_header_update_impl.rs \
     --verbose
   ```

4. **Test**:
   ```bash
   cargo test handle_header_update
   ```

#### 3.2 `rollback_cluster_create` (Line 115-117)

**Current Code**:
```rust
RollbackOperation::ClusterCreate { node_id, direction: _direction, cluster_offset, cluster_size: _cluster_size, cluster_data: _cluster_data } => {
    // TODO: Implement cluster creation rollback
    debug!("Rollback cluster creation for node {} at offset {} (not yet implemented)", node_id, cluster_offset);
```

**Implementation Plan**:

1. **Research Cluster Deallocation**:
   ```bash
   rg "deallocate_cluster" sqlitegraph/src/backend/native/v2/ -A 20
   ```

2. **Create Implementation**:
   - Deallocate cluster storage
   - Update node cluster reference

3. **Apply with Splice**

4. **Test**

#### 3.3 Complete `rollback_node_delete` (Line 200-217)

**Current State**: Partially implemented (verifies slot, but doesn't reinsert node)

**Implementation Plan**:

1. **Research Node Insert Logic**:
   ```bash
   rg "handle_node_insert" sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs -A 50
   ```

2. **Create Complete Implementation**:
   - Reinsert node data at slot_offset
   - Update node metadata

3. **Apply with Splice**

4. **Test**

---

## SPLICE WORKFLOW TEMPLATES

### Template 1: Single Function Implementation

```bash
# Step 1: Research existing code
rg "<symbol_name>" <file_path> -A 50

# Step 2: Create implementation file
cat > /tmp/<implementation_name>.rs << 'EOF'
// Implementation code here
EOF

# Step 3: Commit current state (safety)
git add -A
git commit -m "Pre-splice snapshot: <symbol_name>"

# Step 4: Apply with Splice
splice patch \
  --file <file_path> \
  --symbol <symbol_name> \
  --kind function \
  --with /tmp/<implementation_name>.rs \
  --verbose

# Step 5: Verify compilation
cargo check --package sqlitegraph

# Step 6: Run tests
cargo test <test_name>

# Step 7: Update documentation
# Edit docs/mock_implementation_status_WORKING_20241223.md
```

### Template 2: Multi-Function Plan

```bash
# Create JSON plan file
cat > /tmp/rollback_implementation_plan.json << 'EOF'
{
  "steps": [
    {
      "file": "sqlitegraph/src/backend/native/v2/wal/recovery/replayer/rollback.rs",
      "symbol": "rollback_edge_insert",
      "kind": "function",
      "with": "/tmp/rollback_edge_insert.rs"
    },
    {
      "file": "sqlitegraph/src/backend/native/v2/wal/recovery/replayer/rollback.rs",
      "symbol": "rollback_edge_update",
      "kind": "function",
      "with": "/tmp/rollback_edge_update.rs"
    },
    {
      "file": "sqlitegraph/src/backend/native/v2/wal/recovery/replayer/rollback.rs",
      "symbol": "rollback_edge_delete",
      "kind": "function",
      "with": "/tmp/rollback_edge_delete.rs"
    }
  ]
}
EOF

# Execute plan
splice plan --file /tmp/rollback_implementation_plan.json

# Verify
cargo test rollback_operations
```

---

## SUCCESS CRITERIA

### Phase 1 - Transaction Integrity (Priority 1)

**Target**: Complete edge operation rollbacks

**Success Criteria**:
- ✅ `rollback_edge_insert` fully implemented and tested
- ✅ `rollback_edge_update` fully implemented and tested
- ✅ `rollback_edge_delete` fully implemented and tested
- ✅ All edge rollback tests passing
- ✅ No decrease in existing test coverage (647/647)
- ✅ Documentation updated in `docs/mock_implementation_status_WORKING_20241223.md`

**Test Commands**:
```bash
cargo test rollback_edge_insert
cargo test rollback_edge_update
cargo test rollback_edge_delete
cargo test edge_operations
cargo test --package sqlitegraph  # All tests
```

### Phase 2 - Data Integrity (Priority 2)

**Target**: Complete node delete cleanup

**Success Criteria**:
- ✅ Edge cascade cleanup implemented in `handle_node_delete`
- ✅ Cluster reference cleanup implemented in `handle_node_delete`
- ✅ No dangling edges after node deletion
- ✅ No memory leaks from deleted clusters
- ✅ All node delete tests passing
- ✅ New tests for edge cascade cleanup
- ✅ Documentation updated

**Test Commands**:
```bash
cargo test handle_node_delete
cargo test node_delete_with_edges
cargo test node_delete_cleanup
```

### Phase 3 - Completeness (Priority 3)

**Target**: Complete remaining placeholders

**Success Criteria**:
- ✅ `handle_header_update` fully implemented
- ✅ `rollback_cluster_create` fully implemented
- ✅ `rollback_node_delete` completed
- ✅ All rollback operations implemented
- ✅ 100% operation implementation (11/11)
- ✅ 100% rollback implementation (11/11)
- ✅ All tests passing
- ✅ Documentation updated to show 100% completion

**Test Commands**:
```bash
cargo test --package sqlitegraph  # All 647+ tests
cargo test handle_header_update
cargo test rollback_cluster_create
cargo test rollback_node_delete
```

---

## TRACKING PROGRESS

### Implementation Progress Matrix

| Priority | Item | File | Status | Test | Notes |
|----------|------|------|--------|------|-------|
| P1-HIGH | rollback_edge_insert | rollback.rs:390 | TODO | - | Ready to implement |
| P1-HIGH | rollback_edge_update | rollback.rs:401 | TODO | - | Ready to implement |
| P1-HIGH | rollback_edge_delete | rollback.rs:441 | TODO | - | Ready to implement |
| P2-HIGH | Edge cascade cleanup | operations.rs:239 | TODO | - | Requires handle_node_delete update |
| P2-HIGH | Cluster cleanup | operations.rs:251 | TODO | - | Requires handle_node_delete update |
| P3-MED | handle_header_update | operations.rs:1486 | Doc'd | - | Docs added via Splice, impl next |
| P3-MED | rollback_cluster_create | rollback.rs:115 | TODO | - | Ready to implement |
| P3-MED | rollback_node_delete | rollback.rs:200 | Partial | - | Needs completion |

### Current Metrics

- **Operations Implementation**: 10/11 (91%)
- **Rollback Implementation**: 6/11 (55%)
- **Test Coverage**: 647/647 (100%)
- **Splice-Ready**: ✅ All remaining items can be implemented with Splice

---

## NEXT ACTIONS

### Immediate (Today)

1. ✅ **Complete Splice testing** - DONE
2. ✅ **Create implementation plan** - DONE (this document)
3. **Start Priority 1**: Implement `rollback_edge_insert`
   - Research code: `rg "handle_edge_insert" ...`
   - Create implementation file
   - Apply with Splice
   - Verify with tests
   - Update documentation

### This Week

4. Complete Priority 1: All edge rollbacks
5. Start Priority 2: Node delete cleanup
6. Update documentation after each implementation

### Next Week

7. Complete Priority 2
8. Start Priority 3: Header update and remaining rollbacks
9. Final testing and validation

---

## DOCUMENTATION UPDATES

After each implementation, update:

1. **`docs/mock_implementation_status_WORKING_20241223.md`**:
   - Move item from "TODO" to "Fully Implemented"
   - Update implementation percentages
   - Add test results

2. **`docs/splice_test_summary.md`**:
   - Add implementation success story
   - Document lessons learned

3. **CHANGELOG.md**:
   - Add entry for each completed implementation
   - Cite Splice as the implementation tool

---

## RISK MITIGATION

### Pre-Implementation Checklist

- [ ] Git commit created before Splice operation
- [ ] Implementation file created in `/tmp/` (not in repo)
- [ ] Existing tests pass: `cargo test --package sqlitegraph`
- [ ] Research completed on related code
- [ ] Implementation plan reviewed

### Post-Implementation Checklist

- [ ] Splice patch applied successfully
- [ ] Compilation verified: `cargo check --package sqlitegraph`
- [ ] Tests pass: `cargo test <test_name>`
- [ ] No regression in existing tests (647/647 still passing)
- [ ] Documentation updated
- [ ] Git commit created after implementation

### Rollback Plan

If something goes wrong:
```bash
# Option 1: Git revert (safest)
git revert HEAD

# Option 2: Splice doesn't have undo, so git reset
git reset --hard HEAD~1

# Option 3: Manual fix if small issue
# Edit file directly and re-test
```

---

## CONCLUSION

With Splice now available and tested, we have a safe, efficient way to complete all remaining mocks and placeholders. The plan is organized by priority:

1. **Priority 1 (HIGH)**: Transaction integrity - Complete edge rollbacks
2. **Priority 2 (HIGH)**: Data integrity - Complete node delete cleanup
3. **Priority 3 (MEDIUM)**: Completeness - Header update and remaining rollbacks

Each implementation follows the same Splice workflow:
1. Research existing code
2. Create implementation file
3. Apply with Splice
4. Verify with tests
5. Update documentation

**Goal**: 100% implementation of all operations and rollbacks with 100% test coverage.

**Next Action**: Start with `rollback_edge_insert` (Priority 1.1)
