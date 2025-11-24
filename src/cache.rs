use ahash::AHashMap;
use parking_lot::RwLock;

#[derive(Default)]
pub struct AdjacencyCache {
    inner: RwLock<AHashMap<i64, Vec<i64>>>,
}

impl AdjacencyCache {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(AHashMap::new()),
        }
    }

    pub fn get(&self, key: i64) -> Option<Vec<i64>> {
        self.inner.read().get(&key).cloned()
    }

    pub fn insert(&self, key: i64, value: Vec<i64>) {
        self.inner.write().insert(key, value);
    }

    pub fn clear(&self) {
        self.inner.write().clear();
    }
}
