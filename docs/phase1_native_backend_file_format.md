# Phase 1: Native Backend File Format Design

## Goals

### Primary Objectives
- **Native adjacency storage** for nodes and edges without SQLite dependency
- **Efficient neighbor lookups and k-hop traversals** optimized for graph workloads
- **Compatibility with existing GraphBackend trait** while maintaining performance
- **Integration with existing MVCC model** where applicable without breaking changes

### Performance Targets
- **O(1) node lookup** via indexed storage
- **O(degree) neighbor iteration** with contiguous adjacency storage
- **Memory-mapped I/O** for large datasets (>1M nodes)
- **Cache-friendly data layout** for modern CPU architectures
- **Deterministic traversal order** for reproducible results

## Constraints

### Technical Constraints
- **No mocks/stubs/placeholders:** Must be fully functional implementation
- **No GPU/ROCm/external compute engines:** CPU-only implementation
- **Keep files <300 LOC per module:** Maintain existing code organization standards
- **Minimal Cargo.toml dependencies:** Prefer standard library over external crates
- **No breaking changes to existing APIs:** Maintain GraphBackend trait compatibility

### Functional Constraints
- **Deterministic behavior:** All operations must produce consistent results
- **Thread safety:** Support concurrent read operations when possible
- **Data integrity:** Corruption detection and recovery mechanisms
- **Schema evolution:** Support for future metadata additions

## Proposed File Layout

### Module Structure (Final location TBD)
**Option A:** `sqlitegraph/src/native_backend/` (preferred)
```
native_backend/
├── mod.rs          (Module organization and exports)
├── graph_file.rs   (File operations, headers, mmap)
├── node_store.rs   (Node records and indexing)
├── edge_store.rs   (Edge records and adjacency)
├── adjacency.rs    (Neighbor iteration and traversal)
├── metadata.rs     (Schema and version management)
├── cache.rs        (Read caching and optimization)
└── wal.rs          (Write-ahead log for durability)
```

**Option B:** Separate crate `sqlitegraph-native/`
```
sqlitegraph-native/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── graph_file.rs
│   ├── node_store.rs
│   ├── edge_store.rs
│   ├── adjacency.rs
│   ├── metadata.rs
│   ├── cache.rs
│   └── wal.rs
```

### Module Responsibilities

#### graph_file.rs (~250 LOC)
**Purpose:** File management, headers, and memory mapping
**Responsibilities:**
- File creation and opening with proper permissions
- File header validation and version checking
- Memory mapping setup for efficient I/O
- File growth and space management
- Atomic file operations for durability

**Key Functions:**
```rust
// PLANNED - Not yet implemented
pub struct GraphFile {
    // File handle and mmap region
    // Header metadata
}

impl GraphFile {
    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self, NativeFileError>;
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, NativeFileError>;
    pub fn grow(&mut self, additional_bytes: usize) -> Result<(), NativeFileError>;
    pub fn sync(&self) -> Result<(), NativeFileError>;
}
```

#### node_store.rs (~280 LOC)
**Purpose:** Node record storage and indexing
**Responsibilities:**
- Node record serialization/deserialization
- Node ID allocation and management
- Node lookup by ID (O(1) via indexing)
- Node metadata storage (kind, name, properties)
- Node deletion and garbage collection

**Node Record Format:**
```
[Header: 8 bytes] [Node ID: 8 bytes] [Flags: 4 bytes] [Kind: 2 bytes] [Name Len: 2 bytes] [Data Len: 4 bytes] [Name: variable] [Data: variable]
```

**Key Types:**
```rust
// PLANNED - Not yet implemented
#[derive(Debug, Clone)]
pub struct NodeRecord {
    pub id: NodeId,
    pub flags: NodeFlags,
    pub kind: String,
    pub name: String,
    pub data: serde_json::Value,
    pub outgoing_offset: u64,  // Offset to first outgoing edge
    pub outgoing_count: u32,   // Number of outgoing edges
    pub incoming_offset: u64,  // Offset to first incoming edge
    pub incoming_count: u32,   // Number of incoming edges
}
```

#### edge_store.rs (~290 LOC)
**Purpose:** Edge record storage and adjacency management
**Responsibilities:**
- Edge record serialization/deserialization
- Adjacency list maintenance (both directions)
- Edge ID allocation and management
- Edge type indexing for filtered queries
- Edge deletion and reference cleanup

**Edge Record Format:**
```
[Header: 8 bytes] [Edge ID: 8 bytes] [From ID: 8 bytes] [To ID: 8 bytes] [Type: 2 bytes] [Flags: 2 bytes] [Data Len: 4 bytes] [Next Outgoing: 8 bytes] [Next Incoming: 8 bytes] [Data: variable]
```

**Key Types:**
```rust
// PLANNED - Not yet implemented
#[derive(Debug, Clone)]
pub struct EdgeRecord {
    pub id: EdgeId,
    pub from_id: NodeId,
    pub to_id: NodeId,
    pub edge_type: String,
    pub flags: EdgeFlags,
    pub data: serde_json::Value,
    pub next_outgoing: Option<EdgeId>,  // Linked list for outgoing edges
    pub next_incoming: Option<EdgeId>,  // Linked list for incoming edges
}
```

#### adjacency.rs (~250 LOC)
**Purpose:** Neighbor iteration and efficient traversal
**Responsibilities:**
- O(degree) neighbor iteration using adjacency lists
- Direction-specific traversal (outgoing/incoming)
- Edge type filtering for optimized queries
- Multi-hop traversal with deduplication
- Cache-friendly iteration patterns

**Key Functions:**
```rust
// PLANNED - Not yet implemented
pub struct AdjacencyIterator<'a> {
    // Iterator state for efficient neighbor access
}

impl<'a> AdjacencyIterator<'a> {
    pub fn new_outgoing(&self, node: NodeId) -> Self;
    pub fn new_incoming(&self, node: NodeId) -> Self;
    pub fn with_edge_filter(&self, edge_types: &[&str]) -> Self;
}

impl Iterator for AdjacencyIterator<'_> {
    type Item = NodeId;
    // Efficient iteration implementation
}
```

#### metadata.rs (~200 LOC)
**Purpose:** Schema and version management
**Responsibilities:**
- File format version tracking
- Schema evolution support
- Compatibility checking
- Metadata backup and recovery

**File Header Format:**
```
[Magic: 8 bytes] "SQLTGF" [Version: 4 bytes] [Flags: 4 bytes] [Node Count: 8 bytes] [Edge Count: 8 bytes] [Schema Version: 8 bytes] [Checksum: 8 bytes]
```

#### wal.rs (~180 LOC)
**Purpose:** Write-ahead log for durability
**Responsibilities:**
- Atomic write operations
- Crash recovery support
- Transaction logging and replay
- Log compaction and cleanup

#### cache.rs (~220 LOC)
**Purpose:** Read caching and performance optimization
**Responsibilities:**
- Node and edge record caching
- LRU eviction policies
- Cache consistency with file updates
- Memory usage optimization

## Record Formats

### File Header Format (64 bytes)
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

### Node Record Format (Variable, ~48-256 bytes typical)
```
Offset  Length  Description
------  -------  -----------
0      1        Record header (version + flags)
1      8        Node ID (big-endian)
9      4        Node flags (bitfield)
13     2        Node kind length
15     2        Node name length
17     4        Node data length
21     variable Node kind (UTF-8)
variable  Node name (UTF-8)
variable  Node data (JSON)
variable  Adjacency metadata (8+8+4+4 bytes)
```

### Edge Record Format (Variable, ~32-128 bytes typical)
```
Offset  Length  Description
------  -------  -----------
0      1        Record header (version + flags)
1      8        Edge ID (big-endian)
9      8        From node ID (big-endian)
17     8        To node ID (big-endian)
25     2        Edge type length
27     2        Edge flags (bitfield)
29     4        Edge data length
33     variable Edge type (UTF-8)
variable  Edge data (JSON)
variable  Linked list pointers (8+8 bytes optional)
```

### Adjacency Metadata (24 bytes per node)
```
Offset  Length  Description
------  -------  -----------
0      8        First outgoing edge offset
8      8        First incoming edge offset
16     4        Outgoing edge count
20     4        Incoming edge count
```

## Mapping Between SQLite and Native Storage

### Logical ID Mapping
**SQLite Graph IDs:**
- Auto-incrementing integers starting from 1
- Preserved across sessions
- Referenced by EdgeSpec.from_id/to_id

**Native Storage Mapping:**
- **Strategy:** Direct mapping - use same integer IDs
- **Allocation:** Sequential assignment with gap management
- **Persistence:** IDs survive file close/reopen cycles
- **Migration:** SQLite IDs can be directly imported to native format

### Data Type Compatibility

#### Entity Mapping
**SQLite schema:**
```sql
CREATE TABLE graph_entities (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    kind      TEXT NOT NULL,
    name      TEXT NOT NULL,
    file_path TEXT,
    data      TEXT NOT NULL
);
```

**Native storage:**
- `id`: Direct integer mapping
- `kind`: UTF-8 string (variable length)
- `name`: UTF-8 string (variable length)
- `file_path`: Optional UTF-8 string
- `data`: JSON blob (variable length)

#### Edge Mapping
**SQLite schema:**
```sql
CREATE TABLE graph_edges (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    from_id   INTEGER NOT NULL,
    to_id     INTEGER NOT NULL,
    edge_type TEXT NOT NULL,
    data      TEXT NOT NULL
);
```

**Native storage:**
- `id`: Direct integer mapping
- `from_id`/`to_id`: Direct node ID references
- `edge_type`: UTF-8 string (variable length, indexed)
- `data`: JSON blob (variable length)

### Index and Query Translation

#### SQLite Indexes → Native Optimizations
**SQLite:**
```sql
CREATE INDEX idx_edges_from ON graph_edges(from_id);
CREATE INDEX idx_edges_to ON graph_edges(to_id);
CREATE INDEX idx_edges_type ON graph_edges(edge_type);
```

**Native equivalents:**
- `idx_edges_from`: Direct adjacency list per node
- `idx_edges_to`: Reverse adjacency list per node
- `idx_edges_type`: Edge type grouping for filtered traversal

#### Query Translation Examples
**SQLite neighbor query:**
```sql
SELECT to_id FROM graph_edges WHERE from_id=?1 AND edge_type=?2 ORDER BY to_id;
```

**Native equivalent:**
- Navigate to node's outgoing adjacency list
- Filter by edge type during iteration
- Maintain sorted order by construction

## API Integration

### GraphBackend Trait Implementation Strategy

#### High-Level Implementation Pattern
```rust
// PLANNED - Not yet implemented
pub struct NativeFileBackend {
    graph_file: GraphFile,
    node_store: NodeStore,
    edge_store: EdgeStore,
    cache: RecordCache,
    wal: WriteAheadLog,
}

impl GraphBackend for NativeFileBackend {
    fn insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError> {
        // 1. Validate node spec (same rules as SQLite)
        validate_node_spec(&node)?;

        // 2. Allocate node ID from file header
        let node_id = self.allocate_node_id()?;

        // 3. Create node record
        let record = NodeRecord {
            id: node_id,
            kind: node.kind,
            name: node.name,
            file_path: node.file_path,
            data: node.data,
            // ... adjacency metadata
        };

        // 4. Write to WAL for durability
        self.wal.log_node_insert(&record)?;

        // 5. Store in node cache
        self.cache.store_node(&record)?;

        Ok(node_id)
    }

    fn get_node(&self, id: i64) -> Result<GraphEntity, SqliteGraphError> {
        // 1. Check cache first
        if let Some(record) = self.cache.get_node(id) {
            return Ok(record_to_graph_entity(record));
        }

        // 2. Load from file storage
        let record = self.node_store.load_node(id)?;

        // 3. Cache for future access
        self.cache.store_node(&record)?;

        Ok(record_to_graph_entity(record))
    }

    fn neighbors(&self, node: i64, query: NeighborQuery) -> Result<Vec<i64>, SqliteGraphError> {
        // 1. Validate node exists
        self.ensure_node_exists(node)?;

        // 2. Create adjacency iterator
        let iterator = match query.direction {
            BackendDirection::Outgoing => AdjacencyIterator::new_outgoing(node),
            BackendDirection::Incoming => AdjacencyIterator::new_incoming(node),
        };

        // 3. Apply edge type filter if specified
        let iterator = if let Some(ref edge_type) = query.edge_type {
            iterator.with_edge_filter(&[edge_type])
        } else {
            iterator
        };

        // 4. Collect results (deterministic ordering)
        let mut neighbors: Vec<NodeId> = iterator.collect();
        neighbors.sort_unstable(); // Ensure deterministic ordering

        Ok(neighbors)
    }

    // ... implement remaining 8 methods with similar patterns
}
```

#### Error Handling Translation
```rust
// PLANNED - Not yet implemented
impl From<NativeFileError> for SqliteGraphError {
    fn from(error: NativeFileError) -> Self {
        match error {
            NativeFileError::InvalidInput(msg) => SqliteGraphError::invalid_input(msg),
            NativeFileError::NotFound(msg) => SqliteGraphError::not_found(msg),
            NativeFileError::Corruption(msg) => SqliteGraphError::corruption(msg),
            NativeFileError::IoError(io_err) => SqliteGraphError::io_error(io_err.to_string()),
            // ... other mappings
        }
    }
}
```

### External Caller Compatibility

#### Transparent Backend Selection
**Existing code continues to work:**
```rust
// This code requires no changes
let backend = SqliteGraphBackend::in_memory()?;
let node_id = backend.insert_node(node_spec)?;
let neighbors = backend.neighbors(node_id, neighbor_query)?;
```

**New backend usage:**
```rust
// PLANNED - Not yet implemented
let backend = NativeFileBackend::create("path/to/graph.db")?;
let node_id = backend.insert_node(node_spec)?;
let neighbors = backend.neighbors(node_id, neighbor_query)?;
```

#### Backend-Agnostic Code
```rust
// This code already works and will work with native backend
fn process_graph<B: GraphBackend>(backend: &B) -> Result<(), SqliteGraphError> {
    let nodes = backend.entity_ids()?;
    for node_id in nodes {
        let neighbors = backend.neighbors(node_id, NeighborQuery::default())?;
        // Process neighbors
    }
    Ok(())
}
```

## Test Plan for Native Backend

### Unit Tests Categories

#### File Operations Tests
**File:** tests/native_file_tests.rs (NEW - planned for future phases)
**Test Categories:**
- File creation and header validation
- Memory mapping and unmapping
- File growth and space management
- Atomic operations and crash recovery
- Corruption detection and handling

**Sample Tests:**
```rust
// PLANNED - Not yet implemented
#[test]
fn test_file_header_validation() {
    // Test file magic number validation
    // Test version compatibility checking
    // Test checksum validation
}

#[test]
fn test_memory_mapping() {
    // Test mmap setup and teardown
    // Test access patterns
    // Test error handling
}
```

#### Record Encoding/Decoding Tests
**File:** tests/native_record_tests.rs (NEW - planned for future phases)
**Test Categories:**
- Node record serialization/deserialization
- Edge record serialization/deserialization
- JSON data handling and validation
- Variable-length field management
- Endianness handling

#### Index and Lookup Tests
**File:** tests/native_index_tests.rs (NEW - planned for future phases)
**Test Categories:**
- O(1) node lookup verification
- Adjacency list integrity
- Edge type filtering performance
- Index consistency under modifications
- Cache hit/miss ratios

### Integration Tests Categories

#### Parity with SqliteGraphBackend Tests
**File:** tests/backend_parity_tests.rs (NEW - planned for future phases)
**Test Categories:**
- Identical query results comparison
- Performance benchmarking
- Error handling consistency
- Deterministic behavior verification
- Large dataset scalability

**Sample Test Framework:**
```rust
// PLANNED - Not yet implemented
fn compare_backends() {
    let sqlite_backend = SqliteGraphBackend::in_memory()?;
    let native_backend = NativeFileBackend::create(tempfile())?;

    // Insert identical data into both backends
    let node_id1 = sqlite_backend.insert_node(test_node())?;
    let node_id2 = native_backend.insert_node(test_node())?;

    // Compare results
    assert_eq!(node_id1, node_id2);

    // Test neighbor queries
    let sqlite_neighbors = sqlite_backend.neighbors(node_id1, default_query())?;
    let native_neighbors = native_backend.neighbors(node_id2, default_query())?;
    assert_eq!(sqlite_neighbors, native_neighbors);
}
```

#### Graph Algorithm Tests
**File:** tests/native_algorithm_tests.rs (NEW - planned for future phases)
**Test Categories:**
- BFS traversal correctness and performance
- Shortest path algorithm validation
- K-hop traversal accuracy
- Pattern matching consistency
- Multi-hop chain query functionality

### Regression Tests Categories

#### Existing Behavior Preservation
**Files:** Extend existing tests rather than creating new ones
**Test Modifications Required:**
- `backend_trait_tests.rs`: Add native backend test cases
- `lib_api_smoke_tests.rs`: Add backend-agnostic testing
- `graph_opt_tests.rs`: Test optimization with multiple backends
- `pattern_engine_tests.rs`: Verify pattern matching consistency
- `integration_tests.rs`: Test complete pipelines with native backend

**Example Test Extension:**
```rust
// PLANNED - Modification to existing tests.rs
#[test]
fn test_backend_agnostic_behavior() {
    let backends: Vec<Box<dyn GraphBackend>> = vec![
        Box::new(SqliteGraphBackend::in_memory().unwrap()),
        Box::new(NativeFileBackend::create(tempfile()).unwrap()),
    ];

    for backend in backends {
        // Test identical behavior across all backends
        test_graph_backend_operations(&*backend);
    }
}
```

#### Performance Regression Tests
**File:** tests/native_performance_tests.rs (NEW - planned for future phases)
**Test Categories:**
- Query performance benchmarks
- Memory usage profiling
- Scalability tests with large datasets
- Cache effectiveness measurement
- I/O pattern optimization validation

### Test Data and Fixtures

#### Deterministic Test Graphs
**Standard Test Datasets:**
- Small graph (10 nodes, 20 edges)
- Medium graph (1K nodes, 5K edges)
- Large graph (100K nodes, 500K edges)
- Specialized patterns (stars, chains, cycles, dense clusters)

#### Migration Test Data
**SQLite → Native Migration Tests:**
- Round-trip data preservation
- ID mapping consistency
- Metadata compatibility
- Performance comparison before/after migration

## Conclusion

The native backend file format design provides a comprehensive foundation for implementing SQLiteGraph with native file storage while maintaining full compatibility with existing APIs. The modular design keeps each component under 300 LOC while providing the performance characteristics needed for large-scale graph processing.

The key innovations include:
- **Memory-mapped adjacency storage** for efficient traversals
- **Deterministic ordering guarantees** for reproducible results
- **Seamless GraphBackend trait integration** for drop-in compatibility
- **Comprehensive testing strategy** for reliability and performance validation

This design serves as a specification for future implementation phases while maintaining the architectural principles and constraints established in the SQLiteGraph project.