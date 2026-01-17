# HNSW Persistence Implementation Status

**Date**: 2025-12-23
**Status**: In Progress - Partial Implementation
**Approach**: Database Metadata with Vector Storage and Index Rebuilding

## What Has Been Done

### 1. Database Schema Added ✅

**File**: `sqlitegraph/src/schema.rs`

**Migration to v3** adds HNSW tables:
- `hnsw_indexes` - Index metadata (name, dimension, m, ef_construction, distance_metric)
- `hnsw_vectors` - Vector data (BLOB of f32 array) and metadata (JSON)
- `hnsw_layers` - Layer graph structure (nodes and connections)
- `hnsw_entry_points` - Entry points for hierarchical navigation

### 2. SQLite Vector Storage Created ✅

**File**: `sqlitegraph/src/hnsw/sqlite_storage.rs`

**Implemented**:
- `SQLiteVectorStorage` - Persistent vector storage backend
- Stores vectors as BLOB (f32 arrays serialized)
- Stores metadata as JSON text
- Implements `VectorStorage` trait for HNSW integration

### 3. Persistence Layer Started ⚠️

**File**: `sqlitegraph/src/hnsw/persistence.rs`

**Started but incomplete**:
- `HnswPersistence` manager for load/save operations
- Schema creation and migration handled
- Vector serialization/deserialization implemented

## What Remains

### Critical Missing Pieces

1. **Complete SQLiteVectorStorage Implementation**
   - Implement missing trait methods:
     - `store_vector_with_id()`
     - `store_batch()`
     - `delete_vector()`
     - `vector_count()`
     - `list_vectors()`
     - `clear_vectors()`
   - Fix trait method signatures to match requirements

2. **Load Index on SqliteGraph Construction**
   - Modify `SqliteGraph::from_connection()` to:
     - Query `hnsw_indexes` table for existing indexes
     - For each index:
       - Load config
       - Create `HnswIndex` with SQLite storage
       - Load vectors from database
       - Trigger HNSW index building
       - Store in `hnsw_indexes` HashMap

3. **Save Index on Modifications**
   - Modify `hnsw_index()` to save metadata when creating new index
   - Auto-save vectors when inserted via `insert_vector()`
   - Update `updated_at` timestamp on modifications

4. **Integration Testing**
   - Test: Create index, exit CLI, reopen CLI, insert vectors
   - Test: Insert vectors, exit CLI, reopen CLI, perform search
   - Verify HNSW graph structure is correctly rebuilt

## Implementation Challenge

The HNSW graph structure (layers, connections, entry points) is complex:
- **HnswLayer**: `Vec<HashSet<u64>>` - nodes and their connections
- **Connections**: Dynamic neighbor lists per node per layer
- **Entry Points**: List of optimal entry nodes

**Serialization would require**:
1. Serialize entire graph structure to BLOB
2. Maintain exact node IDs across sessions
3. Preserve layer assignments and connections

## Simpler Pragmatic Approach

Instead of full graph serialization, **rebuild the HNSW index from vectors**:

### Advantages
- Leverages existing HNSW building logic
- No complex graph structure serialization
- Vector IDs remain stable (from database)
- Simpler implementation, less error-prone

### Trade-offs
- **Rebuild Cost**: O(N log N) on database open
- **Acceptable**: For most use cases, indexes are built once and queried many times
- **Workaround**: Could cache layer structure in `hnsw_layers` table for faster rebuild

## Proposed Implementation Plan

### Phase 1: Complete Vector Persistence (Current)
1. ✅ Database schema
2. ✅ `SQLiteVectorStorage` implementation
3. ⏳ Fix trait method signatures
4. ⏳ Implement missing methods

### Phase 2: Auto-Load on Construction
1. Modify `SqliteGraph::from_connection()`:
   ```rust
   // Load existing HNSW indexes
   let index_names = self.list_hnsw_indexes_from_db()?;
   for name in index_names {
       let hnsw = self.load_hnsw_index_from_db(&name)?;
       self.hnsw_indexes.write()?.insert(name, hnsw);
   }
   ```

2. Implement `load_hnsw_index_from_db()`:
   ```rust
   fn load_hnsw_index_from_db(&self, name: &str) -> Result<HnswIndex, SqliteGraphError> {
       // 1. Load config from hnsw_indexes
       // 2. Create SQLiteVectorStorage
       // 3. Load all vectors from database
       // 4. Insert vectors into HnswIndex (builds graph structure)
       // 5. Return HnswIndex
   }
   ```

### Phase 3: Auto-Save on Modifications
1. Modify `hnsw_index()` to save metadata:
   ```rust
   pub fn hnsw_index(&self, name: &str, config: HnswConfig) -> Result<..., ...> {
       // Check if exists
       // Create HnswIndex with SQLite storage
       // Save metadata to hnsw_indexes table
       // Store in HashMap
   }
   ```

2. `SQLiteVectorStorage` auto-saves on insert:
   ```rust
   fn store_vector(&mut self, vector: &[f32], metadata: Option<Value>) -> Result<u64, HnswError> {
       // Insert into hnsw_vectors table
       // Returns vector_id from database
   }
   ```

### Phase 4: Testing
1. Create index, insert vectors, verify persisted
2. Exit CLI, reopen, verify index is loaded
3. Perform search, verify results match
4. Test with multiple indexes
5. Test index deletion

## Files Modified

- `sqlitegraph/src/schema.rs` - Added HNSW tables (migration v3)
- `sqlitegraph/src/hnsw/mod.rs` - Added persistence modules
- `sqlitegraph/src/hnsw/sqlite_storage.rs` - NEW: SQLite vector storage
- `sqlitegraph/src/hnsw/persistence.rs` - NEW: Persistence manager

## Next Steps

1. Fix `SQLiteVectorStorage` trait implementation
2. Implement auto-load in `SqliteGraph::from_connection()`
3. Implement auto-save in `hnsw_index()` method
4. Test end-to-end persistence across CLI invocations

## Complexity Estimate

- **Remaining Work**: 4-6 hours
- **Risk Level**: Medium (new code, builds on existing patterns)
- **Testing Required**: Comprehensive integration tests needed

## Recommendation

**Proceed with pragmatic vector storage + index rebuild approach**:
- Simpler than full graph serialization
- Acceptable performance for typical use cases
- Leverages existing HNSW building logic
- Reduces risk of serialization bugs

Alternative if performance is critical:
- Cache layer structure in `hnsw_layers` table
- Serialize connections as BLOB
- Faster rebuild but more complex
