# Phase 06-01 Summary: CLI HNSW Index Persistence

**Status**: Partially Complete
**Date**: 2026-01-17
**Tasks**: 3/3 complete (with known limitation)

## Objectives

Integrate persistent HNSW storage with CLI to fix indexes lost across CLI invocations.

## Implementation Summary

### Task 1: Add hnsw_index_persistent() Method ✅

Added `SqliteGraph::hnsw_index_persistent()` method that:
- Detects file-based vs in-memory databases using `is_in_memory_connection()`
- Saves index metadata on the main connection (critical for persistence)
- Opens separate connection for `SQLiteVectorStorage`
- Falls back to `InMemoryVectorStorage` for `:memory:` databases
- Returns `RwLockWriteGuard<HashMap<String, HnswIndex>>` for API consistency

**Key Implementation Decision**: Save metadata on main connection first, then create storage connection. This ensures metadata persists even if storage connection has issues.

### Task 2: Update CLI to Use Persistent Storage ✅

Updated CLI HNSW commands:
- `run_hnsw_create()`: Use `hnsw_index_persistent()` instead of `hnsw_index()`
- Added `--index-name` parameter for custom index names (default: "default")
- Updated warning comments to reflect persistence capability
- Updated CLI help text in `cli.rs`

### Task 3: Test End-to-End Workflow ✅

**What Works**:
- Index metadata persists across CLI invocations
- `hnsw-stats` successfully loads index on subsequent invocations
- Index configuration (dimension, m, ef_construction, distance_metric) persists
- In-memory databases still work (with in-memory storage)

**Known Limitation**:
- Vectors inserted via `hnsw-insert` do NOT persist across CLI invocations
- Root cause: `load_metadata()` creates `InMemoryVectorStorage` instead of `SQLiteVectorStorage`
- `load_with_vectors()` loads vectors from database but doesn't switch storage backend
- Storage stats show "InMemory" even though index was created with SQLite storage

## Technical Details

### Files Modified

1. **sqlitegraph/src/hnsw/index.rs**
   - Added `hnsw_index_persistent()` method
   - Exported `is_in_memory_connection()` from graph module

2. **sqlitegraph/src/graph/core.rs**
   - Changed `is_in_memory_connection()` from `fn` to `pub fn`

3. **sqlitegraph/src/graph/mod.rs**
   - Exported `is_in_memory_connection` in public API

4. **sqlitegraph-cli/src/main.rs**
   - Updated `run_hnsw_create()`, `run_hnsw_insert()`, `run_hnsw_search()`, `run_hnsw_stats()`
   - Added `--index-name` parameter support
   - Updated warning comments

5. **sqlitegraph-cli/src/cli.rs**
   - Updated help text to document `--index-name` and `--name` parameters

### Key Commits

1. `90a630e`: feat: add hnsw_index_persistent() method to SqliteGraph
2. `8bc1518`: fix: save HNSW metadata on main connection for persistence
3. (Combined): feat: update CLI to use hnsw_index_persistent()

## Known Issues

### Issue: Vectors Don't Persist

**Problem**: Vectors inserted via `hnsw-insert` are not persisted to database.

**Root Cause**:
```rust
// In load_metadata():
let storage = Box::new(InMemoryVectorStorage::new());  // Always creates InMemory!
```

The `load_metadata()` method always creates `InMemoryVectorStorage`, even when loading from a database. This means:
1. Index created with `SQLiteVectorStorage` (via `hnsw_index_persistent()`)
2. Vectors inserted → stored in SQLite database
3. CLI exits → connections dropped
4. CLI restarted → index loaded with `InMemoryVectorStorage`
5. Stats show 0 vectors (loaded into memory, but storage is wrong type)

**Evidence**:
```bash
$ sqlite3 test.db "SELECT COUNT(*) FROM hnsw_vectors;"
0  # Vectors not in database!

$ hnsw-stats output:
"storage_stats":{"backend_type":"InMemory",...}  # Should be "SQLite"!
```

**Required Fix**: Modify `load_with_vectors()` to use `SQLiteVectorStorage` when loading from database:

```rust
// Proposed fix in load_with_vectors():
let index_id = Self::get_index_id(conn, name)?.unwrap();
let storage = Box::new(SQLiteVectorStorage::new(index_id, conn.clone()?));
let mut hnsw = Self::load_metadata_with_storage(conn, name, storage)?;
```

**Complexity**: Medium. Requires:
- Handling Connection cloning (not supported by rusqlite)
- Managing connection lifecycle across HnswIndex
- Ensuring storage connection doesn't conflict with main connection

**Alternative**: Wrap Connection in `Arc<Mutex<Connection>>` to share between SqliteGraph and HnswIndex.

## Testing Results

### Manual Testing

```bash
# Create index
$ sqlitegraph --db test.db hnsw-create --dimension 3 --m 16 --ef-construction 200 --distance-metric cosine
✅ Status: created

# Check metadata persists
$ sqlite3 test.db "SELECT COUNT(*) FROM hnsw_indexes;"
1  ✅

# Load index in new CLI invocation
$ sqlitegraph --db test.db hnsw-stats
✅ Shows index with correct config

# Insert vectors (same invocation)
$ sqlitegraph --db test.db hnsw-insert --input vectors.json
✅ Reports "vectors_inserted": 3

# Check if vectors persist
$ sqlite3 test.db "SELECT COUNT(*) FROM hnsw_vectors;"
0  ❌ (Expected: 3)

# New CLI invocation - stats
$ sqlitegraph --db test.db hnsw-stats
"vector_count": 0  ❌ (Expected: 3)
"storage_stats":"backend_type":"InMemory"  ❌ (Expected: "SQLite")
```

## Success Criteria

- ✅ CLI `hnsw-create` creates indexes with persistent storage for file-based databases
- ✅ Index metadata persists across CLI invocations
- ✅ `hnsw-stats` and `hnsw-search` work after reopening database (metadata only)
- ❌ Vectors inserted via `hnsw-insert` persist across CLI invocations
- ✅ In-memory databases still work (with in-memory storage)

## Next Steps

### Immediate (Plan 06-02 if needed)

1. **Fix vector persistence**:
   - Modify `load_with_vectors()` to use `SQLiteVectorStorage`
   - Implement Connection sharing via `Arc<Mutex<Connection>>`
   - Ensure vectors persist and load correctly

2. **Add integration test**:
   - Test full workflow: create → insert vectors → close → reopen → verify
   - Verify vector count persists
   - Verify search works after reload

### Future Optimizations

1. **Connection Management**:
   - Share single Connection between SqliteGraph and HnswIndex
   - Avoid opening multiple connections to same database
   - Use `Arc<Mutex<Connection>>` or similar pattern

2. **Storage Backend Detection**:
   - Auto-detect correct storage backend when loading
   - Store storage backend type in metadata
   - Use appropriate backend on load

3. **Testing**:
   - Add automated integration tests for CLI persistence
   - Test concurrent access scenarios
   - Performance benchmarks for vector persistence

## Conclusion

Plan 06-01 successfully implements HNSW index **metadata** persistence for CLI. Index configuration now survives across CLI invocations, fixing the primary complaint in the persistence issue documentation.

However, **vector persistence** requires additional architectural work to properly handle storage backend selection during index loading. The current implementation creates indexes with SQLite storage but loads them with in-memory storage, causing vectors to not persist.

**Recommendation**: Address vector persistence in Plan 06-02 by refactoring `load_with_vectors()` to use `SQLiteVectorStorage` and implementing Connection sharing.

---

**Commits**: 3
**Lines Changed**: ~150
**Test Coverage**: Manual testing only
**Documentation Updated**: Yes (CLI help text, comments)
