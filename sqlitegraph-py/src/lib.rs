//! Python bindings for sqlitegraph via PyO3.
//!
//! Provides a thin wrapper over `SqliteGraphBackend` + HNSW. The default build
//! exposes the graph database surface only. The optional `inference` feature
//! pulls in the experimental sparse-inference engine (with numpy / ndarray).

use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use sqlitegraph::{
    algo::{connected_components, louvain_communities, pagerank},
    backend::{EdgeSpec, NeighborQuery, NodeSpec},
    hnsw::{
        config::HnswConfig,
        errors::{HnswError, HnswIndexError},
        DistanceMetric,
    },
    GraphBackend, GraphEdge, SqliteGraph, SqliteGraphBackend, SqliteGraphError,
};

#[cfg(feature = "inference")]
use sqlitegraph::inference::{InferenceConfig as RustInferenceConfig, SparseInferenceEngine};

// ── Exception hierarchy ──────────────────────────────────────────
//
// Python users see typed exceptions instead of bare `RuntimeError`:
//
//     GraphError              ← base class for every error this package raises
//     ├── NotFoundError       ← node / edge / index not found
//     ├── InvalidArgumentError← bad input, validation failure, duplicate index
//     └── BackendError        ← storage / corruption / unsupported operation

pyo3::create_exception!(_native, GraphError, PyException);
pyo3::create_exception!(_native, NotFoundError, GraphError);
pyo3::create_exception!(_native, InvalidArgumentError, GraphError);
pyo3::create_exception!(_native, BackendError, GraphError);

/// Map a `SqliteGraphError` to the appropriate Python exception class.
fn into_pyerr(err: SqliteGraphError) -> PyErr {
    let message = err.to_string();
    match err {
        SqliteGraphError::NotFound(_) => NotFoundError::new_err(message),
        SqliteGraphError::InvalidInput(_) | SqliteGraphError::ValidationError(_) => {
            InvalidArgumentError::new_err(message)
        }
        SqliteGraphError::GraphCorruption(_)
        | SqliteGraphError::Unsupported(_)
        | SqliteGraphError::FaultInjected(_)
        | SqliteGraphError::NativeError(_) => BackendError::new_err(message),
        SqliteGraphError::ConnectionError(_)
        | SqliteGraphError::SchemaError(_)
        | SqliteGraphError::QueryError(_)
        | SqliteGraphError::TransactionError(_) => GraphError::new_err(message),
    }
}

/// Map an `HnswError` to the appropriate Python exception class.
fn hnsw_to_pyerr(err: HnswError) -> PyErr {
    let message = err.to_string();
    match &err {
        HnswError::Config(_) => InvalidArgumentError::new_err(message),
        HnswError::Storage(_) => BackendError::new_err(message),
        HnswError::Index(idx) => {
            if matches!(
                idx,
                HnswIndexError::VectorNotFound(_) | HnswIndexError::NodeNotFound(_)
            ) {
                NotFoundError::new_err(message)
            } else if matches!(idx, HnswIndexError::IndexCorrupted(_)) {
                BackendError::new_err(message)
            } else {
                InvalidArgumentError::new_err(message)
            }
        }
        HnswError::MultiLayer(_) => BackendError::new_err(message),
    }
}

/// A sqlitegraph database with HNSW vector search.
///
/// Wraps SqliteGraphBackend which implements all GraphBackend trait methods.
/// HNSW indexes are accessed through the underlying SqliteGraph.
#[pyclass(module = "sqlitegraph._native", unsendable)]
pub struct Graph {
    backend: SqliteGraphBackend,
}

#[pymethods]
impl Graph {
    /// Open a file-backed graph database.
    #[staticmethod]
    fn open(path: String) -> PyResult<Self> {
        let graph = SqliteGraph::open(&path).map_err(into_pyerr)?;
        let backend = SqliteGraphBackend::from_graph(graph);
        Ok(Graph { backend })
    }

    /// Open an in-memory graph database (fast, no persistence).
    #[staticmethod]
    fn open_in_memory() -> PyResult<Self> {
        let backend = SqliteGraphBackend::in_memory().map_err(into_pyerr)?;
        Ok(Graph { backend })
    }

    // ── Node operations ──────────────────────────────────────────

    /// Insert a node.
    ///
    /// Args:
    ///     kind: Node type label (e.g. "neuron", "token", "layer").
    ///     name: Human-readable name.
    ///     data: Optional dict of properties.
    ///
    /// Returns:
    ///     The new node's integer ID.
    #[pyo3(signature = (kind, name, data=None))]
    fn add_node(
        &self,
        kind: String,
        name: String,
        data: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<i64> {
        let json_data = match data {
            Some(d) => dict_to_json(d)?,
            None => serde_json::json!({}),
        };
        let spec = NodeSpec {
            kind,
            name,
            file_path: None,
            data: json_data,
        };
        self.backend.insert_node(spec).map_err(into_pyerr)
    }

    /// Get a node by ID. Returns a dict with keys: id, kind, name, data.
    fn get_node<'py>(&self, py: Python<'py>, id: i64) -> PyResult<Bound<'py, PyDict>> {
        let entity = self
            .backend
            .get_node(sqlitegraph::SnapshotId::current(), id)
            .map_err(into_pyerr)?;
        entity_to_dict(py, &entity)
    }

    /// Delete a node and all its edges.
    fn delete_node(&self, id: i64) -> PyResult<()> {
        self.backend.delete_entity(id).map_err(into_pyerr)
    }

    /// Get all node IDs.
    fn node_ids(&self) -> PyResult<Vec<i64>> {
        self.backend.entity_ids().map_err(into_pyerr)
    }

    /// Get nodes of a specific kind.
    fn nodes_by_kind(&self, kind: String) -> PyResult<Vec<i64>> {
        self.backend
            .query_nodes_by_kind(sqlitegraph::SnapshotId::current(), &kind)
            .map_err(into_pyerr)
    }

    /// Get node IDs whose name matches a SQL `GLOB` pattern (e.g. `"Al*"`).
    fn nodes_by_name_pattern(&self, pattern: String) -> PyResult<Vec<i64>> {
        self.backend
            .query_nodes_by_name_pattern(sqlitegraph::SnapshotId::current(), &pattern)
            .map_err(into_pyerr)
    }

    /// Update an existing node in place, preserving its ID.
    ///
    /// Args:
    ///     id: The node ID to update.
    ///     kind: New kind label.
    ///     name: New human-readable name.
    ///     data: New properties (replaces existing data).
    ///
    /// Returns:
    ///     The same node ID that was passed in.
    #[pyo3(signature = (id, kind, name, data=None))]
    fn update_node(
        &self,
        id: i64,
        kind: String,
        name: String,
        data: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<i64> {
        let json_data = match data {
            Some(d) => dict_to_json(d)?,
            None => serde_json::json!({}),
        };
        let spec = NodeSpec {
            kind,
            name,
            file_path: None,
            data: json_data,
        };
        self.backend.update_node(id, spec).map_err(into_pyerr)
    }

    // ── Edge operations ──────────────────────────────────────────

    /// Insert an edge between two nodes.
    ///
    /// Args:
    ///     from_id: Source node ID.
    ///     to_id: Target node ID.
    ///     edge_type: Edge label (e.g. "co_activation", "attention").
    ///     data: Optional dict of properties (e.g. {"weight": 0.85}).
    ///
    /// Returns:
    ///     The new edge's integer ID.
    #[pyo3(signature = (from_id, to_id, edge_type, data=None))]
    fn add_edge(
        &self,
        from_id: i64,
        to_id: i64,
        edge_type: String,
        data: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<i64> {
        let json_data = match data {
            Some(d) => dict_to_json(d)?,
            None => serde_json::json!({}),
        };
        let spec = EdgeSpec {
            from: from_id,
            to: to_id,
            edge_type,
            data: json_data,
        };
        self.backend.insert_edge(spec).map_err(into_pyerr)
    }

    /// Get neighbors of a node.
    ///
    /// Args:
    ///     node_id: The node to query.
    ///     edge_type: Optional edge type filter.
    ///     direction: "outgoing" (default) or "incoming".
    ///
    /// Returns:
    ///     List of neighbor node IDs.
    #[pyo3(signature = (node_id, edge_type=None, direction=None))]
    fn neighbors(
        &self,
        node_id: i64,
        edge_type: Option<String>,
        direction: Option<String>,
    ) -> PyResult<Vec<i64>> {
        let dir = match direction.as_deref() {
            Some("incoming") => sqlitegraph::BackendDirection::Incoming,
            _ => sqlitegraph::BackendDirection::Outgoing,
        };
        let query = NeighborQuery {
            direction: dir,
            edge_type,
        };
        self.backend
            .neighbors(sqlitegraph::SnapshotId::current(), node_id, query)
            .map_err(into_pyerr)
    }

    /// BFS traversal from a node.
    ///
    /// Args:
    ///     start: Node ID to traverse from.
    ///     depth: Maximum hop distance.
    ///     edge_types: Optional list of edge types to traverse along. If
    ///         ``None``, every outgoing edge is followed. If provided as an
    ///         empty list, the result is empty.
    ///     direction: ``"outgoing"`` (default) or ``"incoming"``. Only meaningful
    ///         when ``edge_types`` is provided; the unfiltered path is always
    ///         outgoing.
    #[pyo3(signature = (start, depth, edge_types=None, direction=None))]
    fn bfs(
        &self,
        start: i64,
        depth: u32,
        edge_types: Option<Vec<String>>,
        direction: Option<String>,
    ) -> PyResult<Vec<i64>> {
        let snapshot = sqlitegraph::SnapshotId::current();
        match edge_types {
            None => self.backend.bfs(snapshot, start, depth).map_err(into_pyerr),
            Some(types) => {
                let dir = match direction.as_deref() {
                    Some("incoming") => sqlitegraph::BackendDirection::Incoming,
                    _ => sqlitegraph::BackendDirection::Outgoing,
                };
                let refs: Vec<&str> = types.iter().map(String::as_str).collect();
                self.backend
                    .bfs_filtered(snapshot, start, depth, dir, &refs)
                    .map_err(into_pyerr)
            }
        }
    }

    /// K-hop neighbors.
    ///
    /// Args:
    ///     start: Starting node ID.
    ///     depth: Number of hops.
    ///     direction: ``"outgoing"`` (default) or ``"incoming"``.
    ///     edge_types: Optional list of edge types to traverse along. When
    ///         provided as an empty list, the result is empty.
    #[pyo3(signature = (start, depth, direction=None, edge_types=None))]
    fn k_hop(
        &self,
        start: i64,
        depth: u32,
        direction: Option<String>,
        edge_types: Option<Vec<String>>,
    ) -> PyResult<Vec<i64>> {
        let dir = match direction.as_deref() {
            Some("incoming") => sqlitegraph::BackendDirection::Incoming,
            _ => sqlitegraph::BackendDirection::Outgoing,
        };
        let snapshot = sqlitegraph::SnapshotId::current();
        match edge_types {
            None => self
                .backend
                .k_hop(snapshot, start, depth, dir)
                .map_err(into_pyerr),
            Some(types) => {
                let refs: Vec<&str> = types.iter().map(String::as_str).collect();
                self.backend
                    .k_hop_filtered(snapshot, start, depth, dir, &refs)
                    .map_err(into_pyerr)
            }
        }
    }

    /// Get node degree as ``(in_degree, out_degree)``.
    ///
    /// Note: the underlying Rust trait returns ``(out, in)`` — we swap here
    /// so Python users get the conventional ``(in, out)`` order.
    fn node_degree(&self, node_id: i64) -> PyResult<(usize, usize)> {
        let (out, incoming) = self
            .backend
            .node_degree(sqlitegraph::SnapshotId::current(), node_id)
            .map_err(into_pyerr)?;
        Ok((incoming, out))
    }

    /// Shortest path between two nodes, as a list of node IDs.
    ///
    /// Args:
    ///     start: Source node ID.
    ///     end: Destination node ID.
    ///     edge_types: Optional list of edge types the path may traverse. When
    ///         provided as an empty list, ``None`` is returned.
    ///
    /// Returns ``None`` if no path exists.
    #[pyo3(signature = (start, end, edge_types=None))]
    fn shortest_path(
        &self,
        start: i64,
        end: i64,
        edge_types: Option<Vec<String>>,
    ) -> PyResult<Option<Vec<i64>>> {
        let snapshot = sqlitegraph::SnapshotId::current();
        match edge_types {
            None => self
                .backend
                .shortest_path(snapshot, start, end)
                .map_err(into_pyerr),
            Some(types) => {
                let refs: Vec<&str> = types.iter().map(String::as_str).collect();
                self.backend
                    .shortest_path_filtered(snapshot, start, end, &refs)
                    .map_err(into_pyerr)
            }
        }
    }

    /// Fetch an edge by ID as a dict with keys: id, from_id, to_id, edge_type, data.
    fn get_edge<'py>(&self, py: Python<'py>, id: i64) -> PyResult<Bound<'py, PyDict>> {
        let edge = self.backend.graph().get_edge(id).map_err(into_pyerr)?;
        edge_to_dict(py, &edge)
    }

    /// Delete an edge by ID.
    fn delete_edge(&self, id: i64) -> PyResult<()> {
        self.backend.graph().delete_edge(id).map_err(into_pyerr)
    }

    // ── Graph algorithms ─────────────────────────────────────────

    /// PageRank scores as a list of ``(node_id, score)`` tuples.
    ///
    /// Args:
    ///     damping: Damping factor (default 0.85).
    ///     iterations: Power-iteration count (default 20).
    #[pyo3(signature = (damping=None, iterations=None))]
    fn pagerank(
        &self,
        damping: Option<f64>,
        iterations: Option<usize>,
    ) -> PyResult<Vec<(i64, f64)>> {
        pagerank(
            self.backend.graph(),
            damping.unwrap_or(0.85),
            iterations.unwrap_or(20),
        )
        .map_err(into_pyerr)
    }

    /// Louvain community detection.
    ///
    /// Returns a list of communities; each community is a list of node IDs.
    #[pyo3(signature = (max_iterations=None))]
    fn louvain_communities(&self, max_iterations: Option<usize>) -> PyResult<Vec<Vec<i64>>> {
        louvain_communities(self.backend.graph(), max_iterations.unwrap_or(10)).map_err(into_pyerr)
    }

    /// Connected components (forward reachability).
    ///
    /// Returns a list of components; each component is a list of node IDs.
    fn connected_components(&self) -> PyResult<Vec<Vec<i64>>> {
        connected_components(self.backend.graph()).map_err(into_pyerr)
    }

    // ── HNSW vector index ────────────────────────────────────────

    /// Create an HNSW vector index.
    ///
    /// Args:
    ///     name: Index name.
    ///     dimension: Vector dimensionality.
    ///     m: Connections per node (default 16).
    ///     ef_construction: Build-time candidate list size (default 200).
    ///     metric: Distance metric: "cosine", "euclidean", or "dot" (default "cosine").
    #[pyo3(signature = (name, dimension, m=None, ef_construction=None, metric=None))]
    fn create_hnsw_index(
        slf: Bound<'_, Self>,
        name: String,
        dimension: usize,
        m: Option<usize>,
        ef_construction: Option<usize>,
        metric: Option<String>,
    ) -> PyResult<HnswIndexWrapper> {
        let dist = match metric.as_deref() {
            Some("euclidean") => DistanceMetric::Euclidean,
            Some("dot") => DistanceMetric::DotProduct,
            _ => DistanceMetric::Cosine,
        };
        let config = HnswConfig::new(
            dimension,
            m.unwrap_or(16),
            ef_construction.unwrap_or(200),
            dist,
        );
        let index_name = {
            let this = slf.borrow();
            let graph = this.backend.graph();
            let mut indexes = graph
                .hnsw_index_persistent(&name, config)
                .map_err(into_pyerr)?;
            let index = indexes
                .get_mut(&name)
                .ok_or_else(|| BackendError::new_err("Index not found after creation"))?;
            index.name().to_string()
        };

        Ok(HnswIndexWrapper {
            parent: slf.unbind(),
            name: index_name,
        })
    }

    /// Get an existing HNSW index by name.
    fn get_hnsw_index(slf: Bound<'_, Self>, name: String) -> PyResult<HnswIndexWrapper> {
        {
            let this = slf.borrow();
            let graph = this.backend.graph();
            let indexes = graph
                .hnsw_indexes
                .lock()
                .map_err(|e| BackendError::new_err(format!("Mutex poisoned: {}", e)))?;
            if !indexes.contains_key(&name) {
                return Err(NotFoundError::new_err(format!(
                    "Index '{}' not found",
                    name
                )));
            }
        }
        Ok(HnswIndexWrapper {
            parent: slf.unbind(),
            name,
        })
    }

    /// List all HNSW index names.
    fn list_hnsw_indexes(&self) -> PyResult<Vec<String>> {
        let graph = self.backend.graph();
        graph.list_hnsw_indexes().map_err(into_pyerr)
    }

    /// Force checkpoint (flush WAL to disk).
    fn checkpoint(&self) -> PyResult<()> {
        self.backend.checkpoint().map_err(into_pyerr)
    }
}

/// Wrapper around an HNSW index that re-acquires the Mutex per operation.
///
/// Holds a refcounted reference to the parent `Graph` (via `Py<Graph>`) so the
/// underlying `SqliteGraph` outlives every wrapper for the index.
#[pyclass(module = "sqlitegraph._native", name = "HnswIndex", unsendable)]
pub struct HnswIndexWrapper {
    parent: Py<Graph>,
    name: String,
}

#[pymethods]
impl HnswIndexWrapper {
    /// Insert a vector into the index.
    #[pyo3(signature = (vector, metadata=None))]
    fn insert_vector(
        &self,
        py: Python<'_>,
        vector: Vec<f32>,
        metadata: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<u64> {
        let json_meta = match metadata {
            Some(d) => Some(dict_to_json(d)?),
            None => None,
        };
        let parent = self.parent.borrow(py);
        let graph = parent.backend.graph();

        let mut indexes = graph
            .hnsw_indexes
            .lock()
            .map_err(|e| BackendError::new_err(format!("Mutex poisoned: {}", e)))?;
        let index = indexes
            .get_mut(&self.name)
            .ok_or_else(|| NotFoundError::new_err("Index not found"))?;
        index
            .insert_vector(&vector, json_meta)
            .map_err(hnsw_to_pyerr)
    }

    /// Bulk-insert vectors.
    ///
    /// Args:
    ///     items: List of (vector, metadata_dict_or_none) tuples.
    ///     Metadata dicts are converted at call time.
    ///
    /// Returns:
    ///     List of vector IDs.
    fn bulk_insert_vectors(&self, py: Python<'_>, items: &Bound<'_, PyAny>) -> PyResult<Vec<u64>> {
        let parent = self.parent.borrow(py);
        let graph = parent.backend.graph();
        let mut indexes = graph
            .hnsw_indexes
            .lock()
            .map_err(|e| BackendError::new_err(format!("Mutex poisoned: {}", e)))?;
        let index = indexes
            .get_mut(&self.name)
            .ok_or_else(|| NotFoundError::new_err("Index not found"))?;

        let list = items.cast::<pyo3::types::PyList>().map_err(|_| {
            InvalidArgumentError::new_err("items must be a list of (vector, metadata) tuples")
        })?;

        let mut ids = Vec::with_capacity(list.len());
        for item in list.iter() {
            let tuple = item.cast::<pyo3::types::PyTuple>().map_err(|_| {
                InvalidArgumentError::new_err("each item must be a (vector, metadata) tuple")
            })?;
            if tuple.len() != 2 {
                return Err(InvalidArgumentError::new_err(
                    "each item must be a (vector, metadata) tuple",
                ));
            }
            let vec: Vec<f32> = tuple.get_item(0)?.extract()?;
            let meta_obj = tuple.get_item(1)?;
            let json_meta = if meta_obj.is_none() {
                None
            } else {
                let meta_dict = meta_obj.cast::<PyDict>().map_err(|_| {
                    InvalidArgumentError::new_err("metadata must be a dict or None")
                })?;
                Some(dict_to_json(meta_dict)?)
            };
            let id = index
                .insert_vector(&vec, json_meta)
                .map_err(hnsw_to_pyerr)?;
            ids.push(id);
        }
        Ok(ids)
    }

    /// Fast bulk insert from a numpy 2D float32 array.
    ///
    /// Takes a contiguous numpy array of shape [n_vectors, dimension]
    /// and inserts all rows. No per-vector metadata — uses auto-increment
    /// neuron_id as metadata.
    ///
    /// This is 10-50x faster than bulk_insert_vectors because it avoids
    /// Python list/tuple creation entirely — reads the raw f32 buffer
    /// directly from numpy memory.
    fn bulk_insert_numpy(&self, py: Python<'_>, array: &Bound<'_, PyAny>) -> PyResult<Vec<u64>> {
        let parent = self.parent.borrow(py);
        let graph = parent.backend.graph();
        let mut indexes = graph
            .hnsw_indexes
            .lock()
            .map_err(|e| BackendError::new_err(format!("Mutex poisoned: {}", e)))?;
        let index = indexes
            .get_mut(&self.name)
            .ok_or_else(|| NotFoundError::new_err("Index not found"))?;

        // Get raw buffer from numpy array
        let array_obj = array
            .cast::<PyAny>()
            .map_err(|_| InvalidArgumentError::new_err("argument must be a numpy array"))?;

        // Extract shape and data via numpy's Python API
        let shape: Vec<usize> = array_obj.getattr("shape")?.extract()?;
        if shape.len() != 2 {
            return Err(InvalidArgumentError::new_err(
                "array must be 2D [n_vectors, dimension]",
            ));
        }
        let n_vectors = shape[0];
        let dimension = shape[1];

        // Get contiguous buffer as bytes, then reinterpret as f32
        let bytes_obj = array_obj.call_method0("tobytes")?;
        let bytes: &[u8] = bytes_obj.cast::<pyo3::types::PyBytes>()?.as_bytes();

        // Check alignment and size
        if bytes.len() != n_vectors * dimension * 4 {
            return Err(InvalidArgumentError::new_err(format!(
                "buffer size mismatch: {} bytes vs expected {}",
                bytes.len(),
                n_vectors * dimension * 4
            )));
        }

        let float_slice: &[f32] = unsafe {
            std::slice::from_raw_parts(bytes.as_ptr() as *const f32, n_vectors * dimension)
        };

        let mut ids = Vec::with_capacity(n_vectors);
        for i in 0..n_vectors {
            let vec = &float_slice[i * dimension..(i + 1) * dimension];
            let json_meta = Some(serde_json::json!({"neuron_id": i}));
            let id = index.insert_vector(vec, json_meta).map_err(hnsw_to_pyerr)?;
            ids.push(id);
        }
        Ok(ids)
    }

    /// Search for k nearest neighbors.
    fn search(&self, py: Python<'_>, query: Vec<f32>, k: usize) -> PyResult<Vec<(u64, f32)>> {
        let parent = self.parent.borrow(py);
        let graph = parent.backend.graph();
        let indexes = graph
            .hnsw_indexes
            .lock()
            .map_err(|e| BackendError::new_err(format!("Mutex poisoned: {}", e)))?;
        let index = indexes
            .get(&self.name)
            .ok_or_else(|| NotFoundError::new_err("Index not found"))?;
        index.search(&query, k).map_err(hnsw_to_pyerr)
    }

    /// Get a stored vector by ID.
    fn get_vector<'py>(
        &self,
        py: Python<'py>,
        vector_id: u64,
    ) -> PyResult<Option<(Vec<f32>, Bound<'py, PyDict>)>> {
        let parent = self.parent.borrow(py);
        let graph = parent.backend.graph();
        let indexes = graph
            .hnsw_indexes
            .lock()
            .map_err(|e| BackendError::new_err(format!("Mutex poisoned: {}", e)))?;
        let index = indexes
            .get(&self.name)
            .ok_or_else(|| NotFoundError::new_err("Index not found"))?;
        let result = index.get_vector(vector_id).map_err(hnsw_to_pyerr)?;
        match result {
            Some((vec, meta)) => {
                let dict = json_to_dict(py, &meta)?;
                Ok(Some((vec, dict)))
            }
            None => Ok(None),
        }
    }

    /// Index name.
    fn name(&self) -> &str {
        &self.name
    }

    /// Get the live vector count from the index.
    fn vector_count(&self, py: Python<'_>) -> PyResult<usize> {
        let parent = self.parent.borrow(py);
        let graph = parent.backend.graph();
        let indexes = graph
            .hnsw_indexes
            .lock()
            .map_err(|e| BackendError::new_err(format!("Mutex poisoned: {}", e)))?;
        let index = indexes
            .get(&self.name)
            .ok_or_else(|| NotFoundError::new_err("Index not found"))?;
        Ok(index.vector_count())
    }
}

// ── Conversion helpers ──────────────────────────────────────────

fn dict_to_json(dict: &Bound<'_, PyDict>) -> PyResult<serde_json::Value> {
    let mut map = serde_json::Map::new();
    for (key, value) in dict.iter() {
        let k: String = key.extract()?;
        let v = py_to_json(&value)?;
        map.insert(k, v);
    }
    Ok(serde_json::Value::Object(map))
}

fn py_to_json(value: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    if let Ok(b) = value.extract::<bool>() {
        return Ok(serde_json::Value::Bool(b));
    }
    if let Ok(i) = value.extract::<i64>() {
        return Ok(serde_json::json!(i));
    }
    if let Ok(f) = value.extract::<f64>() {
        return Ok(serde_json::json!(f));
    }
    if let Ok(s) = value.extract::<String>() {
        return Ok(serde_json::Value::String(s));
    }
    if let Ok(list) = value.cast::<pyo3::types::PyList>() {
        let items: PyResult<Vec<serde_json::Value>> =
            list.iter().map(|item| py_to_json(&item)).collect();
        return Ok(serde_json::Value::Array(items?));
    }
    if let Ok(dict) = value.cast::<PyDict>() {
        return dict_to_json(dict);
    }
    if value.is_none() {
        return Ok(serde_json::Value::Null);
    }
    let s: String = value.str()?.to_string_lossy().into();
    Ok(serde_json::Value::String(s))
}

fn json_to_dict<'py>(py: Python<'py>, value: &serde_json::Value) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                dict.set_item(k, json_to_py(py, v)?)?;
            }
        }
        _ => {
            dict.set_item("value", json_to_py(py, value)?)?;
        }
    }
    Ok(dict)
}

fn json_to_py<'py>(py: Python<'py>, value: &serde_json::Value) -> PyResult<Py<PyAny>> {
    match value {
        serde_json::Value::Null => Ok(py.None()),
        serde_json::Value::Bool(b) => {
            let bound = (*b).into_pyobject(py)?;
            Ok(bound.as_any().to_owned().unbind())
        }
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.into_pyobject(py)?.into_any().unbind())
            } else if let Some(f) = n.as_f64() {
                Ok(f.into_pyobject(py)?.into_any().unbind())
            } else {
                Ok(py.None())
            }
        }
        serde_json::Value::String(s) => Ok(s.into_pyobject(py)?.into_any().unbind()),
        serde_json::Value::Array(arr) => {
            let list = pyo3::types::PyList::empty(py);
            for item in arr {
                list.append(json_to_py(py, item)?)?;
            }
            Ok(list.into_any().unbind())
        }
        serde_json::Value::Object(map) => {
            let dict = PyDict::new(py);
            for (k, v) in map {
                dict.set_item(k, json_to_py(py, v)?)?;
            }
            Ok(dict.into_any().unbind())
        }
    }
}

fn entity_to_dict<'py>(
    py: Python<'py>,
    entity: &sqlitegraph::GraphEntity,
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("id", entity.id)?;
    dict.set_item("kind", &entity.kind)?;
    dict.set_item("name", &entity.name)?;
    if let Some(ref fp) = entity.file_path {
        dict.set_item("file_path", fp)?;
    }
    dict.set_item("data", json_to_dict(py, &entity.data)?)?;
    Ok(dict)
}

fn edge_to_dict<'py>(py: Python<'py>, edge: &GraphEdge) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("id", edge.id)?;
    dict.set_item("from_id", edge.from_id)?;
    dict.set_item("to_id", edge.to_id)?;
    dict.set_item("edge_type", &edge.edge_type)?;
    dict.set_item("data", json_to_dict(py, &edge.data)?)?;
    Ok(dict)
}

/// Module initialization.
#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_class::<Graph>()?;
    m.add_class::<HnswIndexWrapper>()?;
    m.add("GraphError", py.get_type::<GraphError>())?;
    m.add("NotFoundError", py.get_type::<NotFoundError>())?;
    m.add(
        "InvalidArgumentError",
        py.get_type::<InvalidArgumentError>(),
    )?;
    m.add("BackendError", py.get_type::<BackendError>())?;
    #[cfg(feature = "inference")]
    m.add_class::<InferenceEngine>()?;
    Ok(())
}

// ── Graph Inference Engine (dense FFN + HNSW attention) ───────
//
// Behind the `inference` cargo feature; off by default. Pulls in numpy / ndarray.

/// Graph-based inference engine: dense FFN + HNSW graph attention.
///
/// Runs the entire token generation loop in Rust — zero Python boundary
/// crossings per token. Attention HNSW indices are built incrementally
/// during generation (no separate build step).
///
/// Usage:
///   engine = InferenceEngine()
///   engine.set_model_info(n_layers, hidden_dim, ffn_dim, vocab_size, n_heads, n_kv_heads)
///   for layer in range(n_layers):
///       engine.load_layer(layer, attn_norm, ffn_norm, wq, wk, wv, wo, ffn_gate, ffn_up, ffn_down,
///                           bq, bk, bv)
///   engine.set_root_weights(token_embd, output_proj, output_norm)
///   tokens = engine.generate(prompt, max_tokens)
#[cfg(feature = "inference")]
#[pyclass(unsendable)]
pub struct InferenceEngine {
    inner: SparseInferenceEngine,
}

#[cfg(feature = "inference")]
#[pymethods]
impl InferenceEngine {
    /// Create a new inference engine.
    ///
    /// Args:
    ///     temperature: Sampling temperature (default 0.8).
    ///     top_p: Nucleus sampling threshold (default 0.9).
    ///     attn_top_k: Past tokens to attend to per head (default 256).
    ///     rope_base: RoPE base frequency (default 1000000.0 for Qwen2).
    #[new]
    #[pyo3(signature = (temperature=None, top_p=None, attn_top_k=None, rope_base=None))]
    fn new(
        temperature: Option<f32>,
        top_p: Option<f32>,
        attn_top_k: Option<usize>,
        rope_base: Option<f32>,
    ) -> Self {
        let config = RustInferenceConfig {
            temperature: temperature.unwrap_or(0.8),
            top_p: top_p.unwrap_or(0.9),
            attn_top_k: attn_top_k.unwrap_or(256),
            rope_base: rope_base.unwrap_or(1000000.0),
        };
        Self {
            inner: SparseInferenceEngine::new(config),
        }
    }

    /// Set model architecture info.
    #[pyo3(signature = (n_layers, hidden_dim, ffn_dim, vocab_size, n_heads, n_kv_heads))]
    fn set_model_info(
        &mut self,
        n_layers: usize,
        hidden_dim: usize,
        ffn_dim: usize,
        vocab_size: usize,
        n_heads: usize,
        n_kv_heads: usize,
    ) -> PyResult<()> {
        self.inner.set_model_info(
            n_layers, hidden_dim, ffn_dim, vocab_size, n_heads, n_kv_heads,
        );
        Ok(())
    }

    /// Load a single layer's weights from numpy arrays.
    ///
    /// Args:
    ///     layer_idx: Layer index (0-based).
    ///     attn_norm: Pre-attention RMSNorm [hidden_dim] (numpy f32).
    ///     ffn_norm: FFN RMSNorm [hidden_dim] (numpy f32).
    ///     wq: Query projection [hidden_dim, hidden_dim] (numpy f32).
    ///     wk: Key projection [n_kv_dim, hidden_dim] (numpy f32).
    ///     wv: Value projection [n_kv_dim, hidden_dim] (numpy f32).
    ///     wo: Output projection [hidden_dim, hidden_dim] (numpy f32).
    ///     ffn_gate: Gate weights [ffn_dim, hidden_dim] (numpy f32).
    ///     ffn_up: Up weights [ffn_dim, hidden_dim] (numpy f32).
    ///     ffn_down: Down weights [ffn_dim, hidden_dim] (numpy f32).
    ///     bq: Query bias [hidden_dim] or empty array (numpy f32).
    ///     bk: Key bias [n_kv_dim] or empty array (numpy f32).
    ///     bv: Value bias [n_kv_dim] or empty array (numpy f32).
    fn load_layer(
        &mut self,
        py: Python<'_>,
        layer_idx: usize,
        attn_norm: &Bound<'_, PyAny>,
        ffn_norm: &Bound<'_, PyAny>,
        wq: &Bound<'_, PyAny>,
        wk: &Bound<'_, PyAny>,
        wv: &Bound<'_, PyAny>,
        wo: &Bound<'_, PyAny>,
        ffn_gate: &Bound<'_, PyAny>,
        ffn_up: &Bound<'_, PyAny>,
        ffn_down: &Bound<'_, PyAny>,
        bq: &Bound<'_, PyAny>,
        bk: &Bound<'_, PyAny>,
        bv: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let attn_norm = numpy_to_f32_slice(py, attn_norm)?;
        let ffn_norm = numpy_to_f32_slice(py, ffn_norm)?;
        let wq = numpy_to_f32_slice(py, wq)?;
        let wk = numpy_to_f32_slice(py, wk)?;
        let wv = numpy_to_f32_slice(py, wv)?;
        let wo = numpy_to_f32_slice(py, wo)?;
        let ffn_gate = numpy_to_f32_slice(py, ffn_gate)?;
        let ffn_up = numpy_to_f32_slice(py, ffn_up)?;
        let ffn_down = numpy_to_f32_slice(py, ffn_down)?;
        let bq = numpy_to_f32_slice(py, bq)?;
        let bk = numpy_to_f32_slice(py, bk)?;
        let bv = numpy_to_f32_slice(py, bv)?;
        self.inner.load_layer(
            layer_idx, &attn_norm, &ffn_norm, &wq, &wk, &wv, &wo, &ffn_gate, &ffn_up, &ffn_down,
            &bq, &bk, &bv,
        );
        Ok(())
    }

    /// Set root model weights from numpy arrays.
    ///
    /// Args:
    ///     token_embd: Token embeddings [vocab_size, hidden_dim] (numpy f32).
    ///     output_proj: Output projection [vocab_size, hidden_dim] (numpy f32).
    ///     output_norm: Output RMSNorm [hidden_dim] (numpy f32).
    fn set_root_weights(
        &mut self,
        py: Python<'_>,
        token_embd: &Bound<'_, PyAny>,
        output_proj: &Bound<'_, PyAny>,
        output_norm: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let embd = numpy_to_f32_slice(py, token_embd)?;
        let proj = numpy_to_f32_slice(py, output_proj)?;
        let norm = numpy_to_f32_slice(py, output_norm)?;
        self.inner.set_root_weights(&embd, &proj, &norm);
        Ok(())
    }

    /// Generate tokens using graph-based inference.
    ///
    /// The entire loop runs in Rust — prompt processing + autoregressive generation.
    /// Attention HNSW indices are built incrementally as tokens are processed.
    ///
    /// Args:
    ///     prompt_tokens: List of token IDs.
    ///     max_tokens: Maximum tokens to generate (after prompt).
    ///
    /// Returns:
    ///     Dict with keys: tokens (list[int]), stats (dict).
    fn generate(
        &mut self,
        py: Python<'_>,
        prompt_tokens: Vec<u64>,
        max_tokens: usize,
    ) -> PyResult<Py<PyAny>> {
        let (tokens, stats) = self.inner.generate(&prompt_tokens, max_tokens);

        let stats_dict = PyDict::new(py);
        stats_dict.set_item("prompt_tokens_processed", stats.prompt_tokens_processed)?;
        stats_dict.set_item("tokens_generated", stats.tokens_generated)?;
        stats_dict.set_item("total_time_s", stats.total_time_s)?;
        stats_dict.set_item("tokens_per_sec", stats.tokens_per_sec)?;
        stats_dict.set_item("avg_token_time_ms", stats.avg_token_time_ms)?;
        stats_dict.set_item("first_token_ms", stats.first_token_ms)?;
        stats_dict.set_item("avg_layer_time_ms", stats.avg_layer_time_ms)?;
        stats_dict.set_item("avg_attn_time_ms", stats.avg_attn_time_ms)?;
        stats_dict.set_item("avg_ffn_time_ms", stats.avg_ffn_time_ms)?;

        let result = PyDict::new(py);
        result.set_item("tokens", tokens)?;
        result.set_item("stats", stats_dict)?;

        Ok(result.into_any().unbind())
    }
}

/// Extract raw f32 data from a numpy array as a Vec<f32>.
#[cfg(feature = "inference")]
fn numpy_to_f32_slice(_py: Python<'_>, array: &Bound<'_, PyAny>) -> PyResult<Vec<f32>> {
    let bytes_obj = array.call_method0("tobytes")?;
    let bytes: &[u8] = bytes_obj.cast::<pyo3::types::PyBytes>()?.as_bytes();
    if bytes.len() % 4 != 0 {
        return Err(InvalidArgumentError::new_err(format!(
            "numpy array size {} not divisible by 4 (f32)",
            bytes.len()
        )));
    }
    let n_floats = bytes.len() / 4;
    let float_slice: &[f32] =
        unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const f32, n_floats) };
    Ok(float_slice.to_vec())
}
