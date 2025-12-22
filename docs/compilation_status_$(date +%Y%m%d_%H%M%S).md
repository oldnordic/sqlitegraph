   Compiling sqlitegraph v0.2.5 (/home/feanor/Projects/sqlitegraph/sqlitegraph)
warning: unused import: `SeekFrom`
   --> sqlitegraph/src/backend/native/graph_file/file_ops.rs:199:25
    |
199 |     use std::io::{Seek, SeekFrom, Write};
    |                         ^^^^^^^^
    |
    = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

warning: unused import: `SeekFrom`
   --> sqlitegraph/src/backend/native/graph_file/io_backend.rs:407:25
    |
407 |     use std::io::{Seek, SeekFrom, Write};
    |                         ^^^^^^^^

warning: unused import: `super::*`
   --> sqlitegraph/src/backend/native/graph_file/memory_mapping.rs:256:9
    |
256 |     use super::*;
    |         ^^^^^^^^

warning: unused imports: `Read`, `SeekFrom`, `Seek`, and `Write`
   --> sqlitegraph/src/backend/native/graph_file/memory_mapping.rs:257:19
    |
257 |     use std::io::{Read, Seek, SeekFrom, Write};
    |                   ^^^^  ^^^^  ^^^^^^^^  ^^^^^

warning: unused import: `tempfile::tempfile`
   --> sqlitegraph/src/backend/native/graph_file/memory_mapping.rs:258:9
    |
258 |     use tempfile::tempfile;
    |         ^^^^^^^^^^^^^^^^^^

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

warning: unused import: `crate::hnsw::errors::HnswConfigError`
   --> sqlitegraph/src/hnsw/builder.rs:273:9
    |
273 |     use crate::hnsw::errors::HnswConfigError;
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `crate::hnsw::builder::HnswConfigBuilder`
   --> sqlitegraph/src/hnsw/config.rs:246:9
    |
246 |     use crate::hnsw::builder::HnswConfigBuilder;
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `crate::hnsw::distance_functions::*`
  --> sqlitegraph/src/hnsw/distance_metric.rs:37:5
   |
37 | use crate::hnsw::distance_functions::*;
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `crate::hnsw::distance_functions::*`
   --> sqlitegraph/src/hnsw/distance_metric.rs:150:9
    |
150 |     use crate::hnsw::distance_functions::*;
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused imports: `SearchResult`, `VectorRecord`, and `compute_distance`
  --> sqlitegraph/src/hnsw/index.rs:54:43
   |
54 |         distance_metric::{DistanceMetric, compute_distance},
   |                                           ^^^^^^^^^^^^^^^^
...
58 |         neighborhood::{NeighborhoodSearch, SearchResult},
   |                                            ^^^^^^^^^^^^
59 |         storage::{InMemoryVectorStorage, VectorRecord, VectorStorage, VectorStorageStats},
   |                                          ^^^^^^^^^^^^

warning: unused import: `rand::SeedableRng`
   --> sqlitegraph/src/hnsw/multilayer.rs:692:9
    |
692 |     use rand::SeedableRng;
    |         ^^^^^^^^^^^^^^^^^

warning: unused import: `crate::backend::native::v2::node_record_v2::NodeRecordV2Ext`
 --> sqlitegraph/src/backend/native/adjacency/core_iterator.rs:7:5
  |
7 | use crate::backend::native::v2::node_record_v2::NodeRecordV2Ext;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `crate::backend::native::v2::node_record_v2::NodeRecordV2Ext`
 --> sqlitegraph/src/backend/native/adjacency/v2_clustered.rs:6:5
  |
6 | use crate::backend::native::v2::node_record_v2::NodeRecordV2Ext;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `crate::backend::native::v2::node_record_v2::NodeRecordV2Ext`
   --> sqlitegraph/src/backend/native/edge_store/mod.rs:100:13
    |
100 |         use crate::backend::native::v2::node_record_v2::NodeRecordV2Ext;
    |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `crate::backend::native::v2::node_record_v2::NodeRecordV2Ext`
   --> sqlitegraph/src/backend/native/edge_store/mod.rs:182:13
    |
182 |         use crate::backend::native::v2::node_record_v2::NodeRecordV2Ext;
    |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `Read`
   --> sqlitegraph/src/backend/native/graph_file/file_lifecycle.rs:251:19
    |
251 |     use std::io::{Read, Write};
    |                   ^^^^

warning: unused import: `Write`
   --> sqlitegraph/src/backend/native/graph_file/file_ops.rs:199:35
    |
199 |     use std::io::{Seek, SeekFrom, Write};
    |                                   ^^^^^

warning: unused import: `Seek`
   --> sqlitegraph/src/backend/native/graph_file/file_ops.rs:199:19
    |
199 |     use std::io::{Seek, SeekFrom, Write};
    |                   ^^^^

warning: unused import: `Write`
   --> sqlitegraph/src/backend/native/graph_file/io_backend.rs:407:35
    |
407 |     use std::io::{Seek, SeekFrom, Write};
    |                                   ^^^^^

warning: unused import: `Seek`
   --> sqlitegraph/src/backend/native/graph_file/io_backend.rs:407:19
    |
407 |     use std::io::{Seek, SeekFrom, Write};
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

warning: unused import: `NodeRecordV2Ext`
 --> sqlitegraph/src/backend/native/node_store.rs:9:64
  |
9 | use crate::backend::native::v2::node_record_v2::{NodeRecordV2, NodeRecordV2Ext};
  |                                                                ^^^^^^^^^^^^^^^

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

warning: unused import: `NodeRecordV2Ext`
  --> sqlitegraph/src/backend/native/v2/wal/checkpoint/operations.rs:18:64
   |
18 | use crate::backend::native::v2::node_record_v2::{NodeRecordV2, NodeRecordV2Ext};
   |                                                                ^^^^^^^^^^^^^^^

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

warning: unused import: `NodeRecordV2Ext`
  --> sqlitegraph/src/backend/native/v2/wal/recovery/validator.rs:18:64
   |
18 | use crate::backend::native::v2::node_record_v2::{NodeRecordV2, NodeRecordV2Ext};
   |                                                                ^^^^^^^^^^^^^^^

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
   --> sqlitegraph/src/backend/native/graph_file/file_ops.rs:232:13
    |
232 |         let mut temp_file = tempfile().unwrap();
    |             ----^^^^^^^^^
    |             |
    |             help: remove this `mut`

warning: variable does not need to be mutable
   --> sqlitegraph/src/backend/native/graph_file/file_ops.rs:244:13
    |
244 |         let mut temp_file = tempfile().unwrap();
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

error[E0596]: cannot borrow `node_store_guard` as mutable, as it is not declared as mutable
   --> sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs:789:34
    |
789 |                 let node_store = node_store_guard.as_mut().ok_or_else(|| {
    |                                  ^^^^^^^^^^^^^^^^ cannot borrow as mutable
    |
help: consider changing this to be mutable
    |
788 |                 let mut node_store_guard = self.node_store.lock();
    |                     +++

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

For more information about this error, try `rustc --explain E0596`.
warning: `sqlitegraph` (lib test) generated 216 warnings
error: could not compile `sqlitegraph` (lib test) due to 1 previous error; 216 warnings emitted
