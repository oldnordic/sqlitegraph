//! Entity CRUD operations for SqliteGraph.

use rusqlite::params;

use crate::errors::SqliteGraphError;

use super::{
    SqliteGraph,
    types::{GraphEntity, row_to_entity, validate_entity},
};

impl SqliteGraph {
    pub fn insert_entity(&self, entity: &GraphEntity) -> Result<i64, SqliteGraphError> {
        validate_entity(entity)?;
        let data = serde_json::to_string(&entity.data)
            .map_err(|e| SqliteGraphError::invalid_input(e.to_string()))?;
        self.connection()
            .execute(
                "INSERT INTO graph_entities(kind, name, file_path, data) VALUES(?1, ?2, ?3, ?4)",
                params![
                    entity.kind.as_str(),
                    entity.name.as_str(),
                    entity.file_path.as_deref(),
                    data,
                ],
            )
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_entity(&self, id: i64) -> Result<GraphEntity, SqliteGraphError> {
        self.connection()
            .query_row(
                "SELECT id, kind, name, file_path, data FROM graph_entities WHERE id=?1",
                params![id],
                row_to_entity,
            )
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => {
                    SqliteGraphError::not_found(format!("entity {id}"))
                }
                other => SqliteGraphError::query(other.to_string()),
            })
    }

    pub fn update_entity(&self, entity: &GraphEntity) -> Result<(), SqliteGraphError> {
        if entity.id <= 0 {
            return Err(SqliteGraphError::invalid_input(
                "entity id must be positive for update",
            ));
        }
        validate_entity(entity)?;
        let data = serde_json::to_string(&entity.data)
            .map_err(|e| SqliteGraphError::invalid_input(e.to_string()))?;
        let affected = self
            .connection()
            .execute(
                "UPDATE graph_entities SET kind=?1, name=?2, file_path=?3, data=?4 WHERE id=?5",
                params![
                    entity.kind.as_str(),
                    entity.name.as_str(),
                    entity.file_path.as_deref(),
                    data,
                    entity.id,
                ],
            )
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        if affected == 0 {
            return Err(SqliteGraphError::not_found(format!("entity {}", entity.id)));
        }
        Ok(())
    }

    pub fn delete_entity(&self, id: i64) -> Result<(), SqliteGraphError> {
        let affected = self
            .connection()
            .execute("DELETE FROM graph_entities WHERE id=?1", params![id])
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        if affected == 0 {
            return Err(SqliteGraphError::not_found(format!("entity {id}")));
        }
        self.connection()
            .execute(
                "DELETE FROM graph_edges WHERE from_id=?1 OR to_id=?1",
                params![id],
            )
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        self.invalidate_caches();
        Ok(())
    }

    pub fn list_entity_ids(&self) -> Result<Vec<i64>, SqliteGraphError> {
        self.all_entity_ids()
    }
}
