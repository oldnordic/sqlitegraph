# Edge Storage Capacity Coordination TDD Implementation Plan

**Date**: 2025-12-18
**Methodology**: Test-Driven Development (TDD)
**Scope**: Fix "Attempted read beyond end of file" issues with real implementation
**Constraint**: Files under 300 lines, no mocks/stubs, real integration tests

---

## 🎯 **Current Architecture Analysis**

### **Existing Code Structure**:
```
sqlitegraph/src/backend/native/edge_store/
├── mod.rs              (84 lines) - EdgeStore facade
├── id_management.rs    (420 lines) - EdgeIdManager, AdjacencyAllocator
├── record_operations.rs (630 lines) - EdgeRecordOperations (CRUD, serialization)
├── utils.rs            (modularized)
└── cluster_utils.rs    (190 lines)
```

### **Key Problem Identified**:

**In `record_operations.rs:43-55` (write_edge method)**:
```rust
// Calculate offset for this edge (fixed-size slot)
let offset = self.edge_offset(edge.id);  // ❌ NO CAPACITY CHECK
let fixed_slot_size = 256usize;

// Write to file
self.graph_file.write_bytes(offset, &buffer)?;  // ❌ FAILS IF FILE TOO SMALL
```

**In `record_operations.rs:85-90` (read_edge method)**:
```rust
// Calculate offset for this edge (fixed-size slot)
let offset = self.edge_offset(edge_id);  // ❌ NO CAPACITY CHECK

// Read the entire fixed-size slot
let mut buffer = vec![0u8; fixed_slot_size];
self.graph_file.read_bytes(offset, &mut buffer)?;  // ❌ FAILS IF FILE TOO SMALL
```

**Root Cause**: **No coordination between edge ID allocation and file size**.

### **Current Offset Calculation** (`record_operations.rs:177-182`):
```rust
fn edge_offset(&self, edge_id: NativeEdgeId) -> FileOffset {
    let base_offset = self.graph_file.persistent_header().edge_data_offset;
    // Fixed-size edge records: 256 bytes per edge
    base_offset + ((edge_id - 1) as u64 * 256)  // ❌ Assumes file is large enough
}
```

---

## 🏗️ **TDD Implementation Strategy**

### **Phase 1: Test Infrastructure (Real, No Mocks)**

**Create dedicated folder structure**:
```
sqlitegraph/src/backend/native/edge_store/capacity_coordinator/
├── mod.rs              (public interface, under 300 lines)
├── coordinator.rs      (core coordination logic, under 300 lines)
├── growth_strategy.rs  (growth algorithms, under 300 lines)
└── tests/
    ├── integration_tests.rs    (real file operations)
    └── regression_tests.rs     (prevent future regressions)
```

### **Phase 2: TDD Test Cases (Real Integration Tests)**

**Test 1: Capacity Coordination Integration**
```rust
// Real file, no mocks
#[test]
fn test_allocate_edge_id_ensures_file_capacity() {
    // Create real temporary file
    let temp_file = NamedTempFile::new().unwrap();
    let mut graph_file = GraphFile::create(temp_file.path()).unwrap();

    // Allocate edge ID should ensure file capacity
    let edge_id = graph_file.edge_store_allocate_edge_with_capacity().unwrap();

    // File should be large enough for this edge
    let edge_offset = calculate_edge_offset(edge_id);
    let file_size = graph_file.file_size().unwrap();
    assert!(file_size >= edge_offset + 256);
}
```

**Test 2: Multi-Edge Capacity Growth**
```rust
#[test]
fn test_multiple_edge_allocation_grows_file_appropriately() {
    let temp_file = NamedTempFile::new().unwrap();
    let mut graph_file = GraphFile::create(temp_file.path()).unwrap();

    // Allocate multiple edges
    let mut edge_ids = Vec::new();
    for _ in 0..10 {
        let edge_id = graph_file.edge_store_allocate_edge_with_capacity().unwrap();
        edge_ids.push(edge_id);
    }

    // Verify file is large enough for all edges
    let file_size = graph_file.file_size().unwrap();
    let max_edge_id = *edge_ids.iter().max().unwrap();
    let max_offset = calculate_edge_offset(max_edge_id);
    assert!(file_size >= max_offset + 256);
}
```

**Test 3: Edge Write Success After Capacity Coordination**
```rust
#[test]
fn test_edge_write_succeeds_after_capacity_ensured() {
    let temp_file = NamedTempFile::new().unwrap();
    let mut graph_file = GraphFile::create(temp_file.path()).unwrap();

    // Allocate edge with capacity coordination
    let edge_id = graph_file.edge_store_allocate_edge_with_capacity().unwrap();
    let edge = EdgeRecord {
        id: edge_id,
        from_id: 1,
        to_id: 2,
        edge_type: "TEST".to_string(),
        flags: EdgeFlags(0),
        data: serde_json::json!({"test": true}),
    };

    // This should not fail
    graph_file.write_edge(&edge).unwrap();

    // Verify we can read it back
    let read_edge = graph_file.read_edge(edge_id).unwrap();
    assert_eq!(read_edge.id, edge_id);
}
```

### **Phase 3: Implementation (Real Code, No Placeholders)**

**Step 1: Create Capacity Coordinator Module**
```rust
// sqlitegraph/src/backend/native/edge_store/capacity_coordinator/coordinator.rs
use crate::backend::native::graph_file::GraphFile;
use crate::backend::native::types::{NativeResult, NativeEdgeId};

/// Coordinates edge ID allocation with file capacity management
pub struct EdgeCapacityCoordinator<'a> {
    graph_file: &'a mut GraphFile,
}

impl<'a> EdgeCapacityCoordinator<'a> {
    pub fn new(graph_file: &'a mut GraphFile) -> Self {
        Self { graph_file }
    }

    /// Allocate edge ID with guaranteed file capacity
    pub fn allocate_edge_id_with_capacity(&mut self) -> NativeResult<NativeEdgeId> {
        // Get current edge count to allocate next ID
        let current_count = self.graph_file.persistent_header().edge_count;
        let new_edge_id = current_count + 1;

        // Ensure file has capacity for this edge
        self.ensure_capacity_for_edge_id(new_edge_id)?;

        // Update edge count in persistent header
        self.graph_file.persistent_header_mut().edge_count = new_edge_id;

        Ok(new_edge_id as NativeEdgeId)
    }

    /// Ensure file is large enough for edge with given ID
    fn ensure_capacity_for_edge_id(&mut self, edge_id: u64) -> NativeResult<()> {
        const EDGE_SLOT_SIZE: u64 = 256;

        let edge_offset = self.calculate_edge_offset(edge_id);
        let required_size = edge_offset + EDGE_SLOT_SIZE;

        let current_file_size = self.graph_file.file_size()?;

        if current_file_size < required_size {
            let growth_amount = self.calculate_growth_amount(required_size);
            self.graph_file.grow(growth_amount)?;
        }

        Ok(())
    }

    /// Calculate file offset for edge
    fn calculate_edge_offset(&self, edge_id: u64) -> u64 {
        let base_offset = self.graph_file.persistent_header().edge_data_offset;
        base_offset + ((edge_id - 1) * 256)
    }

    /// Calculate how much to grow file by
    fn calculate_growth_amount(&self, required_size: u64) -> u64 {
        let current_size = self.graph_file.file_size().unwrap();

        // Stepped growth strategy for efficiency
        match required_size {
            0..=4096 => 4096,      // Grow to 4KB
            4097..=16384 => 16384, // Grow to 16KB
            16385..=65536 => 65536, // Grow to 64KB
            65537..=262144 => 262144, // Grow to 256KB
            _ => {
                // For larger sizes, grow to next multiple of 1MB
                ((required_size + 1048576 - 1) / 1048576) * 1048576 - current_size
            }
        }
    }
}
```

**Step 2: Integrate with EdgeStore**
```rust
// Update mod.rs to use capacity coordinator
impl<'a> EdgeStore<'a> {
    /// Allocate edge ID with capacity coordination
    pub fn allocate_edge_id(&mut self) -> NativeEdgeId {
        use crate::backend::native::edge_store::capacity_coordinator::EdgeCapacityCoordinator;

        let mut coordinator = EdgeCapacityCoordinator::new(self.graph_file);
        coordinator.allocate_edge_id_with_capacity().unwrap()
    }
}
```

**Step 3: Fix Record Operations to Use Coordinated IDs**
```rust
// Update record_operations.rs to add capacity check
impl<'a> EdgeRecordOperations<'a> {
    pub fn write_edge(&mut self, edge: &EdgeRecord) -> NativeResult<()> {
        // Validate edge fields
        self.validate_edge_fields(edge)?;

        // CRITICAL: Ensure capacity before writing
        self.ensure_capacity_for_edge(edge.id)?;

        // Rest of existing write logic...
        let buffer = self.serialize_edge()?;
        let offset = self.edge_offset(edge.id);
        self.graph_file.write_bytes(offset, &buffer)?;

        Ok(())
    }

    /// Ensure file has capacity for this edge
    fn ensure_capacity_for_edge(&mut self, edge_id: NativeEdgeId) -> NativeResult<()> {
        use crate::backend::native::edge_store::capacity_coordinator::EdgeCapacityCoordinator;

        let mut coordinator = EdgeCapacityCoordinator::new(self.graph_file);
        coordinator.ensure_capacity_for_edge_id(edge_id as u64)
    }
}
```

### **Phase 4: Regression Tests**

**Test 4: Prevent Future Regressions**
```rust
#[test]
fn test_edge_operations_never_fail_with_capacity_coordination() {
    let temp_file = NamedTempFile::new().unwrap();
    let mut graph_file = GraphFile::create(temp_file.path()).unwrap();

    // Test many edge operations - none should fail with capacity errors
    for i in 1..100 {
        let edge_id = graph_file.edge_store_allocate_edge_with_capacity().unwrap();

        let edge = EdgeRecord {
            id: edge_id,
            from_id: i as i64,
            to_id: (i + 1) as i64,
            edge_type: format!("EDGE_{}", i),
            flags: EdgeFlags(0),
            data: serde_json::json!({"index": i}),
        };

        // These should never fail
        graph_file.write_edge(&edge).unwrap();
        let read_edge = graph_file.read_edge(edge_id).unwrap();
        assert_eq!(read_edge.id, edge_id);
    }
}
```

---

## 📋 **Implementation Checklist**

### **Real File Operations Required**:
- [ ] Create `capacity_coordinator/` folder structure
- [ ] Implement `EdgeCapacityCoordinator` with real file operations
- [ ] Write integration tests using real temporary files
- [ ] Update `EdgeStore::allocate_edge_id()` to use coordinator
- [ ] Update `EdgeRecordOperations::write_edge()` to ensure capacity
- [ ] Add capacity check to `EdgeRecordOperations::read_edge()`
- [ ] Write regression tests to prevent future failures
- [ ] Verify all failing tests now pass

### **No Mocks/Stubs Policy**:
- [ ] Use real `NamedTempFile` for all test file operations
- [ ] Use real `GraphFile::create()` for initialization
- [ ] Use real file I/O operations (`read_bytes`, `write_bytes`, `grow`)
- [ ] Test with actual edge record serialization/deserialization

### **File Size Constraints**:
- [ ] `coordinator.rs` under 300 lines
- [ ] `growth_strategy.rs` under 300 lines
- [ ] `mod.rs` under 300 lines
- [ ] Each test file under 300 lines

---

## 🎯 **Success Criteria**

### **Test Success**:
- All 5 currently failing edge storage tests pass
- No "Attempted read beyond end of file" errors
- Integration tests verify real file operations work
- Regression tests prevent future issues

### **Implementation Quality**:
- All modules under 300 lines
- Real file operations only (no mocks)
- TDD approach followed (tests first, then implementation)
- Comprehensive integration and regression test coverage

### **Architectural Integrity**:
- Existing API preserved
- Backward compatibility maintained
- Performance impact minimal
- Code quality high with proper error handling

---

**Next Step**: Create the folder structure and begin TDD implementation starting with the integration test that verifies capacity coordination works with real file operations.

---

**Status**: ✅ **PLAN COMPLETE - READY FOR TDD IMPLEMENTATION**