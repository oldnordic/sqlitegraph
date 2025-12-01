use std::{collections::HashSet, sync::Mutex};

#[derive(Default)]
pub struct StatementTracker {
    seen: Mutex<HashSet<String>>,
}

impl StatementTracker {
    pub fn observe(&self, sql: &str) -> CacheObservation {
        let normalized = sql.trim().to_string();
        let mut guard = self.seen.lock().expect("statement tracker poisoned");
        if guard.insert(normalized) {
            CacheObservation::Miss
        } else {
            CacheObservation::Hit
        }
    }
}

pub enum CacheObservation {
    Hit,
    Miss,
}
