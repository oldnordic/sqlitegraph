# Coding Conventions

**Analysis Date:** 2026-01-20

## Naming Patterns

**Files:**
- `snake_case.rs` for all Rust source files
- Module directories use `snake_case`
- Test files use `<topic>_tests.rs` pattern (e.g., `cache_tests.rs`, `integration_tests.rs`)

**Functions:**
- `snake_case` for all functions
- Public API functions use descriptive names: `insert_node`, `fetch_outgoing`, `match_triples`
- Private helper functions often prefixed or have contextual names
- Builder-style methods use the verb form: `start_label()`, `end_property()`, `direction()`

**Variables:**
- `snake_case` for local variables
- Short names for loop counters: `i`, `idx`, `id`
- Descriptive names for graph elements: `start_id`, `end_id`, `edge_type`, `node_id`

**Types:**
- `PascalCase` for structs, enums, and type aliases
- `PascalCase` for trait names
- Newtype wrappers use `PascalCase` with tuple contents: `NodeId(pub i64)`, `EdgeId(pub i64)`
- Error types use descriptive `PascalCase` with `Error` suffix: `SqliteGraphError`, `NativeBackendError`

**Constants:**
- `SCREAMING_SNAKE_CASE` for constants: `BASE_SCHEMA_VERSION`, `SCHEMA_VERSION`
- Static values also use `SCREAMING_SNAKE_CASE`: `MIGRATION_STEPS`

## Code Style

**Formatting:**
- No explicit rustfmt.toml found in project root (uses defaults)
- 100-character line limit typically not enforced (some lines exceed 100 chars)
- Standard Rust formatting with 4-space indentation

**Linting:**
- Clippy configured via `sqlitegraph/clippy.toml`
- MSRV: 1.70.0
- Cognitive complexity threshold: 30
- Type complexity threshold: 300
- Too-many-arguments threshold: 12
- Too-many-lines threshold: 150
- Most lint suppressions handled via `#[allow]` attributes in code rather than global config

**Derive Macros:**
Common derive patterns observed:
- `Debug` on almost all public types
- `Clone` on data-carrying types and newtype wrappers
- `Copy` on newtype wrappers with primitive inner types: `NodeId`, `EdgeId`
- `PartialEq, Eq` on types that need comparison
- `Hash` on types used as map keys
- `Serialize, Deserialize` on types that need persistence (from `serde`)
- `Default` on config/builder types
- `thiserror::Error` on error types

Example:
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub i64);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Label(pub String);

#[derive(Debug, Error)]
pub enum SqliteGraphError {
    #[error("connection error: {0}")]
    ConnectionError(String),
    // ...
}
```

## Import Organization

**Order:**
1. Standard library imports (`std::*`)
2. Third-party crate imports (external dependencies)
3. Local crate imports (`crate::*`)
4. Module imports (`super::*`, `self::*`)

**Path Aliases:**
- `use crate::errors::SqliteGraphError;` - error type pattern
- `use crate::graph::SqliteGraph;` - main type imports
- Re-exports at module level for public API
- `pub use` directives in `lib.rs` to shape public API

**Typical import pattern:**
```rust
use std::collections::VecDeque;
use ahash::{AHashMap, AHashSet};
use crate::{errors::SqliteGraphError, graph::SqliteGraph};
use crate::progress::ProgressCallback;
```

**Common external dependencies:**
- `serde::{Deserialize, Serialize}` - serialization
- `serde_json::json` - JSON macro
- `thiserror::Error` - error derive macro
- `ahash::{AHashMap, AHashSet}` - fast hashmap (preferred over std)
- `parking_lot::{Mutex, RwLock}` - lock primitives (preferred over std)
- `rusqlite` - SQLite bindings
- `log` - logging facade

## Error Handling

**Patterns:**
- All fallible operations return `Result<T, SqliteGraphError>`
- Error variants use descriptive names: `ConnectionError`, `SchemaError`, `QueryError`, `NotFound`, `InvalidInput`
- Convenience constructors on error type: `SqliteGraphError::connection()`, `SqliteGraphError::invalid_input()`
- `?` operator used extensively for error propagation
- Context added via `.map_err(|e| SqliteGraphError::schema(e.to_string()))` pattern
- No `unwrap()` in production code paths (per CLAUDE.md rules)
- `expect()` used only in test code with descriptive messages

**Error type pattern:**
```rust
// errors.rs
#[derive(Debug, Error)]
pub enum SqliteGraphError {
    #[error("connection error: {0}")]
    ConnectionError(String),
    #[error("schema error: {0}")]
    SchemaError(String),
    #[error("query error: {0}")]
    QueryError(String),
    #[error("entity not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("fault injected: {0}")]
    FaultInjected(String),
    #[error("transaction error: {0}")]
    TransactionError(String),
    #[error("validation error: {0}")]
    ValidationError(String),
    #[error("native backend error: {0}")]
    NativeError(#[from] crate::backend::native::types::NativeBackendError),
}

impl SqliteGraphError {
    pub fn connection<T: Into<String>>(msg: T) -> Self {
        SqliteGraphError::ConnectionError(msg.into())
    }
    // ... other convenience constructors
}
```

**Validation pattern:**
```rust
pub fn validate_entity(entity: &GraphEntity) -> Result<(), SqliteGraphError> {
    if entity.kind.trim().is_empty() {
        return Err(SqliteGraphError::invalid_input("entity kind must be set"));
    }
    if entity.name.trim().is_empty() {
        return Err(SqliteGraphError::invalid_input("entity name must be set"));
    }
    Ok(())
}
```

## Logging

**Framework:** `log` crate facade (not `println!` or `eprintln!` in production code)

**Patterns:**
- No direct logging calls observed in main source (uses `log` facade)
- Debug feature available: `debug = []` in Cargo.toml features
- Debug and trace features for V2 I/O: `trace_v2_io = []`
- Logging disabled in release for zero overhead

**Configuration:**
- `debug` feature enables debug/info logging
- `trace_v2_io` feature for V2 I/O operation debugging
- Release builds optimize for zero overhead

## Comments

**When to Comment:**
- Complex algorithm explanations (BFS, graph algorithms)
- Performance characteristics documentation
- Thread safety guarantees
- Memory ordering explanations (for concurrent code)
- Invariant assertions in debug mode

**JSDoc/TSDoc equivalent:**
- Rust doc comments (`///`) used extensively
- Module-level docs with `//!` at file top
- Function documentation includes:
  - Description
  - `# Arguments` section
  - `# Returns` section
  - `# Complexity` (for algorithms)
  - `# Example` (for public APIs)
  - `# Panics` (when applicable)

**Documentation pattern:**
```rust
/// Finds all connected components in the graph using BFS.
///
/// A connected component is a maximal subgraph where any two nodes are connected
/// by a path. This function uses bidirectional BFS (both incoming and outgoing edges).
///
/// # Arguments
/// * `graph` - The graph to analyze
///
/// # Returns
/// Vector of components, where each component is a sorted vector of node IDs.
/// Components are sorted by their smallest node ID.
///
/// # Complexity
/// Time: O(|V| + |E|) - visits each node and edge once
/// Space: O(|V|) for visited set and BFS queue
///
/// # Example
/// ```
/// use sqlitegraph::{SqliteGraph, algo::connected_components};
/// let graph = SqliteGraph::open_in_memory()?;
/// let components = connected_components(&graph)?;
/// # Ok::<(), sqlitegraph::SqliteGraphError>(())
/// ```
```

**Module documentation:**
```rust
//! Graph algorithms for centrality, community detection, and structure analysis.
//!
//! This module provides a collection of graph algorithms for analyzing graph
//! topology, identifying important nodes, and discovering community structure.
```

## Function Design

**Size:** No strict limit observed, but:
- Most functions under 50 lines
- Algorithm implementations can be longer (up to 150+ lines)
- Clippy threshold: 150 lines per function

**Parameters:**
- Prefer specific types over generics when possible
- Borrowed references for read-only: `&SqliteGraph`
- Slice references for collections: `&[ChainStep]`
- Configuration structs for many parameters

**Return Values:**
- `Result<T, SqliteGraphError>` for fallible operations
- `Option<T>` for optional values
- `Vec<T>` for collections
- Tuple returns for multiple related values: `(usize, usize)` for `(incoming, outgoing)` degree

## Module Design

**Exports:**
- Public items re-exported in module `mod.rs` files
- `pub use` extensively for shaping public API
- Private items in submodules with selective re-export

**Barrel Files:**
- `lib.rs` acts as main barrel file
- Module `mod.rs` files re-export sub-items
- Pattern: `pub use self::core::{SqliteGraph, is_in_memory_connection};`

**Module organization:**
```rust
// Public modules
pub mod backend;
pub mod config;
pub mod graph;

// Re-exports for public API
pub use api_ergonomics::{Label, NodeId, PropertyKey, PropertyValue};
pub use graph_opt::{GraphEdgeCreate, GraphEntityCreate, ...};

// Internal modules (private)
mod api_ergonomics;
mod client;
mod reasoning;
```

## Async/Promise Equivalent Patterns

**No async/await:**
- This is a synchronous, blocking codebase
- Uses `Result<T, E>` for error handling (not Promise/Future)
- Thread-safe types use interior mutability: `ArcSwap`, `parking_lot::RwLock`

**Thread Safety:**
- `SqliteGraph` is NOT `Sync` (uses `RefCell`)
- `GraphSnapshot` IS `Sync` (uses Arc)
- `Arc<SnapshotState>` for shared read-only data

## File Size Guidelines

**Per CLAUDE.md:**
- Max 300 LOC per file (600 with justification)
- Large files observed: `algo.rs` (1398 lines), `lib.rs` (340 lines)
- Large algorithm modules justified by complexity

## Newtype Wrapper Pattern

**For type safety:**
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub i64);

impl NodeId {
    pub fn as_i64(self) -> i64 {
        self.0
    }
}

impl From<i64> for NodeId {
    fn value: i64) -> Self {
        NodeId(value)
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
```

## Trait Implementation Patterns

** blanket implementations for references:**
```rust
impl<B> GraphBackend for &B
where
    B: GraphBackend + ?Sized,
{
    fn insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError> {
        (*self).insert_node(node)
    }
    // ...
}
```

**Default implementations:**
```rust
impl Default for SnapshotManager {
    fn default() -> Self {
        Self::new()
    }
}
```

## Attribute Usage

**Conditional compilation:**
```rust
#[cfg(feature = "native-v2")]
pub use backend::native::v2::wal::{V2WALConfig, V2WALManager, ...};

#[cfg(test)]
mod tests { ... }

#[cfg(debug_assertions)]
{
    // Debug-only validation
}
```

**Allow attributes:**
```rust
#[allow(unused_imports)] // Backend trait API types for future use
pub use sqlite::types::{BackendDirection, EdgeSpec, NeighborQuery, NodeSpec};
```

**Test attributes:**
```rust
#[test]
fn test_function_name() { ... }
```

---

*Convention analysis: 2026-01-20*
