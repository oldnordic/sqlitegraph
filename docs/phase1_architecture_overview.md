# Phase 1: Architecture Overview

## Current SQLiteGraph Architecture

### High-Level Architecture
SQLiteGraph is a deterministic, embedded graph database built entirely on SQLite, designed for SynCore/SPEC 16-17 requirements. The architecture follows a layered approach with clear separation of concerns.

### Current Backend: SQLite Backend

#### SqliteGraphBackend (sqlitegraph/src/backend.rs - 294 LOC)
The current backend implementation wraps SQLite database operations and implements the `GraphBackend` trait:

**Core Components:**
- `SqliteGraph` struct (graph/core.rs) - Main SQLite connection and schema management
- `GraphBackend` trait - Backend abstraction with 11 methods for graph operations
- `BackendDirection` enum (Outgoing/Incoming) - Traversal direction specification
- `NodeSpec` and `EdgeSpec` structs - Input specifications for graph mutations

**Key Responsibilities:**
- Entity and edge storage via SQLite tables
- Deterministic adjacency queries using SQL indexes
- Pattern matching and multi-hop traversals
- Integration with MVCC snapshot system
- Schema management and migrations

### Graph Core Components

#### Core Data Structures (graph/types.rs - 79 LOC)
```rust
pub struct GraphEntity {
    pub id: i64,
    pub kind: String,
    pub name: String,
    pub file_path: Option<String>,
    pub data: serde_json::Value,
}

pub struct GraphEdge {
    pub id: i64,
    pub from_id: i64,
    pub to_id: i64,
    pub edge_type: String,
    pub data: serde_json::Value,
}
```

#### SQLite Graph Core (graph/core.rs - 94 LOC)
```rust
pub struct SqliteGraph {
    pub(crate) conn: Connection,
    pub(crate) outgoing_cache: AdjacencyCache,
    pub(crate) incoming_cache: AdjacencyCache,
    pub(crate) metrics: GraphMetrics,
    pub(crate) statement_tracker: StatementTracker,
    pub(crate) snapshot_manager: SnapshotManager,
}
```

**Key Features:**
- Connection management with WAL mode optimizations
- Prepared statement caching (128 statements)
- Performance tuning for file-based databases
- In-memory database support

### Database Schema (schema.rs - 190 LOC)

#### Core Tables
- `graph_entities`: Node storage (id, kind, name, file_path, data)
- `graph_edges`: Edge storage (id, from_id, to_id, edge_type, data)
- `graph_labels`: Entity labels for indexing
- `graph_properties`: Entity properties for metadata
- `graph_meta`: Schema version tracking

#### Indexes for Deterministic Performance
- `idx_edges_from/to`: Fast adjacency lookups
- `idx_edges_type`: Edge type filtering
- `idx_labels_label`: Label-based queries
- `idx_props_key_value`: Property-based queries
- `idx_entities_kind_id`: Entity type filtering

### Pattern Engine and Reasoning Components

#### Pattern Engine (pattern_engine/ - 518 LOC total)
- **pattern_engine/mod.rs** (18 LOC) - Module organization
- **pattern_engine/pattern.rs** (86 LOC) - Pattern query structures
- **pattern_engine/query.rs** (161 LOC) - Query execution engine
- **pattern_engine/matcher.rs** (80 LOC) - Pattern matching logic
- **pattern_engine/property.rs** (61 LOC) - Property-based filtering
- **pattern_engine/tests.rs** (173 LOC) - Test utilities

**Key Types:**
- `PatternQuery` - Structured pattern definitions
- `PatternMatch` - Match results with node sets
- `NodeConstraint` - Node filtering conditions

#### Pattern Engine Cache (pattern_engine_cache/ - 346 LOC total)
- **edge_validation.rs** (49 LOC) - Edge validation logic
- **fast_path_detection.rs** (27 LOC) - Optimization detection
- **fast_path_execution.rs** (124 LOC) - Cached execution paths
- **mod.rs** (13 LOC) - Cache coordination
- **tests.rs** (133 LOC) - Cache validation

#### Reasoning Pipeline (pipeline.rs - 145 LOC)
- `ReasoningPipeline` - Multi-step reasoning sequences
- `ReasoningStep` enum (Pattern, KHops, Filter, Score)
- Pipeline execution with deterministic ordering

#### DSL Support (dsl.rs - 101 LOC)
- `parse_dsl()` function - String to structured queries
- `DslResult` enum - Pattern, Pipeline, Subgraph, Error variants
- Integration with CLI and reasoning components

### MVCC and Snapshot Handling

#### MVCC System (mvcc.rs - 257 LOC)
```rust
pub struct SnapshotState {
    pub outgoing: HashMap<NodeId, Vec<NodeId>>,
    pub incoming: HashMap<NodeId, Vec<NodeId>>,
    pub created_at: std::time::SystemTime,
}

pub struct SnapshotManager {
    current: ArcSwap<Arc<SnapshotState>>,
}
```

**Features:**
- Lock-free atomic updates via ArcSwap
- Immutable snapshot state with cloned adjacency maps
- Deterministic read isolation
- Integration with SQLite read-only connections

#### Graph Snapshots (graph/snapshot.rs - 62 LOC)
- `GraphSnapshot` wrapper for read-only operations
- Snapshot-based query isolation
- Integration with backend abstraction

### Reindexing and Indexing Responsibilities

#### Reindexing System (reindex/ - 733 LOC total)
**Core Components:**
- **reindex/core.rs** (272 LOC) - Main reindexing orchestration
- **reindex/entity_edge.rs** (103 LOC) - Entity/edge index rebuilding
- **reindex/label_property.rs** (109 LOC) - Metadata index rebuilding
- **reindex/validation.rs** (118 LOC) - Index validation
- **reindex/cache.rs** (91 LOC) - Cache rebuilding
- **reindex/progress.rs** (63 LOC) - Progress tracking
- **mod.rs** (21 LOC) - Module organization

**Key Types:**
```rust
pub struct ReindexConfig {
    pub syncore: bool,
    pub sync_graph: bool,
    pub validate: bool,
    pub batch_size: usize,
    pub progress_callback: Option<Box<dyn Fn(ReindexProgress) + Send + Sync>>,
}
```

**Features:**
- Batch processing for large datasets
- Progress tracking and callbacks
- Validation after rebuilding
- Integration with metrics and instrumentation

### Public API Organization (lib.rs - 71 LOC)

#### Core API Modules
```rust
// Core modules - public API
pub mod backend_client;
pub mod errors;
pub mod graph;
pub mod mvcc;
pub mod pattern_engine;
pub mod pattern_engine_cache;
pub mod query;
pub mod recovery;
pub mod reindex;
```

#### Key Public Exports
- **Types:** `NodeId`, `EdgeId`, `Label`, `PropertyKey`, `PropertyValue`
- **Backend:** `SqliteGraphBackend`, `BackendClient`, `GraphBackend` trait
- **Core Graph:** `GraphEntity`, `GraphEdge`, `SqliteGraph`
- **Pattern Matching:** `PatternTriple`, `TripleMatch`, `match_triples`
- **Reasoning:** `ReasoningConfig`, `explain_pipeline`
- **Safety:** `run_safety_checks`, `run_deep_safety_checks`
- **Recovery:** Graph backup and restore utilities

### API Ergonomics (api_ergonomics.rs - 139 LOC)

#### Ergonomic Wrapper Types
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub i64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EdgeId(pub i64);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Label(pub String);
```

#### Pipeline Explanation
```rust
pub struct PipelineExplanation {
    pub steps_summary: Vec<String>,
    pub node_counts_per_step: Vec<usize>,
    pub filters_applied: Vec<String>,
    pub scoring_notes: Vec<String>,
}
```

**Features:**
- Display implementations for human-readable IDs
- Conversion utilities (From<i64> for NodeId)
- Pipeline execution analysis and debugging

## Planned Dual-Backend Architecture

### Architecture Goals

#### Maintain SQLite as Canonical Backend
- Keep `SqliteGraphBackend` as the reference implementation
- Preserve all existing functionality and semantics
- Maintain comprehensive test coverage
- Ensure no breaking changes to public APIs

#### Introduce Native File Backend
- **Planned Component:** `NativeFileBackend` (not yet implemented)
- **Location:** TBD (possibly sqlitegraph/src/native_backend/ or sqlitegraph-native/)
- **Goal:** Native adjacency storage without SQLite dependency
- **Approach:** File-based adjacency lists with mmap access

#### Backend Selection Strategy
- Runtime backend selection via `BackendSelector` (backend_selector.rs - 39 LOC)
- Factory pattern for backend instantiation
- Configuration-driven backend choice
- Backward compatibility with existing code

### Explicit Architectural Boundaries

#### Public API Layer (lib.rs, api_ergonomics.rs)
**Responsibilities:**
- Ergonomic type definitions (NodeId, EdgeId, etc.)
- High-level function exports
- Pipeline explanation utilities
- Error type definitions
- Public re-exports

**Invariant:** No backend-specific logic, pure abstractions only

#### Backend Abstraction Layer (backend.rs)
**Current Components:**
- `GraphBackend` trait with 11 methods
- `BackendDirection` enum
- `NodeSpec` and `EdgeSpec` input types
- `SqliteGraphBackend` implementation

**Planned Extensions:**
- `NativeFileBackend` implementation (future)
- Backend factory abstractions
- Common validation utilities

**Invariant:** All backend operations must be implementable by both SQLite and native backends

#### Graph Core and Algorithms (graph/*, algo.rs, bfs.rs, multi_hop.rs)
**Current Components:**
- Graph data structures and validation
- Algorithm implementations (BFS, multi-hop, pattern matching)
- Schema management and migrations
- MVCC snapshot system
- Reindexing and indexing utilities

**Invariant:** Backend-agnostic algorithms that operate through GraphBackend trait

#### Pattern and Reasoning Layer (pattern_engine/*, pattern.rs, pipeline.rs, reasoning.rs)
**Current Components:**
- Pattern query execution
- Reasoning pipeline orchestration
- DSL parsing and execution
- Cache optimization

**Invariant:** Works with any GraphBackend implementation

### Non-Goals for This Phase

#### No Code Changes
- This phase is documentation-only
- No new Rust files or modifications
- No implementation of native backend
- No breaking changes to existing APIs

#### No File Format Specification
- Native backend file format design is separate (see phase1_native_backend_file_format.md)
- No binary layout decisions in this phase
- No performance optimization commitments

#### No GPU or External Dependencies
- Native backend will remain CPU-only
- No ROCm, CUDA, or external compute engines
- Keep dependencies minimal (current Cargo.toml constraints apply)

#### No Changes to LOC Limits
- Current 300 LOC limit policy remains
- Modularization plan is separate (see phase1_modularization_plan.md)
- No architectural exceptions for file size

### Integration Points for Future Native Backend

#### Required Interface Compliance
- Must implement all 11 `GraphBackend` trait methods
- Must preserve deterministic behavior semantics
- Must support same error handling patterns
- Must integrate with existing MVCC system

#### Performance Expectations
- Target parity with SQLite backend for common operations
- Optimized for adjacency-heavy workloads
- Memory-mapped file I/O for large datasets
- Cache-friendly data layouts

#### Testing Integration
- Must pass all existing backend trait tests
- Must support dual-runtime comparison tests
- Must integrate with existing CLI and reasoning pipelines
- Must preserve all deterministic behavior guarantees

## Conclusion

The current SQLiteGraph architecture provides a solid foundation for dual-backend support through the existing `GraphBackend` trait abstraction. The layered design with clear boundaries between public API, backend abstraction, and graph algorithms enables clean addition of a native file backend without breaking existing functionality.

The documentation in subsequent phases will detail the specific implementation strategies for the native backend, modularization improvements, and comprehensive testing approaches.