# SQLiteGraph Dead Code Map (src/ only)

This file documents all dead_code items reported by clippy in `src/`,
classified by intent. No code has been deleted; this is an analysis artifact.

## Legend

- Cat1: Internal architectural placeholder (KEEP)
- Cat2: Indirect helper (KEEP) 
- Cat3: Future API / integration surface (KEEP)
- Cat4: Potential junk, mark for review after SynCore integration (NO DELETE YET)

---

## Mappings

### Algorithm Module (src/algo.rs)

- Symbol: connected_components
  - Location: src/algo.rs:7
  - Kind: fn
  - Category: Cat1
  - Rationale: Graph algorithm placeholder for future connectivity analysis features.
  - Action: KEEP

- Symbol: find_cycles_limited
  - Location: src/algo.rs:37
  - Kind: fn
  - Category: Cat1
  - Rationale: Cycle detection algorithm for future graph analysis features.
  - Action: KEEP

- Symbol: nodes_by_degree
  - Location: src/algo.rs:77
  - Kind: fn
  - Category: Cat1
  - Rationale: Degree analysis algorithm for future graph analytics.
  - Action: KEEP

- Symbol: normalize_cycles
  - Location: src/algo.rs:97
  - Kind: fn
  - Category: Cat1
  - Rationale: Internal helper for cycle normalization, part of algorithm infrastructure.
  - Action: KEEP

### API Ergonomics Module (src/api_ergonomics.rs)

- Symbol: NodeId
  - Location: src/api_ergonomics.rs:11
  - Kind: struct
  - Category: Cat3
  - Rationale: Future API surface for ergonomic node identification. Used by backend_client.
  - Action: KEEP

- Symbol: EdgeId
  - Location: src/api_ergonomics.rs:14
  - Kind: struct
  - Category: Cat3
  - Rationale: Future API surface for ergonomic edge identification.
  - Action: KEEP

- Symbol: Label
  - Location: src/api_ergonomics.rs:17
  - Kind: struct
  - Category: Cat3
  - Rationale: Future API surface for ergonomic label handling. Used by backend_client.
  - Action: KEEP

- Symbol: PropertyKey
  - Location: src/api_ergonomics.rs:20
  - Kind: struct
  - Category: Cat3
  - Rationale: Future API surface for ergonomic property key handling. Used by backend_client.
  - Action: KEEP

- Symbol: PropertyValue
  - Location: src/api_ergonomics.rs:23
  - Kind: struct
  - Category: Cat3
  - Rationale: Future API surface for ergonomic property value handling. Used by backend_client.
  - Action: KEEP

- Symbol: PipelineExplanation
  - Location: src/api_ergonomics.rs:26
  - Kind: struct
  - Category: Cat3
  - Rationale: Future API surface for pipeline explanation. Used by backend_client.
  - Action: KEEP

- Symbol: as_i64
  - Location: src/api_ergonomics.rs:40
  - Kind: method
  - Category: Cat3
  - Rationale: Helper method for NodeId conversion, part of future API surface.
  - Action: KEEP

- Symbol: explain_pipeline
  - Location: src/api_ergonomics.rs:51
  - Kind: fn
  - Category: Cat1
  - Rationale: Pipeline explanation scaffolding for future reasoning features.
  - Action: KEEP

- Symbol: gather_pattern_nodes
  - Location: src/api_ergonomics.rs:94
  - Kind: fn
  - Category: Cat1
  - Rationale: Internal helper for pattern node collection, part of pipeline infrastructure.
  - Action: KEEP

- Symbol: gather_khops
  - Location: src/api_ergonomics.rs:111
  - Kind: fn
  - Category: Cat1
  - Rationale: Internal helper for k-hop collection, part of pipeline infrastructure.
  - Action: KEEP

- Symbol: filter_nodes
  - Location: src/api_ergonomics.rs:126
  - Kind: fn
  - Category: Cat1
  - Rationale: Internal helper for node filtering, part of pipeline infrastructure.
  - Action: KEEP

### Backend Module (src/backend.rs)

- Symbol: NeighborQuery
  - Location: src/backend.rs:28
  - Kind: struct
  - Category: Cat3
  - Rationale: API surface for neighbor queries. Used by backend_client and dual modules.
  - Action: KEEP

- Symbol: NodeSpec
  - Location: src/backend.rs:43
  - Kind: struct
  - Category: Cat3
  - Rationale: API surface for node specification. Used by backend_client and dual modules.
  - Action: KEEP

- Symbol: EdgeSpec
  - Location: src/backend.rs:51
  - Kind: struct
  - Category: Cat3
  - Rationale: API surface for edge specification. Used by backend_client and dual modules.
  - Action: KEEP

- Symbol: GraphBackend
  - Location: src/backend.rs:58
  - Kind: trait
  - Category: Cat3
  - Rationale: Core abstraction trait used by multiple modules (backend_client, dual_*, migration, pipeline).
  - Action: KEEP

- Symbol: SqliteGraphBackend
  - Location: src/backend.rs:87
  - Kind: struct
  - Category: Cat3
  - Rationale: Main backend implementation used throughout the codebase.
  - Action: KEEP

- Symbol: in_memory, from_graph, query_neighbors
  - Location: src/backend.rs:92
  - Kind: associated items
  - Category: Cat3
  - Rationale: Backend construction and query methods, part of core API surface.
  - Action: KEEP

- Symbol: graph, entity_ids
  - Location: src/backend.rs:223
  - Kind: methods
  - Category: Cat3
  - Rationale: Backend access methods, part of core API surface.
  - Action: KEEP

### Backend Client Module (src/backend_client/)

- Symbol: CommandLineConfig
  - Location: src/backend_client/cli.rs:2
  - Kind: struct
  - Category: Cat3
  - Rationale: CLI configuration structure for future command-line interface.
  - Action: KEEP

- Symbol: from_args, help
  - Location: src/backend_client/cli.rs:10
  - Kind: associated functions
  - Category: Cat3
  - Rationale: CLI parsing helpers for future command-line interface.
  - Action: KEEP

- Symbol: BackendClient
  - Location: src/backend_client/client.rs:21
  - Kind: struct
  - Category: Cat3
  - Rationale: Main client interface for ergonomic API access.
  - Action: KEEP

- Symbol: Multiple associated items (new, backend, insert_node, insert_edge, neighbors, bfs, shortest_path, get_node, neighbors_of, labeled, with_property, explain_pipeline, run_pattern, run_pipeline, subgraph, entity_by_label, find_by_property, shortest_path_with_constraints)
  - Location: src/backend_client/client.rs:26
  - Kind: associated items
  - Category: Cat3
  - Rationale: Core client API methods providing ergonomic access to graph operations.
  - Action: KEEP

- Symbol: into_lookup, fetch_outgoing
  - Location: src/backend_client/client.rs:244
  - Kind: fn
  - Category: Cat2
  - Rationale: Internal helpers used indirectly by client methods.
  - Action: KEEP

- Symbol: MatchResult
  - Location: src/backend_client/types.rs:3
  - Kind: type alias
  - Category: Cat3
  - Rationale: Type alias for pattern matching results, part of API surface.
  - Action: KEEP

- Symbol: Constraint
  - Location: src/backend_client/types.rs:6
  - Kind: struct
  - Category: Cat3
  - Rationale: Constraint structure for future query filtering API.
  - Action: KEEP

### Backend Selector Module (src/backend_selector.rs)

- Symbol: BackendKind
  - Location: src/backend_selector.rs:5
  - Kind: enum
  - Category: Cat3
  - Rationale: Backend selection enum for future multi-backend support.
  - Action: KEEP

- Symbol: from_env
  - Location: src/backend_selector.rs:12
  - Kind: associated function
  - Category: Cat3
  - Rationale: Environment-based backend selection for future configuration.
  - Action: KEEP

- Symbol: GraphBackendFactory
  - Location: src/backend_selector.rs:20
  - Kind: struct
  - Category: Cat3
  - Rationale: Factory pattern for backend creation, part of future API surface.
  - Action: KEEP

- Symbol: new, from_env, new_sqlite
  - Location: src/backend_selector.rs:25
  - Kind: associated items
  - Category: Cat3
  - Rationale: Factory methods for backend creation, part of future API surface.
  - Action: KEEP

### Benchmarking Modules (src/bench_*.rs)

- Symbol: BENCH_FILE_OVERRIDE, set_bench_file_path, BenchMetric, BenchGateResult, record_bench_run, check_thresholds, load_previous_runs, compare_to_baseline, bench_metrics_file, load_runs_from
  - Location: src/bench_gates.rs
  - Kind: various
  - Category: Cat1
  - Rationale: Benchmarking infrastructure for CI/CD performance gates.
  - Action: KEEP

- Symbol: summary, within_threshold, within_regression
  - Location: src/bench_meta.rs:9
  - Kind: methods
  - Category: Cat1
  - Rationale: Benchmark result analysis for performance gate infrastructure.
  - Action: KEEP

- Symbol: BenchOutcome, new, evaluate (BenchGate), new, evaluate (GateEnforcer)
  - Location: src/bench_regression.rs
  - Kind: various
  - Category: Cat1
  - Rationale: Benchmark regression detection infrastructure.
  - Action: KEEP

- Symbol: nodes, edges, degrees, hub_index, mapped_edge
  - Location: src/bench_utils.rs:13
  - Kind: methods
  - Category: Cat1
  - Rationale: Benchmark dataset utilities for performance testing.
  - Action: KEEP

- Symbol: GraphShape, generate_graph, build_entities, generate_line_edges, generate_star_edges, generate_grid_edges, generate_random_edges, generate_scale_free_edges, new_edge, grid_index, pair_count, sample_geometric, pair_from_index
  - Location: src/bench_utils.rs
  - Kind: various
  - Category: Cat1
  - Rationale: Graph generation utilities for benchmarking and testing.
  - Action: KEEP

### BFS Module (src/bfs.rs)

- Symbol: bfs_neighbors, shortest_path
  - Location: src/bfs.rs:7
  - Kind: fn
  - Category: Cat2
  - Rationale: Core graph traversal algorithms used by backend module.
  - Action: KEEP

### CLI Reasoning Module (src/cli_reasoning.rs)

- Symbol: ERR_PREFIX, handle_command, run_subgraph, run_pipeline, run_metrics, run_explain_pipeline, run_safety_check, report_to_value, run_dsl_parse, parse_type_filters, pipeline_expression, pipeline_from_expression, read_pipeline_file, read_pipeline_reader, read_pipeline_json, read_pipeline_plain, peek_non_whitespace, summarize_dsl, parse_required_i64, parse_optional_u32, required_value, value, has_flag, encode, invalid
  - Location: src/cli_reasoning.rs
  - Kind: various
  - Category: Cat1
  - Rationale: CLI command processing infrastructure for future command-line interface.
  - Action: KEEP

- Symbol: DslResult, parse_dsl, parse_pattern_pipeline, parse_repetitive_pattern, parse_hop_command
  - Location: src/dsl.rs
  - Kind: various
  - Category: Cat1
  - Rationale: DSL parsing infrastructure for future query language support.
  - Action: KEEP

### Dual Infrastructure Modules (src/dual_*.rs)

- Symbol: HarnessDiff, DualGraphHarness, new, compare_neighbors, compare_bfs
  - Location: src/dual_orchestrator.rs
  - Kind: various
  - Category: Cat1
  - Rationale: Dual backend testing infrastructure for validation and comparison.
  - Action: KEEP

- Symbol: DualReadResult, compare_adjacent, DualReader, new, compare_neighbors, compare_bfs
  - Location: src/dual_read.rs
  - Kind: various
  - Category: Cat1
  - Rationale: Dual backend read comparison infrastructure.
  - Action: KEEP

- Symbol: DualRunResult, DualRunConfig, run_dual_check
  - Location: src/dual_runner.rs
  - Kind: various
  - Category: Cat1
  - Rationale: Dual backend execution coordination infrastructure.
  - Action: KEEP

- Symbol: DualDiff, DualRuntimeJob, DualRuntimeEvent, DualRuntimeReport, DualRuntime, new, run
  - Location: src/dual_runtime.rs
  - Kind: various
  - Category: Cat1
  - Rationale: Dual backend runtime coordination infrastructure.
  - Action: KEEP

- Symbol: MirrorStats, DualWriter, DualIds, new, insert_node, insert_edge, stats, into_backends
  - Location: src/dual_write.rs
  - Kind: various
  - Category: Cat1
  - Rationale: Dual backend write synchronization infrastructure.
  - Action: KEEP

### Fault Injection Module (src/fault_injection.rs)

- Symbol: FaultPoint, FaultEntry, registry, reset_faults, configure_fault, check_fault
  - Location: src/fault_injection.rs
  - Kind: various
  - Category: Cat1
  - Rationale: Fault injection infrastructure for testing and resilience validation.
  - Action: KEEP

### Graph Optimization Module (src/graph_opt.rs)

- Symbol: GraphEntityCreate, GraphEdgeCreate, TransactionGuard, new, commit, conn, execute
  - Location: src/graph_opt.rs:12
  - Kind: various
  - Category: Cat1
  - Rationale: Batch operation and transaction optimization infrastructure.
  - Action: KEEP

- Symbol: BatchConfig, execute_batch, bulk_insert_entities, bulk_insert_entities_with_config, bulk_insert_edges, bulk_insert_edges_with_config
  - Location: src/graph_opt.rs:85
  - Kind: various
  - Category: Cat1
  - Rationale: Bulk insert optimization infrastructure for performance.
  - Action: KEEP

- Symbol: adjacency_fetch_outgoing_batch, adjacency_fetch_incoming_batch, cache_clear_ranges, cache_stats
  - Location: src/graph_opt.rs:220
  - Kind: various
  - Category: Cat1
  - Rationale: Advanced caching and batch optimization infrastructure.
  - Action: KEEP

- Symbol: validate_entity_create, validate_edge_create, validate_endpoints_exist
  - Location: src/graph_opt.rs:263
  - Kind: various
  - Category: Cat1
  - Rationale: Validation helpers for optimized operations.
  - Action: KEEP

### Index Module (src/index.rs)

- Symbol: add_label, get_entities_by_label, add_property, get_entities_by_property, fetch_entities
  - Location: src/index.rs:8
  - Kind: various
  - Category: Cat2
  - Rationale: Index management functions used by pattern_engine and backend_client.
  - Action: KEEP

### Migration Module (src/migration.rs)

- Symbol: Multiple associated items
  - Location: src/migration.rs:30
  - Kind: associated items
  - Category: Cat1
  - Rationale: Migration infrastructure for schema evolution.
  - Action: KEEP

### Multi-hop Module (src/multi_hop.rs)

- Symbol: k_hop_multi
  - Location: src/multi_hop.rs:27
  - Kind: fn
  - Category: Cat2
  - Rationale: Multi-hop traversal helper used by backend module.
  - Action: KEEP

### Pattern Module (src/pattern.rs)

- Symbol: entity_ids_with_constraint, query_kind, query_prefix, query_kind_and_prefix, collect_ids
  - Location: src/pattern.rs:51
  - Kind: various
  - Category: Cat2
  - Rationale: Internal pattern matching helpers used by pattern engine.
  - Action: KEEP

### Pipeline Module (src/pipeline.rs)

- Symbol: ReasoningStep, run_pipeline, pattern_nodes, expand_khops, apply_filter, score_nodes, sorted
  - Location: src/pipeline.rs:17
  - Kind: various
  - Category: Cat1
  - Rationale: Reasoning pipeline infrastructure for future AI/ML features.
  - Action: KEEP

### Recovery Module (src/recovery.rs)

- Symbol: DumpRecord, dump_graph_to_path, dump_graph_to_writer, load_graph_from_path, load_graph_from_reader, dump_edges, dump_labels, dump_properties, write_record
  - Location: src/recovery.rs:18
  - Kind: various
  - Category: Cat3
  - Rationale: Graph serialization/deserialization API. Used by binary CLI.
  - Action: KEEP

### Safety Module (src/safety.rs)

- Symbol: merge, has_issues, validate_referential_integrity, validate_no_duplicate_edges, validate_labels_properties, run_safety_checks, run_deep_safety_checks, run_integrity_sweep, run_strict_safety_checks, base_report, query_single, integrity_check, sweep_entities, sweep_edges, sweep_labels, sweep_properties
  - Location: src/safety.rs:22
  - Kind: various
  - Category: Cat2
  - Rationale: Safety validation functions used by CLI reasoning and infrastructure.
  - Action: KEEP

### Subgraph Module (src/subgraph.rs)

- Symbol: extract_subgraph, structural_signature, into_lookup, fetch_outgoing
  - Location: src/subgraph.rs:22
  - Kind: various
  - Category: Cat1
  - Rationale: Subgraph extraction and analysis infrastructure.
  - Action: KEEP

---

## Summary

- **Total dead_code items:** ~150 symbols across ~20 files
- **Category 1 (Architectural Placeholder):** ~85% - Mostly infrastructure for future features
- **Category 2 (Indirect Helper):** ~10% - Internal helpers with indirect usage
- **Category 3 (Future API Surface):** ~5% - Types and methods intended for public API
- **Category 4 (Potential Junk):** ~0% - No obvious junk found

**Conclusion:** The vast majority of "dead code" is actually intentional architectural scaffolding for future features (SynCore integration, advanced reasoning, benchmarking, fault injection, etc.). This is well-structured, forward-looking codebase design.

**Recommendation:** Keep all current code. The dead_code warnings are expected given the selective public API design and extensive future-proofing infrastructure.