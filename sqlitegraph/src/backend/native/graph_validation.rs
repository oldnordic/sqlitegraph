//! Validation and error mapping utilities for native graph backend.

use super::types::*;
use crate::SqliteGraphError;
use crate::backend::{EdgeSpec, NodeSpec};
use crate::graph::GraphEntity;

/// Error mapping from NativeBackendError to SqliteGraphError
pub fn map_to_graph_error(err: NativeBackendError) -> SqliteGraphError {
    match err {
        NativeBackendError::Io(e) => SqliteGraphError::connection(e.to_string()),
        NativeBackendError::InvalidNodeId { id, max_id } => {
            SqliteGraphError::query(format!("Invalid node ID: {} (max: {})", id, max_id))
        }
        NativeBackendError::InvalidEdgeId { id, max_id } => {
            SqliteGraphError::query(format!("Invalid edge ID: {} (max: {})", id, max_id))
        }
        NativeBackendError::CorruptNodeRecord { node_id, reason } => {
            SqliteGraphError::connection(format!("Corrupt node record {}: {}", node_id, reason))
        }
        NativeBackendError::CorruptEdgeRecord { edge_id, reason } => {
            SqliteGraphError::connection(format!("Corrupt edge record {}: {}", edge_id, reason))
        }
        NativeBackendError::FileTooSmall { size, min_size } => {
            SqliteGraphError::connection(format!("File too small: {} < {}", size, min_size))
        }
        NativeBackendError::RecordTooLarge { size, max_size } => {
            SqliteGraphError::connection(format!("Record too large: {} > {}", size, max_size))
        }
        NativeBackendError::InconsistentAdjacency {
            node_id,
            count,
            direction,
            file_count,
        } => SqliteGraphError::connection(format!(
            "Inconsistent adjacency for node {}: {} {} != {} in file",
            node_id, direction, count, file_count
        )),
        NativeBackendError::InvalidMagic { expected, found } => {
            SqliteGraphError::connection(format!(
                "Invalid magic number: expected {:#x}, got {:#x}",
                expected, found
            ))
        }
        NativeBackendError::UnsupportedVersion { version } => {
            SqliteGraphError::connection(format!("Unsupported version: {} (supported: 1)", version))
        }
        NativeBackendError::InvalidHeader { field, reason } => {
            SqliteGraphError::connection(format!("Invalid header field '{}': {}", field, reason))
        }
        NativeBackendError::InvalidChecksum { expected, found } => {
            SqliteGraphError::connection(format!(
                "Invalid checksum: expected {:#x}, got {:#x}",
                expected, found
            ))
        }
        NativeBackendError::Utf8Error(e) => SqliteGraphError::connection(e.to_string()),
        NativeBackendError::JsonError(e) => SqliteGraphError::connection(e.to_string()),
        NativeBackendError::InvalidUtf8(e) => SqliteGraphError::connection(e.to_string()),
        NativeBackendError::BufferTooSmall { size, min_size } => {
            SqliteGraphError::connection(format!("Buffer too small: {} < {}", size, min_size))
        }
    }
}

/// Convert NodeSpec to NodeRecord for storage
pub fn node_spec_to_record(spec: NodeSpec, node_id: NativeNodeId) -> NodeRecord {
    NodeRecord::new(node_id, spec.kind, spec.name, spec.data)
}

/// Convert NodeRecord from storage to GraphEntity
pub fn node_record_to_entity(record: NodeRecord) -> GraphEntity {
    GraphEntity {
        id: record.id as i64,
        kind: record.kind,
        name: record.name,
        file_path: None, // Native backend doesn't store file_path
        data: record.data,
    }
}

/// Convert EdgeSpec to EdgeRecord for storage
pub fn edge_spec_to_record(spec: EdgeSpec, edge_id: NativeEdgeId) -> EdgeRecord {
    EdgeRecord::new(
        edge_id,
        spec.from as NativeNodeId,
        spec.to as NativeNodeId,
        spec.edge_type,
        spec.data,
    )
}

/// Validate node exists and is accessible
pub fn validate_node_exists(
    graph_file: &mut super::graph_file::GraphFile,
    node_id: NativeNodeId,
) -> Result<(), NativeBackendError> {
    let mut node_store = super::node_store::NodeStore::new(graph_file);

    // Try to read the node - this will return an error if node doesn't exist
    node_store.read_node(node_id)?;

    Ok(())
}

/// Validate edge exists and is accessible
pub fn validate_edge_exists(
    graph_file: &mut super::graph_file::GraphFile,
    edge_id: NativeEdgeId,
) -> Result<(), NativeBackendError> {
    let mut edge_store = super::edge_store::EdgeStore::new(graph_file);

    // Try to read the edge - this will return an error if edge doesn't exist
    edge_store.read_edge(edge_id)?;

    Ok(())
}

/// Validate node ID is in valid range
pub fn validate_node_id_range(
    graph_file: &super::graph_file::GraphFile,
    node_id: NativeNodeId,
) -> Result<(), NativeBackendError> {
    let header = graph_file.header();

    // Check lower bound (must be positive)
    if node_id <= 0 {
        return Err(NativeBackendError::InvalidNodeId {
            id: node_id,
            max_id: header.node_count as NativeNodeId,
        });
    }

    // For upper bound, allow both existing nodes and reasonable future allocation
    // Allow up to 100,000 OR current node count + space for 1000 more nodes
    let max_allowed = std::cmp::max(100_000, header.node_count + 1000);
    if node_id > max_allowed as NativeNodeId {
        return Err(NativeBackendError::InvalidNodeId {
            id: node_id,
            max_id: max_allowed as NativeNodeId,
        });
    }

    Ok(())
}

/// Validate edge ID is in valid range
pub fn validate_edge_id_range(
    graph_file: &super::graph_file::GraphFile,
    edge_id: NativeEdgeId,
) -> Result<(), NativeBackendError> {
    let header = graph_file.header();

    if edge_id <= 0 || edge_id > header.edge_count as NativeEdgeId {
        return Err(NativeBackendError::InvalidEdgeId {
            id: edge_id,
            max_id: header.edge_count as NativeEdgeId,
        });
    }

    Ok(())
}

/// Check if file operations are in a consistent state
pub fn check_file_consistency(
    graph_file: &super::graph_file::GraphFile,
) -> Result<(), NativeBackendError> {
    let header = graph_file.header();

    // Basic header validation
    if header.node_count < 0 || header.edge_count < 0 {
        return Err(NativeBackendError::CorruptNodeRecord {
            node_id: 0,
            reason: "Negative counts in header".to_string(),
        });
    }

    // Check for reasonable limits
    if header.node_count > 1_000_000 || header.edge_count > 10_000_000 {
        return Err(NativeBackendError::CorruptNodeRecord {
            node_id: 0,
            reason: "Counts exceed reasonable limits".to_string(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::graph_file::GraphFile;
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_error_mapping() {
        let node_error = NativeBackendError::InvalidNodeId { id: 0, max_id: 10 };
        let mapped = map_to_graph_error(node_error);

        match mapped {
            SqliteGraphError::QueryError(msg) => {
                assert!(msg.contains("Invalid node ID"));
                assert!(msg.contains("0"));
                assert!(msg.contains("10"));
            }
            _ => panic!("Expected QueryError"),
        }
    }

    #[test]
    fn test_node_spec_to_record() {
        let spec = NodeSpec {
            kind: "Test".to_string(),
            name: "test_node".to_string(),
            file_path: Some("/path/to/file".to_string()),
            data: serde_json::json!({"key": "value"}),
        };

        let record = node_spec_to_record(spec, 5);
        assert_eq!(record.id, 5);
        assert_eq!(record.kind, "Test");
        assert_eq!(record.name, "test_node");
        assert_eq!(record.data, serde_json::json!({"key": "value"}));
    }

    #[test]
    fn test_node_record_to_entity() {
        let record = NodeRecord::new(
            42,
            "Test".to_string(),
            "test_node".to_string(),
            serde_json::json!({"key": "value"}),
        );

        let entity = node_record_to_entity(record);
        assert_eq!(entity.id, 42);
        assert_eq!(entity.kind, "Test");
        assert_eq!(entity.name, "test_node");
        assert_eq!(entity.data, serde_json::json!({"key": "value"}));
    }

    #[test]
    fn test_validate_node_id_range() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let graph_file = GraphFile::create(path).unwrap();

        // Valid node ID should pass (even though node doesn't exist yet)
        assert!(validate_node_id_range(&graph_file, 1).is_ok());

        // Invalid node IDs should fail
        assert!(validate_node_id_range(&graph_file, 0).is_err());
        assert!(validate_node_id_range(&graph_file, -1).is_err());
        assert!(validate_node_id_range(&graph_file, 1000000).is_err());
    }

    #[test]
    fn test_check_file_consistency() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let graph_file = GraphFile::create(path).unwrap();

        // Fresh file should be consistent
        assert!(check_file_consistency(&graph_file).is_ok());
    }
}
