    Checking sqlitegraph v0.2.5 (/home/feanor/Projects/sqlitegraph/sqlitegraph)
error[E0425]: cannot find value `direction` in this scope
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:375:94
    |
368 |         _direction: crate::backend::native::v2::Direction,
    |         ---------- `_direction` defined here
...
375 |             println!("V2 Edge Insert: {} -> {} (direction: {:?})", source_node, target_node, direction);
    |                                                                                              ^^^^^^^^^
    |
help: the leading underscore in `_direction` marks it as unused, consider renaming it to `direction`
    |
368 -         _direction: crate::backend::native::v2::Direction,
368 +         direction: crate::backend::native::v2::Direction,
    |

error[E0425]: cannot find value `direction` in this scope
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:399:94
    |
392 |         _direction: crate::backend::native::v2::Direction,
    |         ---------- `_direction` defined here
...
399 |             println!("V2 Edge Update: {} -> {} (direction: {:?})", source_node, target_node, direction);
    |                                                                                              ^^^^^^^^^
    |
help: the leading underscore in `_direction` marks it as unused, consider renaming it to `direction`
    |
392 -         _direction: crate::backend::native::v2::Direction,
392 +         direction: crate::backend::native::v2::Direction,
    |

warning: unused import: `hnsw_config`
  --> sqlitegraph/src/hnsw/index.rs:56:9
   |
56 |         hnsw_config,
   |         ^^^^^^^^^^^
   |
   = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

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

warning: unused variable: `timestamp`
   --> sqlitegraph/src/backend/native/v2/wal/checkpoint/operations.rs:186:26
    |
186 |             if let Some(&timestamp) = dirty_blocks.block_timestamps().get(&block_offset) {
    |                          ^^^^^^^^^
    |
    = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default
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
     |
     = note: `#[warn(unused_mut)]` (part of `#[warn(unused)]`) on by default

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

warning: unused variable: `record_type`
   --> sqlitegraph/src/backend/native/v2/wal/reader.rs:252:13
    |
252 |         let record_type = V2WALRecordType::try_from(header_bytes[0])?;
    |             ^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_record_type`

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

For more information about this error, try `rustc --explain E0425`.
warning: `sqlitegraph` (lib) generated 94 warnings
error: could not compile `sqlitegraph` (lib) due to 2 previous errors; 94 warnings emitted
