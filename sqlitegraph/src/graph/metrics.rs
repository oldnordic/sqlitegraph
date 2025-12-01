use std::{
    collections::HashSet,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

pub use instrumented::InstrumentedConnection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphMetricsSnapshot {
    pub prepare_count: u64,
    pub execute_count: u64,
    pub tx_begin_count: u64,
    pub tx_commit_count: u64,
    pub tx_rollback_count: u64,
    pub prepare_cache_hits: u64,
    pub prepare_cache_misses: u64,
}

#[derive(Default)]
pub struct GraphMetrics {
    prepares: AtomicU64,
    executes: AtomicU64,
    tx_begin: AtomicU64,
    tx_commit: AtomicU64,
    tx_rollback: AtomicU64,
    prepare_cache_hits: AtomicU64,
    prepare_cache_misses: AtomicU64,
}

impl GraphMetrics {
    pub fn snapshot(&self) -> GraphMetricsSnapshot {
        GraphMetricsSnapshot {
            prepare_count: self.prepares.load(Ordering::Relaxed),
            execute_count: self.executes.load(Ordering::Relaxed),
            tx_begin_count: self.tx_begin.load(Ordering::Relaxed),
            tx_commit_count: self.tx_commit.load(Ordering::Relaxed),
            tx_rollback_count: self.tx_rollback.load(Ordering::Relaxed),
            prepare_cache_hits: self.prepare_cache_hits.load(Ordering::Relaxed),
            prepare_cache_misses: self.prepare_cache_misses.load(Ordering::Relaxed),
        }
    }

    pub fn reset(&self) {
        self.prepares.store(0, Ordering::Relaxed);
        self.executes.store(0, Ordering::Relaxed);
        self.tx_begin.store(0, Ordering::Relaxed);
        self.tx_commit.store(0, Ordering::Relaxed);
        self.tx_rollback.store(0, Ordering::Relaxed);
        self.prepare_cache_hits.store(0, Ordering::Relaxed);
        self.prepare_cache_misses.store(0, Ordering::Relaxed);
    }

    pub fn record_prepare(&self) {
        self.prepares.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_execute(&self, sql: Option<&str>) {
        self.executes.fetch_add(1, Ordering::Relaxed);
        if let Some(keyword) = sql.and_then(leading_keyword) {
            if keyword.eq_ignore_ascii_case("BEGIN") {
                self.tx_begin.fetch_add(1, Ordering::Relaxed);
            } else if keyword.eq_ignore_ascii_case("COMMIT") {
                self.tx_commit.fetch_add(1, Ordering::Relaxed);
            } else if keyword.eq_ignore_ascii_case("ROLLBACK") {
                self.tx_rollback.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    pub fn record_prepare_cache_hit(&self) {
        self.prepare_cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_prepare_cache_miss(&self) {
        self.prepare_cache_misses.fetch_add(1, Ordering::Relaxed);
        self.record_prepare();
    }
}

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

fn leading_keyword(sql: &str) -> Option<&str> {
    let trimmed = sql.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    let end = trimmed
        .find(|c: char| c.is_ascii_whitespace() || c == ';')
        .unwrap_or(trimmed.len());
    Some(&trimmed[..end])
}

mod instrumented {
    use rusqlite::{CachedStatement, Connection};

    use super::{CacheObservation, GraphMetrics, StatementTracker};

    #[derive(Copy, Clone)]
    pub struct InstrumentedConnection<'a> {
        conn: &'a Connection,
        metrics: &'a GraphMetrics,
        tracker: &'a StatementTracker,
    }

    impl<'a> InstrumentedConnection<'a> {
        pub fn new(
            conn: &'a Connection,
            metrics: &'a GraphMetrics,
            tracker: &'a StatementTracker,
        ) -> Self {
            Self {
                conn,
                metrics,
                tracker,
            }
        }

        pub fn execute<P>(&self, sql: &str, params: P) -> Result<usize, rusqlite::Error>
        where
            P: rusqlite::Params,
        {
            self.metrics.record_execute(Some(sql));
            self.conn.execute(sql, params)
        }

        pub fn prepare_cached<'b>(
            &'b self,
            sql: &str,
        ) -> Result<InstrumentedCachedStatement<'b>, rusqlite::Error> {
            match self.tracker.observe(sql) {
                CacheObservation::Hit => self.metrics.record_prepare_cache_hit(),
                CacheObservation::Miss => self.metrics.record_prepare_cache_miss(),
            }
            Ok(InstrumentedCachedStatement {
                stmt: self.conn.prepare_cached(sql)?,
                metrics: self.metrics,
                sql: sql.to_string(),
            })
        }

        pub fn query_row<P, F, R>(&self, sql: &str, params: P, f: F) -> Result<R, rusqlite::Error>
        where
            P: rusqlite::Params,
            F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<R>,
        {
            self.metrics.record_prepare();
            self.metrics.record_execute(Some(sql));
            self.conn.query_row(sql, params, f)
        }

        pub fn last_insert_rowid(&self) -> i64 {
            self.conn.last_insert_rowid()
        }
    }

    pub struct InstrumentedCachedStatement<'conn> {
        stmt: CachedStatement<'conn>,
        metrics: &'conn GraphMetrics,
        sql: String,
    }

    impl<'conn> InstrumentedCachedStatement<'conn> {
        pub fn execute<P>(&mut self, params: P) -> Result<usize, rusqlite::Error>
        where
            P: rusqlite::Params,
        {
            self.metrics.record_execute(Some(self.sql.as_str()));
            self.stmt.execute(params)
        }

        pub fn query_map<P, F, T>(
            &mut self,
            params: P,
            f: F,
        ) -> Result<rusqlite::MappedRows<'_, F>, rusqlite::Error>
        where
            P: rusqlite::Params,
            F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
        {
            self.metrics.record_execute(Some(self.sql.as_str()));
            self.stmt.query_map(params, f)
        }

        pub fn query_row<P, F, R>(&mut self, params: P, f: F) -> Result<R, rusqlite::Error>
        where
            P: rusqlite::Params,
            F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<R>,
        {
            self.metrics.record_execute(Some(self.sql.as_str()));
            self.stmt.query_row(params, f)
        }
    }
}
