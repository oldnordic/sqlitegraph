# Plan 06-02 Summary: HNSW Index Management CLI Commands

**Status:** COMPLETE
**Date:** 2026-01-17
**Depends On:** 06-01 (HNSW Persistence)

---

## Objective

Add CLI commands for HNSW index management (list, delete, info) to complement the existing create, insert, search, and stats commands.

---

## Implementation Summary

### Commands Added

#### 1. hnsw-list
**Purpose:** List all HNSW indexes in the database

**Usage:**
```bash
sqlitegraph --db /path/to/graph.db hnsw-list
```

**Output:**
```json
{
  "command": "hnsw-list",
  "count": 2,
  "indexes": ["default", "test_index"],
  "status": "completed"
}
```

**Features:**
- Lists all index names from in-memory registry
- Shows total count
- Returns JSON output
- Works with both file-based and in-memory databases

#### 2. hnsw-delete
**Purpose:** Delete an HNSW index and all its vectors

**Usage:**
```bash
sqlitegraph --db /path/to/graph.db hnsw-delete --index-name test_index
```

**Output:**
```json
{
  "command": "hnsw-delete",
  "index_name": "test_index",
  "deleted": true,
  "status": "completed"
}
```

**Error Output (non-existent index):**
```json
{
  "command": "hnsw-delete",
  "index_name": "nonexistent",
  "error": "Index not found",
  "status": "error"
}
```

**Features:**
- Requires --index-name parameter (also accepts --name for consistency)
- Validates index exists before deletion
- CASCADE deletes all vectors from database
- Removes index from in-memory registry
- Non-interactive (no confirmation prompt)
- Returns JSON output

**Safety:**
- Uses CASCADE delete in database schema
- Checks existence first to avoid panics
- Returns error message instead of crashing

#### 3. hnsw-info
**Purpose:** Show detailed information about a specific HNSW index

**Usage:**
```bash
sqlitegraph --db /path/to/graph.db hnsw-info --index-name test_index
```

**Output:**
```json
{
  "command": "hnsw-info",
  "index_name": "test_index",
  "vector_count": 100,
  "layer_count": 16,
  "entry_point_count": 1,
  "dimension": 768,
  "distance_metric": "Cosine",
  "storage": {
    "backend_type": "InMemory",
    "estimated_memory_bytes": 614400
  },
  "layers": [
    {
      "layer": 0,
      "node_count": 100,
      "avg_connections": 16.5
    },
    {
      "layer": 1,
      "node_count": 12,
      "avg_connections": 14.2
    }
  ],
  "status": "completed"
}
```

**Error Output (non-existent index):**
```json
{
  "command": "hnsw-info",
  "index_name": "nonexistent",
  "error": "Index not found",
  "status": "error"
}
```

**Features:**
- Optional --index-name parameter (defaults to "default")
- Shows metadata (dimension, distance metric)
- Shows statistics (vector count, layer count, entry points)
- Shows storage info (backend type, estimated memory)
- Shows per-layer statistics (node count, average connections)
- Returns JSON output

**Difference from hnsw-stats:**
- `hnsw-stats`: Current vector count and layer structure only
- `hnsw-info`: Combined metadata + statistics for quick overview

---

## Files Modified

### 1. sqlitegraph-cli/src/main.rs
**Changes:**
- Added command handlers for `hnsw-list`, `hnsw-delete`, `hnsw-info`
- Implemented `run_hnsw_list()` function
- Implemented `run_hnsw_delete()` function with existence check and CASCADE delete
- Implemented `run_hnsw_info()` function with detailed statistics

**Key Implementation Details:**
- Uses `graph.list_hnsw_indexes()` to get index names
- Uses `HnswIndex::delete_index()` for database deletion
- Manually removes index from in-memory `RwLock<HashMap>`
- Uses `graph.get_hnsw_index_ref()` for read-only statistics access

### 2. sqlitegraph-cli/src/cli.rs
**Changes:**
- Updated help text to include new commands
- Added `hnsw-list` command description
- Added `hnsw-delete --index-name NAME` command description
- Added `hnsw-info [--index-name NAME]` command description

### 3. sqlitegraph/src/graph/core.rs
**Changes:**
- Made `SqliteGraph.conn` public (was `pub(crate)`)
- Made `SqliteGraph.hnsw_indexes` public (was `pub(crate)`)
- Added documentation comments explaining public access for CLI

**Rationale:**
- CLI needs access to `conn` for `HnswIndex::delete_index()`
- CLI needs access to `hnsw_indexes` RwLock for manual removal
- Follows existing pattern of limited public exposure for CLI access

---

## Testing

### Test Workflow

```bash
# 1. Create new database
rm -f /tmp/test_hnsw.db

# 2. List indexes (empty)
sqlitegraph --db /tmp/test_hnsw.db hnsw-list
# Expected: {"count":0,"indexes":[]}

# 3. Create an index
sqlitegraph --db /tmp/test_hnsw.db \
  hnsw-create --dimension 3 --m 16 --ef-construction 200 \
  --distance-metric cosine --index-name test_index
# Expected: {"status":"created"}

# 4. List indexes (should show test_index)
sqlitegraph --db /tmp/test_hnsw.db hnsw-list
# Expected: {"count":1,"indexes":["test_index"]}

# 5. Get index info
sqlitegraph --db /tmp/test_hnsw.db hnsw-info --index-name test_index
# Expected: Detailed statistics

# 6. Delete index
sqlitegraph --db /tmp/test_hnsw.db hnsw-delete --index-name test_index
# Expected: {"deleted":true}

# 7. List indexes (should be empty again)
sqlitegraph --db /tmp/test_hnsw.db hnsw-list
# Expected: {"count":0,"indexes":[]}

# 8. Test error handling
sqlitegraph --db /tmp/test_hnsw.db hnsw-delete --index-name nonexistent
# Expected: {"error":"Index not found"}

sqlitegraph --db /tmp/test_hnsw.db hnsw-info --index-name nonexistent
# Expected: {"error":"Index not found"}
```

### Test Results
All tests passed:
- Empty list returns count=0
- Created index appears in list
- Info shows detailed statistics
- Delete removes index from database and memory
- Error handling for non-existent indexes works correctly

---

## API Methods Used

### From SqliteGraph
- `list_hnsw_indexes()`: Returns `Vec<String>` of index names
- `get_hnsw_index_ref(name, closure)`: Read-only access to index
- `conn`: Public SQLite connection for database operations
- `hnsw_indexes`: Public RwLock for in-memory registry access

### From HnswIndex
- `delete_index(conn, name)`: Static method to delete index from database
- `statistics()`: Returns detailed index statistics

---

## Design Decisions

### 1. No --force Flag
**Decision:** Do not add `--force` flag to `hnsw-delete`

**Rationale:**
- CLI is non-interactive by design
- No confirmation prompts anywhere in CLI
- Error checking prevents accidental deletion of wrong index
- Simpler API is better for automation

### 2. Separate hnsw-info from hnsw-stats
**Decision:** Keep both commands separate

**Rationale:**
- `hnsw-stats`: Live statistics only (vector count, layers)
- `hnsw-info`: Combined view (metadata + statistics)
- Different use cases:
  - Stats: Monitor current state during operations
  - Info: Quick overview for debugging/inspection
- Both commands are useful and serve different purposes

### 3. Public Fields on SqliteGraph
**Decision:** Make `conn` and `hnsw_indexes` public

**Rationale:**
- CLI needs direct access for index management
- Limited exposure (only these two fields)
- Follows existing pattern (CLI is part of workspace)
- Alternative would be adding wrapper methods, but that's more complex
- Documented as "public for CLI access"

### 4. Index Name Parameter Consistency
**Decision:** Support both `--index-name` and `--name`

**Rationale:**
- `--index-name`: Used by create and delete (explicit)
- `--name`: Used by insert, search, stats (shorter)
- Both accepted for consistency across commands
- Reduces user confusion about parameter names

### 5. No --verbose Flag
**Decision:** Keep output simple, no verbose mode

**Rationale:**
- All output is JSON (machine-readable)
- JSON already contains all relevant information
- Verbose flag would complicate output format
- Simpler API is better

---

## Integration with Phase 06-01

This plan builds on the persistence work from 06-01:
- Indexes are now persisted to database
- `hnsw-list` shows persisted indexes
- `hnsw-delete` removes from database (CASCADE)
- `hnsw-info` shows both metadata and statistics

**Dependency Relationship:**
- 06-01 added persistence to CLI
- 06-02 adds management commands for persisted indexes
- Future phases can build on both

---

## Verification Checklist

- [x] All three commands compile and run
- [x] `hnsw-list` shows all indexes with count
- [x] `hnsw-delete` removes indexes completely (database + memory)
- [x] `hnsw-info` shows combined metadata + statistics
- [x] Help text updated for all commands
- [x] Error handling for non-existent indexes works
- [x] Commands work with file-based databases
- [x] Commands work with in-memory databases
- [x] No regressions in existing commands
- [x] cargo check passes
- [x] All tests pass

---

## Future Enhancements (Out of Scope)

1. **Batch Operations**
   - `hnsw-delete-all`: Delete all indexes at once
   - `hnsw-export`: Export index to file
   - `hnsw-import`: Import index from file

2. **Filtering Options**
   - `hnsw-list --min-vectors N`: List indexes with at least N vectors
   - `hnsw-list --dimension N`: List indexes with specific dimension

3. **Additional Metadata**
   - Show creation timestamp in `hnsw-info`
   - Show last update timestamp
   - Show index configuration details

4. **Confirmation Prompts**
   - Optional `--force` flag to skip confirmation (if prompts added)
   - Interactive mode for safety

---

## Conclusion

All three tasks completed successfully:
1. `hnsw-list` command added and tested
2. `hnsw-delete` command added and tested
3. `hnsw-info` command added and tested

The CLI now has complete index management capabilities:
- Create: `hnsw-create`
- Insert: `hnsw-insert`
- Search: `hnsw-search`
- Statistics: `hnsw-stats`
- List: `hnsw-list` (NEW)
- Delete: `hnsw-delete` (NEW)
- Info: `hnsw-info` (NEW)

All commands return JSON output consistent with existing CLI patterns. Error handling is robust and user-friendly.
