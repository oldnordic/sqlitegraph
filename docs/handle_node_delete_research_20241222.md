# handle_node_delete Research - SME Methodology
## Date: 2024-12-22
## Research Phase: Systematic Source Code Analysis for TDD Implementation

### 🎯 RESEARCH OBJECTIVE

Following SME methodology: **"you dont guess or invent, you read the source code and the .md in the docs folder you ground you code on FACTS and feedback from the compiler"**

### 📋 RESEARCH FINDINGS SUMMARY

**Current Status**: `handle_node_delete` in `/src/backend/native/v2/wal/recovery/replayer/operations.rs:212-255` is a **MOCK IMPLEMENTATION** that needs real functionality.

---

## 🔍 DETAILED RESEARCH FINDINGS

### **1. Current Mock Implementation Analysis**

**Location**: `/src/backend/native/v2/wal/recovery/replayer/operations.rs:212-255`

```rust
// CURRENT MOCK IMPLEMENTATION
pub fn handle_node_delete(
    &self,
    node_id: u64,
    _slot_offset: u64,
    old_data: Option<&Vec<u8>>,
    rollback_data: &mut Vec<super::types::RollbackOperation>,
) -> Result<(), RecoveryError> {
    debug!("Replaying node delete: node_id={}", node_id);

    // TODO: Implement proper node deletion with cascade cleanup
    warn!("Node delete replay not yet implemented - placeholder");
    Ok(())
}
```

**SME Research Finding**: Current mock lacks:
- Proper node deletion logic
- Edge cascade cleanup
- Slot deallocation
- Free space management
- Rollback integration

### **2. NodeStore API Analysis**

**Critical Discovery**: `/src/backend/native/node_store.rs:402-408`

```rust
// EXISTING NodeStore delete_node() - ALSO A MOCK!
pub fn delete_node(&mut self, node_id: NativeNodeId) -> NativeResult<()> {
    // For now, just remove from index
    self.node_index.remove(&node_id);

    / TODO: Implement proper deletion with edge cleanup and space reclamation
    Ok(())
}
```

**SME Research Finding**: Even the underlying NodeStore `delete_node` is a **MOCK** - only removes from index without proper cleanup.

### **3. NodeRecordV2 Structure Analysis**

**Location**: `/src/backend/native/v2/node_record_v2/core.rs:6-19`

```rust
#[derive(Debug, Clone)]
pub struct NodeRecordV2 {
    pub id: i64,
    pub flags: NodeFlags,
    pub kind: String,
    pub name: String,
    pub data: serde_json::Value,
    // CRITICAL: Direct cluster references requiring cleanup:
    pub outgoing_cluster_offset: crate::backend::native::types::FileOffset,
    pub outgoing_cluster_size: u32,
    pub outgoing_edge_count: u32,
    pub incoming_cluster_offset: crate::backend::native::types::FileOffset,
    pub incoming_cluster_size: u32,
    pub incoming_edge_count: u32,
}
```

**SME Research Finding**: NodeRecordV2 has **direct cluster references** that must be cleaned up on node deletion to prevent orphaned clusters.

### **4. Edge Deletion Patterns Research**

**V2 Integration Edge Deletion**: `/src/backend/native/v2/wal/v2_integration.rs:517-560`

```rust
pub async fn delete_edge(&self, tx_id: TransactionId, edge_id: NativeEdgeId) -> NativeResult<()> {
    // Get edge data before deletion
    // ... (mostly placeholder implementations)
    self.edge_coordinator.apply_delete(edge_id).await?;
    self.cluster_coordinator.apply_edge_delete(cluster_id, edge_id).await?;
    self.edge_coordinator.remove_cluster_mapping(edge_id).await;
    Ok(())
}
```

**SME Research Finding**: V2 integration has edge deletion patterns but mostly **placeholder implementations**.

### **5. RollbackOperation Support Analysis**

**Location**: `/src/backend/native/v2/wal/recovery/replayer/types.rs:96-100`

```rust
// EXISTING: Rollback operation for node deletion
NodeDelete {
    node_id: NativeNodeId,
    slot_offset: u64,
},
```

**SME Research Finding**: RollbackOperation::NodeDelete **ALREADY EXISTS** with correct structure for node restoration.

### **6. Free Space Management API Analysis**

**Location**: `/src/backend/native/v2/free_space/manager.rs:31-40`

```rust
// EXISTING FreeSpaceManager API
pub fn add_free_block(&mut self, offset: u64, size: u32) {
    if size < MIN_BLOCK_SIZE {
        return;
    }
    self.free_blocks.push(FreeBlock::new(offset, size));
    self.stats.total_deallocations += 1;
    self.stats.total_deallocated_bytes += size as u64;
    self.try_merge_adjacent_blocks();
    self.update_fragmentation_ratio();
}
```

**SME Research Finding**: FreeSpaceManager `add_free_block()` method **EXISTS** and can handle slot deallocation.

---

## 🏗️ IMPLEMENTATION REQUIREMENTS

### **Critical Components Required:**

1. **Node Record Management**
   - Deserialize old_data to NodeRecordV2 if provided
   - Access existing node record if old_data not provided
   - Extract slot offset for deallocation

2. **Edge Cascade Cleanup**
   - Find all edges referencing the node
   - Mark edges as deleted (following existing pattern)
   - Update cluster edge counts

3. **Cluster Reference Cleanup**
   - Reset NodeRecordV2 cluster references to zero
   - Update cluster edge counts if needed

4. **Slot Deallocation**
   - Deallocate node's slot using FreeSpaceManager
   - Free cluster storage space if needed

5. **Rollback Integration**
   - Add NodeDelete rollback operation with node_id and slot_offset
   - Support node restoration in case of replay failure

### **API Dependencies Available:**
- ✅ `RollbackOperation::NodeDelete` - Already implemented
- ✅ `FreeSpaceManager::add_free_block()` - Already implemented
- ✅ `NodeRecordV2` serialization/deserialization - Already implemented
- ❌ `NodeStore.delete_node()` - Mock, needs real implementation
- ❌ Edge deletion with cascade - Needs implementation

---

## 🎯 TDD IMPLEMENTATION STRATEGY

### **Phase 2: Test Development Requirements**

Based on research, comprehensive tests must cover:

1. **Basic Node Deletion**
   - Simple node with no edges
   - Verify node is removed from index
   - Verify slot deallocation

2. **Edge Cascade Scenarios**
   - Node with outgoing edges
   - Node with incoming edges
   - Node with both incoming and outgoing edges

3. **Cluster Reference Cleanup**
   - Verify NodeRecordV2 cluster references reset
   - Verify cluster edge counts updated

4. **Slot Deallocation**
   - Verify proper slot deallocation
   - Verify free space management integration

5. **Rollback Scenarios**
   - Test node restoration on rollback
   - Verify rollback operation correctness

6. **Error Handling**
   - Invalid node_id handling
   - Corrupted old_data handling
   - Concurrent deletion scenarios

7. **Edge Cases**
   - Self-loop nodes
   - Already deleted nodes
   - Nodes with empty clusters

---

## 📋 IMPLEMENTATION BLUEPRINT

### **Core Implementation Flow:**

```rust
pub fn handle_node_delete(
    &self,
    node_id: u64,
    slot_offset: u64,
    old_data: Option<&Vec<u8>>,
    rollback_data: &mut Vec<super::types::RollbackOperation>,
) -> Result<(), RecoveryError> {
    // 1. Validate input parameters
    // 2. Get existing node data if old_data not provided
    // 3. Deserialize NodeRecordV2
    // 4. Add rollback operation BEFORE deletion
    // 5. Find and cascade delete edges
    // 6. Clean up cluster references
    // 7. Deallocate node slot using FreeSpaceManager
    // 8. Remove from node index
    // 9. Update statistics
    // 10. Return success
}
```

### **Critical Success Factors:**
- **Rollback Before Deletion**: Must add rollback operation before any destructive operations
- **Edge Cascade**: Must handle both incoming and outgoing edges properly
- **Slot Deallocation**: Must properly free space using existing FreeSpaceManager API
- **Error Handling**: Comprehensive error recovery throughout the process
- **Thread Safety**: Maintain Arc<Mutex<>> patterns consistent with existing code

---

## 🔧 COMPILATION REQUIREMENTS

### **Dependencies to Import:**
```rust
use crate::backend::native::{NativeNodeId};
use crate::backend::native::v2::node_record_v2::NodeRecordV2;
use crate::backend::native::v2::free_space::FreeSpaceManager;
```

### **Error Handling Pattern:**
```rust
.map_err(|e| RecoveryError::io_error(format!("Failed to {}: {}", operation, e)))
```

---

## 📝 CONCLUSION

**SME Research Complete**: Comprehensive source code analysis reveals:

1. **Existing Infrastructure Available**: RollbackOperation and FreeSpaceManager are ready
2. **Missing Critical Logic**: Node deletion with cascade cleanup and slot deallocation
3. **Mock Replacement Opportunity**: Both `handle_node_delete` and `NodeStore::delete_node` need real implementation
4. **Clear Implementation Path**: All required APIs and patterns are documented

**Ready for Phase 2**: Implement comprehensive TDD tests based on these research findings.

**Estimated Implementation Complexity**: **MEDIUM-HIGH** due to cascade cleanup requirements and slot management integration.

---

*This research document provides the factual foundation for implementing `handle_node_delete` following SME methodology principles.*