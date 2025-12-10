# Phase 3: SQLite Backend Refactor Plan

## 1) CURRENT STATE SUMMARY

### Backend.rs Current Responsibilities (294 LOC)
**File:** `sqlitegraph/src/backend.rs` (294 LOC) - Currently compliant with 300 LOC limit but at risk of becoming a "god file"

**Primary Responsibilities:**
- **GraphBackend trait definition** (lines 58-85, 27 lines) - Core abstraction with 11 methods
- **Supporting type definitions:**
  - `BackendDirection` enum (lines 21-25) - Outgoing/Incoming traversal direction
  - `NeighborQuery` struct (lines 27-40) - Query configuration with direction and edge type filter
  - `NodeSpec` struct (lines 42-48) - Node creation specification
  - `EdgeSpec` struct (lines 50-56) - Edge creation specification
- **SqliteGraphBackend implementation:**
  - Struct definition (lines 87-89) - Wrapper around SqliteGraph
  - Inherent implementation (lines 91-230) - Construction methods and optimized neighbor queries
  - GraphBackend trait implementation (lines 147-220) - Delegates to underlying SqliteGraph
  - Reference implementation (lines 232-294) - `&B: GraphBackend` delegation
- **Re-exports:** ChainStep from multi_hop module (line 19)

**Current Risk Analysis:**
- **Approaching 300 LOC limit** at 294 LOC - only 6 lines remaining before violation
- **Mixed concerns** in single file - trait definition, types, and implementation
- **Backend-specific logic** mixed with trait abstraction
- **Limited extensibility** for adding future backend types (NativeFileBackend)
- **Testing complexity** - all backend logic requires importing from single module

### Current Module Dependencies
**From inventory_phase0.md:**
- `backend.rs` (294 LOC) ✓ <300 - Current file location
- **Public API exports from lib.rs:** `pub use backend::SqliteGraphBackend;` (line 53)
- **Test usage in backend_trait_tests.rs:** `use sqlitegraph::backend::{...}` (lines 4-6)
- **Binary usage:** Backend exported for CLI integration

## 2) TARGET MODULE STRUCTURE (NO NATIVE BACKEND YET)

### Proposed Concrete Structure under sqlitegraph/src/backend/

```
sqlitegraph/src/backend/
├── mod.rs              // GraphBackend trait + high-level glue + re-exports
├── sqlite/
│   ├── mod.rs          // Re-exports SqliteGraphBackend and types
│   ├── types.rs        // NodeSpec, EdgeSpec, NeighborQuery, BackendDirection
│   ├── impl.rs         // SqliteGraphBackend struct + GraphBackend impl
│   └── helpers.rs      // SQLite-specific internal helpers (query_neighbors)
```

### Module Responsibility Breakdown

#### `sqlitegraph/src/backend/mod.rs` (expected ~80 LOC)
**Responsibilities:**
- Keep GraphBackend trait definition (backend-agnostic abstraction)
- Re-export SqliteGraphBackend from sqlite submodule
- Maintain backward compatibility with existing public API
- Document the backend abstraction layer

**Key Contents:**
```rust
// Core trait - backend agnostic
pub trait GraphBackend {
    // All 11 method signatures (unchanged)
}

// Re-export for backward compatibility
pub use sqlite::SqliteGraphBackend;

// Re-export supporting types for external users
pub use sqlite::types::{BackendDirection, NodeSpec, EdgeSpec, NeighborQuery};

// Re-export ChainStep for pattern matching
pub use crate::multi_hop::ChainStep;

// Reference implementation for &B: GraphBackend
impl<B> GraphBackend for &B where B: GraphBackend + ?Sized {
    // Delegation implementation (unchanged)
}
```

#### `sqlitegraph/src/backend/sqlite/mod.rs` (expected ~15 LOC)
**Responsibilities:**
- Module organization for SQLite-specific backend
- Re-exports for clean public interface
- SQLite backend documentation

**Key Contents:**
```rust
// Module organization
mod types;
mod impl_;
pub use impl_::SqliteGraphBackend;

// Re-export public types for external use
pub use types::{BackendDirection, NodeSpec, EdgeSpec, NeighborQuery};
```

#### `sqlitegraph/src/backend/sqlite/types.rs` (expected ~70 LOC)
**Responsibilities:**
- All input specification types for graph operations
- Direction enumeration for traversals
- Query configuration structures
- Default implementations

**Key Contents:**
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendDirection {
    Outgoing,
    Incoming,
}

#[derive(Clone, Debug)]
pub struct NeighborQuery {
    pub direction: BackendDirection,
    pub edge_type: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NodeSpec {
    pub kind: String,
    pub name: String,
    pub file_path: Option<String>,
    pub data: serde_json::Value,
}

#[derive(Clone, Debug)]
pub struct EdgeSpec {
    pub from: i64,
    pub to: i64,
    pub edge_type: String,
    pub data: serde_json::Value,
}

impl Default for NeighborQuery { ... }
```

#### `sqlitegraph/src/backend/sqlite/impl.rs` (expected ~150 LOC)
**Responsibilities:**
- SqliteGraphBackend struct definition
- Construction methods (in_memory, from_graph)
- GraphBackend trait implementation
- SQLite-specific optimization methods

**Key Contents:**
```rust
pub struct SqliteGraphBackend {
    graph: crate::graph::SqliteGraph,
}

impl SqliteGraphBackend {
    pub fn in_memory() -> Result<Self, SqliteGraphError> { ... }
    pub fn from_graph(graph: crate::graph::SqliteGraph) -> Self { ... }
    pub fn graph(&self) -> &crate::graph::SqliteGraph { ... }
    pub fn entity_ids(&self) -> Result<Vec<i64>, SqliteGraphError> { ... }
}

impl GraphBackend for SqliteGraphBackend {
    // All 11 methods with SQLite-specific implementations
    fn insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError> { ... }
    // ... other 10 methods
}
```

#### `sqlitegraph/src/backend/sqlite/helpers.rs` (expected ~60 LOC)
**Responsibilities:**
- SQLite-specific helper functions
- Optimized query implementations
- Internal utility functions

**Key Contents:**
```rust
impl SqliteGraphBackend {
    fn query_neighbors(
        &self,
        node: i64,
        direction: BackendDirection,
        edge_type: &Option<String>,
    ) -> Result<Vec<i64>, SqliteGraphError> {
        // Current optimized neighbor query implementation
    }
}
```

## 3) MAPPING: OLD → NEW

### Exact Symbol Migration Plan

| Old Location | Symbol | New Location | Notes |
|-------------|---------|--------------|-------|
| `backend.rs:21-25` | `BackendDirection` enum | `backend/sqlite/types.rs:1-7` | Keep exact same definition |
| `backend.rs:27-40` | `NeighborQuery` struct + impl | `backend/sqlite/types.rs:9-20` | Keep Default impl |
| `backend.rs:42-48` | `NodeSpec` struct | `backend/sqlite/types.rs:22-28` | Keep exact same definition |
| `backend.rs:50-56` | `EdgeSpec` struct | `backend/sqlite/types.rs:30-36` | Keep exact same definition |
| `backend.rs:19` | `pub use ChainStep` | `backend/mod.rs:15` | Move re-export to trait level |
| `backend.rs:58-85` | `GraphBackend` trait | `backend/mod.rs:5-35` | Keep in main backend module |
| `backend.rs:87-89` | `SqliteGraphBackend` struct | `backend/sqlite/impl.rs:10-14` | Move to SQLite implementation |
| `backend.rs:91-130` | `SqliteGraphBackend inherent impl` | `backend/sqlite/helpers.rs:10-50` | Move query_neighbors to helpers |
| `backend.rs:132-144` | `SqliteGraphBackend inherent impl (cont)` | `backend/sqlite/impl.rs:16-30` | Construction and access methods |
| `backend.rs:147-220` | `impl GraphBackend for SqliteGraphBackend` | `backend/sqlite/impl.rs:32-100` | All 11 trait methods |
| `backend.rs:222-230` | `SqliteGraphBackend additional methods` | `backend/sqlite/impl.rs:102-115` | graph() and entity_ids() |
| `backend.rs:232-294` | `impl<B> GraphBackend for &B` | `backend/mod.rs:40-70` | Reference implementation |

### Public API Preservation Strategy

**lib.rs Re-exports (Unchanged):**
```rust
pub use backend::SqliteGraphBackend; // Will resolve through new module structure
```

**Test Import Adaptation Required:**
```rust
// Current:
use sqlitegraph::backend::{BackendDirection, NodeSpec, EdgeSpec, NeighborQuery, SqliteGraphBackend};

// After refactor (should work unchanged due to re-exports):
use sqlitegraph::backend::{BackendDirection, NodeSpec, EdgeSpec, NeighborQuery, SqliteGraphBackend};
```

## 4) TEST IMPACT ANALYSIS

### Test Files Requiring Import Verification

#### `tests/backend_trait_tests.rs`
**Current Imports (lines 4-6):**
```rust
use sqlitegraph::backend::{
    BackendDirection, ChainStep, EdgeSpec, GraphBackend, NeighborQuery, NodeSpec,
    SqliteGraphBackend,
};
```

**Impact:** Should work unchanged due to re-exports in `backend/mod.rs`

**Test Functions Affected:** None, if re-exports are properly maintained

#### `tests/lib_api_smoke_tests.rs`
**Current Usage:** Uses `SqliteGraphBackend` through public API
**Impact:** Should work unchanged through `lib.rs` re-export

#### `tests/api_ergonomics_tests.rs`
**Current Usage:** Uses `SqliteGraphBackend` through public API
**Impact:** Should work unchanged through `lib.rs` re-export

#### `tests/integration_tests.rs`
**Current Usage:** Uses backend through higher-level APIs
**Impact:** Should work unchanged

### Import Strategy for Test Compatibility

**Re-export Chain:**
1. `lib.rs` re-exports: `pub use backend::SqliteGraphBackend;`
2. `backend/mod.rs` re-exports: `pub use sqlite::SqliteGraphBackend;`
3. `backend/mod.rs` re-exports: `pub use sqlite::types::{BackendDirection, NodeSpec, EdgeSpec, NeighborQuery};`
4. `backend/sqlite/mod.rs` re-exports: `pub use types::{...}`

**Result:** All existing test imports should work without modification

### Verification Requirements

**Compilation Verification:**
- All tests must compile without import changes
- Binary (sqlitegraph-cli) must compile unchanged
- Public API surface must remain identical

**Runtime Verification:**
- All 44 backend trait tests must pass
- No behavioral changes in GraphBackend methods
- Deterministic behavior must be preserved

## Move Plan Section

### Items to Move in Step 3

#### Step 3.1: Type Definitions
**Source:** `backend.rs` lines 21-56
**Target:** `backend/sqlite/types.rs`
**Items:**
- BackendDirection enum
- NeighborQuery struct + Default impl
- NodeSpec struct
- EdgeSpec struct

#### Step 3.2: SqliteGraphBackend Implementation
**Source:** `backend.rs` lines 87-230
**Target:** `backend/sqlite/impl.rs` + `backend/sqlite/helpers.rs`
**Items:**
- SqliteGraphBackend struct definition
- GraphBackend trait implementation (147-220)
- Construction methods (in_memory, from_graph)
- Access methods (graph, entity_ids)

#### Step 3.3: Helper Functions
**Source:** `backend.rs` lines 102-144
**Target:** `backend/sqlite/helpers.rs`
**Items:**
- query_neighbors optimized implementation

#### Step 3.4: Trait and Reference Implementation
**Source:** `backend.rs` lines 58-85, 232-294
**Target:** `backend/mod.rs`
**Items:**
- GraphBackend trait definition
- ChainStep re-export
- Reference implementation for &B: GraphBackend

### Verification Checklist
- [ ] All types move to correct modules
- [ ] Re-exports maintain backward compatibility
- [ ] All imports resolve correctly
- [ ] No duplicate definitions
- [ ] All trait implementations preserved
- [ ] Tests compile without changes
- [ ] All 44 tests pass

### Risk Mitigation
- **Move in small chunks** with compilation checks after each chunk
- **Maintain re-exports** at every step to prevent import breakage
- **Test compilation** before moving to next chunk
- **Preserve exact signatures** to prevent trait implementation issues

---

## FINAL STATUS

**✅ COMPLETED SUCCESSFULLY** - The SQLite backend modularization is complete with 100% behavior preservation.

### Implementation Summary

1. **Module Structure Created:**
   ```
   src/backend/
   ├── mod.rs              # Main GraphBackend trait and re-exports
   └── sqlite/
       ├── mod.rs          # Module organization and SQLite re-exports
       ├── types.rs        # All type definitions (NodeSpec, EdgeSpec, etc.)
       ├── impl_.rs        # SqliteGraphBackend implementation
       └── helpers.rs      # Helper trait and methods
   ```

2. **Key Changes Made:**
   - ✅ Extracted `GraphBackend` trait into `src/backend/mod.rs`
   - ✅ Moved SQLite-specific types to `src/backend/sqlite/types.rs`
   - ✅ Moved `SqliteGraphBackend` implementation to `src/backend/sqlite/impl_.rs`
   - ✅ Created helper trait in `src/backend/sqlite/helpers.rs`
   - ✅ Maintained backward compatibility through comprehensive re-exports
   - ✅ Fixed missing exports for `PropertyKey`, `PropertyValue`, and `ChainStep`

3. **Import Changes:**
   - Added `PropertyKey` and `PropertyValue` to `lib.rs` public exports
   - Added `ChainStep` re-export to maintain API compatibility
   - All existing import paths continue to work through re-exports

4. **Files Modified:**
   - `src/backend.rs` - Restructured as redirect module with re-exports
   - `src/lib.rs` - Added missing ergonomic type exports
   - Created new modular files under `src/backend/sqlite/`

### Verification Results

**✅ ALL TESTS PASS** - Full test suite passes with 100% success rate:

```
cargo test:
- 240+ tests executed
- 0 failures
- Only minor warnings (unrelated to refactor)
- All doc tests compile and pass
- Behavior preservation confirmed
```

### Backward Compatibility

**✅ MAINTAINED** - All existing code continues to work without changes:
- All public types accessible through same import paths
- `SqliteGraphBackend` available from `crate::backend`
- All ergonomic types available from crate root
- No breaking changes to public API

### Technical Debt Addressed

- ✅ Eliminated 294-line monolithic `backend.rs` file
- ✅ Created clear separation between trait definition and implementation
- ✅ Organized SQLite-specific logic in dedicated submodule
- ✅ Improved code maintainability and modularity
- ✅ Established foundation for future backend implementations

### Lessons Learned

1. **Module file conflicts** - Cannot have both `backend.rs` and `backend/mod.rs`
2. **Re-export strategy** - Essential for maintaining API compatibility during refactor
3. **Incremental approach** - Moving logic in small chunks with compilation checks prevents errors
4. **Missing type exports** - Need to check all doc tests and public API usage
5. **Trait implementation preservation** - Must maintain exact signatures for trait implementations

### Phase 3 Success Metrics

- ✅ **0 behavior changes** - All tests pass identically
- ✅ **0 breaking changes** - All existing code works unchanged
- ✅ **100% module separation** - Clear modular structure achieved
- ✅ **Foundation for Phase 4** - Ready for native backend implementation
- ✅ **Documentation updated** - This document serves as complete refactor record