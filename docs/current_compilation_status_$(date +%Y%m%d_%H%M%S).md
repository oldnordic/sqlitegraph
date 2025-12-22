warning: unused import: `super::*`
   --> sqlitegraph/src/backend/native/graph_file/memory_mapping.rs:256:9
    |
256 |     use super::*;
    |         ^^^^^^^^
    |
    = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

warning: unused import: `std::fs::OpenOptions`
   --> sqlitegraph/src/backend/native/graph_file/transaction.rs:286:9
    |
286 |     use std::fs::OpenOptions;
    |         ^^^^^^^^^^^^^^^^^^^^

warning: unused imports: `EdgeSpec` and `NodeSpec`
 --> sqlitegraph/src/backend/native/graph_ops/tests.rs:6:22
  |
6 | use crate::backend::{EdgeSpec, NodeSpec};
  |                      ^^^^^^^^  ^^^^^^^^

warning: unused import: `TempDir`
   --> sqlitegraph/src/backend/native/v2/export/snapshot.rs:401:35
    |
401 |     use tempfile::{NamedTempFile, TempDir};
    |                                   ^^^^^^^

warning: unused import: `NamedTempFile`
   --> sqlitegraph/src/backend/native/v2/import/snapshot.rs:456:20
    |
456 |     use tempfile::{NamedTempFile, TempDir};
    |                    ^^^^^^^^^^^^^

warning: unused import: `NamedTempFile`
   --> sqlitegraph/src/backend/native/v2/snapshot/atomic_ops.rs:252:29
    |
252 |     use tempfile::{TempDir, NamedTempFile};
    |                             ^^^^^^^^^^^^^

warning: unused import: `NamedTempFile`
   --> sqlitegraph/src/backend/native/v2/snapshot/lifecycle.rs:415:29
    |
415 |     use tempfile::{TempDir, NamedTempFile};
    |                             ^^^^^^^^^^^^^

warning: unused imports: `V2WALWriter` and `WALManagerMetrics`
  --> sqlitegraph/src/backend/native/v2/wal/bulk_ingest_tests.rs:11:78
   |
11 |     BulkIngestConfig, BulkIngestExt, V2WALConfig, V2WALManager, V2WALRecord, V2WALWriter,
   |                                                                              ^^^^^^^^^^^
12 |     WALManagerMetrics,
   |     ^^^^^^^^^^^^^^^^^

warning: unused import: `NativeBackendError`
  --> sqlitegraph/src/backend/native/v2/wal/bulk_ingest_tests.rs:14:30
   |
14 | use crate::backend::native::{NativeBackendError, NativeResult};
   |                              ^^^^^^^^^^^^^^^^^^

warning: unused import: `std::path::Path`
  --> sqlitegraph/src/backend/native/v2/wal/bulk_ingest_tests.rs:15:5
   |
15 | use std::path::Path;
   |     ^^^^^^^^^^^^^^^

warning: unused import: `std::path::PathBuf`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/strategies.rs:515:9
    |
515 |     use std::path::PathBuf;
    |         ^^^^^^^^^^^^^^^^^^

warning: unused import: `tempfile::tempdir`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/strategies.rs:516:9
    |
516 |     use tempfile::tempdir;
    |         ^^^^^^^^^^^^^^^^^

warning: unused imports: `HashMap` and `HashSet`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/consistency.rs:489:28
    |
489 |     use std::collections::{HashMap, HashSet};
    |                            ^^^^^^^  ^^^^^^^

warning: unused import: `std::collections::HashMap`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/reporting.rs:602:9
    |
602 |     use std::collections::HashMap;
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `std::io::Write`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/rules.rs:342:9
    |
342 |     use std::io::Write;
    |         ^^^^^^^^^^^^^^

warning: unused import: `std::time::SystemTime`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/errors.rs:542:9
    |
542 |     use std::time::SystemTime;
    |         ^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `super::*`
   --> sqlitegraph/src/backend/native/v2/wal/metrics/collection.rs:428:9
    |
428 |     use super::*;
    |         ^^^^^^^^

warning: unnecessary parentheses around block return value
   --> sqlitegraph/src/backend/native/v2/wal/metrics/analysis.rs:586:9
    |
586 |         (write_score * 0.4 + read_score * 0.6)
    |         ^                                    ^
    |
    = note: `#[warn(unused_parens)]` (part of `#[warn(unused)]`) on by default
help: remove these parentheses
    |
586 -         (write_score * 0.4 + read_score * 0.6)
586 +         write_score * 0.4 + read_score * 0.6
    |

warning: unnecessary parentheses around block return value
   --> sqlitegraph/src/backend/native/v2/wal/metrics/analysis.rs:629:9
    |
629 |         (memory_score * 0.3 + buffer_score * 0.3 + cpu_score * 0.2 + disk_score * 0.2)
    |         ^                                                                            ^
    |
help: remove these parentheses
    |
629 -         (memory_score * 0.3 + buffer_score * 0.3 + cpu_score * 0.2 + disk_score * 0.2)
629 +         memory_score * 0.3 + buffer_score * 0.3 + cpu_score * 0.2 + disk_score * 0.2
    |

warning: unnecessary parentheses around block return value
   --> sqlitegraph/src/backend/native/v2/wal/metrics/analysis.rs:711:9
    |
711 |         (buffer_efficiency * 0.4 + cluster_efficiency * 0.3 + operation_efficiency * 0.3)
    |         ^                                                                               ^
    |
help: remove these parentheses
    |
711 -         (buffer_efficiency * 0.4 + cluster_efficiency * 0.3 + operation_efficiency * 0.3)
711 +         buffer_efficiency * 0.4 + cluster_efficiency * 0.3 + operation_efficiency * 0.3
    |

warning: unnecessary parentheses around block return value
   --> sqlitegraph/src/backend/native/v2/wal/metrics/analysis.rs:727:9
    |
727 |         (category_scores.throughput * 0.25
    |         ^
...
731 |             + category_scores.efficiency * 0.10)
    |                                                ^
    |
help: remove these parentheses
    |
727 ~         category_scores.throughput * 0.25
728 |             + category_scores.latency * 0.30
729 |             + category_scores.resources * 0.15
730 |             + category_scores.reliability * 0.20
731 ~             + category_scores.efficiency * 0.10
    |

warning: unused imports: `IssueSeverity` and `RecommendationPriority`
   --> sqlitegraph/src/backend/native/v2/wal/metrics/mod.rs:370:31
    |
370 |         use super::analysis::{IssueSeverity, PerformanceAnalyzer, RecommendationPriority};
    |                               ^^^^^^^^^^^^^                       ^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `crate::backend::native::GraphFile`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:938:9
    |
938 |     use crate::backend::native::GraphFile;
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `std::path::PathBuf`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:940:9
    |
940 |     use std::path::PathBuf;
    |         ^^^^^^^^^^^^^^^^^^

warning: unused import: `std::path::PathBuf`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/scanner.rs:599:9
    |
599 |     use std::path::PathBuf;
    |         ^^^^^^^^^^^^^^^^^^

warning: unused import: `tempfile::tempdir`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/scanner.rs:600:9
    |
600 |     use tempfile::tempdir;
    |         ^^^^^^^^^^^^^^^^^

warning: unused import: `std::io::Write`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/states.rs:318:9
    |
318 |     use std::io::Write;
    |         ^^^^^^^^^^^^^^

warning: unused import: `std::time::SystemTime`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/errors/core.rs:692:9
    |
692 |     use std::time::SystemTime;
    |         ^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `std::time::SystemTime`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/errors/mod.rs:136:9
    |
136 |     use std::time::SystemTime;
    |         ^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `std::time::Duration`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/mod.rs:383:9
    |
383 |     use std::time::Duration;
    |         ^^^^^^^^^^^^^^^^^^^

warning: unused import: `super::*`
   --> sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs:967:9
    |
967 |     use super::*;
    |         ^^^^^^^^

warning: unused import: `crate::backend::native::v2::wal::V2WALConfig`
   --> sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs:968:9
    |
968 |     use crate::backend::native::v2::wal::V2WALConfig;
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `std::path::PathBuf`
   --> sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs:969:9
    |
969 |     use std::path::PathBuf;
    |         ^^^^^^^^^^^^^^^^^^

warning: unused import: `tempfile::tempdir`
   --> sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs:970:9
    |
970 |     use tempfile::tempdir;
    |         ^^^^^^^^^^^^^^^^^

warning: unused import: `crate::backend::native::v2::wal::V2WALConfig`
    --> sqlitegraph/src/backend/native/v2/wal/v2_integration.rs:1002:9
     |
1002 |     use crate::backend::native::v2::wal::V2WALConfig;
     |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `tempfile::tempdir`
    --> sqlitegraph/src/backend/native/v2/wal/v2_integration.rs:1003:9
     |
1003 |     use tempfile::tempdir;
     |         ^^^^^^^^^^^^^^^^^

warning: unused imports: `SearchResult` and `VectorRecord`
  --> sqlitegraph/src/hnsw/index.rs:58:44
   |
58 |         neighborhood::{NeighborhoodSearch, SearchResult},
   |                                            ^^^^^^^^^^^^
59 |         storage::{InMemoryVectorStorage, VectorRecord, VectorStorage, VectorStorageStats},
   |                                          ^^^^^^^^^^^^

warning: unused import: `rand::SeedableRng`
   --> sqlitegraph/src/hnsw/multilayer.rs:692:9
    |
692 |     use rand::SeedableRng;
    |         ^^^^^^^^^^^^^^^^^

warning: unused import: `Read`
   --> sqlitegraph/src/backend/native/graph_file/file_lifecycle.rs:251:19
    |
251 |     use std::io::{Read, Write};
    |                   ^^^^

warning: unused import: `Seek`
  --> sqlitegraph/src/backend/native/graph_file/mod.rs:42:21
   |
42 | use std::io::{Read, Seek, Write};
   |                     ^^^^

warning: unused import: `Write`
  --> sqlitegraph/src/backend/native/graph_file/mod.rs:42:27
   |
42 | use std::io::{Read, Seek, Write};
   |                           ^^^^^

warning: unused import: `Read`
  --> sqlitegraph/src/backend/native/graph_file/mod.rs:42:15
   |
42 | use std::io::{Read, Seek, Write};
   |               ^^^^

warning: unused import: `std::io::Write`
   --> sqlitegraph/src/backend/native/v2/export/snapshot.rs:374:13
    |
374 |         use std::io::Write;
    |             ^^^^^^^^^^^^^^

warning: unused import: `std::io::Write`
   --> sqlitegraph/src/backend/native/v2/import/snapshot.rs:402:17
    |
402 |             use std::io::Write;
    |                 ^^^^^^^^^^^^^^

warning: unused import: `std::io::Write`
   --> sqlitegraph/src/backend/native/v2/import/snapshot.rs:416:17
    |
416 |             use std::io::Write;
    |                 ^^^^^^^^^^^^^^

warning: unused import: `std::io::Read`
  --> sqlitegraph/src/backend/native/v2/wal/performance.rs:10:5
   |
10 | use std::io::Read;
   |     ^^^^^^^^^^^^^

warning: unused import: `std::io::Write`
 --> sqlitegraph/src/backend/native/v2/wal/record.rs:9:5
  |
9 | use std::io::Write;
  |     ^^^^^^^^^^^^^^

warning: unused variable: `avx512_resolved`
   --> sqlitegraph/src/backend/native/cpu_tuning.rs:342:13
    |
342 |         let avx512_resolved = resolve_cpu_profile(CpuProfile::X86Avx512);
    |             ^^^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_avx512_resolved`
    |
    = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

warning: unused variable: `id_manager`
   --> sqlitegraph/src/backend/native/edge_store/id_management.rs:408:17
    |
408 |         let mut id_manager = EdgeIdManager::new(&mut graph_file);
    |                 ^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_id_manager`

warning: variable does not need to be mutable
   --> sqlitegraph/src/backend/native/edge_store/id_management.rs:408:13
    |
408 |         let mut id_manager = EdgeIdManager::new(&mut graph_file);
    |             ----^^^^^^^^^^
    |             |
    |             help: remove this `mut`
    |
    = note: `#[warn(unused_mut)]` (part of `#[warn(unused)]`) on by default

warning: unused variable: `operations`
   --> sqlitegraph/src/backend/native/edge_store/record_operations/tests.rs:117:13
    |
117 |         let operations = EdgeRecordOperations::new(&mut graph_file);
    |             ^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_operations`

warning: variable does not need to be mutable
   --> sqlitegraph/src/backend/native/graph_file/file_ops.rs:231:13
    |
231 |         let mut temp_file = tempfile().unwrap();
    |             ----^^^^^^^^^
    |             |
    |             help: remove this `mut`

warning: variable does not need to be mutable
   --> sqlitegraph/src/backend/native/graph_file/file_ops.rs:243:13
    |
243 |         let mut temp_file = tempfile().unwrap();
    |             ----^^^^^^^^^
    |             |
    |             help: remove this `mut`

warning: unused variable: `header_written`
   --> sqlitegraph/src/backend/native/graph_file/graph_file_coordinator.rs:331:13
    |
331 |         let header_written = false;
    |             ^^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_header_written`

warning: unused variable: `synced`
   --> sqlitegraph/src/backend/native/graph_file/graph_file_coordinator.rs:332:13
    |
332 |         let synced = false;
    |             ^^^^^^ help: if this is intentional, prefix it with an underscore: `_synced`

warning: variable does not need to be mutable
   --> sqlitegraph/src/backend/native/graph_file/graph_file_coordinator.rs:435:13
    |
435 |         let mut coordinator = GraphFileCoordinator::new(&mut header, &mut tx_state);
    |             ----^^^^^^^^^^^
    |             |
    |             help: remove this `mut`

warning: variable does not need to be mutable
   --> sqlitegraph/src/backend/native/graph_file/header.rs:419:13
    |
419 |         let mut stats = HeaderStatistics {
    |             ----^^^^^
    |             |
    |             help: remove this `mut`

warning: variable does not need to be mutable
   --> sqlitegraph/src/backend/native/graph_file/memory_resource_manager/mod.rs:143:13
    |
143 |         let mut manager = MemoryResourceManager::new(
    |             ----^^^^^^^
    |             |
    |             help: remove this `mut`

warning: unused variable: `backend`
   --> sqlitegraph/src/backend/native/graph_backend.rs:242:13
    |
242 |         let backend = NativeGraphBackend::new_temp().unwrap();
    |             ^^^^^^^ help: if this is intentional, prefix it with an underscore: `_backend`

warning: unused variable: `graph_file`
   --> sqlitegraph/src/backend/native/v2/export/snapshot.rs:416:14
    |
416 |         let (graph_file, graph_path) = create_test_graph_file().expect("Failed to create test graph");
    |              ^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_graph_file`

warning: value assigned to `all_files_exist` is never read
   --> sqlitegraph/src/backend/native/v2/import/importer.rs:201:17
    |
201 |         let mut all_files_exist = true;
    |                 ^^^^^^^^^^^^^^^
    |
    = help: maybe it is overwritten before being read?
    = note: `#[warn(unused_assignments)]` (part of `#[warn(unused)]`) on by default

warning: value assigned to `all_files_exist` is never read
   --> sqlitegraph/src/backend/native/v2/import/importer.rs:219:13
    |
219 |             all_files_exist = false;
    |             ^^^^^^^^^^^^^^^
    |
    = help: maybe it is overwritten before being read?

warning: unused variable: `result`
   --> sqlitegraph/src/backend/native/v2/import/snapshot.rs:479:13
    |
479 |         let result = match exporter.export_snapshot() {
    |             ^^^^^^ help: if this is intentional, prefix it with an underscore: `_result`

warning: unused variable: `recovery_metrics`
   --> sqlitegraph/src/backend/native/v2/wal/bulk_ingest_tests.rs:179:9
    |
179 |     let recovery_metrics = recovery_manager.get_metrics();
    |         ^^^^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_recovery_metrics`

warning: unused variable: `reopened_metrics`
   --> sqlitegraph/src/backend/native/v2/wal/bulk_ingest_tests.rs:274:9
    |
274 |     let reopened_metrics = reopened_manager.get_metrics();
    |         ^^^^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_reopened_metrics`

warning: unused variable: `timestamp`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/operations.rs:186:26
    |
186 |             if let Some(&timestamp) = dirty_blocks.block_timestamps().get(&block_offset) {
    |                          ^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
186 |             if let Some(&_timestamp) = dirty_blocks.block_timestamps().get(&block_offset) {
    |                          +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
186 -             if let Some(&timestamp) = dirty_blocks.block_timestamps().get(&block_offset) {
186 +             if let Some(&backend::native::v2::wal::lsn::LSN_INVALID) = dirty_blocks.block_timestamps().get(&block_offset) {
    |

warning: variable does not need to be mutable
    --> sqlitegraph/src/backend/native/v2/wal/checkpoint/operations.rs:1177:13
     |
1177 |         let mut node_store = self
     |             ----^^^^^^^^^^
     |             |
     |             help: remove this `mut`

warning: unused variable: `executor`
    --> sqlitegraph/src/backend/native/v2/wal/checkpoint/operations.rs:1415:13
     |
1415 |         let executor = CheckpointExecutor::new(config)?;
     |             ^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_executor`

warning: unused variable: `graph_file`
    --> sqlitegraph/src/backend/native/v2/wal/checkpoint/operations.rs:1475:13
     |
1475 |         let graph_file = GraphFile::create(&v2_graph_path).map_err(|e| {
     |             ^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_graph_file`

warning: unused variable: `timestamp`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/coordinator/executor.rs:173:26
    |
173 |             if let Some(&timestamp) = dirty_blocks.block_timestamps().get(&block_offset) {
    |                          ^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
173 |             if let Some(&_timestamp) = dirty_blocks.block_timestamps().get(&block_offset) {
    |                          +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
173 -             if let Some(&timestamp) = dirty_blocks.block_timestamps().get(&block_offset) {
173 +             if let Some(&backend::native::v2::wal::lsn::LSN_INVALID) = dirty_blocks.block_timestamps().get(&block_offset) {
    |

warning: unused variable: `cluster_key`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/coordinator/executor.rs:181:14
    |
181 |         for (cluster_key, cluster_blocks) in dirty_blocks.cluster_dirty_blocks() {
    |              ^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_cluster_key`

warning: unused variable: `start_lsn`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/coordinator/executor.rs:166:9
    |
166 |         start_lsn: u64,
    |         ^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
166 |         _start_lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
166 -         start_lsn: u64,
166 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `end_lsn`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/coordinator/executor.rs:167:9
    |
167 |         end_lsn: u64,
    |         ^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
167 |         _end_lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
167 -         end_lsn: u64,
167 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:286:9
    |
286 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
286 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
286 -         lsn: u64,
286 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:316:9
    |
316 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
316 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
316 -         lsn: u64,
316 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `slot_offset`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:341:51
    |
341 |     fn apply_node_delete(&mut self, node_id: i64, slot_offset: u64, lsn: u64) -> CheckpointResult<()> {
    |                                                   ^^^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
341 |     fn apply_node_delete(&mut self, node_id: i64, _slot_offset: u64, lsn: u64) -> CheckpointResult<()> {
    |                                                   +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
341 -     fn apply_node_delete(&mut self, node_id: i64, slot_offset: u64, lsn: u64) -> CheckpointResult<()> {
341 +     fn apply_node_delete(&mut self, node_id: i64, backend::native::v2::wal::lsn::LSN_INVALID: u64, lsn: u64) -> CheckpointResult<()> {
    |

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:341:69
    |
341 |     fn apply_node_delete(&mut self, node_id: i64, slot_offset: u64, lsn: u64) -> CheckpointResult<()> {
    |                                                                     ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
341 |     fn apply_node_delete(&mut self, node_id: i64, slot_offset: u64, _lsn: u64) -> CheckpointResult<()> {
    |                                                                     +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
341 -     fn apply_node_delete(&mut self, node_id: i64, slot_offset: u64, lsn: u64) -> CheckpointResult<()> {
341 +     fn apply_node_delete(&mut self, node_id: i64, slot_offset: u64, backend::native::v2::wal::lsn::LSN_INVALID: u64) -> CheckpointResult<()> {
    |

warning: unused variable: `edge_data`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:367:9
    |
367 |         edge_data: &[u8],
    |         ^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_edge_data`

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:369:9
    |
369 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
369 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
369 -         lsn: u64,
369 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `new_data`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:391:9
    |
391 |         new_data: &[u8],
    |         ^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_new_data`

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:393:9
    |
393 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
393 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
393 -         lsn: u64,
393 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:416:9
    |
416 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
416 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
416 -         lsn: u64,
416 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `direction`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:437:9
    |
437 |         direction: crate::backend::native::v2::Direction,
    |         ^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_direction`

warning: unused variable: `edge_data`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:440:9
    |
440 |         edge_data: &[u8],
    |         ^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_edge_data`

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:441:9
    |
441 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
441 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
441 -         lsn: u64,
441 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:465:75
    |
465 |     fn apply_string_insert(&mut self, string_id: u32, string_value: &str, lsn: u64) -> CheckpointResult<()> {
    |                                                                           ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
465 |     fn apply_string_insert(&mut self, string_id: u32, string_value: &str, _lsn: u64) -> CheckpointResult<()> {
    |                                                                           +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
465 -     fn apply_string_insert(&mut self, string_id: u32, string_value: &str, lsn: u64) -> CheckpointResult<()> {
465 +     fn apply_string_insert(&mut self, string_id: u32, string_value: &str, backend::native::v2::wal::lsn::LSN_INVALID: u64) -> CheckpointResult<()> {
    |

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:480:83
    |
480 |     fn apply_free_space_allocate(&mut self, region_offset: u64, region_size: u32, lsn: u64) -> CheckpointResult<()> {
    |                                                                                   ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
480 |     fn apply_free_space_allocate(&mut self, region_offset: u64, region_size: u32, _lsn: u64) -> CheckpointResult<()> {
    |                                                                                   +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
480 -     fn apply_free_space_allocate(&mut self, region_offset: u64, region_size: u32, lsn: u64) -> CheckpointResult<()> {
480 +     fn apply_free_space_allocate(&mut self, region_offset: u64, region_size: u32, backend::native::v2::wal::lsn::LSN_INVALID: u64) -> CheckpointResult<()> {
    |

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:500:9
    |
500 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
500 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
500 -         lsn: u64,
500 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `edge_store`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:531:17
    |
531 |             let edge_store = self.edge_store.lock().map_err(|e| {
    |                 ^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_edge_store`

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:527:9
    |
527 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
527 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
527 -         lsn: u64,
527 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `edge_store`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:557:17
    |
557 |             let edge_store = self.edge_store.lock().map_err(|e| {
    |                 ^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_edge_store`

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:553:9
    |
553 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
553 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
553 -         lsn: u64,
553 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `node_string`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:583:13
    |
583 |         let node_string = format!("node_{}", node_record.node_id());
    |             ^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_node_string`

warning: unused variable: `slot_offset`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:604:36
    |
604 |     fn from_wal_data(node_id: i64, slot_offset: u64, data: &[u8]) -> CheckpointResult<Self> {
    |                                    ^^^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
604 |     fn from_wal_data(node_id: i64, _slot_offset: u64, data: &[u8]) -> CheckpointResult<Self> {
    |                                    +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
604 -     fn from_wal_data(node_id: i64, slot_offset: u64, data: &[u8]) -> CheckpointResult<Self> {
604 +     fn from_wal_data(node_id: i64, backend::native::v2::wal::lsn::LSN_INVALID: u64, data: &[u8]) -> CheckpointResult<Self> {
    |

warning: unused variable: `integrator`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:637:13
    |
637 |         let integrator = V2GraphIntegrator::new(v2_graph_path)?;
    |             ^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_integrator`

warning: unused variable: `dirty_blocks`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/mod.rs:160:9
    |
160 |         dirty_blocks: &DirtyBlockTracker,
    |         ^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_dirty_blocks`

warning: unused variable: `checkpoint_state`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/mod.rs:161:9
    |
161 |         checkpoint_state: &CheckpointState,
    |         ^^^^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_checkpoint_state`

warning: unused variable: `checkpoint_progress`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/mod.rs:162:9
    |
162 |         checkpoint_progress: &CheckpointProgress,
    |         ^^^^^^^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_checkpoint_progress`

warning: unused variable: `max_pending_blocks`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/mod.rs:166:9
    |
166 |         max_pending_blocks: u64,
    |         ^^^^^^^^^^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
166 |         _max_pending_blocks: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
166 -         max_pending_blocks: u64,
166 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `dirty_blocks`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/mod.rs:473:9
    |
473 |         dirty_blocks: &mut DirtyBlockTracker,
    |         ^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_dirty_blocks`

warning: unused variable: `now`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/consistency.rs:204:13
    |
204 |         let now = SystemTime::now()
    |             ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
204 |         let _now = SystemTime::now()
    |             +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
204 -         let now = SystemTime::now()
204 +         let backend::native::v2::wal::lsn::LSN_INVALID = SystemTime::now()
    |

warning: unused variable: `validator`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/consistency.rs:543:13
    |
543 |         let validator = CheckpointConsistencyValidator::new(config);
    |             ^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_validator`

warning: unused variable: `alignment`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/invariants.rs:180:13
    |
180 |         let alignment = v2::V2_CLUSTER_ALIGNMENT;
    |             ^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
180 |         let _alignment = v2::V2_CLUSTER_ALIGNMENT;
    |             +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
180 -         let alignment = v2::V2_CLUSTER_ALIGNMENT;
180 +         let backend::native::v2::wal::lsn::LSN_INVALID = v2::V2_CLUSTER_ALIGNMENT;
    |

warning: unused variable: `dirty_blocks`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/invariants.rs:174:9
    |
174 |         dirty_blocks: &DirtyBlockTracker,
    |         ^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_dirty_blocks`

warning: variable does not need to be mutable
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/invariants.rs:176:13
    |
176 |         let mut violations = Vec::new();
    |             ----^^^^^^^^^^
    |             |
    |             help: remove this `mut`

warning: unused variable: `state`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/invariants.rs:231:9
    |
231 |         state: &CheckpointState,
    |         ^^^^^ help: if this is intentional, prefix it with an underscore: `_state`

warning: variable does not need to be mutable
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/invariants.rs:233:13
    |
233 |         let mut violations = Vec::new();
    |             ----^^^^^^^^^^
    |             |
    |             help: remove this `mut`

warning: variable does not need to be mutable
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/invariants.rs:376:13
    |
376 |         let mut violations = Vec::new();
    |             ----^^^^^^^^^^
    |             |
    |             help: remove this `mut`

warning: value assigned to `v2_version` is never read
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/invariants.rs:406:17
    |
406 |         let mut v2_version = None;
    |                 ^^^^^^^^^^
    |
    = help: maybe it is overwritten before being read?

warning: unused variable: `validator`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/invariants.rs:555:13
    |
555 |         let validator = V2InvariantValidator::new(config);
    |             ^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_validator`

warning: unused variable: `reporter`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/reporting.rs:631:13
    |
631 |         let reporter = CheckpointValidationReporter::new(config);
    |             ^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_reporter`

warning: unused variable: `validator`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/mod.rs:659:13
    |
659 |         let validator = CheckpointValidator::new(config);
    |             ^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_validator`

warning: unused variable: `metrics`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/mod.rs:672:13
    |
672 |         let metrics = CheckpointMetrics::new(config);
    |             ^^^^^^^ help: if this is intentional, prefix it with an underscore: `_metrics`

warning: unused variable: `cleanup`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/mod.rs:685:13
    |
685 |         let cleanup = CheckpointCleanup::new(config);
    |             ^^^^^^^ help: if this is intentional, prefix it with an underscore: `_cleanup`

warning: unused variable: `manager`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/mod.rs:228:13
    |
228 |         let manager = CheckpointFactory::create_manager(config, strategy)?;
    |             ^^^^^^^ help: if this is intentional, prefix it with an underscore: `_manager`

warning: unused variable: `manager`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/mod.rs:249:13
    |
249 |         let manager = CheckpointFactory::create_adaptive_manager(
    |             ^^^^^^^ help: if this is intentional, prefix it with an underscore: `_manager`

warning: unused variable: `transaction`
   --> sqlitegraph/src/backend/native/v2/wal/manager.rs:396:13
    |
396 |         let transaction = transaction.ok_or_else(|| NativeBackendError::InvalidTransaction {
    |             ^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_transaction`

warning: unused variable: `checkpoint_lsn`
   --> sqlitegraph/src/backend/native/v2/wal/manager.rs:456:13
    |
456 |         let checkpoint_lsn = {
    |             ^^^^^^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
456 |         let _checkpoint_lsn = {
    |             +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
456 -         let checkpoint_lsn = {
456 +         let backend::native::v2::wal::lsn::LSN_INVALID = {
    |

warning: unused variable: `cluster_key`
   --> sqlitegraph/src/backend/native/v2/wal/manager.rs:572:18
    |
572 |             for (cluster_key, records) in org.cluster_groups.drain() {
    |                  ^^^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
572 |             for (_cluster_key, records) in org.cluster_groups.drain() {
    |                  +
help: you might have meant to pattern match on the similarly named constant `SCHEMA_VERSION`
    |
572 -             for (cluster_key, records) in org.cluster_groups.drain() {
572 +             for (schema::SCHEMA_VERSION, records) in org.cluster_groups.drain() {
    |

warning: value assigned to `prev_cumulative` is never read
   --> sqlitegraph/src/backend/native/v2/wal/metrics/aggregation.rs:273:17
    |
273 |         let mut prev_cumulative = 0;
    |                 ^^^^^^^^^^^^^^^
    |
    = help: maybe it is overwritten before being read?

warning: unused variable: `error_tracker`
   --> sqlitegraph/src/backend/native/v2/wal/metrics/analysis.rs:753:9
    |
753 |         error_tracker: &ErrorTracker,
    |         ^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_error_tracker`

warning: unused variable: `throughput_tracker`
   --> sqlitegraph/src/backend/native/v2/wal/metrics/analysis.rs:825:9
    |
825 |         throughput_tracker: &ThroughputTracker,
    |         ^^^^^^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_throughput_tracker`

warning: unused variable: `counters`
   --> sqlitegraph/src/backend/native/v2/wal/metrics/mod.rs:286:13
    |
286 |         let counters = metrics.get_counters();
    |             ^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_counters`

warning: unused variable: `global_counters`
   --> sqlitegraph/src/backend/native/v2/wal/metrics/mod.rs:287:13
    |
287 |         let global_counters = metrics.get_global_counters();
    |             ^^^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_global_counters`

warning: unused variable: `resource_tracker`
   --> sqlitegraph/src/backend/native/v2/wal/metrics/mod.rs:326:13
    |
326 |         let resource_tracker = ResourceTracker::new();
    |             ^^^^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_resource_tracker`

warning: unused variable: `cluster_metrics`
   --> sqlitegraph/src/backend/native/v2/wal/metrics/mod.rs:327:13
    |
327 |         let cluster_metrics = ClusterPerformanceMetrics::new();
    |             ^^^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_cluster_metrics`

warning: unused variable: `error_tracker`
   --> sqlitegraph/src/backend/native/v2/wal/metrics/mod.rs:328:13
    |
328 |         let error_tracker = ErrorTracker::new();
    |             ^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_error_tracker`

warning: unused variable: `analyzer`
   --> sqlitegraph/src/backend/native/v2/wal/metrics/mod.rs:352:13
    |
352 |         let analyzer = utils::create_default_analyzer();
    |             ^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_analyzer`

warning: unused variable: `immediate_recs`
   --> sqlitegraph/src/backend/native/v2/wal/metrics/mod.rs:385:13
    |
385 |         let immediate_recs = analysis.get_immediate_recommendations();
    |             ^^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_immediate_recs`

warning: unused variable: `record_type`
   --> sqlitegraph/src/backend/native/v2/wal/reader.rs:252:13
    |
252 |         let record_type = V2WALRecordType::try_from(header_bytes[0])?;
    |             ^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_record_type`

warning: variable does not need to be mutable
   --> sqlitegraph/src/backend/native/v2/wal/reader.rs:532:13
    |
532 |         let mut stats = WALStatistics::default();
    |             ----^^^^^
    |             |
    |             help: remove this `mut`

warning: unused variable: `record_count`
   --> sqlitegraph/src/backend/native/v2/wal/record.rs:438:40
    |
438 |             Self::TransactionPrepare { record_count, .. } => base_size + 8 + 8 + 8,
    |                                        ^^^^^^^^^^^^-
    |                                        |
    |                                        help: try removing the field

warning: unused variable: `context`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/coordinator.rs:203:41
    |
203 |     fn perform_existing_recovery(&self, context: &RecoveryContext) -> NativeResult<super::core::RecoverySuccess> {
    |                                         ^^^^^^^ help: if this is intentional, prefix it with an underscore: `_context`

warning: unused variable: `attempt`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/core.rs:260:32
    |
260 |     fn attempt_recovery(&self, attempt: u32) -> Result<Vec<String>, RecoveryError> {
    |                                ^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
260 |     fn attempt_recovery(&self, _attempt: u32) -> Result<Vec<String>, RecoveryError> {
    |                                +
help: you might have meant to pattern match on the similarly named constant `V2_FORMAT_VERSION`
    |
260 -     fn attempt_recovery(&self, attempt: u32) -> Result<Vec<String>, RecoveryError> {
260 +     fn attempt_recovery(&self, backend::native::v2::V2_FORMAT_VERSION: u32) -> Result<Vec<String>, RecoveryError> {
    |

warning: unused variable: `scanner`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/core.rs:450:13
    |
450 |         let scanner = WALScanner::new();
    |             ^^^^^^^ help: if this is intentional, prefix it with an underscore: `_scanner`

warning: unused variable: `start_time`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:314:13
    |
314 |         let start_time = Instant::now();
    |             ^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_start_time`

warning: unused variable: `tx_index`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:311:9
    |
311 |         tx_index: usize,
    |         ^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
311 |         _tx_index: usize,
    |         +
help: you might have meant to pattern match on the similarly named constant `MAX_AVG_EDGE_SIZE`
    |
311 -         tx_index: usize,
311 +         backend::native::v2::performance_targets::MAX_AVG_EDGE_SIZE: usize,
    |

warning: unused variable: `total_txs`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:312:9
    |
312 |         total_txs: usize,
    |         ^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
312 |         _total_txs: usize,
    |         +
help: you might have meant to pattern match on the similarly named constant `MAX_AVG_EDGE_SIZE`
    |
312 -         total_txs: usize,
312 +         backend::native::v2::performance_targets::MAX_AVG_EDGE_SIZE: usize,
    |

warning: variable does not need to be mutable
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:317:13
    |
317 |         let mut warnings = Vec::new();
    |             ----^^^^^^^^
    |             |
    |             help: remove this `mut`

warning: unused variable: `old_data`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:617:9
    |
617 |         old_data: Option<&Vec<u8>>,
    |         ^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_old_data`

warning: unused variable: `slot_offset`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:666:9
    |
666 |         slot_offset: u64,
    |         ^^^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
666 |         _slot_offset: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
666 -         slot_offset: u64,
666 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `old_data`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:667:9
    |
667 |         old_data: Option<&Vec<u8>>,
    |         ^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_old_data`

warning: unused variable: `node_id`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:765:45
    |
765 |             RollbackOperation::NodeInsert { node_id, node_data } => {
    |                                             ^^^^^^^ help: try ignoring the field: `node_id: _`

warning: unused variable: `node_id`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:774:45
    |
774 |             RollbackOperation::NodeUpdate { node_id, old_data } => {
    |                                             ^^^^^^^ help: try ignoring the field: `node_id: _`

warning: unused variable: `slot_offset`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:785:17
    |
785 |                 slot_offset,
    |                 ^^^^^^^^^^^ help: try ignoring the field: `slot_offset: _`

warning: unused variable: `node_id`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:810:9
    |
810 |         node_id: u64,
    |         ^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
810 |         _node_id: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
810 -         node_id: u64,
810 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `direction`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:811:9
    |
811 |         direction: crate::backend::native::v2::edge_cluster::Direction,
    |         ^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_direction`

warning: unused variable: `cluster_offset`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:812:9
    |
812 |         cluster_offset: u64,
    |         ^^^^^^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
812 |         _cluster_offset: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
812 -         cluster_offset: u64,
812 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `cluster_size`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:813:9
    |
813 |         cluster_size: u64,
    |         ^^^^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
813 |         _cluster_size: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
813 -         cluster_size: u64,
813 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `edge_data`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:814:9
    |
814 |         edge_data: &[u8],
    |         ^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_edge_data`

warning: unused variable: `rollback_data`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:815:9
    |
815 |         rollback_data: &mut Vec<RollbackOperation>,
    |         ^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_rollback_data`

warning: unused variable: `edge_record`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:825:9
    |
825 |         edge_record: &CompactEdgeRecord,
    |         ^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_edge_record`

warning: unused variable: `insertion_point`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:826:9
    |
826 |         insertion_point: u32,
    |         ^^^^^^^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
826 |         _insertion_point: u32,
    |         +
help: you might have meant to pattern match on the similarly named constant `V2_FORMAT_VERSION`
    |
826 -         insertion_point: u32,
826 +         backend::native::v2::V2_FORMAT_VERSION: u32,
    |

warning: unused variable: `rollback_data`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:827:9
    |
827 |         rollback_data: &mut Vec<RollbackOperation>,
    |         ^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_rollback_data`

warning: unused variable: `cluster_key`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:836:9
    |
836 |         cluster_key: (u64, u64),
    |         ^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_cluster_key`

warning: unused variable: `new_edge`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:837:9
    |
837 |         new_edge: &CompactEdgeRecord,
    |         ^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_new_edge`

warning: unused variable: `position`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:838:9
    |
838 |         position: u32,
    |         ^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
838 |         _position: u32,
    |         +
help: you might have meant to pattern match on the similarly named constant `V2_FORMAT_VERSION`
    |
838 -         position: u32,
838 +         backend::native::v2::V2_FORMAT_VERSION: u32,
    |

warning: unused variable: `old_edge`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:839:9
    |
839 |         old_edge: Option<&CompactEdgeRecord>,
    |         ^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_old_edge`

warning: unused variable: `rollback_data`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:840:9
    |
840 |         rollback_data: &mut Vec<RollbackOperation>,
    |         ^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_rollback_data`

warning: unused variable: `cluster_key`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:849:9
    |
849 |         cluster_key: (u64, u64),
    |         ^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_cluster_key`

warning: unused variable: `position`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:850:9
    |
850 |         position: u32,
    |         ^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
850 |         _position: u32,
    |         +
help: you might have meant to pattern match on the similarly named constant `V2_FORMAT_VERSION`
    |
850 -         position: u32,
850 +         backend::native::v2::V2_FORMAT_VERSION: u32,
    |

warning: unused variable: `old_edge`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:851:9
    |
851 |         old_edge: Option<&CompactEdgeRecord>,
    |         ^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_old_edge`

warning: unused variable: `rollback_data`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:852:9
    |
852 |         rollback_data: &mut Vec<RollbackOperation>,
    |         ^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_rollback_data`

warning: unused variable: `string_id`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:861:9
    |
861 |         string_id: u64,
    |         ^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
861 |         _string_id: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
861 -         string_id: u64,
861 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `string_value`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:862:9
    |
862 |         string_value: &str,
    |         ^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_string_value`

warning: unused variable: `rollback_data`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:863:9
    |
863 |         rollback_data: &mut Vec<RollbackOperation>,
    |         ^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_rollback_data`

warning: unused variable: `block_offset`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:872:9
    |
872 |         block_offset: u64,
    |         ^^^^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
872 |         _block_offset: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
872 -         block_offset: u64,
872 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `block_size`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:873:9
    |
873 |         block_size: u64,
    |         ^^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
873 |         _block_size: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
873 -         block_size: u64,
873 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `block_type`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:874:9
    |
874 |         block_type: u8,
    |         ^^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
874 |         _block_type: u8,
    |         +
help: you might have meant to pattern match on the similarly named constant `CHECKSUM_ALGORITHM`
    |
874 -         block_type: u8,
874 +         backend::native::v2::wal::recovery::constants::format::CHECKSUM_ALGORITHM: u8,
    |

warning: unused variable: `rollback_data`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:875:9
    |
875 |         rollback_data: &mut Vec<RollbackOperation>,
    |         ^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_rollback_data`

warning: unused variable: `block_offset`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:884:9
    |
884 |         block_offset: u64,
    |         ^^^^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
884 |         _block_offset: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
884 -         block_offset: u64,
884 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `block_size`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:885:9
    |
885 |         block_size: u64,
    |         ^^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
885 |         _block_size: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
885 -         block_size: u64,
885 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `block_type`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:886:9
    |
886 |         block_type: u8,
    |         ^^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
886 |         _block_type: u8,
    |         +
help: you might have meant to pattern match on the similarly named constant `CHECKSUM_ALGORITHM`
    |
886 -         block_type: u8,
886 +         backend::native::v2::wal::recovery::constants::format::CHECKSUM_ALGORITHM: u8,
    |

warning: unused variable: `rollback_data`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:887:9
    |
887 |         rollback_data: &mut Vec<RollbackOperation>,
    |         ^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_rollback_data`

warning: unused variable: `header_offset`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:896:9
    |
896 |         header_offset: u64,
    |         ^^^^^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
896 |         _header_offset: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
896 -         header_offset: u64,
896 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `new_data`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:897:9
    |
897 |         new_data: &[u8],
    |         ^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_new_data`

warning: unused variable: `old_data`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:898:9
    |
898 |         old_data: Option<&[u8]>,
    |         ^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_old_data`

warning: unused variable: `rollback_data`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:899:9
    |
899 |         rollback_data: &mut Vec<RollbackOperation>,
    |         ^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_rollback_data`

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/scanner.rs:442:9
    |
442 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
442 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
442 -         lsn: u64,
442 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `graph_file_size`
  --> sqlitegraph/src/backend/native/v2/wal/recovery/states.rs:58:9
   |
58 |         graph_file_size: Option<u64>,
   |         ^^^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_graph_file_size`

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/validator.rs:343:9
    |
343 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
343 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
343 -         lsn: u64,
343 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/validator.rs:440:9
    |
440 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
440 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
440 -         lsn: u64,
440 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/validator.rs:504:9
    |
504 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
504 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
504 -         lsn: u64,
504 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `direction`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/validator.rs:555:9
    |
555 |         direction: Direction,
    |         ^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_direction`

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/validator.rs:559:9
    |
559 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
559 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
559 -         lsn: u64,
559 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/validator.rs:651:9
    |
651 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
651 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
651 -         lsn: u64,
651 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/validator.rs:715:9
    |
715 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
715 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
715 -         lsn: u64,
715 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/validator.rs:770:9
    |
770 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
770 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
770 -         lsn: u64,
770 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/validator.rs:815:9
    |
815 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
815 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
815 -         lsn: u64,
815 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/validator.rs:868:9
    |
868 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
868 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
868 -         lsn: u64,
868 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `lsn`
   --> sqlitegraph/src/backend/native/v2/wal/recovery/validator.rs:921:9
    |
921 |         lsn: u64,
    |         ^^^
    |
help: if this is intentional, prefix it with an underscore
    |
921 |         _lsn: u64,
    |         +
help: you might have meant to pattern match on the similarly named constant `LSN_INVALID`
    |
921 -         lsn: u64,
921 +         backend::native::v2::wal::lsn::LSN_INVALID: u64,
    |

warning: unused variable: `has_cluster_create`
    --> sqlitegraph/src/backend/native/v2/wal/recovery/validator.rs:1024:21
     |
1024 |                 let has_cluster_create = transaction.records.iter().any(
     |                     ^^^^^^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_has_cluster_create`

warning: unused variable: `new_label`
   --> sqlitegraph/src/backend/native/v2/wal/v2_integration.rs:673:21
    |
673 |         if let Some(new_label) = updates.label {
    |                     ^^^^^^^^^
    |
help: if this is intentional, prefix it with an underscore
    |
673 |         if let Some(_new_label) = updates.label {
    |                     +
help: you might have meant to pattern match on the similarly named constant `V2_FORMAT_VERSION`
    |
673 -         if let Some(new_label) = updates.label {
673 +         if let Some(backend::native::v2::V2_FORMAT_VERSION) = updates.label {
    |

warning: unused variable: `start_time`
   --> sqlitegraph/src/backend/native/v2/wal/writer.rs:329:13
    |
329 |         let start_time = Instant::now();
    |             ^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_start_time`

warning: unused variable: `start_time`
   --> sqlitegraph/src/backend/native/v2/wal/writer.rs:370:13
    |
370 |         let start_time = Instant::now();
    |             ^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_start_time`

warning: variable does not need to be mutable
   --> sqlitegraph/src/hnsw/neighborhood.rs:555:13
    |
555 |         let mut layer = HnswLayer::new(0, 4); // Empty layer
    |             ----^^^^^
    |             |
    |             help: remove this `mut`

warning: variable does not need to be mutable
   --> sqlitegraph/src/hnsw/neighborhood.rs:592:13
    |
592 |         let mut metrics = SearchMetrics::new();
    |             ----^^^^^^^
    |             |
    |             help: remove this `mut`

warning: function `unlikely` is never used
  --> sqlitegraph/src/backend/native/adjacency/mod.rs:58:15
   |
58 | pub(crate) fn unlikely(cond: bool) -> bool {
   |               ^^^^^^^^
   |
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: fields `cached_node` and `node_hot` are never read
  --> sqlitegraph/src/backend/native/adjacency/core_iterator.rs:34:16
   |
20 | pub struct AdjacencyIterator<'a> {
   |            ----------------- fields in this struct
...
34 |     pub(crate) cached_node: Option<NodeRecord>,
   |                ^^^^^^^^^^^
...
38 |     pub(crate) node_hot: Option<NodeHot>,
   |                ^^^^^^^^

warning: method `reset` is never used
  --> sqlitegraph/src/backend/native/adjacency/instrumentation.rs:95:12
   |
29 | impl AdjacencyMetrics {
   | --------------------- method in this implementation
...
95 |     pub fn reset(&self) {
   |            ^^^^^

warning: field `total_collect_operations` is never read
   --> sqlitegraph/src/backend/native/adjacency/instrumentation.rs:112:9
    |
109 | pub struct MetricsSnapshot {
    |            --------------- field in this struct
...
112 |     pub total_collect_operations: usize,
    |         ^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: `MetricsSnapshot` has derived impls for the traits `Clone` and `Debug`, but these are intentionally ignored during dead code analysis

warning: method `has_warnings` is never used
   --> sqlitegraph/src/backend/native/adjacency/instrumentation.rs:263:12
    |
234 | impl ValidationReport {
    | --------------------- method in this implementation
...
263 |     pub fn has_warnings(&self) -> bool {
    |            ^^^^^^^^^^^^

warning: fields `current_index` and `total_count` are never read
   --> sqlitegraph/src/backend/native/adjacency/instrumentation.rs:271:9
    |
270 |     IndexOutOfBounds {
    |     ---------------- fields in this variant
271 |         current_index: u32,
    |         ^^^^^^^^^^^^^
272 |         total_count: u32,
    |         ^^^^^^^^^^^
    |
    = note: `ValidationError` has a derived impl for the trait `Debug`, but this is intentionally ignored during dead code analysis

warning: fields `current_index` and `cached_len` are never read
   --> sqlitegraph/src/backend/native/adjacency/instrumentation.rs:275:9
    |
274 |     IndexBeyondCache {
    |     ---------------- fields in this variant
275 |         current_index: u32,
    |         ^^^^^^^^^^^^^
276 |         cached_len: usize,
    |         ^^^^^^^^^^
    |
    = note: `ValidationError` has a derived impl for the trait `Debug`, but this is intentionally ignored during dead code analysis

warning: field `total_count` is never read
   --> sqlitegraph/src/backend/native/adjacency/instrumentation.rs:279:9
    |
278 |     EmptyCacheNonZeroCount {
    |     ---------------------- field in this variant
279 |         total_count: u32,
    |         ^^^^^^^^^^^
    |
    = note: `ValidationError` has a derived impl for the trait `Debug`, but this is intentionally ignored during dead code analysis

warning: fields `cached_len` and `total_count` are never read
   --> sqlitegraph/src/backend/native/adjacency/instrumentation.rs:285:30
    |
285 |     InconsistentCacheState { cached_len: usize, total_count: u32 },
    |     ----------------------   ^^^^^^^^^^         ^^^^^^^^^^^
    |     |
    |     fields in this variant
    |
    = note: `ValidationWarning` has a derived impl for the trait `Debug`, but this is intentionally ignored during dead code analysis

warning: method `clear_v2_cluster_metadata_on_rollback` is never used
   --> sqlitegraph/src/backend/native/graph_file/mod.rs:113:8
    |
 99 | impl GraphFile {
    | -------------- method in this implementation
...
113 |     fn clear_v2_cluster_metadata_on_rollback(&mut self) -> NativeResult<()> {
    |        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: associated function `begin_cluster_commit` is never used
   --> sqlitegraph/src/backend/native/graph_file/file_lifecycle.rs:180:8
    |
 17 | impl FileLifecycleManager {
    | ------------------------- associated function in this implementation
...
180 |     fn begin_cluster_commit(
    |        ^^^^^^^^^^^^^^^^^^^^

warning: method `initialize_v2_header` is never used
   --> sqlitegraph/src/backend/native/graph_file/graph_file_core.rs:151:8
    |
 13 | impl GraphFile {
    | -------------- method in this implementation
...
151 |     fn initialize_v2_header(&mut self) {
    |        ^^^^^^^^^^^^^^^^^^^^

warning: method `direct_read_with_sync` is never used
   --> sqlitegraph/src/backend/native/graph_file/memory_resource_manager/operations.rs:189:8
    |
 20 | impl<'a> MemoryResourceManager<'a> {
    | ---------------------------------- method in this implementation
...
189 |     fn direct_read_with_sync(
    |        ^^^^^^^^^^^^^^^^^^^^^

warning: method `validate_node_fields` is never used
   --> sqlitegraph/src/backend/native/node_store.rs:423:8
    |
 18 | impl<'a> NodeStore<'a> {
    | ---------------------- method in this implementation
...
423 |     fn validate_node_fields(&self, node: &NodeRecord) -> NativeResult<()> {
    |        ^^^^^^^^^^^^^^^^^^^^

warning: field `strict_guard` is never read
  --> sqlitegraph/src/backend/native/v2/edge_cluster/cluster_trace.rs:28:5
   |
27 | pub struct TraceGuard {
   |            ---------- field in this struct
28 |     strict_guard: StrictModeGuard,
   |     ^^^^^^^^^^^^

warning: field `wal_config` is never read
  --> sqlitegraph/src/backend/native/v2/import/importer.rs:89:5
   |
81 | pub struct V2Importer {
   |            ---------- field in this struct
...
89 |     wal_config: V2WALConfig,
   |     ^^^^^^^^^^

warning: method `replay_wal_records` is never used
   --> sqlitegraph/src/backend/native/v2/import/importer.rs:300:8
    |
 92 | impl V2Importer {
    | --------------- method in this implementation
...
300 |     fn replay_wal_records(&self, _wal_records: &[V2WALRecord]) -> NativeResult<()> {
    |        ^^^^^^^^^^^^^^^^^^

warning: fields `manifest`, `export_dir`, and `target_path` are never read
  --> sqlitegraph/src/backend/native/v2/import/validation.rs:17:5
   |
15 | pub struct ImportValidator {
   |            --------------- fields in this struct
16 |     /// Export manifest
17 |     manifest: ExportManifest,
   |     ^^^^^^^^
...
20 |     export_dir: PathBuf,
   |     ^^^^^^^^^^
...
23 |     target_path: PathBuf,
   |     ^^^^^^^^^^^

warning: fields `wal_path`, `graph_path`, and `expected_lsn` are never read
  --> sqlitegraph/src/backend/native/v2/import/validation.rs:80:5
   |
78 | pub struct PostImportValidator {
   |            ------------------- fields in this struct
79 |     /// WAL file path
80 |     wal_path: PathBuf,
   |     ^^^^^^^^
...
83 |     graph_path: PathBuf,
   |     ^^^^^^^^^^
...
86 |     expected_lsn: u64,
   |     ^^^^^^^^^^^^

warning: fields `existing_path` and `export_manifest` are never read
   --> sqlitegraph/src/backend/native/v2/import/validation.rs:145:5
    |
143 | pub struct MergeCompatibilityChecker {
    |            ------------------------- fields in this struct
144 |     /// Existing graph path
145 |     existing_path: PathBuf,
    |     ^^^^^^^^^^^^^
...
148 |     export_manifest: ExportManifest,
    |     ^^^^^^^^^^^^^^^

warning: function `create_test_node_record` is never used
   --> sqlitegraph/src/backend/native/v2/wal/bulk_ingest_tests.rs:288:4
    |
288 | fn create_test_node_record(node_id: i64) -> V2WALRecord {
    |    ^^^^^^^^^^^^^^^^^^^^^^^

warning: function `create_test_cluster_record` is never used
   --> sqlitegraph/src/backend/native/v2/wal/bulk_ingest_tests.rs:297:4
    |
297 | fn create_test_cluster_record(node_id: i64) -> V2WALRecord {
    |    ^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: field `config` is never read
  --> sqlitegraph/src/backend/native/v2/wal/checkpoint/core.rs:53:5
   |
51 | pub struct V2WALCheckpointManager {
   |            ---------------------- field in this struct
52 |     /// WAL configuration
53 |     config: V2WALConfig,
   |     ^^^^^^

warning: methods `apply_cluster_update`, `apply_cluster_delete`, and `apply_string_table_delete` are never used
    --> sqlitegraph/src/backend/native/v2/wal/checkpoint/operations.rs:1132:8
     |
 429 | impl V2GraphIntegrator {
     | ---------------------- methods in this implementation
...
1132 |     fn apply_cluster_update(
     |        ^^^^^^^^^^^^^^^^^^^^
...
1152 |     fn apply_cluster_delete(
     |        ^^^^^^^^^^^^^^^^^^^^
...
1226 |     fn apply_string_table_delete(&mut self, string_id: u64, _lsn: u64) -> CheckpointResult<()> {
     |        ^^^^^^^^^^^^^^^^^^^^^^^^^

warning: field `graph_file` is never read
  --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:18:5
   |
17 | pub struct V2GraphIntegrator {
   |            ----------------- field in this struct
18 |     graph_file: Arc<RwLock<GraphFile>>,
   |     ^^^^^^^^^^

warning: methods `apply_edge_insert`, `apply_edge_update`, and `apply_edge_delete` are never used
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:363:8
    |
 25 | impl V2GraphIntegrator {
    | ---------------------- methods in this implementation
...
363 |     fn apply_edge_insert(
    |        ^^^^^^^^^^^^^^^^^
...
387 |     fn apply_edge_update(
    |        ^^^^^^^^^^^^^^^^^
...
411 |     fn apply_edge_delete(
    |        ^^^^^^^^^^^^^^^^^

warning: field `config` is never read
  --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/mod.rs:54:5
   |
53 | pub struct CheckpointValidator {
   |            ------------------- field in this struct
54 |     config: V2WALConfig,
   |     ^^^^^^

warning: field `config` is never read
  --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/consistency.rs:76:5
   |
75 | pub struct CheckpointConsistencyValidator {
   |            ------------------------------ field in this struct
76 |     config: V2WALConfig,
   |     ^^^^^^

warning: field `config` is never read
  --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/invariants.rs:69:5
   |
68 | pub struct V2InvariantValidator {
   |            -------------------- field in this struct
69 |     config: V2WALConfig,
   |     ^^^^^^

warning: field `config` is never read
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/reporting.rs:101:5
    |
100 | pub struct CheckpointValidationReporter {
    |            ---------------------------- field in this struct
101 |     config: V2WALConfig,
    |     ^^^^^^

warning: fields `wal_tx_id`, `isolation_level`, and `start_time` are never read
  --> sqlitegraph/src/backend/native/v2/wal/graph_integration.rs:56:5
   |
54 | struct GraphTransaction {
   |        ---------------- fields in this struct
55 |     /// WAL transaction ID
56 |     wal_tx_id: u64,
   |     ^^^^^^^^^
...
59 |     isolation_level: TransactionIsolation,
   |     ^^^^^^^^^^^^^^^
...
68 |     start_time: std::time::Instant,
   |     ^^^^^^^^^^
   |
   = note: `GraphTransaction` has a derived impl for the trait `Debug`, but this is intentionally ignored during dead code analysis

warning: fields `tx_id`, `start_time`, `start_lsn`, and `isolation_level` are never read
  --> sqlitegraph/src/backend/native/v2/wal/manager.rs:21:5
   |
19 | struct ActiveTransaction {
   |        ----------------- fields in this struct
20 |     /// Transaction identifier
21 |     tx_id: u64,
   |     ^^^^^
...
24 |     start_time: Instant,
   |     ^^^^^^^^^^
...
27 |     start_lsn: u64,
   |     ^^^^^^^^^
...
33 |     isolation_level: TransactionIsolation,
   |     ^^^^^^^^^^^^^^^
   |
   = note: `ActiveTransaction` has derived impls for the traits `Clone` and `Debug`, but these are intentionally ignored during dead code analysis

warning: field `reader` is never read
  --> sqlitegraph/src/backend/native/v2/wal/manager.rs:96:5
   |
88 | pub struct V2WALManager {
   |            ------------ field in this struct
...
96 |     reader: Arc<Mutex<Option<V2WALReader>>>,
   |     ^^^^^^

warning: methods `ensure_reader_initialized` and `get_reader` are never used
   --> sqlitegraph/src/backend/native/v2/wal/manager.rs:222:8
    |
159 | impl V2WALManager {
    | ----------------- methods in this implementation
...
222 |     fn ensure_reader_initialized(&self) -> NativeResult<()> {
    |        ^^^^^^^^^^^^^^^^^^^^^^^^^
...
233 |     fn get_reader(&self) -> NativeResult<parking_lot::MutexGuard<'_, Option<V2WALReader>>> {
    |        ^^^^^^^^^^

warning: field `level` is never read
   --> sqlitegraph/src/backend/native/v2/wal/performance.rs:110:5
    |
108 | pub struct WALRecordCompressor {
    |            ------------------- field in this struct
109 |     algorithm: CompressionAlgorithm,
110 |     level: u8,
    |     ^^^^^

warning: fields `config` and `backup_path` are never read
   --> sqlitegraph/src/backend/native/v2/wal/recovery/core.rs:101:5
    |
100 | pub struct V2WALRecoveryEngine {
    |            ------------------- fields in this struct
101 |     config: V2WALConfig,
    |     ^^^^^^
...
108 |     backup_path: Option<PathBuf>,
    |     ^^^^^^^^^^^

warning: fields `database_path`, `string_table`, and `free_space_manager` are never read
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:102:5
    |
100 | pub struct V2GraphFileReplayer {
    |            ------------------- fields in this struct
101 |     /// Database file path
102 |     database_path: PathBuf,
    |     ^^^^^^^^^^^^^
...
110 |     string_table: Arc<Mutex<StringTable>>,
    |     ^^^^^^^^^^^^
111 |     /// Free space manager for V2 operations
112 |     free_space_manager: Arc<Mutex<FreeSpaceManager>>,
    |     ^^^^^^^^^^^^^^^^^^

warning: fields `offset`, `size`, `edge_count`, `last_modified_lsn`, and `created_lsn` are never read
   --> sqlitegraph/src/backend/native/v2/wal/recovery/validator.rs:105:5
    |
104 | struct ClusterMetadata {
    |        --------------- fields in this struct
105 |     offset: u64,
    |     ^^^^^^
106 |     size: u32,
    |     ^^^^
107 |     edge_count: u32,
    |     ^^^^^^^^^^
108 |     last_modified_lsn: u64,
    |     ^^^^^^^^^^^^^^^^^
109 |     created_lsn: u64,
    |     ^^^^^^^^^^^
    |
    = note: `ClusterMetadata` has derived impls for the traits `Clone` and `Debug`, but these are intentionally ignored during dead code analysis

warning: fields `commit_timeout`, `max_retries`, and `retry_delay` are never read
   --> sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs:128:5
    |
120 | pub struct TwoPhaseCommitCoordinator {
    |            ------------------------- fields in this struct
...
128 |     commit_timeout: Duration,
    |     ^^^^^^^^^^^^^^
...
131 |     max_retries: u32,
    |     ^^^^^^^^^^^
132 |     retry_delay: Duration,
    |     ^^^^^^^^^^^

warning: field `lock_timeout` is never read
   --> sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs:144:5
    |
136 | pub struct V2LockManager {
    |            ------------- field in this struct
...
144 |     lock_timeout: Duration,
    |     ^^^^^^^^^^^^

warning: fields `last_detection` and `detection_interval` are never read
   --> sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs:254:5
    |
249 | pub struct DeadlockDetector {
    |            ---------------- fields in this struct
...
254 |     last_detection: Arc<Mutex<Instant>>,
    |     ^^^^^^^^^^^^^^
...
257 |     detection_interval: Duration,
    |     ^^^^^^^^^^^^^^^^^^

warning: fields `prefetch_queue` and `access_stats` are never read
   --> sqlitegraph/src/backend/native/v2/wal/v2_integration.rs:120:5
    |
115 | pub struct V2NodeCoordinator {
    |            ----------------- fields in this struct
...
120 |     prefetch_queue: Arc<Mutex<VecDeque<NativeNodeId>>>,
    |     ^^^^^^^^^^^^^^
...
123 |     access_stats: Arc<RwLock<HashMap<NativeNodeId, NodeAccessStats>>>,
    |     ^^^^^^^^^^^^

warning: field `assignment_strategy` is never read
   --> sqlitegraph/src/backend/native/v2/wal/v2_integration.rs:167:5
    |
162 | pub struct V2EdgeCoordinator {
    |            ----------------- field in this struct
...
167 |     assignment_strategy: ClusterAssignmentStrategy,
    |     ^^^^^^^^^^^^^^^^^^^

warning: fields `cluster_manager` and `access_patterns` are never read
   --> sqlitegraph/src/backend/native/v2/wal/v2_integration.rs:189:5
    |
187 | pub struct V2ClusterCoordinator {
    |            -------------------- fields in this struct
188 |     /// Cluster manager
189 |     cluster_manager: Arc<Mutex<EdgeCluster>>,
    |     ^^^^^^^^^^^^^^^
...
195 |     access_patterns: Arc<RwLock<HashMap<i64, ClusterAccessPattern>>>,
    |     ^^^^^^^^^^^^^^^

warning: method `serialize_for_wal` is never used
   --> sqlitegraph/src/backend/native/v2/wal/v2_integration.rs:981:8
    |
975 | impl NodeRecordV2 {
    | ----------------- method in this implementation
...
981 |     fn serialize_for_wal(&self) -> NativeResult<Vec<u8>> {
    |        ^^^^^^^^^^^^^^^^^

warning: method `serialize_for_wal` is never used
   --> sqlitegraph/src/backend/native/v2/wal/v2_integration.rs:993:8
    |
987 | impl CompactEdgeRecord {
    | ---------------------- method in this implementation
...
993 |     fn serialize_for_wal(&self) -> NativeResult<Vec<u8>> {
    |        ^^^^^^^^^^^^^^^^^

warning: fields `lsn`, `timestamp`, and `committed` are never read
  --> sqlitegraph/src/backend/native/v2/wal/writer.rs:71:5
   |
66 | struct BufferedRecord {
   |        -------------- fields in this struct
...
71 |     lsn: u64,
   |     ^^^
...
74 |     timestamp: Instant,
   |     ^^^^^^^^^
...
77 |     committed: bool,
   |     ^^^^^^^^^
   |
   = note: `BufferedRecord` has derived impls for the traits `Clone` and `Debug`, but these are intentionally ignored during dead code analysis

warning: fields `last_commit` and `active_transactions` are never read
  --> sqlitegraph/src/backend/native/v2/wal/writer.rs:93:5
   |
82 | struct GroupCommitState {
   |        ---------------- fields in this struct
...
93 |     last_commit: Instant,
   |     ^^^^^^^^^^^
...
96 |     active_transactions: u32,
   |     ^^^^^^^^^^^^^^^^^^^
   |
   = note: `GroupCommitState` has a derived impl for the trait `Debug`, but this is intentionally ignored during dead code analysis

warning: method `underlying_connection` is never used
  --> sqlitegraph/src/graph/adjacency.rs:15:19
   |
 9 | impl SqliteGraph {
   | ---------------- method in this implementation
...
15 |     pub(crate) fn underlying_connection(&self) -> &rusqlite::Connection {
   |                   ^^^^^^^^^^^^^^^^^^^^^

warning: struct `EdgeId` is never constructed
 --> sqlitegraph/src/api_ergonomics.rs:7:12
  |
7 | pub struct EdgeId(pub i64);
  |            ^^^^^^

warning: variant `Phase75V2ClusterMetadataBeforeCommit` is never constructed
  --> sqlitegraph/src/fault_injection.rs:13:5
   |
 9 | pub enum FaultPoint {
   |          ---------- variant in this enum
...
13 |     Phase75V2ClusterMetadataBeforeCommit,
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: `FaultPoint` has derived impls for the traits `Debug` and `Clone`, but these are intentionally ignored during dead code analysis

warning: function `reset_faults` is never used
  --> sqlitegraph/src/fault_injection.rs:25:8
   |
25 | pub fn reset_faults() {
   |        ^^^^^^^^^^^^

warning: function `configure_fault` is never used
  --> sqlitegraph/src/fault_injection.rs:29:8
   |
29 | pub fn configure_fault(point: FaultPoint, failures: usize) {
   |        ^^^^^^^^^^^^^^^

warning: method `validate_search_parameters` is never used
   --> sqlitegraph/src/hnsw/neighborhood.rs:396:8
    |
207 | impl NeighborhoodSearch {
    | ----------------------- method in this implementation
...
396 |     fn validate_search_parameters(
    |        ^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: comparison is useless due to type limits
   --> sqlitegraph/src/backend/native/edge_store/capacity_coordinator/coordinator.rs:244:17
    |
244 |         assert!(max_supported >= 0);
    |                 ^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(unused_comparisons)]` on by default

warning: unused `Result` that must be used
  --> sqlitegraph/src/backend/native/graph_file/graph_file_core.rs:82:9
   |
82 |         coordinator.begin_transaction(tx_id);
   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: this `Result` may be an `Err` variant, which should be handled
   = note: `#[warn(unused_must_use)]` (part of `#[warn(unused)]`) on by default
help: use `let _ = ...` to ignore the resulting value
   |
82 |         let _ = coordinator.begin_transaction(tx_id);
   |         +++++++

warning: comparison is useless due to type limits
   --> sqlitegraph/src/backend/native/graph_file/node_edge_access.rs:228:16
    |
228 |             && node.outgoing_cluster_offset >= 0
    |                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: comparison is useless due to type limits
   --> sqlitegraph/src/backend/native/graph_file/node_edge_access.rs:229:16
    |
229 |             && node.incoming_cluster_offset >= 0
    |                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: comparison is useless due to type limits
   --> sqlitegraph/src/backend/native/graph_file/node_edge_access.rs:230:16
    |
230 |             && node.outgoing_edge_count >= 0
    |                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: comparison is useless due to type limits
   --> sqlitegraph/src/backend/native/graph_file/node_edge_access.rs:231:16
    |
231 |             && node.incoming_edge_count >= 0
    |                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: comparison is useless due to type limits
   --> sqlitegraph/src/backend/native/graph_validation.rs:262:8
    |
262 |     if header.node_count < 0 || header.edge_count < 0 {
    |        ^^^^^^^^^^^^^^^^^^^^^

warning: comparison is useless due to type limits
   --> sqlitegraph/src/backend/native/graph_validation.rs:262:33
    |
262 |     if header.node_count < 0 || header.edge_count < 0 {
    |                                 ^^^^^^^^^^^^^^^^^^^^^

warning: comparison is useless due to type limits
   --> sqlitegraph/src/backend/native/v2/export/snapshot.rs:268:12
    |
268 |         if header.node_count < 0 || header.edge_count < 0 {
    |            ^^^^^^^^^^^^^^^^^^^^^

warning: comparison is useless due to type limits
   --> sqlitegraph/src/backend/native/v2/export/snapshot.rs:268:37
    |
268 |         if header.node_count < 0 || header.edge_count < 0 {
    |                                     ^^^^^^^^^^^^^^^^^^^^^

warning: comparison is useless due to type limits
   --> sqlitegraph/src/backend/native/v2/planner.rs:176:12
    |
176 |         if header.node_count < 0 || header.edge_count < 0 {
    |            ^^^^^^^^^^^^^^^^^^^^^

warning: comparison is useless due to type limits
   --> sqlitegraph/src/backend/native/v2/planner.rs:176:37
    |
176 |         if header.node_count < 0 || header.edge_count < 0 {
    |                                     ^^^^^^^^^^^^^^^^^^^^^

warning: hiding a lifetime that's elided elsewhere is confusing
   --> sqlitegraph/src/backend/native/v2/wal/bulk_ingest.rs:137:26
    |
137 |     fn begin_bulk_ingest(&self, config: BulkIngestConfig) -> NativeResult<BulkIngestGuard>;
    |                          ^^^^^ the lifetime is elided here                ^^^^^^^^^^^^^^^ the same lifetime is hidden here
    |
    = help: the same lifetime is referred to in inconsistent ways, making the signature confusing
    = note: `#[warn(mismatched_lifetime_syntaxes)]` on by default
help: use `'_` for type paths
    |
137 |     fn begin_bulk_ingest(&self, config: BulkIngestConfig) -> NativeResult<BulkIngestGuard<'_>>;
    |                                                                                          ++++

warning: hiding a lifetime that's elided elsewhere is confusing
   --> sqlitegraph/src/backend/native/v2/wal/bulk_ingest.rs:166:26
    |
166 |     fn begin_bulk_ingest(&self, config: BulkIngestConfig) -> NativeResult<BulkIngestGuard> {
    |                          ^^^^^ the lifetime is elided here                ^^^^^^^^^^^^^^^ the same lifetime is hidden here
    |
    = help: the same lifetime is referred to in inconsistent ways, making the signature confusing
help: use `'_` for type paths
    |
166 |     fn begin_bulk_ingest(&self, config: BulkIngestConfig) -> NativeResult<BulkIngestGuard<'_>> {
    |                                                                                          ++++

warning: hiding a lifetime that's elided elsewhere is confusing
   --> sqlitegraph/src/backend/native/v2/wal/reader.rs:382:17
    |
382 |     pub fn iter(&mut self) -> WALRecordIterator {
    |                 ^^^^^^^^^     ^^^^^^^^^^^^^^^^^ the same lifetime is hidden here
    |                 |
    |                 the lifetime is elided here
    |
    = help: the same lifetime is referred to in inconsistent ways, making the signature confusing
help: use `'_` for type paths
    |
382 |     pub fn iter(&mut self) -> WALRecordIterator<'_> {
    |                                                ++++

warning: hiding a lifetime that's elided elsewhere is confusing
   --> sqlitegraph/src/backend/native/v2/wal/reader.rs:390:23
    |
390 |     pub fn iter_until(&mut self, end_lsn: u64) -> WALRecordIterator {
    |                       ^^^^^^^^^                   ^^^^^^^^^^^^^^^^^ the same lifetime is hidden here
    |                       |
    |                       the lifetime is elided here
    |
    = help: the same lifetime is referred to in inconsistent ways, making the signature confusing
help: use `'_` for type paths
    |
390 |     pub fn iter_until(&mut self, end_lsn: u64) -> WALRecordIterator<'_> {
    |                                                                    ++++

warning: comparison is useless due to type limits
   --> sqlitegraph/src/hnsw/multilayer.rs:804:17
    |
804 |         assert!(level1 >= 0);
    |                 ^^^^^^^^^^^

warning: `sqlitegraph` (lib test) generated 268 warnings (run `cargo fix --lib -p sqlitegraph --tests` to apply 110 suggestions)
    Finished `test` profile [optimized + debuginfo] target(s) in 0.05s
     Running unittests src/lib.rs (target/debug/deps/sqlitegraph-b037d878d661cf76)

running 611 tests
test backend::native::cpu_tuning::tests::test_caching ... ok
test backend::native::cpu_tuning::tests::test_detect_cpu_profile ... ok
test backend::native::cpu_tuning::tests::test_get_optimization_hints ... ok
test backend::native::adjacency::tests::test_adjacency_iterator_empty ... ok
test backend::native::cpu_tuning::tests::test_has_feature ... ok
test backend::native::adjacency::tests::test_adjacency_validation ... ok
test backend::native::cpu_tuning::tests::test_profile_conversions ... ok
test backend::native::cpu_tuning::tests::test_resolve_cpu_profile ... ok
test backend::native::edge_store::capacity_coordinator::coordinator::tests::test_capacity_coordinator_creation ... ok
test backend::native::edge_store::capacity_coordinator::coordinator::tests::test_capacity_statistics ... ok
test backend::native::edge_store::capacity_coordinator::coordinator::tests::test_edge_offset_calculation ... ok
test backend::native::edge_store::cluster_utils::tests::test_calculate_edge_data_offset_in_cluster ... ok
test backend::native::edge_store::capacity_coordinator::coordinator::tests::test_growth_amount_calculation ... ok
test backend::native::edge_store::cluster_utils::tests::test_calculate_neighbor_offset_in_cluster ... ok
test backend::native::edge_store::cluster_utils::tests::test_calculate_optimal_cluster_size ... ok
test backend::native::edge_store::cluster_utils::tests::test_validate_cluster_size ... ok
test backend::native::edge_store::id_management::tests::test_adjacency_validation ... ok
test backend::native::edge_store::id_management::tests::test_adjacency_allocation ... ok
test backend::native::edge_store::cluster_utils::tests::test_calculate_optimal_cluster_size_alignment ... ok
test backend::native::edge_store::id_management::tests::test_edge_id_allocation ... ok
test backend::native::edge_store::id_management::tests::test_edge_id_overflow ... ok
test backend::native::edge_store::id_management::tests::test_edge_id_validation ... ok
test backend::native::edge_store::id_management::tests::test_edge_statistics ... ok
test backend::native::edge_store::id_management::tests::test_utilization_metrics ... ok
test backend::native::edge_store::record_operations::tests::tests::test_edge_serialization ... ok
test backend::native::edge_store::record_operations::tests::tests::test_edge_roundtrip ... ok
test backend::native::edge_store::record_operations::tests::tests::test_edge_deletion ... ok
test backend::native::edge_store::record_operations::tests::tests::test_serialization_deserialization_standalone ... ok
test backend::native::edge_store::record_operations::tests::tests::test_edge_update ... ok
test backend::native::edge_store::record_operations::tests::tests::test_read_nonexistent_edge ... ok
test backend::native::edge_store::record_operations::tests::tests::test_edge_offset_calculation ... ok
test backend::native::edge_store::record_operations::tests::tests::test_edge_validation ... ok
test backend::native::edge_store::utils::tests::test_check_for_overlap_cluster_overlap ... ok
test backend::native::edge_store::record_operations::tests::tests::test_validation_standalone ... ok
test backend::native::edge_store::record_operations::tests::tests::test_serialization_with_null_data ... ok
test backend::native::edge_store::utils::tests::test_check_for_overlap_no_overlap ... ok
test backend::native::edge_store::utils::tests::test_check_for_overlap_header_region ... ok
test backend::native::edge_store::utils::tests::test_check_for_overlap_node_region ... ok
test backend::native::graph_file::debug::tests::test_convenience_audit_transaction_begin_disabled ... ok
test backend::native::graph_backend::tests::test_native_backend_creation ... ok
test backend::native::graph_file::debug::tests::test_convenience_debug_edge_cluster_state_disabled ... ok
test backend::native::graph_backend::tests::test_interior_mutability ... ok
test backend::native::graph_file::debug::tests::test_log_transaction_phase ... ok
test backend::native::graph_file::debug::tests::test_log_cluster_offset_fix ... ok
test backend::native::graph_file::encoding::tests::test_decode_header_too_small ... ok
test backend::native::graph_file::encoding::tests::test_get_slice_safe_overflow ... ok
test backend::native::graph_file::encoding::tests::test_header_constants_consistency ... ok
test backend::native::graph_file::encoding::tests::test_get_slice_safe_valid ... ok
test backend::native::graph_file::encoding::tests::test_persistent_header_encode_decode_roundtrip ... ok
test backend::native::graph_file::encoding::tests::test_persistent_header_encode_size ... ok
test backend::native::graph_file::encoding::tests::test_get_slice_safe_out_of_bounds ... ok
test backend::native::graph_file::file_lifecycle::tests::test_header_validation ... ok
test backend::native::graph_file::file_lifecycle::tests::test_file_sync ... ok
test backend::native::graph_file::file_lifecycle::tests::test_create_new_graph_file ... ok
test backend::native::graph_file::file_management::tests::test_invalidate_read_buffer ... ok
test backend::native::graph_file::file_management::tests::test_grow_file ... ok
test backend::native::graph_file::file_management::tests::test_grow_file_zero_bytes ... ok
test backend::native::graph_file::file_management::tests::test_flush_complete ... ok
test backend::native::graph_file::file_management::tests::test_validate_file_size ... ok
test backend::native::graph_file::file_ops::tests::test_ensure_file_len_at_least_failure ... ok
test backend::native::graph_file::file_ops::tests::test_ensure_file_len_at_least_success ... ok
test backend::native::graph_file::file_ops::tests::test_file_size ... ok
test backend::native::graph_file::file_ops::tests::test_io_mode_detection ... ok
test backend::native::graph_file::file_ops::tests::test_read_write_bytes_direct ... ok
test backend::native::graph_file::file_ops::tests::test_validate_file_size_empty ... ok
test backend::native::graph_file::file_ops::tests::test_read_node_slot_for_debug ... ok
test backend::native::graph_file::file_ops::tests::test_validate_file_size_valid ... ok
test backend::native::graph_file::graph_file_coordinator::tests::test_begin_transaction ... ok
test backend::native::graph_file::graph_file_coordinator::tests::test_commit_transaction ... ok
test backend::native::graph_file::graph_file_coordinator::tests::test_coordinator_creation ... ok
test backend::native::graph_file::graph_file_coordinator::tests::test_reset_cluster_offsets ... ok
test backend::native::graph_file::graph_file_coordinator::tests::test_rollback_transaction_no_truncation ... ok
test backend::native::graph_file::graph_file_coordinator::tests::test_rollback_transaction_with_truncation ... ok
test backend::native::graph_file::graph_file_coordinator::tests::test_transaction_statistics ... ok
test backend::native::graph_file::graph_file_coordinator::tests::test_validate_transaction_state ... ok
test backend::native::graph_file::header::tests::test_cluster_utilization ... ok
test backend::native::graph_file::header::tests::test_get_header_statistics ... ok
test backend::native::graph_file::header::tests::test_initialize_v2_header ... ok
test backend::native::graph_file::header::tests::test_validate_header_invariants ... ok
test backend::native::graph_file::header::tests::test_validate_header_invariants_invalid_magic ... ok
test backend::native::graph_file::header::tests::test_validate_header_inversions_invalid_offsets ... ok
test backend::native::graph_file::io_backend::tests::test_backend_description ... ok
test backend::native::graph_file::io_backend::tests::test_io_backend_statistics ... ok
test backend::native::graph_file::io_backend::tests::test_buffered_write ... ok
test backend::native::graph_file::io_backend::tests::test_io_mode_properties ... ok
test backend::native::graph_file::io_backend::tests::test_standard_read_write ... ok
test backend::native::graph_file::io_operations::tests::test_ensure_file_len_at_least ... ok
test backend::native::graph_file::io_operations::tests::test_flush_write_buffer ... ok
test backend::native::graph_file::io_operations::tests::test_read_with_ahead ... ok
test backend::native::graph_file::io_operations::tests::test_read_write_bytes_std ... ok
test backend::native::graph_file::io_operations::tests::test_write_bytes_direct ... ok
test backend::native::graph_file::memory_resource_manager::tests::test_buffer_optimization ... ok
test backend::native::graph_file::memory_resource_manager::tests::test_header_region_protection ... ok
test backend::native::graph_file::memory_resource_manager::tests::test_io_mode_detection ... ok
test backend::native::graph_file::memory_resource_manager::tests::test_memory_manager_creation ... ok
test backend::native::graph_file::memory_resource_manager::tests::test_memory_utils ... ok
test backend::native::graph_file::memory_resource_manager::tests::test_node_slot_detection ... ok
test backend::native::graph_file::memory_resource_manager::tests::test_write_buffer_management ... ok
test backend::native::graph_file::mmap_ops::tests::test_mmap_config ... ok
test backend::native::graph_file::mmap_ops::tests::test_mmap_config_builder ... ok
test backend::native::graph_file::mmap_ops::tests::test_mmap_config_disabled ... ok
test backend::native::graph_file::mmap_ops::tests::test_mmap_manager_availability ... ok
test backend::native::graph_file::mmap_ops::tests::test_mmap_statistics ... ok
test backend::native::graph_file::mmap_ops::tests::test_mmap_statistics_uninitialized ... ok
test backend::native::graph_file::mmap_ops::tests::test_recursion_depth_check_failure ... ok
test backend::native::graph_file::mmap_ops::tests::test_recursion_depth_check_success ... ok
test backend::native::graph_file::node_edge_access::tests::test_get_edge_record_size ... ok
test backend::native::graph_file::node_edge_access::tests::test_is_valid_edge_offset ... ok
test backend::native::graph_file::node_edge_access::tests::test_read_edge_at_offset ... ok
test backend::native::graph_file::node_edge_access::tests::test_read_edge_invalid_offset ... ok
test backend::native::graph_file::node_edge_access::tests::test_read_node_at ... ok
test backend::native::graph_file::node_edge_access::tests::test_validate_edge_record ... ok
test backend::native::graph_file::node_edge_access::tests::test_validate_node_record ... ok
test backend::native::graph_file::transaction::tests::test_clear_v2_cluster_metadata ... ok
test backend::native::graph_file::transaction::tests::test_cluster_commit_operations ... ok
test backend::native::graph_file::transaction::tests::test_rollback_with_no_truncation ... ok
test backend::native::graph_file::transaction::tests::test_transaction_statistics ... ok
test backend::native::graph_file::transaction_auditor::tests::test_audit_report_generation ... ok
test backend::native::graph_file::transaction::tests::test_write_read_commit_marker ... ok
test backend::native::graph_file::transaction_auditor::tests::test_audit_transaction_begin_disabled ... ok
test backend::native::graph_file::transaction_auditor::tests::test_clear_modified_nodes ... ok
test backend::native::graph_file::transaction_auditor::tests::test_clear_v2_cluster_metadata_on_rollback ... ok
test backend::native::graph_file::transaction_auditor::tests::test_node_modification_tracking ... ok
test backend::native::graph_file::transaction_auditor::tests::test_debug_edge_cluster_disabled ... ok
test backend::native::graph_file::transaction_auditor::tests::test_statistics ... ok
test backend::native::graph_file::transaction_auditor::tests::test_transaction_auditor_creation ... ok
test backend::native::graph_file::validation::tests::test_calculate_minimum_expected_size ... ok
test backend::native::graph_file::validation::tests::test_validate_file_size_minimum_header ... ok
test backend::native::graph_file::validation::tests::test_validate_file_size_minimum_with_data ... ok
test backend::native::graph_file::validation::tests::test_verify_commit_marker_clean ... ok
test backend::native::graph_file::validation::tests::test_verify_commit_marker_dirty ... ok
test backend::native::graph_validation::tests::test_error_mapping ... ok
test backend::native::graph_validation::tests::test_check_file_consistency ... ok
test backend::native::graph_validation::tests::test_node_record_to_entity ... ok
test backend::native::graph_validation::tests::test_node_spec_to_record ... ok
test backend::native::node_cache::tests::test_access_order_update ... ok
test backend::native::graph_validation::tests::test_validate_node_id_range ... ok
test backend::native::node_cache::tests::test_cache_basic_operations ... ok
test backend::native::node_cache::tests::test_lru_eviction ... ok
test backend::native::graph_ops::tests::test_native_bfs_simple ... ok
test backend::native::graph_ops::tests::test_native_shortest_path ... ok
test backend::native::optimizations::tests::test_neighbor_pointer_table_basic ... ok
test backend::native::optimizations::tests::test_node_hot_cache_basic ... ok
test backend::native::v2::edge_cluster::cluster_serialization::tests::test_deserialize_empty_cluster ... ok
test backend::native::v2::edge_cluster::cluster_serialization::tests::test_serialize_empty_cluster ... ok
test backend::native::v2::edge_cluster::cluster_serialization::tests::test_round_trip_serialization ... ok
test backend::native::v2::edge_cluster::cluster_serialization::tests::test_serialize_single_edge ... ok
test backend::native::v2::edge_cluster::cluster_serialization::tests::test_verify_empty_cluster ... ok
test backend::native::v2::edge_cluster::cluster_serialization::tests::test_verify_truncated_header ... ok
test backend::native::v2::edge_cluster::cluster_serialization::tests::test_verify_valid_layout ... ok
test backend::native::v2::edge_cluster::cluster_trace::tests::test_direction_equality ... ok
test backend::native::v2::edge_cluster::cluster_trace::tests::test_format_strict_reason_with_context ... ok
test backend::native::v2::edge_cluster::cluster_trace::tests::test_format_strict_reason_without_context ... ok
test backend::native::v2::edge_cluster::cluster_trace::tests::test_strict_mode_guard ... ok
test backend::native::v2::edge_cluster::cluster_trace::tests::test_trace_context_creation ... ok
test backend::native::v2::edge_cluster::cluster_trace::tests::test_trace_guard ... ok
test backend::native::v2::edge_cluster::cluster_trace::tests::test_with_trace_context ... ok
test backend::native::v2::export::snapshot::tests::test_snapshot_exporter_fails_with_missing_file ... ok
test backend::native::v2::export::snapshot::tests::test_snapshot_exporter_creation ... ok
test backend::native::v2::import::snapshot::tests::test_snapshot_importer_fails_with_missing_export_dir ... ok
test backend::native::v2::planner::tests::test_planner_active_transactions ... ok
test backend::native::v2::export::tests::test_export_factory_creation ... ok
test backend::native::v2::import::tests::test_import_factory_creation ... ok
test backend::native::v2::planner::tests::test_planner_deterministic ... ok
test backend::native::v2::planner::tests::test_planner_dirty_wal ... ok
test backend::native::v2::planner::tests::test_planner_decision_rules ... ok
test backend::native::v2::planner::tests::test_planner_unstable_graph ... ok
test backend::native::v2::planner::tests::test_planner_is_snapshot_advisable ... ok
test backend::native::v2::import::snapshot::tests::test_snapshot_importer_creation ... ok
test backend::native::v2::snapshot::atomic_ops::tests::test_atomic_copy_missing_source ... ok
test backend::native::v2::snapshot::atomic_ops::tests::test_atomic_copy_missing_parent_directory ... ok
test backend::native::v2::snapshot::atomic_ops::tests::test_atomic_copy_crash_safety_simulation ... ok
test backend::native::v2::snapshot::lifecycle::tests::test_lifecycle_state_properties ... ok
test backend::native::v2::snapshot::atomic_ops::tests::test_atomic_copy_file_to_new_location ... ok
test backend::native::v2::snapshot::atomic_ops::tests::test_atomic_copy_overwrite_protection ... ok
test backend::native::v2::snapshot::atomic_ops::tests::test_atomic_copy_rejects_directory ... ok
test backend::native::v2::snapshot::lifecycle::tests::test_snapshot_lifecycle_incomplete_export ... ok
test backend::native::v2::snapshot::lifecycle::tests::test_snapshot_lifecycle_obsolete_after_import ... ok
test backend::native::v2::snapshot::lifecycle::tests::test_snapshot_lifecycle_clean_export ... ok
test backend::native::v2::snapshot::lifecycle::tests::test_snapshot_lifecycle_importable ... ok
test backend::native::v2::snapshot::lifecycle::tests::test_snapshot_metadata ... ok
test backend::native::v2::wal::checkpoint::constants::tests::test_checkpoint_magic_constants ... ok
test backend::native::v2::wal::checkpoint::constants::tests::test_header_size_calculation ... ok
test backend::native::v2::wal::checkpoint::constants::tests::test_performance_constants ... ok
test backend::native::v2::wal::checkpoint::constants::tests::test_size_constants ... ok
test backend::native::v2::wal::checkpoint::constants::tests::test_strategy_constants ... ok
test backend::native::v2::wal::checkpoint::constants::tests::test_constants_reasonableness ... ok
test backend::native::v2::wal::checkpoint::constants::tests::test_v2_constants ... ok
test backend::native::v2::wal::checkpoint::constants::tests::test_validation_constants ... ok
test backend::native::v2::wal::checkpoint::core::tests::test_checkpoint_manager_creation ... ok
test backend::native::v2::wal::checkpoint::core::tests::test_dirty_block_tracker_capacity_limits ... ok
test backend::native::v2::wal::checkpoint::core::tests::test_checkpoint_statistics ... ok
test backend::native::v2::wal::checkpoint::core::tests::test_dirty_block_tracker_operations ... ok
test backend::native::v2::wal::checkpoint::core::tests::test_checkpoint_state_transitions ... ok
test backend::native::v2::wal::checkpoint::core::tests::test_mark_block_dirty ... ok
test backend::native::v2::wal::checkpoint::errors::tests::test_checkpoint_error_creation ... ok
test backend::native::v2::wal::checkpoint::core::tests::test_mark_invalid_block ... ok
test backend::native::v2::wal::checkpoint::errors::tests::test_checkpoint_error_from_native ... ok
test backend::native::v2::wal::checkpoint::errors::tests::test_checkpoint_error_with_context ... ok
test backend::native::v2::wal::checkpoint::errors::tests::test_diagnostic_report ... ok
test backend::native::v2::wal::checkpoint::errors::tests::test_checkpoint_error_recovery ... ok
test backend::native::v2::wal::checkpoint::errors::tests::test_error_collection ... ok
test backend::native::v2::wal::checkpoint::errors::tests::test_macro_usage ... ok
test backend::native::v2::wal::bulk_ingest_tests::test_bulk_ingest_rollback ... ok
test backend::native::v2::wal::checkpoint::io::block_flusher::tests::test_block_flusher_creation ... ok
test backend::native::v2::wal::checkpoint::errors::tests::test_error_severity_levels ... ok
test backend::native::v2::wal::checkpoint::io::block_flusher::tests::test_block_flusher_invalid_offset ... ok
test backend::native::v2::wal::checkpoint::io::block_flusher::tests::test_block_flusher_offset_beyond_file ... ok
test backend::native::v2::wal::checkpoint::io::checkpoint_writer::tests::test_checkpoint_header_writing ... ok
test backend::native::v2::wal::checkpoint::io::block_flusher::tests::test_block_flusher_with_real_v2_file ... ok
test backend::native::v2::wal::checkpoint::io::checkpoint_writer::tests::test_progress_writing ... ok
test backend::native::v2::wal::checkpoint::io::checkpoint_writer::tests::test_completion_writing ... ok
test backend::native::v2::wal::checkpoint::operations::tests::test_block_flusher_creation ... ok
test backend::native::v2::wal::checkpoint::io::block_flusher::tests::test_block_flusher_multiple_blocks ... ok
test backend::native::v2::wal::checkpoint::operations::tests::test_block_flusher_invalid_offset ... ok
test backend::native::v2::wal::checkpoint::operations::tests::test_block_flusher_with_real_v2_file ... ok
test backend::native::v2::wal::checkpoint::operations::tests::test_checkpoint_executor_creation ... ok
test backend::native::v2::wal::checkpoint::operations::tests::test_checkpoint_header_writing ... ok
test backend::native::v2::wal::checkpoint::operations::tests::test_v2_graph_integrator_creation ... ok
test backend::native::v2::wal::checkpoint::operations::tests::test_v2_graph_integrator_invalid_node_data ... ok
test backend::native::v2::wal::checkpoint::strategies::tests::test_checkpoint_strategy_default ... ok
test backend::native::v2::wal::checkpoint::strategies::tests::test_checkpoint_trigger_creation ... ok
test backend::native::v2::wal::checkpoint::operations::tests::test_v2_graph_integrator_node_insert ... ok
test backend::native::v2::wal::checkpoint::strategies::tests::test_strategy_metrics_recommendations ... ok
test backend::native::v2::wal::checkpoint::record::integrator::tests::test_v2_graph_integrator_creation ... ok
test backend::native::v2::wal::checkpoint::strategies::tests::test_strategy_metrics_urgent ... ok
test backend::native::v2::wal::checkpoint::strategies::tests::test_strategy_validator_adaptive ... ok
test backend::native::v2::wal::checkpoint::strategies::tests::test_strategy_validator_time_interval ... ok
test backend::native::v2::wal::checkpoint::strategies::tests::test_strategy_validator_size_threshold ... ok
test backend::native::v2::wal::checkpoint::tests::test_checkpoint_error_display ... ok
test backend::native::v2::wal::checkpoint::tests::test_checkpoint_error_from_native ... ok
test backend::native::v2::wal::checkpoint::strategies::tests::test_strategy_validator_transaction_count ... ok
test backend::native::v2::wal::checkpoint::tests::test_checkpoint_utils_calculate_optimal_size ... ok
test backend::native::v2::wal::checkpoint::validation::consistency::tests::test_consistency_severity_ordering ... ok
test backend::native::v2::wal::checkpoint::tests::test_checkpoint_factory_adaptive_manager ... ok
test backend::native::v2::wal::checkpoint::validation::consistency::tests::test_consistency_utils_group_violations ... ok
test backend::native::v2::wal::checkpoint::tests::test_checkpoint_factory_create_manager ... ok
test backend::native::v2::wal::checkpoint::validation::consistency::tests::test_consistency_utils_requires_action ... ok
test backend::native::v2::wal::checkpoint::validation::consistency::tests::test_consistency_validator_creation ... ok
test backend::native::v2::wal::checkpoint::validation::consistency::tests::test_consistency_validator_dirty_blocks ... ok
test backend::native::v2::wal::checkpoint::validation::consistency::tests::test_consistency_validator_invalid_lsn_range ... ok
test backend::native::v2::wal::checkpoint::validation::consistency::tests::test_consistency_utils_score_calculation ... ok
test backend::native::v2::wal::bulk_ingest_tests::test_bulk_ingest_recovery_consistency ... ok
test backend::native::v2::wal::checkpoint::validation::consistency::tests::test_consistency_validator_lsn_discontinuity ... ok
test backend::native::v2::wal::checkpoint::validation::consistency::tests::test_consistency_validator_progress ... ok
test backend::native::v2::wal::checkpoint::validation::consistency::tests::test_consistency_validator_valid_lsn_range ... ok
test backend::native::v2::wal::checkpoint::validation::invariants::tests::test_checkpoint_state_invariants ... ok
test backend::native::v2::wal::checkpoint::validation::invariants::tests::test_cluster_alignment_invariants ... ok
test backend::native::v2::wal::checkpoint::validation::invariants::tests::test_v2_invariant_severity_ordering ... ok
test backend::native::v2::wal::checkpoint::validation::invariants::tests::test_v2_graph_file_invariants ... ok
test backend::native::v2::wal::checkpoint::validation::invariants::tests::test_v2_invariant_utils_compliance_score ... ok
test backend::native::v2::wal::checkpoint::validation::invariants::tests::test_v2_invariant_utils_severity ... ok
test backend::native::v2::wal::checkpoint::validation::invariants::tests::test_v2_invariant_summary ... ok
test backend::native::v2::wal::checkpoint::validation::invariants::tests::test_v2_invariant_validator_creation ... ok
test backend::native::v2::wal::checkpoint::validation::reporting::tests::test_trend_analysis ... ok
test backend::native::v2::wal::checkpoint::validation::reporting::tests::test_detailed_text_report_generation ... ok
test backend::native::v2::wal::checkpoint::validation::reporting::tests::test_performance_report_generation ... ok
test backend::native::v2::wal::checkpoint::validation::reporting::tests::test_validation_report_generation ... ok
test backend::native::v2::wal::checkpoint::validation::reporting::tests::test_validation_report_with_violations ... ok
test backend::native::v2::wal::checkpoint::validation::reporting::tests::test_validation_reporter_creation ... ok
test backend::native::v2::wal::checkpoint::validation::reporting::tests::test_validation_status_determination ... ok
test backend::native::v2::wal::checkpoint::validation::rules::tests::test_file_validation_rules_file_not_exists ... ok
test backend::native::v2::wal::checkpoint::validation::rules::tests::test_file_validation_rules_empty_file ... ok
test backend::native::v2::wal::checkpoint::validation::rules::tests::test_validation_config ... ok
test backend::native::v2::wal::checkpoint::validation::rules::tests::test_file_validation_rules_size_limits ... ok
test backend::native::v2::wal::checkpoint::validation::rules::tests::test_validation_context ... ok
test backend::native::v2::wal::checkpoint::validation::rules::tests::test_validation_rule_engine_creation ... ok
test backend::native::v2::wal::checkpoint::validation::rules::tests::test_validation_context_rule_execution ... ok
test backend::native::v2::wal::checkpoint::validation::rules::tests::test_validation_rule_management ... ok
test backend::native::v2::wal::checkpoint::validation::rules::tests::test_validation_severity_ordering ... ok
test backend::native::v2::wal::checkpoint::validation::tests::test_anomaly_detector_creation ... ok
test backend::native::v2::wal::checkpoint::validation::tests::test_backward_compatibility ... ok
test backend::native::v2::wal::checkpoint::validation::tests::test_checkpoint_cleanup_creation ... ok
test backend::native::v2::wal::checkpoint::validation::tests::test_checkpoint_metrics_creation ... ok
test backend::native::v2::wal::checkpoint::validation::tests::test_checkpoint_metrics_data_default ... ok
test backend::native::v2::wal::checkpoint::validation::tests::test_checkpoint_validator_creation ... ok
test backend::native::v2::wal::checkpoint::validation::tests::test_checkpoint_validator_factory ... ok
test backend::native::v2::wal::checkpoint::validation::tests::test_validation_components_structure ... ok
test backend::native::v2::wal::manager::tests::test_cluster_organizer ... ok
test backend::native::v2::wal::graph_integration::tests::test_graph_wal_integrator_create ... ok
test backend::native::v2::wal::graph_integration::tests::test_transaction_lifecycle ... ok
test backend::native::v2::wal::graph_integration::tests::test_node_insertion ... ok
test backend::native::v2::wal::graph_integration::tests::test_transaction_rollback ... ok
test backend::native::v2::wal::manager::tests::test_transaction_coordinator ... ok
test backend::native::v2::wal::manager::tests::test_isolation_levels ... ok
test backend::native::v2::wal::manager::tests::test_wal_manager_metrics ... ok
test backend::native::v2::wal::metrics::aggregation::tests::test_bucket_index_calculation ... ok
test backend::native::v2::wal::metrics::aggregation::tests::test_comprehensive_latency_stats ... ok
test backend::native::v2::wal::manager::tests::test_enhanced_wal_manager_create ... ok
test backend::native::v2::wal::metrics::aggregation::tests::test_latency_histogram_new ... ok
test backend::native::v2::wal::metrics::aggregation::tests::test_latency_histogram_recording ... ok
test backend::native::v2::wal::metrics::aggregation::tests::test_latency_histogram_percentiles ... ok
test backend::native::v2::wal::metrics::aggregation::tests::test_latency_histogram_reset ... ok
test backend::native::v2::wal::metrics::aggregation::tests::test_throughput_tracker_new ... ok
test backend::native::v2::wal::manager::tests::test_transaction_rollback ... ok
test backend::native::v2::wal::metrics::aggregation::tests::test_throughput_tracker_recording ... ok
test backend::native::v2::wal::metrics::analysis::tests::test_analysis_metadata ... ok
test backend::native::v2::wal::metrics::aggregation::tests::test_throughput_tracker_peak ... ok
test backend::native::v2::wal::metrics::aggregation::tests::test_throughput_tracker_reset ... ok
test backend::native::v2::wal::manager::tests::test_transaction_lifecycle ... ok
test backend::native::v2::wal::metrics::analysis::tests::test_get_immediate_recommendations ... ok
test backend::native::v2::wal::metrics::analysis::tests::test_implementation_difficulty_ordering ... ok
test backend::native::v2::wal::manager::tests::test_wal_manager_shutdown ... ok
test backend::native::v2::wal::metrics::analysis::tests::test_issue_severity_ordering ... ok
test backend::native::v2::wal::metrics::analysis::tests::test_metric_thresholds ... ok
test backend::native::v2::wal::metrics::analysis::tests::test_get_critical_issues ... ok
test backend::native::v2::wal::metrics::analysis::tests::test_performance_analysis_new ... ok
test backend::native::v2::wal::metrics::analysis::tests::test_performance_analysis_summary ... ok
test backend::native::v2::wal::metrics::analysis::tests::test_performance_analysis_with_data ... ok
test backend::native::v2::wal::metrics::analysis::tests::test_performance_analyzer_analyze ... ok
test backend::native::v2::wal::metrics::analysis::tests::test_performance_analyzer_new ... ok
test backend::native::v2::wal::metrics::analysis::tests::test_performance_trend ... ok
test backend::native::v2::wal::metrics::analysis::tests::test_recommendation_priority_ordering ... ok
test backend::native::v2::wal::metrics::collection::tests::test_buffer_utilization_calculation ... ok
test backend::native::v2::wal::metrics::collection::tests::test_cluster_specific_metrics ... ok
test backend::native::v2::wal::metrics::collection::tests::test_edge_operation_metrics ... ok
test backend::native::v2::wal::metrics::collection::tests::test_free_space_operation_metrics ... ok
test backend::native::v2::wal::metrics::collection::tests::test_node_operation_metrics ... ok
test backend::native::v2::wal::metrics::collection::tests::test_record_error ... ok
test backend::native::v2::wal::metrics::collection::tests::test_record_read_operation ... ok
test backend::native::v2::wal::metrics::collection::tests::test_record_write_operation ... ok
test backend::native::v2::wal::metrics::collection::tests::test_running_average_calculation ... ok
test backend::native::v2::wal::metrics::collection::tests::test_string_table_operation_metrics ... ok
test backend::native::v2::wal::metrics::core::tests::test_cluster_operation_counters_default ... ok
test backend::native::v2::wal::metrics::core::tests::test_edge_operation_metrics_default ... ok
test backend::native::v2::wal::metrics::core::tests::test_global_counters_atomic_operations ... ok
test backend::native::v2::wal::metrics::core::tests::test_node_operation_metrics_default ... ok
test backend::native::v2::wal::metrics::core::tests::test_performance_counters_default ... ok
test backend::native::v2::wal::metrics::core::tests::test_v2_wal_metrics_creation ... ok
test backend::native::v2::wal::metrics::core::tests::test_v2_wal_metrics_reset ... ok
test backend::native::v2::wal::metrics::integration_tests::test_backward_compatibility ... ok
test backend::native::v2::wal::metrics::integration_tests::test_full_metrics_workflow ... ok
test backend::native::v2::wal::metrics::integration_tests::test_metrics_configuration ... ok
test backend::native::v2::wal::metrics::integration_tests::test_modular_api_access ... ok
test backend::native::v2::wal::metrics::integration_tests::test_serde_compatibility ... ok
test backend::native::v2::wal::metrics::reporting::tests::test_cluster_compression_ratio ... ok
test backend::native::v2::wal::metrics::integration_tests::test_utility_functions ... ok
test backend::native::v2::wal::metrics::reporting::tests::test_cluster_io_efficiency_calculation ... ok
test backend::native::v2::wal::metrics::reporting::tests::test_cluster_global_metrics ... ok
test backend::native::v2::wal::metrics::reporting::tests::test_cluster_performance_metrics_new ... ok
test backend::native::v2::wal::metrics::reporting::tests::test_cluster_reset ... ok
test backend::native::v2::wal::metrics::reporting::tests::test_cluster_summary ... ok
test backend::native::v2::wal::metrics::reporting::tests::test_cluster_update_access ... ok
test backend::native::v2::wal::metrics::reporting::tests::test_cluster_update_stats ... ok
test backend::native::v2::wal::metrics::reporting::tests::test_error_tracker_multiple ... ok
test backend::native::v2::wal::metrics::reporting::tests::test_error_tracker_new ... ok
test backend::native::v2::wal::metrics::integration_tests::test_analysis_integration ... ok
test backend::native::v2::wal::metrics::reporting::tests::test_error_tracker_reset ... ok
test backend::native::v2::wal::metrics::reporting::tests::test_error_tracker_record ... ok
test backend::native::v2::wal::metrics::reporting::tests::test_error_tracker_summary ... ok
test backend::native::v2::wal::metrics::reporting::tests::test_error_tracker_top_errors ... ok
test backend::native::v2::wal::metrics::reporting::tests::test_resource_tracker_new ... ok
test backend::native::v2::wal::metrics::reporting::tests::test_metrics_report_serialization ... ok
test backend::native::v2::wal::bulk_ingest_tests::test_bulk_ingest_batches_flushes ... ok
test backend::native::v2::wal::metrics::reporting::tests::test_resource_tracker_reset ... ok
test backend::native::v2::wal::metrics::reporting::tests::test_resource_tracker_summary ... ok
test backend::native::v2::wal::metrics::reporting::tests::test_resource_tracker_update ... ok
test backend::native::v2::wal::performance::tests::test_io_batcher ... ok
test backend::native::v2::wal::performance::tests::test_rle_compression ... ok
test backend::native::v2::wal::performance::tests::test_adaptive_performance_tuner ... ok
test backend::native::v2::wal::reader::tests::test_wal_read_filter ... ok
test backend::native::v2::wal::performance::tests::test_cluster_affinity_optimizer ... ok
test backend::native::v2::wal::performance::tests::test_compression_algorithm_validation ... ok
test backend::native::v2::wal::reader::tests::test_wal_statistics ... ok
test backend::native::v2::wal::record::tests::test_record_serialization_roundtrip ... ok
test backend::native::v2::wal::reader::tests::test_wal_reader_create ... ok
test backend::native::v2::wal::record::tests::test_record_type_properties ... ok
test backend::native::v2::wal::record::tests::test_serialized_size_estimation ... ok
test backend::native::v2::wal::record::tests::test_v2_wal_record_cluster_key ... ok
test backend::native::v2::wal::recovery::constants::tests::test_format_constants ... ok
test backend::native::v2::wal::recovery::constants::tests::test_performance_constants ... ok
test backend::native::v2::wal::recovery::constants::tests::test_reasonableness ... ok
test backend::native::v2::wal::recovery::constants::tests::test_recovery_magic_constants ... ok
test backend::native::v2::wal::recovery::constants::tests::test_size_constants ... ok
test backend::native::v2::wal::recovery::constants::tests::test_strategy_constants ... ok
test backend::native::v2::wal::recovery::constants::tests::test_v2_constants ... ok
test backend::native::v2::wal::recovery::constants::tests::test_validation_constants ... ok
test backend::native::v2::wal::recovery::coordinator::tests::test_recovery_decision_clean_shutdown ... ok
test backend::native::v2::wal::recovery::coordinator::tests::test_recovery_coordinator_creation ... ok
test backend::native::v2::wal::recovery::coordinator::tests::test_recovery_decision_dirty_shutdown ... ok
test backend::native::v2::wal::recovery::coordinator::tests::test_recovery_decision_corrupt ... ok
test backend::native::v2::wal::recovery::core::tests::test_recovery_options_default ... ok
test backend::native::v2::wal::recovery::errors::core::tests::test_error_collection ... ok
test backend::native::v2::wal::recovery::errors::core::tests::test_error_severity_levels ... ok
test backend::native::v2::wal::recovery::core::tests::test_recovery_state_transitions ... ok
test backend::native::v2::wal::recovery::errors::core::tests::test_recovery_error_creation ... ok
test backend::native::v2::wal::recovery::errors::core::tests::test_recovery_error_with_context ... ok
test backend::native::v2::wal::recovery::errors::core::tests::test_diagnostic_report ... ok
test backend::native::v2::wal::recovery::errors::replayer::tests::test_replayer_error_context_batch ... ok
test backend::native::v2::wal::recovery::errors::replayer::tests::test_replayer_error_context_operation ... ok
test backend::native::v2::wal::recovery::errors::replayer::tests::test_replayer_error_context_transaction ... ok
test backend::native::v2::wal::recovery::errors::replayer::tests::test_replayer_error_extension ... ok
test backend::native::v2::wal::recovery::errors::replayer::tests::test_replayer_error_factory_dependency ... ok
test backend::native::v2::wal::recovery::errors::replayer::tests::test_replayer_error_factory_initialization ... ok
test backend::native::v2::wal::recovery::errors::replayer::tests::test_replayer_error_factory_operation ... ok
test backend::native::v2::wal::recovery::errors::replayer::tests::test_replayer_error_factory_resource ... ok
test backend::native::v2::wal::recovery::errors::replayer::tests::test_replayer_error_factory_timeout ... ok
test backend::native::v2::wal::recovery::errors::replayer::tests::test_replayer_error_factory_rollback ... ok
test backend::native::v2::wal::recovery::errors::scanner::tests::test_scanner_error_context_wal_parse ... ok
test backend::native::v2::wal::recovery::errors::replayer::tests::test_replayer_error_recovery_levels ... ok
test backend::native::v2::wal::recovery::errors::replayer::tests::test_replayer_error_factory_transaction ... ok
test backend::native::v2::wal::recovery::errors::scanner::tests::test_scanner_error_context_wal_read ... ok
test backend::native::v2::wal::recovery::errors::scanner::tests::test_scanner_error_context_wal_sequence ... ok
test backend::native::v2::wal::recovery::errors::scanner::tests::test_scanner_error_extension ... ok
test backend::native::v2::wal::recovery::errors::scanner::tests::test_scanner_error_factory_buffer_allocation ... ok
test backend::native::v2::wal::recovery::errors::scanner::tests::test_scanner_error_factory_disk_space ... ok
test backend::native::v2::wal::recovery::errors::scanner::tests::test_scanner_error_factory_file_not_found ... ok
test backend::native::v2::wal::recovery::errors::scanner::tests::test_scanner_error_factory_timeout ... ok
test backend::native::v2::wal::recovery::errors::scanner::tests::test_scanner_error_factory_initialization ... ok
test backend::native::v2::wal::recovery::errors::scanner::tests::test_scanner_error_factory_wal_parse ... ok
test backend::native::v2::wal::recovery::errors::scanner::tests::test_scanner_error_factory_wal_read ... ok
test backend::native::v2::wal::recovery::errors::scanner::tests::test_scanner_error_recovery_levels ... ok
test backend::native::v2::wal::recovery::errors::scanner::tests::test_scanner_error_factory_permission ... ok
test backend::native::v2::wal::recovery::errors::tests::test_backward_compatibility_all_types_available ... ok
test backend::native::v2::wal::recovery::errors::tests::test_convenience_methods ... ok
test backend::native::v2::wal::recovery::errors::tests::test_error_collection ... ok
test backend::native::v2::wal::recovery::errors::tests::test_extension_traits ... ok
test backend::native::v2::wal::recovery::errors::tests::test_macro_usage ... ok
test backend::native::v2::wal::recovery::errors::tests::test_reexports_work ... ok
test backend::native::v2::wal::recovery::errors::tests::test_specialized_error_contexts ... ok
test backend::native::v2::wal::recovery::errors::tests::test_specialized_error_factories ... ok
test backend::native::v2::wal::recovery::errors::validation::tests::test_validation_error_context_checksum ... ok
test backend::native::v2::wal::recovery::errors::validation::tests::test_validation_error_context_v2_format ... ok
test backend::native::v2::wal::recovery::errors::validation::tests::test_validation_error_extension ... ok
test backend::native::v2::wal::recovery::errors::validation::tests::test_validation_error_factory_checksum ... ok
test backend::native::v2::wal::recovery::errors::validation::tests::test_validation_error_factory_consistency ... ok
test backend::native::v2::wal::recovery::errors::validation::tests::test_validation_error_factory_schema ... ok
test backend::native::v2::wal::recovery::errors::validation::tests::test_validation_error_factory_structural ... ok
test backend::native::v2::wal::recovery::errors::validation::tests::test_validation_error_recovery_levels ... ok
test backend::native::v2::wal::recovery::replayer::tests::test_replay_config_default ... ok
test backend::native::v2::wal::recovery::replayer::tests::test_replay_statistics ... ok
test backend::native::v2::wal::recovery::replayer::tests::test_rollback_operation_serialization ... ok
test backend::native::v2::wal::recovery::replayer::tests::test_v2_graph_integrity ... ok
test backend::native::v2::wal::recovery::replayer::tests::test_replayer_file_validation ... ok
test backend::native::v2::wal::recovery::scanner::tests::test_scan_statistics_default ... ok
test backend::native::v2::wal::recovery::scanner::tests::test_scanner_config_default ... ok
test backend::native::v2::wal::recovery::scanner::tests::test_transaction_id_extraction ... ok
test backend::native::v2::wal::recovery::scanner::tests::test_wal_scanner_creation ... ok
test backend::native::v2::wal::recovery::states::tests::test_authority_resolution ... ok
test backend::native::v2::wal::recovery::states::tests::test_recovery_state_clean_shutdown ... ok
test backend::native::v2::wal::recovery::states::tests::test_recovery_state_unrecoverable_no_graph ... ok
test backend::native::v2::wal::recovery::states::tests::test_recovery_context_creation ... ok
test backend::native::v2::wal::recovery::tests::test_optimal_batch_size ... ok
test backend::native::v2::wal::recovery::tests::test_recovery_estimation ... ok
test backend::native::v2::wal::recovery::tests::test_recovery_severity ... ok
test backend::native::v2::wal::recovery::tests::test_recovery_statistics ... ok
test backend::native::v2::wal::recovery::tests::test_recovery_factory_validate_prerequisites ... ok
test backend::native::v2::wal::recovery::tests::test_recovery_factory_create_engine ... ok
test backend::native::v2::wal::recovery::validator::tests::test_invalid_node_insert ... ok
test backend::native::v2::wal::recovery::validator::tests::test_recovery_validator_creation ... ok
test backend::native::v2::wal::recovery::validator::tests::test_transaction_validator_creation ... ok
test backend::native::v2::wal::tests::test_lsn_utilities ... ok
test backend::native::v2::wal::recovery::validator::tests::test_transaction_validator_initialization ... ok
test backend::native::v2::wal::recovery::validator::tests::test_valid_node_insert ... ok
test backend::native::v2::wal::tests::test_v2_wal_config_for_graph_file ... ok
test backend::native::v2::wal::tests::test_v2_wal_config_default ... ok
test backend::native::v2::wal::transaction_coordinator::tests::test_savepoint_rollback ... ignored
test backend::native::v2::wal::transaction_coordinator::tests::test_transaction_coordinator_basic ... ignored
test backend::native::v2::wal::tests::test_v2_wal_config_validation ... ok
test backend::native::v2::wal::tests::test_v2_wal_header ... ok
test backend::native::v2::wal::v2_integration::tests::test_batch_buffer ... ok
test backend::native::v2::wal::v2_integration::tests::test_v2_integrator_creation ... ignored
test backend::native::v2::wal::v2_integration::tests::test_change_tracker ... ok
test backend::native::v2::wal::writer::tests::test_flush_and_sync ... ok
test backend::native::v2::wal::writer::tests::test_v2_wal_writer_create ... ok
test backend::native::v2::wal::writer::tests::test_write_records_batch ... ok
test backend::native::v2::wal::writer::tests::test_write_single_record ... ok
test config::tests::test_graph_config_constructors ... ok
test config::tests::test_backend_kind_default ... ok
test backend::native::v2::wal::writer::tests::test_writer_shutdown ... ok
test config::tests::test_graph_config_default ... ok
test config::tests::test_graph_config_with_cpu_profile ... ok
test config::tests::test_native_config_builder ... ok
test config::tests::test_open_graph_native ... ok
test config::tests::test_sqlite_config_builder ... ok
test hnsw::builder::tests::test_builder_all_distance_metrics ... ok
test hnsw::builder::tests::test_builder_basic ... ok
test hnsw::builder::tests::test_builder_defaults_multilayer_disabled ... ok
test hnsw::builder::tests::test_builder_multilayer_level_distribution_base ... ok
test hnsw::builder::tests::test_builder_multilayer_full_configuration ... ok
test hnsw::builder::tests::test_builder_multilayer_vs_single_layer ... ok
test hnsw::builder::tests::test_builder_multilayer_level_distribution_base_none ... ok
test hnsw::builder::tests::test_builder_multilayer_deterministic_seed ... ok
test hnsw::builder::tests::test_builder_validation_dimension_zero - should panic ... ok
test hnsw::builder::tests::test_builder_validation_ef_search_zero - should panic ... ok
test hnsw::builder::tests::test_builder_validation_m_zero - should panic ... ok
test hnsw::builder::tests::test_builder_multilayer_methods ... ok
test hnsw::config::tests::test_default_config ... ok
test hnsw::config::tests::test_config_clone ... ok
test hnsw::builder::tests::test_builder_validation_ef_construction_less_than_m - should panic ... ok
test hnsw::config::tests::test_high_precision_config ... ok
test hnsw::config::tests::test_hnsw_config_function ... ok
test hnsw::builder::tests::test_builder_validation_ml_zero - should panic ... ok
test hnsw::config::tests::test_fast_construction_config ... ok
test hnsw::config::tests::test_multilayer_config_defaults ... ok
test hnsw::config::tests::test_multilayer_config_defaults_derivation ... ok
test hnsw::config::tests::test_multilayer_config_enabled ... ok
test hnsw::config::tests::test_multilayer_config_validation ... ok
test hnsw::config::tests::test_single_layer_vs_multi_layer_config ... ok
test hnsw::distance_functions::tests::test_all_metrics_identical_vectors ... ok
test hnsw::distance_functions::tests::test_cosine_similarity_identical ... ok
test hnsw::distance_functions::tests::test_cosine_similarity_opposite ... ok
test hnsw::distance_functions::tests::test_cosine_similarity_orthogonal ... ok
test hnsw::distance_functions::tests::test_different_lengths_panic - should panic ... ok
test hnsw::distance_functions::tests::test_dot_product_basic ... ok
test hnsw::distance_functions::tests::test_euclidean_distance_identical ... ok
test hnsw::distance_functions::tests::test_empty_vectors_panic - should panic ... ok
test hnsw::distance_functions::tests::test_negative_values ... ok
test hnsw::distance_functions::tests::test_high_dimensional_vectors ... ok
test hnsw::distance_functions::tests::test_zero_magnitude_panic - should panic ... ok
test hnsw::distance_functions::tests::test_euclidean_distance_unit ... ok
test hnsw::distance_functions::tests::test_manhattan_distance_basic ... ok
test hnsw::distance_metric::tests::test_all_metrics_identical_vectors ... ok
test hnsw::distance_metric::tests::test_compute_distance_cosine ... ok
test hnsw::distance_metric::tests::test_compute_distance_dot_product ... ok
test hnsw::distance_metric::tests::test_compute_distance_euclidean ... ok
test hnsw::distance_metric::tests::test_compute_distance_manhattan ... ok
test hnsw::distance_metric::tests::test_distance_metric_default ... ok
test hnsw::distance_metric::tests::test_distance_metric_display ... ok
test hnsw::distance_metric::tests::test_distance_metric_equality ... ok
test hnsw::errors::tests::test_config_error_display ... ok
test hnsw::errors::tests::test_error_conversions ... ok
test hnsw::errors::tests::test_error_debug_format ... ok
test hnsw::errors::tests::test_error_equality ... ok
test hnsw::errors::tests::test_hnsw_error_display ... ok
test hnsw::errors::tests::test_index_error_display ... ok
test hnsw::index::tests::test_dimension_mismatch_error ... ok
test hnsw::index::tests::test_basic_search_functionality ... ok
test hnsw::index::tests::test_empty_search ... ok
test hnsw::index::tests::test_hnsw_index_creation ... ok
test hnsw::index::tests::test_index_statistics ... ok
test hnsw::index::tests::test_vector_insertion ... ok
test hnsw::index::tests::test_vector_retrieval ... ok
test hnsw::layer::tests::test_add_connection_nonexistent_node ... ok
test hnsw::layer::tests::test_add_connection_self_connection ... ok
test hnsw::layer::tests::test_add_connection_success ... ok
test hnsw::layer::tests::test_add_node_out_of_order ... ok
test hnsw::layer::tests::test_add_node_sequential ... ok
test hnsw::layer::tests::test_clear_layer ... ok
test hnsw::layer::tests::test_entry_points_initial ... ok
test hnsw::layer::tests::test_get_connections_nonexistent ... ok
test hnsw::layer::tests::test_higher_layer_properties ... ok
test hnsw::layer::tests::test_layer_creation ... ok
test hnsw::layer::tests::test_layer_level_scaling ... ok
test hnsw::layer::tests::test_get_statistics ... ok
test hnsw::layer::tests::test_connection_pruning ... ok
test hnsw::layer::tests::test_layer_level_scaling_minimum ... ok
test config::tests::test_open_graph_sqlite ... ok
test hnsw::layer::tests::test_memory_usage ... ok
test hnsw::layer::tests::test_update_entry_points ... ok
test hnsw::multilayer::tests::test_layer_mappings_basic_operations ... ok
test hnsw::index::tests::test_sqlite_graph_integration ... ok
test hnsw::multilayer::tests::test_layer_mappings_sequential_assignment ... ok
test hnsw::multilayer::tests::test_layer_mappings_sequential_violation ... ok
test hnsw::multilayer::tests::test_level_distributor_deterministic ... ok
test hnsw::multilayer::tests::test_level_distributor_mathematical_properties ... ok
test hnsw::multilayer::tests::test_multilayer_node_manager_basic_operations ... ok
test hnsw::multilayer::tests::test_multilayer_node_manager_consistency ... ok
test hnsw::multilayer::tests::test_multilayer_node_manager_removal ... ok
test hnsw::neighborhood::tests::test_compute_distance_dimension_mismatch ... ok
test hnsw::multilayer::tests::test_multilayer_node_manager_statistics ... ok
test hnsw::neighborhood::tests::test_compute_distance_success ... ok
test hnsw::neighborhood::tests::test_neighborhood_search_creation ... ok
test hnsw::neighborhood::tests::test_neighborhood_search_default ... ok
test hnsw::neighborhood::tests::test_search_candidate_creation ... ok
test hnsw::neighborhood::tests::test_search_candidate_ordering ... ok
test hnsw::neighborhood::tests::test_search_layer_basic ... ok
test hnsw::neighborhood::tests::test_search_layer_empty_layer ... ok
test hnsw::neighborhood::tests::test_search_layer_k_zero ... ok
test hnsw::neighborhood::tests::test_different_distance_metrics ... ok
test hnsw::neighborhood::tests::test_search_layer_no_entry_points ... ok
test hnsw::neighborhood::tests::test_search_metrics ... ok
test hnsw::storage::tests::test_in_memory_storage_clear ... ok
test hnsw::neighborhood::tests::test_search_result_accessors ... ok
test hnsw::neighborhood::tests::test_search_result_empty ... ok
test hnsw::storage::tests::test_in_memory_storage_statistics ... ok
test hnsw::storage::tests::test_in_memory_batch_storage ... ok
test hnsw::storage::tests::test_in_memory_storage ... ok
test hnsw::storage::tests::test_in_memory_storage_with_id ... ok
test hnsw::storage::tests::test_vector_memory_usage ... ok
test hnsw::storage::tests::test_vector_record_creation ... ok
test hnsw::storage::tests::test_in_memory_vector_deletion ... ok
test hnsw::storage::tests::test_in_memory_vector_listing ... ok
test hnsw::storage::tests::test_vector_batch_creation ... ok
test hnsw::storage::tests::test_vector_record_validation ... ok
test hnsw::tests::test_default_configuration ... ok
test hnsw::storage::tests::test_vector_batch_size_mismatch ... ok
test hnsw::tests::test_distance_metrics ... ok
test hnsw::tests::test_error_handling ... ok
test hnsw::tests::test_fast_construction_configuration ... ok
test hnsw::tests::test_high_precision_configuration ... ok
test hnsw::tests::test_hnsw_config_builder ... ok
test mvcc::tests::test_snapshot_manager ... ok
test mvcc::tests::test_snapshot_state_creation ... ok
test hnsw::tests::test_hnsw_config_function ... ok
test hnsw::tests::test_hnsw_module_imports ... ok
test pattern_engine::tests::test_pattern_triple_builder ... ok
test pattern_engine::tests::test_pattern_triple_validation ... ok
test pattern_engine_cache::tests::tests::test_can_use_fast_path_detection ... ok
test pattern_engine::tests::test_match_triples_basic ... ok
test pattern_engine::tests::test_match_triples_with_labels ... ok
test query_cache::tests::test_cache_basic_operations ... ok
test pattern_engine::tests::test_match_triples_direction ... ok
test pattern_engine::tests::test_match_triples_with_properties ... ok
test pattern_engine_cache::tests::tests::test_fast_path_vs_sql_equality ... ok
test query_cache::tests::test_cache_key_hashing ... ok
test query_cache::tests::test_k_hop_filtered_cache ... ok
test pattern_engine_cache::tests::tests::test_fast_path_basic_functionality ... ok
test pattern_engine_cache::tests::tests::test_fast_path_deterministic_ordering ... ok
test query_cache::tests::test_shortest_path_cache ... ok
test hnsw::storage::tests::test_vector_record_touch ... ok

test result: ok. 608 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 1.01s

