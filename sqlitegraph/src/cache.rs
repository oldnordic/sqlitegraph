use std::sync::atomic::{AtomicU64, Ordering};

use ahash::AHashMap;
use parking_lot::RwLock;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub entries: usize,
}

#[derive(Default)]
pub struct AdjacencyCache {
    inner: RwLock<AHashMap<i64, Vec<i64>>>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl AdjacencyCache {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(AHashMap::new()),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    pub fn get(&self, key: i64) -> Option<Vec<i64>> {
        if let Some(value) = self.inner.read().get(&key).cloned() {
            self.hits.fetch_add(1, Ordering::Relaxed);
            Some(value)
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    pub fn insert(&self, key: i64, value: Vec<i64>) {
        self.inner.write().insert(key, value);
    }

    pub fn clear(&self) {
        self.inner.write().clear();
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
    }

    pub fn remove(&self, key: i64) {
        self.inner.write().remove(&key);
    }

    pub fn stats(&self) -> CacheStats {
        let entries = self.inner.read().len();
        CacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            entries,
        }
    }

    /// Get a reference to the inner HashMap for snapshot creation
    /// This method provides access to the underlying data structure
    pub fn inner(&self) -> std::collections::HashMap<i64, Vec<i64>> {
        let ahash_map = self.inner.read().clone();
        ahash_map.into_iter().collect()
    }
}
