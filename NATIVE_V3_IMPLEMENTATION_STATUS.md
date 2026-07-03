# Native-v3 Implementation Status

**Date:** 2026-07-03
**Status:** Partial parity; storage paths are strong, query parity is incomplete
**Evidence source:** [NATIVE_V3_AUDIT.md](/home/feanor/Projects/sqlitegraph/NATIVE_V3_AUDIT.md:1)

## Overview

Native-v3 is substantially more complete than some older caveats suggested, but it does **not** currently have full feature parity with the SQLite backend.

The audited picture is:

- storage, persistence, MVCC, transactions, async traversal, HNSW, and turbovec support are confirmed working
- trait-level `pattern_search` now works on V3
- some Cypher behavior is still proven only on the SQLite execution path, and V3 multi-hop Cypher remains incomplete

## Capability Matrix

| Area | Native-v3 status | Notes |
| --- | --- | --- |
| Basic CRUD | Confirmed working | Covered by native-v3 integration and persistence tests |
| Reopen durability | Confirmed working | Reopen, WAL recovery, and checkpoint flows passed audited tests |
| KV durability | Confirmed working | Flush/truncate cycle and reopen recovery passed |
| MVCC snapshots | Confirmed working | Snapshot versioning and isolation are covered by passing tests |
| Transactions and savepoints | Confirmed working | Transaction and rollback flows are covered by passing tests |
| Public SQL interface | Confirmed working | `execute_sql*` paths are covered by passing tests |
| HNSW vector search | Confirmed working | Persistence and integration tests passed |
| Turbovec routing | Confirmed working | Covered by comprehensive feature tests |
| Async traversal | Confirmed working | Async traversal suites passed |
| `pattern_search` | Confirmed working | Trait-level pattern expansion now works on V3 |
| `snapshot_import` | Not implemented | Backend explicitly returns `Unsupported` |
| `query_nodes_by_kind` | Confirmed working | Current implementation uses the kind index |
| `query_nodes_by_name_pattern` | Semantics mismatch | V3 does not match SQLite `GLOB` behavior |
| Cypher parser | Confirmed working | Parser behavior is exercised by the generic Cypher suite |
| Cypher execution parity on V3 | Not yet proven | Existing suite primarily validates SQLite execution |
| Multi-hop Cypher on V3 | Likely incomplete | Current code path strongly suggests a gap |
| Historical snapshot semantics for helper queries | Not yet proven | Acceptance is tested; correctness is not fully proven |
| Power-loss durability after KV checkpoint rename | Not yet proven | No reproduced bug, but proof is incomplete |

## Confirmed Working Areas

The following areas are backed by direct audit evidence in [NATIVE_V3_AUDIT.md](/home/feanor/Projects/sqlitegraph/NATIVE_V3_AUDIT.md:1):

- reopen durability and file persistence
- KV durability and WAL recovery
- MVCC snapshot versioning and isolation
- transactions and nested savepoints
- public SQL access
- HNSW persistence and turbovec integration
- async traversal support

## Confirmed Gaps

These are explicit current gaps, not speculative concerns:

- `snapshot_import` is unsupported on native-v3
- `query_nodes_by_name_pattern` does not match SQLite `GLOB` semantics
- multi-hop Cypher on V3 is still incomplete

## Cypher Status

Native-v3 should not currently be described as having full Cypher parity.

- The generic `cypher_tests` suite demonstrates parser behavior and SQLite-backed execution semantics.
- That suite does not by itself prove equivalent V3 execution behavior.
- Multi-hop Cypher on V3 is not proven and current code inspection suggests it is incomplete.

## Documentation Correction

This document supersedes older wording such as:

- "full feature parity"
- "feature-complete for cypher queries"

Those claims were broader than the audited evidence supports.

## Recommended Next Work

The audit now recommends this order:

1. keep docs aligned with the audited capability matrix
2. implement `snapshot_import` or explicitly scope it out
3. align `query_nodes_by_name_pattern` with SQLite `GLOB` semantics if parity is the goal
4. close the multi-hop Cypher gap on V3
5. continue expanding V3-native Cypher execution tests
