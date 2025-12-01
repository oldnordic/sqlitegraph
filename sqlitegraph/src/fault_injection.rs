use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use crate::SqliteGraphError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FaultPoint {
    BulkInsertEntitiesBeforeCommit,
    BulkInsertEdgesBeforeCommit,
    RecoveryLoadBeforeCommit,
}

struct FaultEntry {
    remaining: usize,
}

fn registry() -> &'static Mutex<HashMap<FaultPoint, FaultEntry>> {
    static REGISTRY: OnceLock<Mutex<HashMap<FaultPoint, FaultEntry>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn reset_faults() {
    registry().lock().expect("fault registry poisoned").clear();
}

pub fn configure_fault(point: FaultPoint, failures: usize) {
    let mut guard = registry().lock().expect("fault registry poisoned");
    if failures == 0 {
        guard.remove(&point);
    } else {
        guard.insert(
            point,
            FaultEntry {
                remaining: failures,
            },
        );
    }
}

pub(crate) fn check_fault(point: FaultPoint) -> Result<(), SqliteGraphError> {
    let mut guard = registry().lock().expect("fault registry poisoned");
    if let Some(entry) = guard.get_mut(&point)
        && entry.remaining > 0
    {
        entry.remaining -= 1;
        if entry.remaining == 0 {
            guard.remove(&point);
        }
        return Err(SqliteGraphError::fault_injection(format!("{point:?}")));
    }
    Ok(())
}
