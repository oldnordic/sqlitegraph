# Phase 1: Modularization Plan

## Current LOC-Limit Analysis

### Files Exceeding 300 LOC Limit

From the inventory_phase0.md analysis, the following files exceed the 300 LOC limit:

#### Source Code Violations (8 files)

1. **graph_opt.rs (302 LOC)** - 2 lines over limit
   - **File:** `sqlitegraph/src/graph_opt.rs`
   - **Issue:** Bulk graph operations and optimization utilities
   - **Root Cause:** Mixed responsibilities (bulk insert + cache management + validation)

2. **safety.rs (303 LOC)** - 3 lines over limit
   - **File:** `sqlitegraph/src/safety.rs`
   - **Issue:** Comprehensive safety validation and reporting
   - **Root Cause:** Multiple validation categories in single file

3. **sqlitegraph-cli/src/reasoning.rs (362 LOC)** - 62 lines over limit
   - **File:** `sqlitegraph-cli/src/reasoning.rs`
   - **Issue:** CLI command handling and pipeline execution
   - **Root Cause:** Multiple command categories mixed together

#### Test File Violations (12 files)

Large test files exceeding 300 LOC (not primary modularization target but noted for reference):
- `backend_trait_tests.rs` (369 LOC)
- `deterministic_index_tests.rs` (526 LOC)
- `pattern_cache_fastpath_tests.rs` (597 LOC)
- `pattern_engine_tests.rs` (417 LOC)
- `mvcc_snapshot_tests.rs` (720 LOC)
- `wal_tuning_tests.rs` (332 LOC)
- `pipeline_tests.rs` (247 LOC)
- `cli_reasoning_tests.rs` (286 LOC)
- `cli_safety_tests.rs` (240 LOC)

## Split Strategy for Each Over-300 File

### 1. graph_opt.rs (302 LOC) → 3 modules

#### Problem Analysis
Current responsibilities mixed in single file:
- Bulk entity insertion utilities
- Bulk edge insertion utilities
- Cache statistics and management
- Graph optimization helpers

#### Proposed Split

**graph_opt/bulk_insert.rs (~120 LOC)**
```rust
// PLANNED - Not yet implemented
pub mod bulk_insert {
    pub fn bulk_insert_entities<B: GraphBackend>(
        backend: &B,
        entities: Vec<GraphEntityCreate>,
    ) -> Result<Vec<i64>, SqliteGraphError>;

    pub fn bulk_insert_edges<B: GraphBackend>(
        backend: &B,
        edges: Vec<GraphEdgeCreate>,
    ) -> Result<Vec<i64>, SqliteGraphError>;
}
```

**graph_opt/cache_utils.rs (~100 LOC)**
```rust
// PLANNED - Not yet implemented
pub mod cache_utils {
    pub fn cache_stats() -> CacheStats;
    pub fn clear_cache() -> Result<(), SqliteGraphError>;
    pub fn optimize_cache() -> Result<(), SqliteGraphError>;
}
```

**graph_opt/optimization.rs (~80 LOC)**
```rust
// PLANNED - Not yet implemented
pub mod optimization {
    pub fn optimize_graph<B: GraphBackend>(backend: &B) -> Result<(), SqliteGraphError>;
    pub fn compact_graph<B: GraphBackend>(backend: &B) -> Result<(), SqliteGraphError>;
}
```

**graph_opt/mod.rs (reduced to ~20 LOC)**
```rust
// Module organization and re-exports
pub mod bulk_insert;
pub mod cache_utils;
pub mod optimization;

// Re-export existing public functions for backward compatibility
pub use bulk_insert::*;
pub use cache_utils::*;
pub use optimization::*;
```

### 2. safety.rs (303 LOC) → 4 modules

#### Problem Analysis
Current safety validation combines multiple concerns:
- Referential integrity validation
- Duplicate edge detection
- Label/property validation
- Comprehensive safety reporting
- CLI command integration

#### Proposed Split

**safety/integrity.rs (~80 LOC)**
```rust
// PLANNED - Not yet implemented
pub mod integrity {
    pub fn validate_referential_integrity<B: GraphBackend>(
        backend: &B,
    ) -> Result<IntegrityReport, SqliteGraphError>;

    pub fn detect_orphan_edges<B: GraphBackend>(
        backend: &B,
    ) -> Result<Vec<OrphanEdge>, SqliteGraphError>;

    pub fn detect_duplicate_edges<B: GraphBackend>(
        backend: &B,
    ) -> Result<Vec<DuplicateEdge>, SqliteGraphError>;
}
```

**safety/metadata.rs (~70 LOC)**
```rust
// PLANNED - Not yet implemented
pub mod metadata {
    pub fn validate_labels_and_properties<B: GraphBackend>(
        backend: &B,
    ) -> Result<MetadataReport, SqliteGraphError>;

    pub fn detect_invalid_metadata<B: GraphBackend>(
        backend: &B,
    ) -> Result<Vec<InvalidMetadata>, SqliteGraphError>;
}
```

**safety/reporting.rs (~90 LOC)**
```rust
// PLANNED - Not yet implemented
pub mod reporting {
    pub struct SafetyReport {
        // Report structure with detailed findings
    }

    pub fn generate_safety_report<B: GraphBackend>(
        backend: &B,
        strict: bool,
    ) -> Result<SafetyReport, SqliteGraphError>;

    pub fn format_safety_report(report: &SafetyReport) -> String;
}
```

**safety/cli_integration.rs (~60 LOC)**
```rust
// PLANNED - Not yet implemented
pub mod cli_integration {
    pub fn run_safety_check_command<B: GraphBackend>(
        backend: &B,
        args: &[String],
    ) -> Result<String, SqliteGraphError>;

    pub fn run_strict_safety_check<B: GraphBackend>(
        backend: &B,
        args: &[String],
    ) -> Result<(), SqliteGraphError>;
}
```

**safety/mod.rs (reduced to ~15 LOC)**
```rust
// Module organization and re-exports
pub mod integrity;
pub mod metadata;
pub mod reporting;
pub mod cli_integration;

// Re-export main functions for backward compatibility
pub use reporting::generate_safety_report as run_safety_checks;
pub use cli_integration::run_strict_safety_check;
```

### 3. sqlitegraph-cli/src/reasoning.rs (362 LOC) → 5 modules

#### Problem Analysis
CLI reasoning combines multiple command categories:
- Subgraph extraction commands
- Pipeline execution commands
- Safety check commands
- DSL parsing commands
- File I/O operations
- Error handling and reporting

#### Proposed Split

**sqlitegraph-cli/src/reasoning/subgraph.rs (~80 LOC)**
```rust
// PLANNED - Not yet implemented
pub mod subgraph {
    use crate::BackendClient;
    use crate::SqliteGraphError;

    pub fn run_subgraph_command(
        client: &BackendClient,
        args: &[String],
    ) -> Result<String, SqliteGraphError>;

    fn extract_subgraph_with_filters(
        client: &BackendClient,
        root: i64,
        depth: u32,
        edge_filters: Vec<String>,
        node_filters: Vec<String>,
    ) -> Result<crate::subgraph::SubgraphOutput, SqliteGraphError>;
}
```

**sqlitegraph-cli/src/reasoning/pipeline.rs (~90 LOC)**
```rust
// PLANNED - Not yet implemented
pub mod pipeline {
    use crate::BackendClient;
    use crate::SqliteGraphError;

    pub fn run_pipeline_command(
        client: &BackendClient,
        args: &[String],
    ) -> Result<String, SqliteGraphError>;

    fn execute_pipeline_from_dsl(
        client: &BackendClient,
        dsl: &str,
    ) -> Result<crate::pipeline::PipelineOutput, SqliteGraphError>;

    fn explain_pipeline_from_dsl(
        dsl: &str,
    ) -> Result<crate::api_ergonomics::PipelineExplanation, SqliteGraphError>;
}
```

**sqlitegraph-cli/src/reasoning/safety.rs (~70 LOC)**
```rust
// PLANNED - Not yet implemented
pub mod safety {
    use crate::BackendClient;
    use crate::SqliteGraphError;

    pub fn run_safety_check_command(
        client: &BackendClient,
        args: &[String],
    ) -> Result<String, SqliteGraphError>;

    fn generate_safety_report_json(
        client: &BackendClient,
        strict: bool,
        deep: bool,
    ) -> Result<serde_json::Value, SqliteGraphError>;
}
```

**sqlitegraph-cli/src/reasoning/dsl.rs (~60 LOC)**
```rust
// PLANNED - Not yet implemented
pub mod dsl {
    use crate::SqliteGraphError;

    pub fn run_dsl_parse_command(args: &[String]) -> Result<String, SqliteGraphError>;

    fn parse_and_classify_dsl(input: &str) -> Result<crate::dsl::DslResult, SqliteGraphError>;

    fn generate_dsl_explanation(result: &crate::dsl::DslResult) -> Result<serde_json::Value, SqliteGraphError>;
}
```

**sqlitegraph-cli/src/reasoning/utils.rs (~60 LOC)**
```rust
// PLANNED - Not yet implemented
pub mod utils {
    use std::io::{BufRead, Read};
    use crate::SqliteGraphError;

    pub fn parse_required_i64(args: &[String], flag: &str) -> Result<i64, SqliteGraphError>;
    pub fn parse_optional_u32(args: &[String], flag: &str) -> Option<u32>;
    pub fn required_value(args: &[String], flag: &str) -> Result<String, SqliteGraphError>;
    pub fn has_flag(args: &[String], flag: &str) -> bool;

    pub fn peek_non_whitespace<R: BufRead>(reader: &mut R) -> Result<Option<u8>, SqliteGraphError>;
}
```

**sqlitegraph-cli/src/reasoning/mod.rs (reduced to ~20 LOC)**
```rust
// Module organization and main command dispatcher
pub mod subgraph;
pub mod pipeline;
pub mod safety;
pub mod dsl;
pub mod utils;

use crate::BackendClient;
use crate::SqliteGraphError;

pub fn handle_command(
    client: &BackendClient,
    command: &str,
    args: &[String],
) -> Result<Option<String>, SqliteGraphError> {
    match command {
        "subgraph" => subgraph::run_subgraph_command(client, args).map(Some),
        "pipeline" => pipeline::run_pipeline_command(client, args).map(Some),
        "safety-check" => safety::run_safety_check_command(client, args).map(Some),
        "dsl-parse" => dsl::run_dsl_parse_command(args).map(Some),
        _ => Ok(None),
    }
}
```

## Final Target Layout for Backend-Related Code

### Backend Core Organization

#### Current Structure (to be preserved)
```
sqlitegraph/src/
├── backend.rs (294 LOC) ✓ <300
├── backend_selector.rs (39 LOC) ✓ <300
└── backend_client/ (206 LOC total)
    ├── mod.rs (7 LOC) ✓ <300
    ├── client.rs (274 LOC) ✗ >300
    ├── cli.rs (85 LOC) ✓ <300
    └── types.rs (9 LOC) ✓ <300
```

#### Refactored Backend Client
**Issue:** `client.rs` (274 LOC) - needs splitting

**Proposed Split for backend_client/client.rs:**
- **backend_client/core.rs (~150 LOC)** - Core client functionality
- **backend_client/operations.rs (~120 LOC)** - High-level operations
- **backend_client/client.rs (reduced to ~20 LOC)** - Module organization

#### New Graph Optimization Module
```
sqlitegraph/src/graph_opt/ (restructured)
├── mod.rs (~20 LOC) - Module organization
├── bulk_insert.rs (~120 LOC) - Bulk insertion utilities
├── cache_utils.rs (~100 LOC) - Cache management
└── optimization.rs (~80 LOC) - Optimization functions
```

#### New Safety Module
```
sqlitegraph/src/safety/ (restructured)
├── mod.rs (~15 LOC) - Module organization
├── integrity.rs (~80 LOC) - Referential integrity validation
├── metadata.rs (~70 LOC) - Metadata validation
├── reporting.rs (~90 LOC) - Safety report generation
└── cli_integration.rs (~60 LOC) - CLI command integration
```

### CLI Structure

#### Refactored CLI Reasoning
```
sqlitegraph-cli/src/reasoning/ (restructured)
├── mod.rs (~20 LOC) - Command dispatcher
├── subgraph.rs (~80 LOC) - Subgraph commands
├── pipeline.rs (~90 LOC) - Pipeline commands
├── safety.rs (~70 LOC) - Safety check commands
├── dsl.rs (~60 LOC) - DSL parsing commands
└── utils.rs (~60 LOC) - Shared utilities
```

## Modularization Rules

### Code Organization Rules

#### 1. Single Responsibility Principle
- Each module has one clearly defined responsibility
- No mixed concerns within single files
- Clear separation between algorithms, I/O, and data structures

#### 2. File Size Limits
- **Hard limit:** No module >300 LOC (except rare test file exceptions)
- **Target range:** 50-250 LOC for optimal maintainability
- **Module granularity:** Break down large files until compliant

#### 3. Module Boundary Rules
- **No circular dependencies:** Modules must form DAG (Directed Acyclic Graph)
- **Clear interface boundaries:** Public APIs minimal and well-defined
- **Dependency direction:** Higher-level modules depend on lower-level modules

#### 4. Re-export Strategy
- **Backward compatibility:** Maintain existing public API through re-exports
- **Module isolation:** Internal refactoring doesn't break external users
- **Gradual migration:** Allow phased adoption of new module structure

### Dependency Management Rules

#### Internal Dependencies
```rust
// Example of clean dependency hierarchy
// Lower level: core data structures
mod types;
mod storage;

// Mid level: algorithms and operations
mod algorithms;
mod queries;

// Higher level: client and API
mod client;
mod api;

// Each level only depends on lower levels
```

#### External Dependencies
- **Minimal dependency usage:** Prefer standard library over external crates
- **Feature-gated dependencies:** Use Cargo features for optional functionality
- **Version compatibility:** Maintain compatible dependency versions

#### Testing Dependencies
- **Unit tests:** Each module has corresponding unit tests
- **Integration tests:** Test module interactions
- **End-to-end tests:** Test complete workflows

## Implementation Contract

### Phase 1: Documentation Only
- ✅ **Complete:** This phase defines the modularization plan
- ❌ **No code changes:** No .rs files modified in this phase
- ❌ **No breaking changes:** All existing APIs preserved

### Phase 2: Implementation (Future)
- Implement module splits exactly as specified
- Maintain all existing public APIs through re-exports
- Ensure all tests pass after refactoring
- Verify no performance regressions

### Validation Criteria
- **All files under 300 LOC** (except approved test file exceptions)
- **No circular dependencies** between modules
- **All existing tests pass** without modification
- **Public API compatibility** maintained through re-exports
- **No performance degradation** in critical operations

## Risk Mitigation

### Backward Compatibility Risks
**Mitigation Strategy:**
- Comprehensive re-export of all existing public functions
- Maintain existing module structure for external consumers
- Gradual migration path with deprecation warnings

### Performance Risks
**Mitigation Strategy:**
- Benchmark critical operations before and after refactoring
- Monitor function call overhead for module boundaries
- Optimize hot paths that cross module boundaries

### Test Coverage Risks
**Mitigation Strategy:**
- Ensure all existing tests continue to pass
- Add module-specific unit tests during refactoring
- Maintain integration test coverage across module boundaries

### Development Workflow Risks
**Mitigation Strategy:**
- Implement one module split at a time
- Run full test suite after each change
- Use feature flags for staged rollouts

## Conclusion

This modularization plan addresses all current LOC-limit violations while maintaining backward compatibility and code quality. The systematic approach ensures that:

1. **All files comply with <300 LOC limit** through careful responsibility separation
2. **Existing APIs remain unchanged** through strategic re-exports
3. **Module dependencies remain acyclic** through careful design
4. **Future maintenance is simplified** through single-responsibility modules

The modularization serves as a contract for future implementation phases, ensuring that SQLiteGraph continues to meet its architectural standards while preparing for enhanced maintainability and the addition of the native file backend.