# Phase 4: Native Backend Implementation Plan

## 1) DESIGN RECAP (LINKED TO PHASE 1)

### Finalized Record Layouts from Phase 1

Based on `docs/phase1_native_backend_file_format.md`, the v1 design specifies:

#### Node Record Fields
```rust
pub struct NodeRecord {
    pub id: NodeId,                    // 8 bytes - big-endian i64
    pub flags: NodeFlags,             // 4 bytes - bitfield
    pub kind: String,                 // variable - UTF-8
    pub name: String,                 // variable - UTF-8
    pub data: serde_json::Value,      // variable - JSON
    pub outgoing_offset: u64,         // 8 bytes - file offset to first outgoing edge
    pub outgoing_count: u32,          // 4 bytes - number of outgoing edges
    pub incoming_offset: u64,         // 8 bytes - file offset to first incoming edge
    pub incoming_count: u32,          // 4 bytes - number of incoming edges
}
```

#### Edge Record Fields
```rust
pub struct EdgeRecord {
    pub id: EdgeId,                   // 8 bytes - big-endian i64
    pub from_id: NodeId,              // 8 bytes - big-endian i64
    pub to_id: NodeId,                // 8 bytes - big-endian i64
    pub edge_type: String,            // variable - UTF-8
    pub flags: EdgeFlags,             // 2 bytes - bitfield
    pub data: serde_json::Value,      // variable - JSON
}
```

#### File Header Format (64 bytes)
```
Offset  Length  Description
------  -------  -----------
0      8        Magic bytes: "SQLTGF\0"
8      4        File format version (current: 1)
12     4        Feature flags (bitfield)
16     8        Total node count
24     8        Total edge count
32     8        Schema version
40     8        Node data offset
48     8        Edge data offset
56     8        Header checksum
```

### Confirmed Adjacency Storage Model

**CSR-style contiguous adjacency slices**:
- Each node maintains offset/count to its outgoing edges in edge file
- Each node maintains offset/count to its incoming edges in edge file
- Edges are stored in contiguous blocks per node for cache-friendly iteration
- Deterministic ordering by insertion order

### Confirmed On-Disk Representation

**Endianness**: Big-endian for all multi-byte integers (ensures cross-platform compatibility)

**Alignment**: No padding assumptions - use explicit byte-level encoding/decoding

**Versioning**: Header version field (starting at 1) with magic number "SQLTGF\0"

**File Layout**:
```
graph.db (main file):
├── Header (64 bytes)
├── Node records section (variable length)
└── Edge records section (variable length)
```

## 2) TARGET MODULE TREE (FINAL DECISION)

### Chosen Structure: backend/native/ (consistent with Phase 3 refactor)

```
sqlitegraph/src/backend/native/
├── mod.rs            // Module organization + public API (NOT GraphBackend impl)
├── graph_file.rs     // File operations + header encode/decode
├── node_store.rs     // Node record management
├── edge_store.rs     // Edge record management + adjacency routines
├── adjacency.rs      // In-memory neighbor iteration helpers
├── types.rs          // Core type definitions and error handling
└── constants.rs      // Magic numbers, versions, header sizes
```

### Module Responsibility Boundaries

#### `mod.rs` (~30 LOC)
**Purpose**: Module organization and minimal public interface
**Responsibilities**:
- Declare submodules
- Expose only types needed for future GraphBackend integration
- NO GraphBackend implementation
**Boundaries**: Must not depend on other backend implementations

#### `constants.rs` (~40 LOC)
**Purpose**: All constants for file format
**Responsibilities**:
- Magic numbers, version constants
- Header field offsets and sizes
- Error codes
**Boundaries**: Pure constants, no mutable state

#### `types.rs` (~150 LOC)
**Purpose**: Core type definitions and error handling
**Responsibilities**:
- `NodeId`, `EdgeId`, `NodeRecord`, `EdgeRecord` structs
- `NodeFlags`, `EdgeFlags` bitfield enums
- `NativeBackendError` enum with all error variants
- Header struct definition
**Boundaries**: Type definitions only, no I/O operations

#### `graph_file.rs` (~280 LOC)
**Purpose**: File management and header operations
**Responsibilities**:
- File creation/opening with proper permissions
- Header encode/decode with validation
- File growth and space management
- Memory mapping setup (if implemented)
**Boundaries**: Handles file-level operations only, not record logic

#### `node_store.rs` (~270 LOC)
**Purpose**: Node record storage and indexing
**Responsibilities**:
- Node record serialization/deserialization
- Node ID to file offset computation (O(1))
- Node record read/write operations
- Node count management
**Boundaries**: Only handles node records, no adjacency logic

#### `edge_store.rs` (~290 LOC)
**Purpose**: Edge record storage and adjacency layout
**Responsibilities**:
- Edge record serialization/deserialization
- Adjacency slice allocation and management
- Edge ID allocation
- Edge record read/write operations
**Boundaries**: Only handles edge records and storage layout

#### `adjacency.rs` (~240 LOC)
**Purpose**: In-memory neighbor iteration using stored records
**Responsibilities**:
- Neighbor iteration using node adjacency metadata
- Direction-specific traversal (outgoing/incoming)
- Edge type filtering helpers
- Consistency validation helpers
**Boundaries**: Uses node/edge stores but does not modify storage

## 3) TYPE & ERROR MODEL

### Core Types

#### ID Types (Alias for i64)
```rust
pub type NativeNodeId = i64;  // Same as existing NodeId for compatibility
pub type NativeEdgeId = i64;  // Same as existing EdgeId for compatibility
```

#### File Offset Types
```rust
pub type FileOffset = u64;    // Byte offset within file
pub type RecordSize = u32;    // Size of variable-length records
```

#### Flags Types
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeFlags(u32);    // Bitfield for node state

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeFlags(u16);    // Bitfield for edge state
```

### Error Model

#### New Internal Error Type
```rust
#[derive(Debug, thiserror::Error)]
pub enum NativeBackendError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid magic number: expected {expected:08x}, found {found:08x}")]
    InvalidMagic { expected: u64, found: u64 },

    #[error("Unsupported version: {version} (supported: 1)")]
    UnsupportedVersion { version: u32 },

    #[error("Invalid header checksum: expected {expected}, found {found}")]
    InvalidChecksum { expected: u64, found: u64 },

    #[error("Node ID {id} out of bounds (max: {max_id})")]
    InvalidNodeId { id: NativeNodeId, max_id: NativeNodeId },

    #[error("Edge ID {id} out of bounds (max: {max_id})")]
    InvalidEdgeId { id: NativeEdgeId, max_id: NativeEdgeId },

    #[error("Corrupt node record at offset {offset}: {reason}")]
    CorruptNodeRecord { offset: FileOffset, reason: String },

    #[error("Corrupt edge record at offset {offset}: {reason}")]
    CorruptEdgeRecord { offset: FileOffset, reason: String },

    #[error("Inconsistent adjacency: node {node_id} has {count} {direction} edges but file size indicates {file_count}")]
    InconsistentAdjacency {
        node_id: NativeNodeId,
        count: u32,
        direction: String,
        file_count: u32
    },

    #[error("File too small: {size} bytes (minimum {min_size} bytes required)")]
    FileTooSmall { size: u64, min_size: u64 },

    #[error("Record too large: {size} bytes (maximum {max_size} bytes)")]
    RecordTooLarge { size: u32, max_size: u32 },
}
```

#### Error Integration Strategy
- Native backend uses its own error type for internal operations
- Future GraphBackend implementation will translate `NativeBackendError` to `SqliteGraphError`
- All internal native operations return `Result<T, NativeBackendError>`

### Invariants

#### File-Level Invariants
1. **Header Validity**: Magic number must be "SQLTGF\0", version must be 1
2. **Checksum Consistency**: Header checksum must match computed value
3. **Size Sanity**: File must be at least header size + record metadata
4. **Offset Ordering**: Node section offset < Edge section offset

#### Node Record Invariants
1. **ID Bounds**: For any valid node_id (1 ≤ node_id ≤ node_count), node record must exist at computed offset
2. **Adjacency Consistency**: outgoing_offset + (outgoing_count * edge_record_size) must not exceed file size
3. **Offset Sanity**: All offsets must be >= header_size + node_section_offset
4. **Count Consistency**: outgoing_count and incoming_count must match actual edge records

#### Edge Record Invariants
1. **Node ID Validity**: from_id and to_id must be valid node IDs (1 ≤ id ≤ node_count)
2. **ID Ordering**: Edge IDs are sequential starting from 1 with no gaps
3. **Type Validity**: edge_type must be valid UTF-8 string
4. **JSON Validity**: data field must contain valid JSON

#### Cross-Component Invariants
1. **Total Counts**: Header node_count/edge_count must match actual records
2. **Adjacency Completeness**: All edges referenced by node adjacency must exist and be consistent
3. **Deterministic Ordering**: Edges in adjacency slices must maintain insertion order
4. **No Orphan Edges**: Every edge must be referenced by exactly one node's outgoing adjacency

## 4) TEST PLAN FOR PHASE 4

### Unit Tests Location and Coverage

#### Test File: `sqlitegraph/tests/native_backend_storage_tests.rs`

#### Header Tests
- **test_header_roundtrip_basic**
  - Create temporary file, write header, read back, assert exact equality
  - Verify all fields: magic, version, flags, counts, offsets, checksum
- **test_header_invalid_magic**
  - Corrupt magic bytes, ensure validation returns `InvalidMagic` error
- **test_header_invalid_version**
  - Set version to 2, ensure validation returns `UnsupportedVersion` error
- **test_header_checksum_validation**
  - Corrupt checksum, ensure validation returns `InvalidChecksum` error

#### Node Record Tests
- **test_node_roundtrip_basic**
  - Create storage, write 3 nodes with different data, read all back, assert exact equality
  - Test varying string lengths and JSON data sizes
- **test_node_invalid_id**
  - Request node_id = 999 when only 10 nodes exist, expect `InvalidNodeId` error
- **test_node_zero_id**
  - Request node_id = 0, expect `InvalidNodeId` error
- **test_node_json_serialization**
  - Test complex nested JSON data preservation through roundtrip

#### Edge Record Tests
- **test_edge_roundtrip_basic**
  - Create storage, write 5 edges with various types, read all back, assert exact equality
  - Test different edge_type strings and JSON payloads
- **test_edge_invalid_node_reference**
  - Write edge with from_id/to_id referencing non-existent node, expect validation error
- **test_edge_id_allocation**
  - Verify sequential edge ID allocation (1, 2, 3...)

#### Adjacency Integration Tests
- **test_single_node_neighbors_outgoing**
  - Create 1 node with 3 outgoing edges, use adjacency helper to fetch neighbors, assert expected vector [2,3,4]
- **test_single_node_neighbors_incoming**
  - Create 1 node with 2 incoming edges, use adjacency helper, assert expected vector [5,6]
- **test_multi_node_adjacency**
  - Create 5 nodes with varying degrees (0-4 edges each), verify correct neighbor sets for all nodes
- **test_adjacency_consistency**
  - Create complex graph, verify total outgoing edges = total incoming edges = edge_count

#### Corruption and Edge Case Tests
- **test_corrupt_node_degree_mismatch**
  - Set outgoing_count = 5 but only store 3 edges, expect `InconsistentAdjacency` error
- **test_corrupt_edge_beyond_file**
  - Set adjacency offset beyond file size, expect `CorruptNodeRecord` error
- **test_empty_graph_operations**
  - Test all operations on graph with 0 nodes/edges
- **test_maximum_size_records**
  - Test with near-maximum string lengths and JSON sizes

#### Performance and Stress Tests
- **test_large_graph_basic_operations**
  - Create graph with 1000 nodes and 5000 edges, verify basic operations complete in reasonable time
- **test_memory_usage_stability**
  - Perform many operations, monitor for memory leaks (basic check)

### Test Infrastructure

#### Temporary File Management
```rust
fn setup_temp_graph_file() -> (NamedTempFile, PathBuf) {
    // Creates temporary file that persists for test duration
}

fn cleanup_temp_file(path: PathBuf) {
    // Explicit cleanup for deterministic behavior
}
```

#### Test Data Generators
```rust
fn create_test_node(id: i64, kind: &str, name: &str, data: serde_json::Value) -> NodeRecord {
    // Helper to create valid test nodes
}

fn create_test_edge(id: i64, from: i64, to: i64, edge_type: &str, data: serde_json::Value) -> EdgeRecord {
    // Helper to create valid test edges
}
```

#### Validation Helpers
```rust
fn assert_adjacency_consistency(storage: &NativeStorage) {
    // Verify all adjacency metadata matches actual edge records
}
```

### Test Execution Strategy

#### Compilation Verification
- All new modules must compile without breaking existing code
- No modifications to lib.rs or backend/mod.rs in this phase
- Native modules must be completely isolated

#### Test Isolation
- Each test uses isolated temporary files
- No shared state between tests
- Deterministic results across runs

#### Coverage Requirements
- Every public function in native modules must have at least one test
- Every error variant must be exercised in at least one test
- All documented invariants must have corresponding validation tests

This test plan ensures comprehensive coverage of the native storage layer while maintaining complete isolation from existing SQLite backend functionality.