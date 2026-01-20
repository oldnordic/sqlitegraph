# Coding Conventions

**Analysis Date:** 2026-01-20

## Naming Patterns

**Files:**
- `snake_case.rs` for all source files
- `mod.rs` for module directories containing submodules
- `tests.rs` for module-level test files within a module
- `lib.rs` for library root

**Functions:**
- `snake_case` for all functions and methods
- Builder pattern methods: `snake_case` (e.g., `dimension()`, `m()`, `ef_construction()`)
- Constructor: `new()` for standard, `from_xxx()` for conversions

**Variables:**
- `snake_case` for local variables and parameters
- `SCREAMING_SNAKE_CASE` for constants (e.g., `V2_MAGIC`, `NODE_SLOT_SIZE`)

**Types:**
- `PascalCase` for structs, enums, and type aliases
- `PascalCase` for traits
- Newtype wrappers follow `PascalCase` (e.g., `NodeId`, `EdgeId`, `Label`, `PropertyKey`)

**Modules:**
- `snake_case` for module directories and declarations
- Submodules organized by feature (e.g., `backend/native/v2/wal/checkpoint`)

## Code Style

**Formatting:**
- Tool: `rustfmt` (default configuration)
- No custom `rustfmt.toml` at project root
- Edition: Rust 2024
- Max line length: Not enforced (no `max_width` override)
- Edition-specific features: 2024 edition enabled in `sqlitegraph/Cargo.toml`

**Linting:**
- Tool: `clippy` with custom configuration at `sqlitegraph/clippy.toml`
- MSRV: 1.70.0
- Key thresholds:
  - `cognitive-complexity-threshold = 30`
  - `type-complexity-threshold = 300`
  - `too-many-arguments-threshold = 12`
  - `too-many-lines-threshold = 150`

**Dead code policy:**
- Dead code is intentionally NOT silenced globally
- Cross-target dead_code false positives documented in `clippy.toml`
- Use `#[allow(dead_code)]` sparingly and with justification

## Import Organization

**Order:**
1. Standard library imports (`std::*`)
2. Third-party crate imports (alphabetical)
3. Local crate imports (`crate::*` or `super::*`)
4. Module declarations (`mod xxx;`)

**Path Aliases:**
- No `path` aliases configured in `Cargo.toml`
- Use `crate::` for absolute paths within the crate
- Re-exports at module root for public API (`pub use xxx::Yyy;`)

**Example from `sqlitegraph/src/lib.rs`:**
```rust
// Third-party
use std::fmt;

// Local crate
use crate::cache::AdjacencyCache;
use crate::errors::SqliteGraphError;

// Re-exports for public API
pub use api_ergonomics::{Label, NodeId, PropertyKey, PropertyValue};
```

## Error Handling

**Patterns:**
- Use `thiserror` for error enums with `#[derive(Debug, thiserror::Error)]`
- Error variants use descriptive names with context
- Conversion errors via `#[from]` attribute
- Contextual errors with manual formatting using `#[error("...")]`

**Error type structure from `sqlitegraph/src/errors.rs`:**
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SqliteGraphError {
    #[error("connection error: {0}")]
    ConnectionError(String),
    #[error("schema error: {0}")]
    SchemaError(String),
    #[error("entity not found: {0}")]
    NotFound(String),
    // ... more variants
}
```

**Native backend errors from `backend/native/types/errors.rs`:**
```rust
#[derive(Debug, thiserror::Error)]
pub enum NativeBackendError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid magic number: expected {expected:08x}, found {found:08x}")]
    InvalidMagic { expected: u64, found: u64 },

    #[error("Node {node_id} not found during {operation}")]
    NodeNotFound { node_id: NativeNodeId, operation: String },
    // ... more variants
}
```

**Constructor helpers:**
- Each error variant has a constructor helper method
- Pattern: `pub fn variant_name<T: Into<String>>(msg: T) -> Self`
- Example: `SqliteGraphError::connection("database busy")`

**Return type pattern:**
- Public API: `Result<T, SqliteGraphError>`
- Backend-specific: `Result<T, NativeBackendError>` aliased as `NativeResult<T>`

## Logging

**Framework:**
- `log` crate for facaded logging (version 0.4)
- `debug` feature flag for conditional debug logging
- No direct logging in release builds (zero overhead)

**Patterns:**
- Use `log::{debug, info, warn, error, trace}` macros
- Debug-only logging with `#[cfg(feature = "debug")]`
- Error messages go to stderr via `eprintln!` for critical failures

**Example from `sqlitegraph/src/hnsw/index.rs`:**
```rust
// Debug-only logging
#[cfg(feature = "debug")]
{
    log::debug!("Loading HNSW index: {}", name);
}
```

## Comments

**When to Comment:**
- Module-level: Always provide `//!` documentation
- Public API: Always document with `///` doc comments
- Complex algorithms: Explain approach with inline comments
- Invariants: Document critical guarantees
- Performance notes: Document O(n) complexity

**JSDoc/TSDoc equivalent (rustdoc):**
- Use `///` for item documentation
- Use `//!` for module-level documentation
- Include examples in doc comments with `rust,ignore` flag
- Document panics, errors, and safety in `# Panics`, `# Errors`, `# Safety` sections

**Example from `sqlitegraph/src/lib.rs`:**
```rust
//! SQLite-based graph database with unified backend support.
//!
//! This crate provides a lightweight, deterministic graph database...
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use sqlitegraph::{open_graph, GraphConfig, BackendKind};
//!
//! let graph = open_graph("my_graph.db", &GraphConfig::sqlite())?;
//! ```
```

**Function documentation:**
```rust
/// Load HNSW indexes from database
///
/// This is called during SqliteGraph construction to restore any
/// previously created HNSW indexes with full vector data.
///
/// # Errors
///
/// Returns `SqliteGraphError::InvalidInput` if index loading fails.
fn load_hnsw_indexes(conn: &Connection) -> Result<HashMap<String, HnswIndex>, SqliteGraphError>
```

## Function Design

**Size:**
- Target: Keep functions under 50 lines
- Acceptable: Up to 150 lines (per clippy config)
- Beyond 150: Consider splitting into helper functions

**Parameters:**
- Prefer 3-5 parameters
- Maximum: 12 parameters (per clippy config)
- Beyond 5: Consider builder pattern or struct parameter

**Return Values:**
- Use `Result<T, Error>` for fallible operations
- Use `Option<T>` for optional returns (not errors)
- Public API never panics (documented invariants only)
- Use `unwrap()` sparingly, only in tests or with justification

**Example parameter pattern (builder for config):**
```rust
// From hnsw/config.rs
pub fn dimension(mut self, dimension: usize) -> Self {
    self.dimension = dimension;
    self
}

pub fn m(mut self, m: usize) -> Self {
    self.m = m;
    self
}
```

## Module Design

**Exports:**
- Public API re-exported at `lib.rs` level
- Internal modules marked `pub(crate)` or `pub` for tests
- Module-level `tests.rs` files for co-located tests
- Pattern: `pub use self::inner::PublicType;` for convenience

**Barrel Files:**
- `mod.rs` re-exports submodule contents
- Example from `backend/native/v2/mod.rs`:
```rust
pub mod edge_cluster;
pub mod free_space;
pub mod wal;

// Re-export V2 types
pub use edge_cluster::{CompactEdgeRecord, Direction, EdgeCluster};
pub use wal::{V2WALManager, V2WALConfig, WALManagerMetrics};
```

**Module visibility for tests:**
- Test utilities: `pub mod xxx; // Public for tests`
- Internal modules with tests: Keep module-private, use `#[cfg(test)]`
- Benchmark utilities: `pub mod bench_utils; // Public for tests`

**Feature-gated modules:**
```rust
#[cfg(feature = "native-v2")]
pub mod v2;

// Or at file level:
#![cfg(feature = "v2_experimental")]
```

## Constants and Magic Numbers

**Named constants:**
- File format constants: `pub const V2_MAGIC: [u8; 8]`
- Size limits: `const NODE_SLOT_SIZE: u64 = 4096`
- Performance targets: `pub const MAX_AVG_EDGE_SIZE: usize = 100`

**Organization:**
- Module-level constants in dedicated `constants.rs` or at module top
- Test constants in test module or test function
- Document units and purpose in comments

**Example from tests:**
```rust
const NODE_SLOT_SIZE: u64 = 4096; // From code: hardcoded 4096 bytes per node slot
const EDGE_SLOT_SIZE: u64 = 256;  // From code: hardcoded 256 bytes per edge slot
```

## Newtype Wrapper Pattern

**Purpose:**
- Type safety for IDs (prevent mixing `i64` values)
- Display implementation for user-friendly output
- Conversion methods for underlying access

**Pattern from `api_ergonomics.rs`:**
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub i64);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<i64> for NodeId {
    fn from(value: i64) -> Self {
        NodeId(value)
    }
}
```

**String wrappers:**
```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Label(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PropertyKey(pub String);
```

## Generic Bounds

**Naming:**
- Type parameters: `T`, `U`, `E` for errors
- Iterator types: `I`, `IT`
- Use descriptive names when generic serves specific purpose

**Pattern from error types:**
```rust
#[error("Invalid parameter: {context}")]
InvalidParameter {
    context: String,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
},
```

## Unsafe Code

**Policy:**
- Use only when necessary (FFI, raw pointer manipulation)
- Document safety invariants in `# Safety` sections
- Prefer `bytemuck` for safe byte casting

**Example pattern:**
```rust
// From hnsw/index.rs
let vector_bytes = bytemuck::cast_slice::<f32, u8>(&vector).to_vec();
```

---

*Convention analysis: 2026-01-20*
