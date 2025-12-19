# Query Cache Modularization Implementation Plan

## Executive Summary

**File**: `sqlitegraph/src/query_cache.rs`
**Current Size**: 416 lines
**Target**: Core file ≤300 lines (98% reduction)
**Implementation Strategy**: Phased extraction of cache keys, storage, and utilities
**Risk Level**: LOW (well-defined type system and clean architecture)
**Estimated Timeline**: 1 day with comprehensive testing

## Detailed Implementation Plan

### Phase 0: Pre-Implementation Validation (Day 1 - 1 hour)

#### 0.1 Baseline Establishment
```bash
# Establish current behavior baseline
cargo test --lib query_cache -- --nocapture
cargo test --lib QueryCache -- --nocapture
cargo test --lib test_cache_basic_operations -- --nocapture

# Test all cache patterns
cargo test --lib test_cache_key_hashing -- --nocapture
cargo test --lib test_k_hop_filtered_cache -- --nocapture
cargo test --lib test_shortest_path_cache -- --nocapture
```

#### 0.2 Dependency Mapping
- [x] **Confirmed**: Used in `graph/core.rs` as field in SqliteGraph struct
- [x] **Confirmed**: Used in `graph/adjacency.rs` for cache invalidation
- [x] **Confirmed**: Exported via `lib.rs` as part of public API
- [x] **Confirmed**: Minimal external dependencies (only BackendDirection)

#### 0.3 Current Usage Validation
```bash
# Verify all usage patterns work
cargo test --lib graph -- --nocapture

# Test cache integration with SqliteGraph
cargo test --lib sqlitegraph -- --nocapture 2>/dev/null || echo "Test name may differ"
```

### Phase 1: Extract Test Suite (Day 1 - 1 hour)

#### 1.1 Create `query_cache_tests.rs`
**Target Size**: 86 lines (move all tests)
**Implementation**:

```rust
//! Comprehensive tests for query cache functionality

use super::*;
use crate::backend::BackendDirection;

#[test]
fn test_cache_key_hashing() {
    // Test that identical keys produce identical hashes
    let key1 = super::query_cache_keys::QueryCacheKey::Bfs(
        super::query_cache_keys::BfsCacheKey { start: 42, depth: 3 }
    );
    let key2 = super::query_cache_keys::QueryCacheKey::Bfs(
        super::query_cache_keys::BfsCacheKey { start: 42, depth: 3 }
    );
    assert_eq!(key1.hash(), key2.hash());

    // Test that different keys produce different hashes
    let key3 = super::query_cache_keys::QueryCacheKey::Bfs(
        super::query_cache_keys::BfsCacheKey { start: 42, depth: 4 }
    );
    assert_ne!(key1.hash(), key3.hash());
}

#[test]
fn test_cache_basic_operations() {
    let cache = super::QueryCache::new();

    // Test cache miss
    assert_eq!(cache.get_bfs(1, 2), None);

    // Test cache put and hit
    cache.put_bfs(1, 2, vec![3, 4, 5]);
    assert_eq!(cache.get_bfs(1, 2), Some(vec![3, 4, 5]));

    // Test cache size
    assert_eq!(cache.size(), 1);
    assert!(!cache.is_empty());

    // Test cache invalidation
    cache.invalidate_all();
    assert_eq!(cache.get_bfs(1, 2), None);
    assert_eq!(cache.size(), 0);
    assert!(cache.is_empty());
}

#[test]
fn test_k_hop_filtered_cache() {
    let cache = super::QueryCache::new();
    let edge_types = vec!["friend", "colleague"];

    // Test cache miss
    assert_eq!(
        cache.get_k_hop_filtered(1, 2, BackendDirection::Outgoing, &edge_types),
        None
    );

    // Test cache put and hit
    cache.put_k_hop_filtered(1, 2, BackendDirection::Outgoing, &edge_types, vec![3, 4]);
    assert_eq!(
        cache.get_k_hop_filtered(1, 2, BackendDirection::Outgoing, &edge_types),
        Some(vec![3, 4])
    );

    // Test that different edge types don't interfere
    assert_eq!(
        cache.get_k_hop_filtered(1, 2, BackendDirection::Outgoing, &["enemy"]),
        None
    );
}

#[test]
fn test_shortest_path_cache() {
    let cache = super::QueryCache::new();

    // Test caching None result
    cache.put_shortest_path(1, 5, None);
    assert_eq!(cache.get_shortest_path(1, 5), Some(None));

    // Test caching Some result
    cache.put_shortest_path(1, 3, Some(vec![1, 2, 3]));
    assert_eq!(cache.get_shortest_path(1, 3), Some(Some(vec![1, 2, 3])));

    // Test cache size
    assert_eq!(cache.size(), 2);
}

#[test]
fn test_query_cache_key_equality() {
    use super::query_cache_keys::*;

    let key1 = KHopFilteredCacheKey {
        start: 1,
        depth: 2,
        direction: BackendDirection::Outgoing,
        allowed_edge_types: vec!["friend".to_string(), "colleague".to_string()],
    };

    let key2 = KHopFilteredCacheKey {
        start: 1,
        depth: 2,
        direction: BackendDirection::Outgoing,
        allowed_edge_types: vec!["friend".to_string(), "colleague".to_string()],
    };

    assert_eq!(key1, key2);

    // Test inequality
    let key3 = KHopFilteredCacheKey {
        start: 1,
        depth: 3, // Different depth
        direction: BackendDirection::Outgoing,
        allowed_edge_types: vec!["friend".to_string(), "colleague".to_string()],
    };

    assert_ne!(key1, key3);
}
```

#### 1.2 Update Core Module
```rust
// Remove entire #[cfg(test)] mod tests section from query_cache.rs
// File size reduced by 86 lines
```

#### 1.3 Update Module Structure
```rust
// In query_cache.rs
#[cfg(test)]
mod query_cache_tests;
```

#### 1.4 Validation
```bash
# Test all query_cache tests in new location
cargo test --lib query_cache_tests -- --nocapture

# Ensure no tests lost
cargo test --lib -- --list | grep query_cache

# Verify graph module still works
cargo test --lib graph::adjacency -- --nocapture
```

**Expected Result**: 416 → 330 lines (21% reduction, still over 300 LOC target)

### Phase 2: Extract Cache Key Types (Day 1 - 2 hours)

#### 2.1 Create `query_cache_keys.rs`
**Target Size**: 190 lines
**Implementation**:

```rust
//! Cache key types and implementations for query caching

use std::hash::{Hash, Hasher};
use crate::backend::BackendDirection;

/// Cache key for BFS queries
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BfsCacheKey {
    pub start: i64,
    pub depth: u32,
}

/// Cache key for k-hop queries
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KHopCacheKey {
    pub start: i64,
    pub depth: u32,
    pub direction: BackendDirection,
}

/// Cache key for filtered k-hop queries
#[derive(Debug, Clone)]
pub struct KHopFilteredCacheKey {
    pub start: i64,
    pub depth: u32,
    pub direction: BackendDirection,
    pub allowed_edge_types: Vec<String>,
}

/// Cache key for shortest path queries
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortestPathCacheKey {
    pub start: i64,
    pub end: i64,
}

/// Enumeration of all supported query cache keys
#[derive(Debug, Clone)]
pub enum QueryCacheKey {
    Bfs(BfsCacheKey),
    KHop(KHopCacheKey),
    KHopFiltered(KHopFilteredCacheKey),
    ShortestPath(ShortestPathCacheKey),
}

/// Cache entry containing query results
#[derive(Debug, Clone)]
pub struct QueryCacheEntry {
    pub result: QueryResult,
}

/// Enumeration of cached query results
#[derive(Debug, Clone)]
pub enum QueryResult {
    Bfs(Vec<i64>),
    KHop(Vec<i64>),
    ShortestPath(Option<Vec<i64>>),
}

impl QueryCacheKey {
    /// Create a deterministic hash for the cache key
    pub fn hash(&self) -> u64 {
        let mut hasher = ahash::AHasher::default();
        match self {
            QueryCacheKey::Bfs(key) => {
                0u8.hash(&mut hasher);
                key.start.hash(&mut hasher);
                key.depth.hash(&mut hasher);
            }
            QueryCacheKey::KHop(key) => {
                1u8.hash(&mut hasher);
                key.start.hash(&mut hasher);
                key.depth.hash(&mut hasher);
                super::query_cache_utils::hash_direction(key.direction).hash(&mut hasher);
            }
            QueryCacheKey::KHopFiltered(key) => {
                2u8.hash(&mut hasher);
                key.start.hash(&mut hasher);
                key.depth.hash(&mut hasher);
                super::query_cache_utils::hash_direction(key.direction).hash(&mut hasher);
                key.allowed_edge_types.len().hash(&mut hasher);
                for edge_type in &key.allowed_edge_types {
                    edge_type.hash(&mut hasher);
                }
            }
            QueryCacheKey::ShortestPath(key) => {
                3u8.hash(&mut hasher);
                key.start.hash(&mut hasher);
                key.end.hash(&mut hasher);
            }
        }
        hasher.finish()
    }
}

impl PartialEq for QueryCacheKey {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (QueryCacheKey::Bfs(a), QueryCacheKey::Bfs(b)) => a == b,
            (QueryCacheKey::KHop(a), QueryCacheKey::KHop(b)) => a == b,
            (QueryCacheKey::KHopFiltered(a), QueryCacheKey::KHopFiltered(b)) => {
                a.start == b.start
                    && a.depth == b.depth
                    && a.direction == b.direction
                    && a.allowed_edge_types == b.allowed_edge_types
            }
            (QueryCacheKey::ShortestPath(a), QueryCacheKey::ShortestPath(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for QueryCacheKey {}

impl Hash for QueryCacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash().hash(state);
    }
}

impl PartialEq for KHopFilteredCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start
            && self.depth == other.depth
            && self.direction == other.direction
            && self.allowed_edge_types == other.allowed_edge_types
    }
}

impl Eq for KHopFilteredCacheKey {}

impl Hash for KHopFilteredCacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.start.hash(state);
        self.depth.hash(state);
        super::query_cache_utils::hash_direction(self.direction).hash(state);
        self.allowed_edge_types.len().hash(state);
        for edge_type in &self.allowed_edge_types {
            edge_type.hash(state);
        }
    }
}
```

#### 2.2 Create `query_cache_utils.rs`
**Target Size**: 30 lines
**Implementation**:

```rust
//! Common utilities for query cache operations

use std::hash::{Hash, Hasher};
use crate::backend::BackendDirection;

/// Hash backend direction to u8 for consistent hashing
pub fn hash_direction(direction: BackendDirection) -> u8 {
    match direction {
        BackendDirection::Outgoing => 0u8,
        BackendDirection::Incoming => 1u8,
    }
}

/// Create deterministic hash for multiple values
pub fn create_hash<T: Hash>(values: &[T]) -> u64 {
    let mut hasher = ahash::AHasher::default();
    for value in values {
        value.hash(&mut hasher);
    }
    hasher.finish()
}

/// Convert string slice references to owned strings for cache keys
pub fn convert_edge_types(edge_types: &[&str]) -> Vec<String> {
    edge_types.iter().map(|s| s.to_string()).collect()
}
```

#### 2.3 Update Core Query Cache Module
```rust
// In query_cache.rs, keep only imports and re-exports

//! High-level query cache layer for SQLiteGraph.

pub use query_cache_keys::*;
pub use query_cache_utils::*;

// Re-export the main cache storage
pub use query_cache_storage::QueryCache;

// Module declarations
mod query_cache_keys;
mod query_cache_storage;
mod query_cache_utils;

#[cfg(test)]
mod query_cache_tests;
```

#### 2.4 Validation
```bash
# Test key extraction
cargo test --lib test_cache_key_hashing -- --nocapture
cargo test --lib query_cache_tests::test_cache_key_hashing -- --nocapture

# Test utilities
cargo test --lib query_cache_utils -- --nocapture 2>/dev/null || echo "Test module name differs"
```

**Expected Result**: 330 → 150 lines (55% additional reduction)

### Phase 3: Extract Cache Storage (Day 1 - 2 hours)

#### 3.1 Create `query_cache_storage.rs`
**Target Size**: 140 lines
**Implementation**:

```rust
//! Thread-safe query cache storage implementation

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use super::query_cache_keys::{QueryCacheKey, QueryCacheEntry, QueryResult};
use super::query_cache_utils::convert_edge_types;
use crate::backend::BackendDirection;

/// Thread-safe query cache storage
#[derive(Debug)]
pub struct QueryCache {
    cache: Arc<RwLock<HashMap<QueryCacheKey, QueryCacheEntry>>>,
}

impl QueryCache {
    /// Create a new query cache
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generic cache get operation with result extractor
    fn get_with_extractor<T, F>(&self, key: &QueryCacheKey, extractor: F) -> Option<T>
    where
        F: FnOnce(&QueryResult) -> Option<T>,
    {
        let cache = self.cache.read().unwrap();
        cache.get(key).and_then(|entry| extractor(&entry.result))
    }

    /// Generic cache put operation
    fn put_result(&self, key: QueryCacheKey, result: QueryResult) {
        let entry = QueryCacheEntry { result };
        let mut cache = self.cache.write().unwrap();
        cache.insert(key, entry);
    }

    /// Get a cached result for a BFS query
    pub fn get_bfs(&self, start: i64, depth: u32) -> Option<Vec<i64>> {
        let key = QueryCacheKey::Bfs(super::query_cache_keys::BfsCacheKey { start, depth });
        self.get_with_extractor(&key, |result| match result {
            QueryResult::Bfs(r) => Some(r.clone()),
            _ => None,
        })
    }

    /// Cache a BFS query result
    pub fn put_bfs(&self, start: i64, depth: u32, result: Vec<i64>) {
        let key = QueryCacheKey::Bfs(super::query_cache_keys::BfsCacheKey { start, depth });
        self.put_result(key, QueryResult::Bfs(result));
    }

    /// Get a cached result for a k-hop query
    pub fn get_k_hop(
        &self,
        start: i64,
        depth: u32,
        direction: BackendDirection,
    ) -> Option<Vec<i64>> {
        let key = QueryCacheKey::KHop(super::query_cache_keys::KHopCacheKey {
            start,
            depth,
            direction,
        });
        self.get_with_extractor(&key, |result| match result {
            QueryResult::KHop(r) => Some(r.clone()),
            _ => None,
        })
    }

    /// Cache a k-hop query result
    pub fn put_k_hop(&self, start: i64, depth: u32, direction: BackendDirection, result: Vec<i64>) {
        let key = QueryCacheKey::KHop(super::query_cache_keys::KHopCacheKey {
            start,
            depth,
            direction,
        });
        self.put_result(key, QueryResult::KHop(result));
    }

    /// Get a cached result for a filtered k-hop query
    pub fn get_k_hop_filtered(
        &self,
        start: i64,
        depth: u32,
        direction: BackendDirection,
        allowed_edge_types: &[&str],
    ) -> Option<Vec<i64>> {
        let edge_types = convert_edge_types(allowed_edge_types);
        let key = QueryCacheKey::KHopFiltered(super::query_cache_keys::KHopFilteredCacheKey {
            start,
            depth,
            direction,
            allowed_edge_types: edge_types,
        });
        self.get_with_extractor(&key, |result| match result {
            QueryResult::KHop(r) => Some(r.clone()),
            _ => None,
        })
    }

    /// Cache a filtered k-hop query result
    pub fn put_k_hop_filtered(
        &self,
        start: i64,
        depth: u32,
        direction: BackendDirection,
        allowed_edge_types: &[&str],
        result: Vec<i64>,
    ) {
        let edge_types = convert_edge_types(allowed_edge_types);
        let key = QueryCacheKey::KHopFiltered(super::query_cache_keys::KHopFilteredCacheKey {
            start,
            depth,
            direction,
            allowed_edge_types: edge_types,
        });
        self.put_result(key, QueryResult::KHop(result));
    }

    /// Get a cached result for a shortest path query
    pub fn get_shortest_path(&self, start: i64, end: i64) -> Option<Option<Vec<i64>>> {
        let key = QueryCacheKey::ShortestPath(super::query_cache_keys::ShortestPathCacheKey { start, end });
        self.get_with_extractor(&key, |result| match result {
            QueryResult::ShortestPath(r) => Some(r.clone()),
            _ => None,
        })
    }

    /// Cache a shortest path query result
    pub fn put_shortest_path(&self, start: i64, end: i64, result: Option<Vec<i64>>) {
        let key = QueryCacheKey::ShortestPath(super::query_cache_keys::ShortestPathCacheKey { start, end });
        self.put_result(key, QueryResult::ShortestPath(result));
    }

    /// Clear all cached queries (MVCC invalidation)
    pub fn invalidate_all(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.clear();
    }

    /// Get cache statistics for monitoring
    pub fn size(&self) -> usize {
        let cache = self.cache.read().unwrap();
        cache.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        let cache = self.cache.read().unwrap();
        cache.is_empty()
    }
}

impl Default for QueryCache {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for QueryCache {
    fn clone(&self) -> Self {
        Self {
            cache: Arc::clone(&self.cache),
        }
    }
}
```

#### 3.2 Update Module Structure
```rust
// In query_cache.rs, add storage module
mod query_cache_storage;
```

#### 3.3 Validation
```bash
# Test storage extraction
cargo test --lib test_cache_basic_operations -- --nocapture
cargo test --lib query_cache_tests::test_cache_basic_operations -- --nocapture

# Test cache storage functionality
cargo test --lib QueryCache -- --nocapture
```

**Expected Result**: 150 → 15 lines (90% additional reduction)

### Phase 4: Final Integration and Validation (Day 1 - 1 hour)

#### 4.1 Final Core Module Structure
**Minimal remaining file**:

```rust
//! High-level query cache layer for SQLiteGraph.
//!
//! This module provides deterministic, MVCC-aware caching for expensive graph traversal
//! queries. The cache is transparent to callers and lives entirely inside the SQLiteGraph
//! implementation without requiring any API changes.

// Re-export all types and implementations for backward compatibility
pub use query_cache_keys::*;
pub use query_cache_storage::QueryCache;
pub use query_cache_utils::*;

// Internal module organization
mod query_cache_keys;
mod query_cache_storage;
mod query_cache_utils;

#[cfg(test)]
mod query_cache_tests;
```

#### 4.2 Update Module Exports
```rust
// In lib.rs, ensure proper exports
pub use query_cache::{QueryCache, QueryCacheKey, QueryResult};
```

#### 4.3 Comprehensive Testing
```bash
# Full test suite with all modules
cargo test --workspace --all-features

# Specific integration tests
cargo test --lib query_cache -- --nocapture
cargo test --lib graph -- --nocapture

# Performance testing (if benchmarks exist)
cargo bench --bench query_cache 2>/dev/null || echo "No bench found"
```

#### 4.4 Line Count Validation
```bash
# Count lines in modularized core file
wc -l sqlitegraph/src/query_cache.rs

# Count lines in all new modules
find sqlitegraph/src -name "*query_cache*" -exec wc -l {} +
```

**Expected Result**: 15 → 10 lines (33% additional reduction from final cleanup)

## Risk Mitigation Strategies

### Low Risk Implementation

1. **Type System Preservation**: Maintain all existing types and trait implementations
2. **Backward Compatibility**: Use re-exports to maintain identical public API
3. **Thread Safety**: Preserve Arc<RwLock<>> pattern exactly
4. **Incremental Testing**: Test each phase immediately after implementation

### Minimal Validation Required

1. **API Consistency**: Verify all cache operations work identically
2. **Test Coverage**: Ensure no test functionality is lost
3. **Performance**: Confirm no performance degradation from modularization
4. **Integration**: Ensure SqliteGraph integration works correctly

## Expected Outcomes

### Size Reduction Analysis

**Current**: 416 lines
**After Phase 1**: 416 → 330 lines (21% reduction - still over target)
**After Phase 2**: 330 → 150 lines (55% additional reduction)
**After Phase 3**: 150 → 15 lines (90% additional reduction)
**After Phase 4**: 15 → 10 lines (33% additional reduction)

**Final Result**: 10 lines (98% total reduction, 306 lines under 300 LOC target)

### Module Distribution

1. **Core Cache Module**: 10 lines - Essential re-exports and coordination
2. **Test Suite**: 86 lines - Comprehensive testing (separate file)
3. **Cache Keys**: 190 lines - Key types and trait implementations
4. **Cache Storage**: 140 lines - Storage operations and management
5. **Cache Utilities**: 30 lines - Common hashing and conversion utilities

### Quality Improvements

1. **Design Compliance**: Achieves 300 LOC target after Phase 2
2. **Functional Separation**: Clear boundaries between keys, storage, and utilities
3. **Code Reusability**: Extracted utilities can be used by other cache implementations
4. **Test Organization**: Tests properly isolated with shared utilities
5. **Maintainability**: Focused, single-responsibility modules

## Success Criteria

### Functional Requirements
- [ ] All existing cache operations work identically
- [ ] `graph/core.rs` continues working without changes
- [ ] `graph/adjacency.rs` cache invalidation works correctly
- [ ] All tests pass in new location
- [ ] No performance regression

### Design Requirements
- [ ] Core file ≤300 lines (achieved after Phase 2)
- [ ] Each extracted module ≤300 lines
- [ ] Clear separation of concerns
- [ ] No circular dependencies
- [ ] Preserved public API

### Quality Requirements
- [ ] All modules documented
- [ ] Test coverage maintained
- [ ] Code quality standards met
- [ ] Import statements clean
- [ ] Compilation successful

## Critical Success Factors

### API Preservation
1. **Type Compatibility**: Must maintain all existing cache key types
2. **Method Signatures**: Preserve all cache operation signatures
3. **Thread Safety**: Maintain identical concurrent access patterns
4. **Hash Consistency**: Ensure identical hash generation for cache keys

### Test Reliability
1. **Complete Test Migration**: No tests lost in extraction
2. **Test Independence**: Tests should work with extracted modules
3. **Cache Behavior**: All cache hit/miss behavior must be preserved
4. **Edge Cases**: All edge cases still covered

### Integration Stability
1. **Import Resolution**: All imports resolve correctly after extraction
2. **Module Dependencies**: No circular dependencies created
3. **Build Success**: Project compiles without errors
4. **Runtime Stability**: All runtime operations work correctly

## Special Considerations

### Minimum Success Requirement

Unlike previous files, this modularization requires at least **Phase 2 completion** to achieve the 300 LOC target. Test extraction alone (Phase 1) leaves the file at 330 lines, still exceeding the limit.

### Generic Operations

The extraction of generic cache operations is critical for maintaining code quality and eliminating the repetitive get/put patterns that contribute to the file size.

---

**Implementation Plan Created**: 2025-01-19
**Strategy**: Phased key, storage, and utility extraction
**Risk Level**: LOW (high confidence in success)
**Expected Timeline**: 1 day with comprehensive testing
**Key Challenge**: Requires 2 phases minimum to achieve 300 LOC target