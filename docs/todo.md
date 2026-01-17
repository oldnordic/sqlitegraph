# TODO / Upcoming Enhancements

## Native V2 Completion Items
- **Wire the WAL/Checkpoint helpers** (`sqlitegraph/src/backend/native/v2/wal/**/*`): dozens of validator/replayer modules exist but many functions are unused placeholders. Hooking them into the production edge/node flow closes the “unused path” warnings and unlocks recovery + checkpoint guarantees.
- **Finalize Node/Edge cluster metadata updates** (`node_store.rs`, `edge_store/mod.rs`): today cluster offsets/edge counts are patched only in a few hot paths. Completing the “apply_*” helpers listed in `v2/wal/checkpoint/operations.rs` would keep adjacency caches consistent and prevent manual offset hacks.

## CLI & Tooling Parity
- **Update Magellan/Splice integration**: both tools still expect the pre-modularized layouts. Once they ingest the new graph/wal modules you can automate refactors (unused modules, instrumentation toggles) instead of hand-editing.
- **Expose Native V2 stability commands** (`sqlitegraph-cli`): several CLI docs describe WAL metrics/edge cascade cleanups, but the binaries still hide those commands. Surfacing them helps users validate the new backend without diving into APIs.

## Observability & Hygiene
- **Cull or gate verbose debug prints** (`V2_SLOT_DEBUG`, `[CLUSTER_DEBUG]`, `[phase76]`): these logs are invaluable during bring-up but flood users. Add a single env/feature gate and document it in README so production runs stay quiet.
- **Convert “unused” scaffolding into tracked issues**: many warning messages map to future work (e.g., `GraphTransaction`, `WALRecordCompressor`, `ImportValidator`). Documenting the intent here keeps contributors from deleting them and provides a backlog reference.

## Testing & Release Automation
- **Codify regression gates**: add smoke tests that flip between `native-v2` (std IO) and `v2_experimental` (mmap) to catch read/write drift like the recent corrupted node bug.
- **Publish pipeline**: automate the “bump version → changelog update → publish” workflow so crates.io releases cannot drift from git history, especially once Magellan/Splice begin consuming published crates directly.
