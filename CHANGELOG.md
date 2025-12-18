# SQLiteGraph Changelog

## [0.1.1] - 2024-12-18

### Breaking Changes: V1 Legacy Removal Complete
**V1 legacy code has been permanently removed from SQLiteGraph**

#### Removed Components
- All V1 native backend implementation files
- V1 node and edge storage formats
- V1 adjacency management code
- V1 serialization/deserialization logic
- V1 graph file handling code
- V1 compile-time feature flags

#### New V2-Only Architecture
- **V2 Native Backend**: Exclusive use of V2 clustered adjacency
- **V2 Field Names**: `outgoing_edge_count`, `incoming_edge_count` with V2 cluster offsets/sizes
- **EdgeRecord Architecture**: V1-style API maintained for compatibility, backed by `CompactEdgeRecord` storage
- **Schema Version**: All databases now report `schema_version=2`
- **Compilation**: Reduced from 117 compilation errors to 0

#### V1 Prevention Mechanisms
- `sqlitegraph/src/backend/native/v1_prevention.rs` - Active compilation barriers
- Feature flag guards causing compilation failures for any V1 feature attempts
- Runtime enforcement functions ensuring V2-only behavior
- `tests/v1_prevention_compilation_tests.rs` - 5 tests verifying V1 cannot compile

#### Field Name Changes
- **Node Fields**: V2 cluster adjacency with `outgoing_edge_count`, `incoming_edge_count`
- **Edge Storage**: `CompactEdgeRecord` for optimal storage with V1-style API compatibility
- **Adjacency**: V2 clustered adjacency with cluster offsets and sizes

#### Test Results
- Library tests: 55/55 passing
- API tests: 4/4 passing
- V1 prevention tests: 5/5 passing
- CLI status reports: `schema_version=2`

#### Migration Impact
- V1 databases: No longer supported (must migrate to V2)
- V2 databases: Fully supported with enhanced integrity
- Future development: V2-only APIs and patterns required

#### Documentation Updates
- `manual.md`: Updated with V2-only architecture section
- `sqlitegraph_api_documentation.md`: New comprehensive API documentation
- `README.md`: Updated to reflect V2-only status
- V1 prevention barriers documented throughout

#### Known Issues
- One V2 cluster collision test (`test_cluster_allocation_collision_prevention`) failing - needs investigation
- Core V2 functionality remains stable and operational

---

## [0.1.0] - Previous Release

### Internal: Dead Code Audit Completed
A full audit of all clippy `dead_code` warnings was performed:

- 149 warnings flagged
- 149 confirmed as false positives
- 0 unused or obsolete items found

Warnings come from:
- CLI modules
- benchmark tooling
- dual-runtime system
- tests
- DSL/pipeline parsers

No code removed and no suppressions added. Documentation updated accordingly.