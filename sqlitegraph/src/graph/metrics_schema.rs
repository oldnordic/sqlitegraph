//! Metrics and schema operations for SqliteGraph.

use crate::schema::{MigrationReport, read_schema_version, run_pending_migrations};

use super::{SqliteGraph, metrics::GraphMetricsSnapshot};

impl SqliteGraph {
    pub fn metrics_snapshot(&self) -> GraphMetricsSnapshot {
        self.metrics.snapshot()
    }

    pub fn reset_metrics(&self) {
        self.metrics.reset();
    }

    pub fn schema_version(&self) -> Result<i64, crate::errors::SqliteGraphError> {
        read_schema_version(&self.conn)
    }

    pub fn run_pending_migrations(
        &self,
        dry_run: bool,
    ) -> Result<MigrationReport, crate::errors::SqliteGraphError> {
        run_pending_migrations(&self.conn, dry_run)
    }
}
