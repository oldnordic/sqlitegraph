# SQLiteGraph API Documentation

## Status: V2-Only Architecture

**V1 legacy code has been permanently removed** - SQLiteGraph now operates exclusively with V2 native backend architecture.

---

## Core Architecture

### Backend Structure
- **SQLite Backend**: `SqliteGraph` - Main SQLite-backed graph implementation
- **Native Backend**: V2 native backend with clustered adjacency and optimized storage
- **Backend Abstraction**: `GraphBackend` trait for backend independence
- **Ergonomic Client**: `BackendClient` wrapper with helper types

### V2 Storage Model
SQLiteGraph uses V2 clustered adjacency storage:
- **Node Records**: V2 format with `outgoing_edge_count` and `incoming_edge_count` fields
- **Cluster Storage**: V2 clustered adjacency for efficient edge management
- **Compact Edge Records**: `CompactEdgeRecord` for optimal storage with V1-style API compatibility

---

## Public API Reference

### Core Graph Operations

```rust
// Graph creation
SqliteGraph::open(path: &str) -> Result<SqliteGraph>
SqliteGraph::open_in_memory() -> Result<SqliteGraph>

// Entity operations
insert_entity(name: &str, entity_type: &str) -> Result<i64>
get_entity(id: i64) -> Result<Option<GraphEntity>>
update_entity(id: i64, name: &str, entity_type: &str) -> Result<()>
delete_entity(id: i64) -> Result<()>

// Edge operations
insert_edge(from_id: i64, to_id: i64, edge_type: &str) -> Result<i64>
get_edge(id: i64) -> Result<Option<GraphEdge>>
update_edge(id: i64, edge_type: &str) -> Result<()>
delete_edge(id: i64) -> Result<()>
```

### GraphQuery API

```rust
// Traversal operations
neighbors(node_id: i64) -> Result<Vec<GraphEntity>>
incoming(node_id: i64) -> Result<Vec<GraphEntity>>
outgoing(node_id: i64) -> Result<Vec<GraphEntity>>
edges_of_type(node_id: i64, edge_type: &str) -> Result<Vec<GraphEdge>>

// Multi-hop operations
k_hop_outgoing(start_id: i64, depth: usize) -> Result<Vec<GraphEntity>>
k_hop_filtered(start_id: i64, depth: usize, allowed_types: &[&str]) -> Result<Vec<GraphEntity>>
chain(start_id: i64, pattern: &str) -> Result<Vec<GraphEntity>>

// Path operations
has_path(from_id: i64, to_id: i64) -> Result<bool>
shortest_path(from_id: i64, to_id: i64) -> Result<Option<Vec<i64>>>

// Pattern matching
pattern_matches(pattern: &PatternQuery) -> Result<Vec<PatternMatch>>
```

### Backend Types

```rust
// Core types
pub type GraphEntity = structs::GraphEntity;
pub type GraphEdge = structs::GraphEdge;
pub type PatternQuery = pattern::PatternQuery;

// Backend traits
pub trait GraphBackend {
    fn insert_entity(&mut self, name: &str, entity_type: &str) -> Result<i64>;
    fn get_entity(&self, id: i64) -> Result<Option<GraphEntity>>;
    // ... other backend methods
}

pub struct SqliteGraphBackend {
    // SQLite-backed implementation
}

// Ergonomic client wrapper
pub struct BackendClient<B: GraphBackend> {
    backend: B,
}

// Helper types
pub struct NodeId(pub i64);
pub struct EdgeId(pub i64);
pub struct Label(pub String);
pub struct PropertyKey(pub String);
pub struct PropertyValue(pub serde_json::Value);
```

### Pattern Engine

```rust
// Pattern queries
pub struct PatternQuery {
    pub legs: Vec<PatternLeg>,
}

pub struct PatternLeg {
    pub edge_type: String,
    pub direction: PatternDirection,
    pub node_constraint: Option<NodeConstraint>,
}

// Pattern execution
fn analyze(
    start_id: i64,
    pattern: &PatternQuery,
    config: &ReasoningConfig
) -> Result<Vec<ReasoningCandidate>>;
```

### Reasoning Pipeline

```rust
pub struct ReasoningConfig {
    pub max_depth: usize,
    pub max_candidates: usize,
    pub scoring_weights: ScoringWeights,
}

pub struct ReasoningCandidate {
    pub node_id: i64,
    pub score: f64,
    pub path: Vec<i64>,
    pub explanation: String,
}

// Pipeline execution
pub fn run_pipeline(
    graph: &SqliteGraph,
    pipeline: &ReasoningPipeline
) -> Result<PipelineResult>;
```

### DSL API

```rust
// DSL parsing
pub fn parse_dsl(input: &str) -> DslResult {
    // Returns parsed DSL structure
}

// DSL execution types
pub enum DslResult {
    Pattern(PatternQuery),
    Pipeline(ReasoningPipeline),
    Subgraph(SubgraphRequest),
    Error(String),
}

// Subgraph requests
pub struct SubgraphRequest {
    pub root_id: i64,
    pub depth: usize,
    pub allowed_node_types: Vec<String>,
    pub allowed_edge_types: Vec<String>,
}
```

### Safety API

```rust
// Safety checks
pub fn run_safety_checks(graph: &SqliteGraph) -> Result<SafetyReport>;
pub fn run_strict_safety_checks(graph: &SqliteGraph) -> Result<(), SafetyError>;

// Safety report
pub struct SafetyReport {
    pub orphan_edges: usize,
    pub duplicate_edges: usize,
    pub invalid_labels: usize,
    pub invalid_properties: usize,
}

// Integrity sweep
pub fn run_integrity_sweep(graph: &SqliteGraph) -> Result<IntegrityReport>;
```

### Migration API

```rust
pub struct MigrationManager {
    primary: SqliteGraphBackend,
    shadow: SqliteGraphBackend,
    cutover_active: bool,
}

impl MigrationManager {
    pub fn new(primary_path: &str, shadow_path: &str) -> Result<Self>;
    pub fn insert_node(&mut self, spec: &NodeSpec) -> Result<i64>;
    pub fn insert_edge(&mut self, from_id: i64, to_id: i64, edge_type: &str) -> Result<i64>;
    pub fn shadow_read(&self, job: &DualRuntimeJob) -> Result<DualRuntimeReport>;
    pub fn cutover(&mut self) -> Result<()>;
    pub fn is_cutover(&self) -> bool;
    pub fn active_backend(&self) -> &SqliteGraphBackend;
}
```

### CLI API Types

```rust
// CLI command types
pub enum SubgraphRequest {
    root: i64,
    depth: usize,
    types: HashMap<String, Vec<String>>,
}

// Pipeline execution
pub struct PipelineRequest {
    pub dsl: String,
}

// Explanation results
pub struct PipelineExplanation {
    pub steps_summary: Vec<String>,
    pub node_counts: Vec<usize>,
    pub filters: Vec<String>,
    pub scoring: String,
}
```

---

## Field Name Changes (V1 → V2)

### Node Fields
- **Removed**: V1 node fields (no longer applicable)
- **Current V2**: `outgoing_edge_count`, `incoming_edge_count` in V2 clustered adjacency

### Edge Fields
- **V1-style API**: Maintained for compatibility - `EdgeRecord` struct
- **V2 Storage**: `CompactEdgeRecord` for efficient storage
- **Fields**: `id`, `from_id`, `to_id`, `edge_type`, `flags`, `data`

### Adjacency Fields
- **V2 Clustered**: Uses V2 cluster offsets and sizes
- **Field Names**: `cluster_offset`, `cluster_size` for adjacency management

---

## Type System Changes

### Removed V1 Types
- `NodeRecordV1` - Removed, replaced by V2 clustered adjacency
- `GraphFileV1` - Removed, replaced by V2 graph file handling
- `EdgeRecordV1` - Removed, replaced by `CompactEdgeRecord` storage

### Current V2 Types
- `NodeRecordV2` - V2 clustered adjacency node format
- `CompactEdgeRecord` - Optimized edge storage format
- `V2ClusteredAdjacency` - V2 adjacency management system
- `EdgeRecord` - V1-style API for compatibility (backed by `CompactEdgeRecord`)

---

## Schema Versions

### Current State
- **Schema Version**: 2 (reported by CLI `status` command)
- **V1 Databases**: No longer supported - V1 code completely removed
- **V2 Databases**: Fully supported with V2 native backend
- **Migration**: Automatic from older versions via `run_pending_migrations`

### Version 2 Features
- `graph_meta_history` table for migration tracking
- V2 clustered adjacency storage
- Enhanced integrity checks
- V2 field naming conventions

---

## V1 Prevention Barriers

SQLiteGraph includes compile-time barriers to prevent V1 code reintroduction:

### Compilation Barriers
```rust
// Feature flag barriers
#[cfg(feature = "v1_experimental")]
compile_error!("V1_EXPERIMENTAL FEATURE DETECTED: V1 has been permanently removed");

#[cfg(feature = "enable_v1")]
compile_error!("ENABLE_V1 FEATURE DETECTED: V1 has been permanently removed");
```

### Runtime Enforcement
```rust
// V2-only enforcement function
sqlitegraph::backend::native::v1_prevention::enforce_v2_only();
```

### Prevention Tests
- 5 tests in `v1_prevention_compilation_tests.rs`
- Verify V1 feature flags cause compilation failures
- Ensure V2-only behavior is enforced at runtime
- Validate V1 quarantine mechanisms are active

---

## Performance Characteristics

### V2 Optimizations
- **Clustered Adjacency**: Efficient edge storage and retrieval
- **Compact Edge Records**: Reduced memory footprint
- **Optimized Serialization**: V2-specific serialization formats
- **Deterministic Ordering**: Guaranteed sorted adjacency operations

### Benchmark Gates
- `sqlitegraph_bench.json` contains deterministic baseline metrics
- `bench_gates::check_thresholds` enforces performance limits
- CI integration prevents performance regressions

---

## CLI Operations

### Supported Commands
```bash
sqlitegraph --command status                    # Backend info + entity count
sqlitegraph --command list                      # Entity IDs + names (sorted)
sqlitegraph --command subgraph --root N --depth D  # JSON neighborhood extraction
sqlitegraph --command pipeline --dsl "EXPR"     # Reasoning pipeline execution
sqlitegraph --command explain-pipeline --dsl "EXPR"  # Pipeline step breakdown
sqlitegraph --command dsl-parse --input "EXPR"   # DSL validation
sqlitegraph --command safety-check [--strict]   # Integrity validation
sqlitegraph --command metrics [--reset-metrics] # Performance counters
```

### Status Output
```
backend=sqlite schema_version=2 nodes=123
```

---

## Error Handling

### Result Types
All operations return `Result<T, SQLiteGraphError>` with specific error types:

- `DatabaseError` - SQLite operation failures
- `IntegrityError` - Constraint violations
- `MigrationError` - Schema migration issues
- `SafetyError` - Safety check failures (in strict mode)
- `DslError` - DSL parsing/validation errors

### V1-Specific Errors
Any attempt to use V1 features will result in:
- **Compilation Errors**: For V1 feature flags
- **Runtime Panics**: For V1 code paths (blocked by prevention barriers)
- **Safety Violations**: Detected by V2 integrity checks

---

## Development Guidelines

### V2-Only Development
- All new code must use V2 APIs and patterns
- V1 compatibility layers are for API compatibility only
- V2 field names must be used consistently
- V2 serialization formats for all storage operations

### Testing Requirements
- All tests must pass with V2-only backend
- V1 prevention tests must continue to pass
- Performance gates must be maintained
- Safety checks must pass in strict mode

---

This API documentation reflects the current V2-only state of SQLiteGraph. V1 legacy code has been permanently removed, and compile-time barriers prevent any V1 reintroduction.