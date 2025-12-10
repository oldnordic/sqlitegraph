# Phase 1: Backend Abstractions

## GraphBackend Trait Analysis

### Current Trait Definition (sqlitegraph/src/backend.rs - lines 58-85)

```rust
pub trait GraphBackend {
    fn insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError>;
    fn get_node(&self, id: i64) -> Result<GraphEntity, SqliteGraphError>;
    fn insert_edge(&self, edge: EdgeSpec) -> Result<i64, SqliteGraphError>;
    fn neighbors(&self, node: i64, query: NeighborQuery) -> Result<Vec<i64>, SqliteGraphError>;
    fn bfs(&self, start: i64, depth: u32) -> Result<Vec<i64>, SqliteGraphError>;
    fn shortest_path(&self, start: i64, end: i64) -> Result<Option<Vec<i64>>, SqliteGraphError>;
    fn node_degree(&self, node: i64) -> Result<(usize, usize), SqliteGraphError>;
    fn k_hop(
        &self,
        start: i64,
        depth: u32,
        direction: BackendDirection,
    ) -> Result<Vec<i64>, SqliteGraphError>;
    fn k_hop_filtered(
        &self,
        start: i64,
        depth: u32,
        direction: BackendDirection,
        allowed_edge_types: &[&str],
    ) -> Result<Vec<i64>, SqliteGraphError>;
    fn chain_query(&self, start: i64, chain: &[ChainStep]) -> Result<Vec<i64>, SqliteGraphError>;
    fn pattern_search(
        &self,
        start: i64,
        pattern: &PatternQuery,
    ) -> Result<Vec<PatternMatch>, SqliteGraphError>;
}
```

### Method-by-Method Analysis

#### 1. Node and Edge Operations

**`insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError>`**
- **Purpose:** Insert a new graph node and return its assigned ID
- **Input Invariants:** `NodeSpec.kind` and `NodeSpec.name` must be non-empty strings
- **Output Guarantees:** Returns positive integer ID >= 1, or error for invalid input
- **Error Conditions:**
  - Empty kind or name fields
  - Database/storage errors
  - Constraint violations
- **Performance Expectations:** Should be O(1) for ID assignment

**`get_node(&self, id: i64) -> Result<GraphEntity, SqliteGraphError>`**
- **Purpose:** Retrieve node metadata by ID
- **Input Invariants:** ID must be positive integer (> 0)
- **Output Guarantees:** Returns complete `GraphEntity` with all fields populated
- **Error Conditions:**
  - Node not found (id <= 0 or doesn't exist)
  - Database/storage errors
- **Performance Expectations:** Should be O(1) lookup

**`insert_edge(&self, edge: EdgeSpec) -> Result<i64, SqliteGraphError>`**
- **Purpose:** Insert a new graph edge and return its assigned ID
- **Input Invariants:**
  - `EdgeSpec.edge_type` must be non-empty string
  - `from_id` and `to_id` must be positive integers (> 0)
  - Both endpoint nodes must exist
- **Output Guarantees:** Returns positive integer ID >= 1
- **Error Conditions:**
  - Invalid edge type or endpoint IDs
  - Referential integrity violations (missing nodes)
  - Database/storage errors
- **Performance Expectations:** Should be O(1) for ID assignment

#### 2. Traversal Operations

**`neighbors(&self, node: i64, query: NeighborQuery) -> Result<Vec<i64>, SqliteGraphError>`**
- **Purpose:** Get adjacent node IDs with optional filtering
- **Input Invariants:**
  - `node` must be positive integer (> 0) and exist
  - `query.direction` must be valid (Outgoing/Incoming)
  - `query.edge_type` (if specified) must be valid string
- **Output Guarantees:** Returns sorted list of unique neighbor IDs
- **Error Conditions:**
  - Invalid node ID or missing node
  - Database/storage errors
- **Performance Expectations:** Should be O(degree) for degree-size results

**`bfs(&self, start: i64, depth: u32) -> Result<Vec<i64>, SqliteGraphError>`**
- **Purpose:** Breadth-first search from start node to specified depth
- **Input Invariants:**
  - `start` must be positive integer (> 0) and exist
  - `depth` must be reasonable (typically < 10 for performance)
- **Output Guarantees:** Returns nodes reachable within depth, sorted by discovery order
- **Error Conditions:**
  - Invalid start node
  - Database/storage errors
- **Performance Expectations:** O(nodes + edges) up to specified depth

#### 3. Advanced Traversal Operations

**`shortest_path(&self, start: i64, end: i64) -> Result<Option<Vec<i64>>, SqliteGraphError>`**
- **Purpose:** Find shortest path between two nodes using BFS
- **Input Invariants:** Both nodes must be positive integers and exist
- **Output Guarantees:**
  - Some(path) with ordered list including start/end nodes
  - None if no path exists
- **Error Conditions:** Invalid node IDs or database errors
- **Performance Expectations:** O(nodes + edges) for reachable graph

**`node_degree(&self, node: i64) -> Result<(usize, usize), SqliteGraphError>`**
- **Purpose:** Return (outgoing_degree, incoming_degree) for a node
- **Input Invariants:** Node must be positive integer and exist
- **Output Guarantees:** Accurate degree counts (0 or positive integers)
- **Error Conditions:** Invalid node ID or database errors
- **Performance Expectations:** O(1) lookup

**`k_hop(&self, start: i64, depth: u32, direction: BackendDirection) -> Result<Vec<i64>, SqliteGraphError>`**
- **Purpose:** Multi-hop traversal from start node to specified depth
- **Input Invariants:** Valid start node, reasonable depth, valid direction
- **Output Guarantees:** All nodes reachable within depth, sorted deterministically
- **Error Conditions:** Invalid inputs or database errors
- **Performance Expectations:** O(nodes^depth) worst-case, optimized for typical cases

**`k_hop_filtered(&self, start: i64, depth: u32, direction: BackendDirection, allowed_edge_types: &[&str]) -> Result<Vec<i64>, SqliteGraphError>`**
- **Purpose:** Multi-hop traversal with edge type filtering
- **Input Invariants:** Same as k_hop plus valid edge type list
- **Output Guarantees:** Nodes reachable via allowed edge types only
- **Error Conditions:** Invalid inputs or database errors
- **Performance Expectations:** Similar to k_hop with additional filtering cost

#### 4. Complex Query Operations

**`chain_query(&self, start: i64, chain: &[ChainStep]) -> Result<Vec<i64>, SqliteGraphError>`**
- **Purpose:** Execute chained traversal pattern (from multi_hop.rs)
- **Input Invariants:**
  - Valid start node
  - Non-empty chain with valid ChainStep elements
  - ChainStep must specify valid directions and edge types
- **Output Guarantees:** Nodes matching complete chain pattern
- **Error Conditions:** Invalid chain specification or database errors
- **Performance Expectations:** O(chain_length * average_degree)

**`pattern_search(&self, start: i64, pattern: &PatternQuery) -> Result<Vec<PatternMatch>, SqliteGraphError>`**
- **Purpose:** Execute structural pattern matching (from pattern.rs)
- **Input Invariants:**
  - Valid start node
  - Well-formed PatternQuery with valid legs
  - Pattern constraints must be consistent
- **Output Guarantees:** All pattern matches from start node
- **Error Conditions:** Invalid pattern or database errors
- **Performance Expectations:** Variable based on pattern complexity

## Supporting Types and Structures

### BackendDirection (backend.rs - lines 21-25)
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendDirection {
    Outgoing,
    Incoming,
}
```

**Purpose:** Specify traversal direction for neighbor queries and multi-hop operations

### NeighborQuery (backend.rs - lines 27-31)
```rust
#[derive(Clone, Debug)]
pub struct NeighborQuery {
    pub direction: BackendDirection,
    pub edge_type: Option<String>,
}
```

**Purpose:** Query configuration for neighbor lookups with optional edge type filtering

### NodeSpec and EdgeSpec (backend.rs - lines 42-56)
```rust
#[derive(Clone, Debug)]
pub struct NodeSpec {
    pub kind: String,
    pub name: String,
    pub file_path: Option<String>,
    pub data: Value,
}

#[derive(Clone, Debug)]
pub struct EdgeSpec {
    pub from: i64,
    pub to: i64,
    pub edge_type: String,
    pub data: Value,
}
```

**Purpose:** Input specifications for node and edge creation, used by insert operations

### ChainStep (multi_hop.rs - exported via backend.rs)
**Purpose:** Individual step in chain query traversal pattern
**Fields:** Direction, edge_type constraints, and traversal parameters

## SqliteGraphBackend Implementation Analysis

### Core Structure (backend.rs - lines 87-89)
```rust
pub struct SqliteGraphBackend {
    graph: SqliteGraph,
}
```

### Implementation Strategy (lines 147-220)

#### Direct Delegation Pattern
SqliteGraphBackend implements GraphBackend by delegating to underlying SqliteGraph methods:

**Node Operations:**
```rust
fn insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError> {
    self.graph.insert_entity(&GraphEntity {
        id: 0,  // Auto-assigned by SQLite
        kind: node.kind,
        name: node.name,
        file_path: node.file_path,
        data: node.data,
    })
}
```

**Traversal Operations:**
```rust
fn bfs(&self, start: i64, depth: u32) -> Result<Vec<i64>, SqliteGraphError> {
    bfs_neighbors(&self.graph, start, depth)  // Delegates to bfs.rs
}
```

**Advanced Queries:**
```rust
fn pattern_search(&self, start: i64, pattern: &PatternQuery) -> Result<Vec<PatternMatch>, SqliteGraphError> {
    pattern::execute_pattern(&self.graph, start, pattern)  // Delegates to pattern.rs
}
```

### Optimized Query Implementation (lines 102-144)

SqliteGraphBackend implements optimized neighbor queries with edge type filtering:

```rust
fn query_neighbors(
    &self,
    node: i64,
    direction: BackendDirection,
    edge_type: &Option<String>,
) -> Result<Vec<i64>, SqliteGraphError> {
    match (direction, edge_type) {
        (BackendDirection::Outgoing, None) => self.graph.fetch_outgoing(node),
        (BackendDirection::Incoming, None) => self.graph.fetch_incoming(node),
        (BackendDirection::Outgoing, Some(edge_type)) => {
            // Optimized SQL with prepared statements
            let mut stmt = conn.prepare_cached(
                "SELECT to_id FROM graph_edges WHERE from_id=?1 AND edge_type=?2 ORDER BY to_id, id"
            )?;
            // Execute query and collect results
        }
        // Similar for incoming with edge type filter
    }
}
```

### Access Methods (lines 222-230)
```rust
impl SqliteGraphBackend {
    pub fn graph(&self) -> &SqliteGraph { &self.graph }
    pub fn entity_ids(&self) -> Result<Vec<i64>, SqliteGraphError> {
        self.graph.all_entity_ids()
    }
}
```

## Target Abstraction for Native Backend

### NativeFileBackend Design Requirements

#### Core Implementation Structure
```rust
// PLANNED - Not yet implemented
pub struct NativeFileBackend {
    // File handles and memory mapping
    // Adjacency storage structures
    // Metadata and indexing
}

impl GraphBackend for NativeFileBackend {
    // Implement all 11 trait methods with native file operations
}
```

#### Implementation Requirements for Each Method

**Node Operations:**
- `insert_node`: Append to node storage, assign sequential ID
- `get_node`: Lookup node in indexed storage (hash map or B-tree)
- Both must handle serde JSON data serialization/deserialization

**Edge Operations:**
- `insert_edge`: Append to edge storage, maintain adjacency lists
- Must update both outgoing and incoming adjacency structures
- Handle edge type indexing for filtered queries

**Traversal Operations:**
- `neighbors`: Direct adjacency list iteration
- `bfs`: Queue-based traversal using adjacency lists
- `k_hop`: Multi-level neighbor expansion with deduplication
- All must maintain deterministic ordering (sorted by node ID)

**Advanced Operations:**
- `shortest_path`: BFS with parent tracking
- `chain_query`: Sequential adjacency traversal
- `pattern_search`: Complex pattern matching using adjacency data

#### Performance Expectations
- **Goal:** Match or exceed SQLite backend performance
- **Target:** O(1) node lookups, O(degree) neighbor queries
- **Optimization:** Memory-mapped adjacency for large graphs

### Additional Helper Traits and Types

#### BackendFactory Pattern (backend_selector.rs - 39 LOC)
**Current Implementation:**
```rust
pub enum BackendKind {
    Sqlite,
    // PLANNED: Native
}

pub struct GraphBackendFactory {
    // Factory methods for creating backends
}
```

**Planned Extensions:**
- Native backend creation methods
- Configuration-driven backend selection
- Backend capability introspection

#### Validation and Error Handling
**Requirements:**
- Consistent error types across backends
- Same validation rules for inputs
- Uniform error message formats
- Database vs file-specific error translation

## Test Strategy for Backends

### Existing Test Files Requiring Extension

#### backend_trait_tests.rs (369 LOC - EXCEEDS 300 LIMIT)
**Current Coverage:**
- GraphBackend trait method testing
- SqliteGraphBackend specific behavior
- Edge case and error condition testing

**Future Test Scenarios:**
- **Backend-agnostic behavior tests:**
  - Test GraphBackend trait contract for all implementations
  - Verify deterministic ordering guarantees
  - Test error handling consistency across backends
- **Regression tests for SQLite backend:**
  - Ensure no behavioral changes to existing functionality
  - Performance regression prevention
  - Schema compatibility verification
- **Compatibility tests between backends:**
  - Compare query results between SQLite and native backends
  - Verify identical deterministic behavior
  - Test edge case handling consistency

#### lib_api_smoke_tests.rs (206 LOC)
**Current Coverage:**
- High-level API integration testing
- End-to-end workflow validation
- Public API contract testing

**Future Test Scenarios:**
- **Backend selection testing:**
  - Test BackendFactory with multiple backends
  - Verify backend-agnostic API behavior
  - Test backend switching at runtime
- **Performance consistency:**
  - Measure operation timing across backends
  - Verify performance expectations are met
  - Identify backend-specific optimization opportunities

#### graph_opt_tests.rs (167 LOC)
**Current Coverage:**
- Graph optimization utilities testing
- Bulk operation performance
- Cache behavior validation

**Future Test Scenarios:**
- **Backend-agnostic optimization:**
  - Test bulk operations across different backends
  - Verify cache invalidation strategies
  - Test optimization heuristics consistency

#### integration_tests.rs (154 LOC)
**Current Coverage:**
- Cross-module integration testing
- End-to-end pipeline validation
- CLI and reasoning integration

**Future Test Scenarios:**
- **Multi-backend integration:**
  - Test reasoning pipelines with different backends
  - Verify pattern engine behavior consistency
  - Test DSL parsing and execution across backends

### New Test Categories Required

#### Backend Comparison Tests
**File:** tests/backend_comparison_tests.rs (NEW)
**Content:**
- Deterministic result comparison between backends
- Performance benchmarking across backend implementations
- Edge case behavior consistency validation

#### Native Backend Specific Tests
**File:** tests/native_backend_tests.rs (NEW - planned for future phases)
**Content:**
- File format validation tests
- Memory mapping and I/O error handling
- Large dataset performance validation
- Corruption recovery and data integrity

#### Backend Factory Tests
**File:** tests/backend_factory_tests.rs (NEW - planned for future phases)
**Content:**
- Backend instantiation and configuration
- Runtime backend switching
- Backend capability introspection
- Error handling during backend creation

### Test Data and Fixtures

#### Deterministic Test Graphs
**Requirements:**
- Consistent test datasets across all backends
- Verifiable graph properties and query results
- Performance benchmarks with known characteristics

#### Migration and Compatibility Testing
**Requirements:**
- Test data migration between backends
- Verify schema compatibility
- Test round-trip data preservation

## Conclusion

The existing `GraphBackend` trait provides a solid abstraction for implementing multiple backends. SqliteGraphBackend demonstrates a clean implementation pattern that delegates to specialized modules (bfs.rs, pattern.rs, multi_hop.rs) while maintaining deterministic behavior.

The native backend implementation will need to carefully match all the semantics, performance characteristics, and error handling patterns established by the SQLite backend. The comprehensive test strategy ensures that both backend-agnostic behavior and backend-specific optimizations are properly validated.