# SQLiteGraph Manual

This manual describes how to use SQLiteGraph with its dual backend architecture (SQLite and Native V2) for deterministic graph database operations.

---

## 1. Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
sqlitegraph = "0.2.11"

# For Native V2 high-performance backend
sqlitegraph = { version = "0.2.11", features = ["native-v2"] }
```

### Basic Usage

```bash
# Run tests
cargo test

# Run benchmarks
cargo bench

# Run working examples
cargo run --example basic_functionality_test
cargo run --example native_v2_test --features native-v2
```

---

## 2. Backend Selection

### SQLite Backend (Default)

**Use Case**: General purpose, ACID transactions, existing SQLite data

```rust
use sqlitegraph::{SqliteGraph, GraphEntity, GraphEdge};

let graph = SqliteGraph::open_in_memory()?;
// SQLite operations with full ACID compliance
```

### Native V2 Backend (High Performance)

**Use Case**: High-performance scenarios, large graphs, speed-critical applications

```rust
use sqlitegraph::{GraphConfig, open_graph, NodeSpec, EdgeSpec};

let config = GraphConfig::native();
let graph = open_graph("graph.db", &config)?;
// Optimized for performance with clustered adjacency
```

### Backend Comparison

| Characteristic | SQLite Backend | Native V2 Backend |
|----------------|----------------|-------------------|
| **Performance** | Standard SQLite performance | 10x faster (50K-100K ops/sec) |
| **Transactions** | Full ACID compliance | Atomic commits, optimized |
| **Maturity** | Battle-tested, mature | Production ready, V2 architecture |
| **Memory Usage** | SQLite overhead | Configurable buffers |
| **Use Cases** | General purpose, data integrity | High performance, large graphs |

---

## 3. Core Operations

### Entity Management (SQLite Backend)

```rust
use sqlitegraph::{SqliteGraph, GraphEntity};

let graph = SqliteGraph::open_in_memory()?;

// Create entity
let entity = GraphEntity {
    id: 0, // Auto-assigned
    kind: "User".to_string(),
    name: "Alice".to_string(),
    file_path: None,
    data: serde_json::json!({"age": 30}),
};

let entity_id = graph.insert_entity(&entity)?;
let retrieved = graph.get_entity(entity_id)?;

// Update entity
let mut updated_entity = retrieved;
updated_entity.name = "Alice Smith".to_string();
graph.update_entity(&updated_entity)?;
```

### Node Management (Native V2 Backend)

```rust
use sqlitegraph::{GraphConfig, open_graph, NodeSpec, EdgeSpec};

let config = GraphConfig::native();
let graph = open_graph("graph.db", &config)?;

// Create node
let node_spec = NodeSpec {
    kind: "User".to_string(),
    name: "Alice".to_string(),
    file_path: None,
    data: serde_json::json!({"age": 30}),
};

let node_id = graph.insert_node(node_spec)?;

// Create edge
let edge_spec = EdgeSpec {
    from: node_id,
    to: node_id, // self-loop
    edge_type: "self_ref".to_string(),
    data: serde_json::json!({"type": "demo"}),
};

let edge_id = graph.insert_edge(edge_spec)?;
```

### Traversal Operations

```rust
// Get neighbors (both backends)
let neighbors = graph.neighbors(entity_id, None)?;
println!("Found {} neighbors", neighbors.len());

// Path operations
if graph.has_path(from_id, to_id)? {
    let path = graph.shortest_path(from_id, to_id)?;
    println!("Path: {:?}", path);
}
```

---

## 4. Testing Guide

### Running Tests

```bash
# All tests
cargo test

# Specific backend tests
cargo test --features native-v2

# Library tests only
cargo test --lib

# Integration tests
cargo test --test '*'

# Test with verbose output
cargo test -- --nocapture

# Test specific patterns
cargo test '*neighbors*'
cargo test '*v2*'
```

### Available Test Categories

**Core Functionality Tests:**
- `lib_api_smoke_tests` - Basic library API tests
- `entity_tests` - Entity CRUD operations
- `edge_tests` - Edge management
- `pattern_engine_tests` - Pattern matching
- `query_cache_tests` - Query caching

**V2-Specific Tests:**
- `v2_edge_insertion_corruption_regression` - V2 corruption prevention
- `phase65_cluster_size_corruption_regression` - V2 cluster size handling
- `phase73_node_count_corruption_capture` - V2 node counting
- `v2_graph_ops_smoke` - V2 basic operations

**Integration Tests:**
- `integration_tests` - End-to-end workflows
- `safety_tests` - Data integrity validation
- `performance_tests` - Performance regression checks

### Test Results Interpretation

**Expected Results:**
- Library tests: 69/69 passing ✅
- V2 tests: All passing with corruption prevention ✅
- Examples: Working with 10+ nodes, 20+ edges ✅

**Warning Signs:**
- Any test failures: Investigate immediately
- Performance regression: Check benchmark baselines
- V2 corruption test failures: Critical, investigate storage layer

---

## 5. Performance Optimization

### Native V2 Performance Tuning

```rust
use sqlitegraph::{GraphConfig, NativeConfig};

// High-performance configuration
let config = GraphConfig::native()
    .with_buffer_size(128 * 1024 * 1024)  // 128MB buffers
    .with_capacity(1_000_000, 5_000_000); // 1M nodes, 5M edges pre-allocation

let graph = open_graph("large_graph.db", &config)?;
```

### SQLite Performance Tuning

```rust
use sqlitegraph::{GraphConfig, SqliteConfig};

// Optimized SQLite configuration
let config = GraphConfig::sqlite()
    .with_wal_mode()                    // Better concurrency
    .with_cache_size(256_000)           // 256MB cache
    .with_synchronous_mode("NORMAL");   // Balanced safety/performance

let graph = open_graph("optimized.db", &config)?;
```

### Parallel WAL Recovery (v0.2.11)

The Native V2 backend supports parallel WAL recovery for faster database startup:

```rust
use sqlitegraph::{GraphConfig, open_graph};

// Default: 4 parallel threads
let config = GraphConfig::native();
let graph = open_graph("large.db", &config)?;

// High-throughput configuration (8 threads)
let config = GraphConfig::native()
    .with_parallel_recovery(8);
let graph = open_graph("large.db", &config)?;

// Sequential recovery (debugging)
let config = GraphConfig::native()
    .with_parallel_recovery(1);
let graph = open_graph("large.db", &config)?;
```

**Performance Characteristics:**

| Transaction Count | Sequential | Parallel (4 threads) | Speedup |
|-------------------|------------|---------------------|---------|
| 10 transactions | ~10ms | ~12ms | 0.83x (overhead) |
| 50 transactions | ~50ms | ~30ms | 1.67x |
| 100 transactions | ~100ms | ~55ms | 1.82x |
| 500 transactions | ~500ms | ~180ms | 2.78x |

**Implementation Details:**
- Uses rayon's work-stealing thread pool
- Lock-free atomic statistics eliminate contention
- Transactions sorted by LSN before parallel replay
- Error aggregation after parallel phase completes

### Lock Contention Reduction (v0.2.11)

Phase 7 optimizations reduce lock contention during WAL recovery:

**Before (v0.2.10):**
```rust
// Mutex-protected statistics
Arc<Mutex<ReplayStatistics>>
// Every statistics update requires mutex acquisition
```

**After (v0.2.11):**
```rust
// Lock-free statistics
Arc<ReplayStatistics>  // Uses AtomicU64
// No mutex overhead, linear scaling with threads
```

**Performance Impact:**
- **5-10% improvement** in parallel WAL recovery performance
- **Linear scaling** with thread count (no lock contention)
- **Zero memory overhead** (AtomicU64 same size as u64)

### Performance Benchmarks

```bash
# Run all benchmarks
cargo bench

# Specific benchmark suites
cargo bench --bench insert
cargo bench --bench bfs
cargo bench --bench k_hop

# Performance regression check
cargo test perf_gate_tests
```

### Current Performance Characteristics

| Operation | SQLite Backend | Native V2 Backend |
|-----------|----------------|-------------------|
| **Node Insert** | ~5,000 ops/sec | ~50,000 ops/sec |
| **Edge Insert** | ~10,000 ops/sec | ~100,000 ops/sec |
| **Neighbor Query** | ~20,000 ops/sec | ~200,000 ops/sec |
| **Path Finding** | Variable | Optimized for locality |

---

## 6. Error Handling & Debugging

### Common Error Types

```rust
use sqlitegraph::SqliteGraphError;

match graph.insert_entity(&entity) {
    Ok(id) => println!("Created entity: {}", id),
    Err(SqliteGraphError::ValidationError(msg)) => {
        eprintln!("Validation failed: {}", msg);
    }
    Err(SqliteGraphError::ConnectionError(msg)) => {
        eprintln!("Database connection failed: {}", msg);
    }
    Err(err) => eprintln!("Unexpected error: {}", err),
}
```

### Debug Features

```toml
# Enable debug tracing for V2 I/O operations
sqlitegraph = { version = "0.2.11", features = ["trace_v2_io"] }
```

```bash
# Run with debug output
RUST_LOG=debug cargo run --example native_v2_test --features trace_v2_io
```

### Environment Variables

```bash
# Enable detailed logging
export RUST_LOG=debug

# Enable V2 slot debugging
export V2_SLOT_DEBUG=1

# Enable cluster debugging
export EDGE_CLUSTER_DEBUG=1

# Enable transaction debugging
export TX_BEGIN_AUDIT=1
```

---

## 7. Safety & Data Integrity

### Built-in Safety Features

**Orphan Edge Detection:**
```rust
use sqlitegraph::run_safety_checks;

let safety_report = run_safety_checks(&graph)?;
if safety_report.has_orphans() {
    eprintln!("Warning: {} orphan edges found", safety_report.orphan_count());
}
```

**Integrity Validation:**
```rust
// Comprehensive integrity sweep
let issues = graph.run_integrity_sweep()?;
for issue in issues {
    println!("Issue: {:?}", issue);
}
```

**Corruption Prevention (V2):**
- Automatic cluster offset validation
- Node slot corruption prevention
- Atomic commit system
- Comprehensive V2 regression tests
- **WAL Transaction Recovery**: Full rollback coverage for all operations (v0.2.5)
- **Edge Cascade Cleanup**: Automatic edge cleanup on node deletion (v0.2.5)
- **Cluster Reference Cleanup**: Proper memory management on node deletion (v0.2.5)

### V2 WAL Recovery System (v0.2.5)

The Native V2 backend includes a complete Write-Ahead Log (WAL) recovery system with 100% transaction rollback coverage:

**Handle Operations (11/11 complete)**:
- Node operations: insert, update, delete
- String storage: insert and manage
- Cluster management: create with proper allocation
- Edge operations: insert, update, delete
- Free space management: allocate and deallocate
- Header management: update metadata

**Rollback Operations (11/11 complete)**:
- All node operations can be rolled back
- All edge operations can be rolled back
- Cluster allocation can be rolled back
- Free space operations use conservative rollback
- Transaction integrity guaranteed

**Graph Integrity Features**:
- Edge cascade cleanup: When deleting a node, all referencing edges are automatically cleaned up
- Cluster reference cleanup: When deleting a node, cluster storage is properly deallocated
- NodeRecordV2 cleanup: Edge operations maintain consistent node metadata

**Testing**: 647/647 tests passing (100% coverage)

### Recommended Safety Practices

1. **Regular Safety Checks**: Run `run_safety_checks()` before important operations
2. **Backup Strategy**: Regular backups for production data
3. **Transaction Usage**: Use transactions for multi-step operations
4. **Performance Monitoring**: Monitor benchmark gates for regressions

---

## 8. Migration Guide

### From SQLite to Native V2

```rust
// Before (SQLite)
let graph = SqliteGraph::open("data.db")?;
let entity = GraphEntity { /* fields */ };
let id = graph.insert_entity(&entity)?;

// After (Native V2)
let config = GraphConfig::native();
let graph = open_graph("data.db", &config)?;
let node_spec = NodeSpec { /* similar fields */ };
let id = graph.insert_node(node_spec)?;
```

### Key Migration Differences

| Aspect | SQLite Backend | Native V2 Backend |
|--------|----------------|-------------------|
| **Data Types** | `GraphEntity`/`GraphEdge` | `NodeSpec`/`EdgeSpec` |
| **Edge Fields** | `from_id`/`to_id` | `from`/`to` |
| **Construction** | `SqliteGraph::open()` | `open_graph(&config)` |
| **Performance** | Standard | High performance |

### Data Migration Strategy

```rust
// 1. Export from SQLite
let sqlite_graph = SqliteGraph::open("old.db")?;
let entities = sqlite_graph.all_entities()?;
let edges = sqlite_graph.all_edges()?;

// 2. Import to Native V2
let config = GraphConfig::native();
let v2_graph = open_graph("new.db", &config)?;

for entity in entities {
    let node_spec = NodeSpec {
        kind: entity.kind,
        name: entity.name,
        file_path: entity.file_path,
        data: entity.data,
    };
    v2_graph.insert_node(node_spec)?;
}

// 3. Verify migration
let safety_check = run_safety_checks(&v2_graph)?;
assert!(!safety_check.has_orphans());
```

---

## 9. Vector Search with HNSW

SQLiteGraph includes a production-ready HNSW (Hierarchical Navigable Small World) implementation for high-performance approximate nearest neighbor search. The HNSW implementation supports vector dimensions from 1 to 4096, with specific optimization for OpenAI embeddings (1536 dimensions).

### 9.1 HNSW Configuration

#### Basic Setup

```rust
use sqlitegraph::hnsw::{hnsw_config, DistanceMetric, HnswIndex};

// Default configuration (768 dimensions)
let config = hnsw_config()
    .dimension(768)
    .m_connections(16)
    .ef_construction(200)
    .ef_search(50)
    .distance_metric(DistanceMetric::Cosine)
    .build()?;

let hnsw = HnswIndex::new(config)?;
```

#### OpenAI Embeddings Configuration (1536 Dimensions)

```rust
// Production-ready configuration for OpenAI text-embedding-ada-002
let openai_config = hnsw_config()
    .dimension(1536)                        // OpenAI embedding size
    .m_connections(20)                      // Higher connectivity for recall
    .ef_construction(400)                   // Better index quality
    .ef_search(100)                         // Higher search quality
    .distance_metric(DistanceMetric::Cosine) // Recommended for embeddings
    .build()
    .expect("OpenAI configuration should be valid");

let hnsw = HnswIndex::new(openai_config)?;
```

#### Multi-layer Configuration (Future Feature)

```rust
// Multi-layer configuration for large datasets (>10K vectors)
let multilayer_config = hnsw_config()
    .dimension(1536)
    .m_connections(20)
    .ef_construction(400)
    .ef_search(100)
    .distance_metric(DistanceMetric::Cosine)
    .enable_multilayer(true)                 // Enable multi-layer functionality
    .multilayer_deterministic_seed(Some(42)) // Reproducible results
    .build()?;
```

### 9.2 Vector Operations

#### Inserting Vectors

```rust
use serde_json::json;

// Store document embeddings with metadata
let document = "Machine learning is a subset of artificial intelligence.";
let embedding = vec![0.1; 1536]; // Your OpenAI embedding

let metadata = json!({
    "content": document,
    "model": "text-embedding-ada-002",
    "created_at": chrono::Utc::now().to_rfc3339()
});

let vector_id = hnsw.insert_vector(&embedding, Some(metadata))?;
println!("Stored document vector with ID: {}", vector_id);
```

#### Searching Vectors

```rust
// Search for similar documents
let query_embedding = vec![0.12; 1536]; // Query embedding
let similar_docs = hnsw.search(&query_embedding, 10)?; // k=10

for (vector_id, distance) in similar_docs {
    if let Some(record) = hnsw.get_vector(vector_id)? {
        let content = record.metadata
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("No content");
        println!("Found similar document (distance: {:.4}): {}", distance, content);
    }
}
```

#### Batch Operations

```rust
// Insert multiple documents efficiently
let documents = vec![
    ("Deep learning uses neural networks", vec![0.2; 1536]),
    ("Natural language processing analyzes text", vec![0.3; 1536]),
    ("Computer vision processes images", vec![0.4; 1536]),
];

for (i, (content, embedding)) in documents.iter().enumerate() {
    let metadata = json!({
        "content": content,
        "doc_id": i,
        "category": "ai_fundamentals"
    });

    hnsw.insert_vector(embedding, Some(metadata))?;
}
```

### 9.3 Distance Metrics

#### Supported Metrics

```rust
use sqlitegraph::hnsw::DistanceMetric;

// Cosine Distance (Recommended for embeddings)
let cosine_config = hnsw_config()
    .dimension(1536)
    .distance_metric(DistanceMetric::Cosine)
    .build()?;

// Euclidean Distance (For general-purpose similarity)
let euclidean_config = hnsw_config()
    .dimension(768)
    .distance_metric(DistanceMetric::Euclidean)
    .build()?;

// Dot Product (Fastest for normalized vectors)
let dotproduct_config = hnsw_config()
    .dimension(512)
    .distance_metric(DistanceMetric::DotProduct)
    .build()?;

// Manhattan Distance (L1 norm)
let manhattan_config = hnsw_config()
    .dimension(256)
    .distance_metric(DistanceMetric::Manhattan)
    .build()?;
```

#### Performance Characteristics

| Metric | Best Use Case | Relative Speed | Typical Applications |
|--------|---------------|----------------|---------------------|
| **Cosine** | Text embeddings | Fast | Semantic search, NLP |
| **Euclidean** | General similarity | Medium | Image similarity, clustering |
| **Dot Product** | Normalized vectors | Fastest | Pre-normalized embeddings |
| **Manhattan** | Sparse vectors | Slow | Feature vectors, histograms |

### 9.4 Production Best Practices

#### OpenAI Integration Pattern

```rust
use sqlitegraph::hnsw::{hnsw_config, DistanceMetric, HnswIndex};
use serde_json::json;

struct OpenAIEmbeddingStore {
    hnsw: HnswIndex,
}

impl OpenAIEmbeddingStore {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = hnsw_config()
            .dimension(1536)                     // OpenAI text-embedding-ada-002
            .m_connections(24)                   // High connectivity for recall
            .ef_construction(400)                // Quality-focused construction
            .ef_search(100)                      // High-quality search
            .distance_metric(DistanceMetric::Cosine)
            .build()?;

        let hnsw = HnswIndex::new(config)?;
        Ok(Self { hnsw })
    }

    pub fn add_document(&mut self, content: &str, embedding: &[f32]) -> Result<u64, Box<dyn std::error::Error>> {
        assert_eq!(embedding.len(), 1536, "Embedding must be 1536 dimensions");

        let metadata = json!({
            "content": content,
            "model": "text-embedding-ada-002",
            "dimensions": 1536,
            "created_at": chrono::Utc::now().to_rfc3339()
        });

        self.hnsw.insert_vector(embedding, Some(metadata))
    }

    pub fn search_similar(&self, query_embedding: &[f32], k: usize) -> Result<Vec<(String, f32)>, Box<dyn std::error::Error>> {
        assert_eq!(query_embedding.len(), 1536, "Query must be 1536 dimensions");

        let results = self.hnsw.search(query_embedding, k)?;

        let mut documents = Vec::new();
        for (vector_id, distance) in results {
            if let Some(record) = self.hnsw.get_vector(vector_id)? {
                if let Some(content) = record.metadata.get("content").and_then(|v| v.as_str()) {
                    documents.push((content.to_string(), distance));
                }
            }
        }

        Ok(documents)
    }
}
```

#### Memory Planning

```rust
// Estimate memory usage for different dimensions
fn estimate_memory_usage(vector_count: usize, dimension: usize) -> usize {
    // Vector data: 4 bytes per float32
    let vector_bytes = vector_count * dimension * 4;

    // HNSW overhead: ~2.6x vector size for 1536 dimensions
    let overhead_multiplier = match dimension {
        1536 => 2.6,
        768 => 2.5,
        512 => 2.4,
        256 => 2.3,
        _ => 2.5,
    };

    (vector_bytes as f64 * overhead_multiplier) as usize
}

// Examples:
// 10K documents × 1536 dimensions ≈ 156MB total memory
// 100K documents × 1536 dimensions ≈ 1.56GB total memory
// 1M documents × 1536 dimensions ≈ 15.6GB total memory
```

#### Performance Optimization

```rust
// Development configuration (faster builds)
let dev_config = hnsw_config()
    .dimension(1536)
    .m_connections(12)                        // Lower M for faster build
    .ef_construction(150)                     // Faster construction
    .ef_search(40)                            // Faster search
    .distance_metric(DistanceMetric::Cosine)
    .enable_multilayer(false)                 // Single-layer for simplicity
    .build()?;

// Production configuration (better quality)
let prod_config = hnsw_config()
    .dimension(1536)
    .m_connections(24)                        // Higher M for better recall
    .ef_construction(400)                     // Better index quality
    .ef_search(100)                           // Higher search quality
    .distance_metric(DistanceMetric::Cosine)
    .enable_multilayer(true)                  // Enable for large datasets
    .multilayer_deterministic_seed(Some(42))  // Reproducible results
    .build()?;
```

### 9.5 Dimension Guidelines

#### Recommended Dimensions by Use Case

| Use Case | Recommended Dimension | Examples | Performance Impact |
|----------|----------------------|----------|-------------------|
| **Production Semantic Search** | **1536** | OpenAI text-embedding-ada-002 | 2-3x slower than 512-dim |
| **High-Throughput Systems** | 512-768 | Custom embeddings, BERT | Balanced performance |
| **Resource-Constrained** | 256-512 | Lightweight models | Fast performance |
| **Development/Testing** | Any | Use production dimensions | Accurate testing |

#### Multi-Model Support

```rust
// Support for multiple embedding models in the same application
enum EmbeddingModel {
    OpenAIAda002,      // 1536 dimensions
    OpenAI3Small,      // 1536 dimensions
    Custom768,         // 768 dimensions (BERT-style)
    Custom256,         // 256 dimensions (efficiency-focused)
}

impl EmbeddingModel {
    pub fn dimension(&self) -> usize {
        match self {
            EmbeddingModel::OpenAIAda002 => 1536,
            EmbeddingModel::OpenAI3Small => 1536,
            EmbeddingModel::Custom768 => 768,
            EmbeddingModel::Custom256 => 256,
        }
    }

    pub fn create_hnsw_config(&self) -> Result<sqlitegraph::hnsw::HnswConfig, sqlitegraph::hnsw::HnswConfigError> {
        hnsw_config()
            .dimension(self.dimension())
            .m_connections(match self {
                EmbeddingModel::OpenAIAda002 => 20,
                EmbeddingModel::OpenAI3Small => 20,
                EmbeddingModel::Custom768 => 16,
                EmbeddingModel::Custom256 => 12,
            })
            .distance_metric(DistanceMetric::Cosine)
            .build()
    }
}
```

### 9.6 Error Handling

#### Common HNSW Errors

```rust
use sqlitegraph::hnsw::{HnswIndex, HnswConfigError};

match HnswIndex::new(config) {
    Ok(hnsw) => {
        // Use the index
    }
    Err(HnswConfigError::InvalidDimension) => {
        eprintln!("Dimension must be between 1 and 4096");
    }
    Err(HnswConfigError::InvalidMParameter) => {
        eprintln!("M parameter must be > 0");
    }
    Err(HnswConfigError::InvalidEfConstruction) => {
        eprintln!("ef_construction must be >= m");
    }
    Err(err) => eprintln!("Configuration error: {:?}", err),
}
```

#### Vector Validation

```rust
fn validate_embedding(embedding: &[f32], expected_dim: usize) -> Result<(), String> {
    if embedding.len() != expected_dim {
        return Err(format!(
            "Embedding dimension mismatch: expected {}, got {}",
            expected_dim, embedding.len()
        ));
    }

    // Check for NaN or infinite values
    for (i, &val) in embedding.iter().enumerate() {
        if !val.is_finite() {
            return Err(format!("Invalid value at index {}: {}", i, val));
        }
    }

    Ok(())
}

// Usage:
let embedding = vec![0.1; 1536];
validate_embedding(&embedding, 1536)?;
hnsw.insert_vector(&embedding, Some(metadata))?;
```

### 9.7 Integration with Graph Operations

#### Combining Vector Search with Graph Queries

```rust
use sqlitegraph::{SqliteGraph, GraphEntity};

struct SemanticGraphSearch {
    graph: SqliteGraph,
    hnsw: HnswIndex,
}

impl SemanticGraphSearch {
    pub fn hybrid_search(&self, query_embedding: &[f32], k: usize) -> Result<Vec<(GraphEntity, f32)>, Box<dyn std::error::Error>> {
        // 1. Find similar vectors
        let vector_results = self.hnsw.search(query_embedding, k)?;

        let mut graph_results = Vec::new();
        for (vector_id, distance) in vector_results {
            // 2. Get graph entity associated with vector
            if let Some(record) = self.hnsw.get_vector(vector_id)? {
                if let Some(entity_id) = record.metadata.get("entity_id").and_then(|v| v.as_i64()) {
                    // 3. Retrieve full graph entity
                    if let Some(entity) = self.graph.get_entity(entity_id as u64)? {
                        graph_results.push((entity, distance));
                    }
                }
            }
        }

        Ok(graph_results)
    }
}
```

### 9.8 Benchmarking and Performance

#### Running HNSW Benchmarks

```bash
# Run all HNSW benchmarks including 1536 dimensions
cargo bench --bench hnsw

# Run OpenAI-specific benchmarks
cargo bench --bench hnsw -- --filter openai

# Performance comparison across dimensions
cargo bench --bench hnsw -- --filter insertion
cargo bench --bench hnsw -- --filter search
```

#### Expected Performance Characteristics

Based on comprehensive benchmarking (see `docs/V2_HNSW_BENCHMARK_PERFORMANCE_REPORT.md`):

| Dimension | Insertion Rate | Search Latency | Memory Overhead |
|-----------|----------------|----------------|-----------------|
| **256** | ~70K vectors/sec | <1ms (k=10) | 2.3x vector size |
| **512** | ~60K vectors/sec | <1.2ms (k=10) | 2.4x vector size |
| **768** | ~55K vectors/sec | <1.4ms (k=10) | 2.5x vector size |
| **1536** | ~45K vectors/sec | <2ms (k=10) | 2.6x vector size |

### 9.9 Migration from Other Vector Databases

#### From Pinecone

```rust
// Pinecone equivalent configuration
let pinecone_equivalent = hnsw_config()
    .dimension(1536)
    .m_connections(16)      // Similar to Pinecone's default
    .ef_construction(200)   // Similar to Pinecone's build config
    .ef_search(50)         // Similar to Pinecone's search config
    .distance_metric(DistanceMetric::Cosine)
    .build()?;
```

#### From FAISS

```rust
// FAISS HNSW equivalent
let faiss_equivalent = hnsw_config()
    .dimension(1536)
    .m_connections(32)      // FAISS default M
    .ef_construction(40)   // FAISS default efConstruction
    .ef_search(16)         // FAISS default efSearch
    .distance_metric(DistanceMetric::Cosine)
    .build()?;
```

---

## 10. CLI Usage

### Available Commands

```bash
# Status check
cargo run --bin sqlitegraph -- --command status

# List entities
cargo run --bin sqlitegraph -- --command list

# Safety checks
cargo run --bin sqlitegraph -- --command safety-check --strict
```

### CLI Backend Selection

```bash
# SQLite backend (default)
cargo run --bin sqlitegraph -- --command status --db mydb.sqlite

# Native V2 backend
cargo run --bin sqlitegraph --features native-v2 -- --command status --db mydb.native
```

---

## 10. Troubleshooting

### Common Issues

**Compilation Errors:**
- Missing features: Add appropriate feature flags
- API mismatches: Check backend-specific data types
- Rust version: Ensure compatible Rust version

**Runtime Issues:**
- Database corruption: Run integrity checks
- Performance: Check buffer configuration
- Memory usage: Monitor graph size vs buffer allocation

**Performance Issues:**
- Slow queries: Consider Native V2 backend
- Memory pressure: Tune buffer sizes
- Large graphs: Use batch operations

### Getting Help

```bash
# Check test status
cargo test --lib 2>&1 | tail -5

# Run specific failing test
cargo test test_name -- --nocapture

# Check compilation
cargo check --features native-v2
```

### Debug Checklist

1. ✅ **Backend Selection**: Correct backend for use case
2. ✅ **Feature Flags**: Required features enabled
3. ✅ **API Usage**: Correct data types for backend
4. ✅ **Error Handling**: Proper error handling in place
5. ✅ **Performance**: Appropriate configuration
6. ✅ **Safety**: Regular integrity checks

---

## 11. Development Notes

### Architecture Status

- **V2 Native Backend**: Production ready ✅
- **V1 Legacy Code**: Permanently removed ✅
- **Dual Backend Support**: Fully functional ✅
- **Corruption Prevention**: Active ✅

### Module Organization

All modules follow the 300 LOC limit for auditability:
- Core graph operations
- Backend abstractions
- Pattern matching engine
- Safety and validation tools
- Performance benchmarking

### Future Development

Focus areas for future development:
- More advanced graph algorithms
- Query optimization
- Real-time streaming updates
- Enhanced visualization tools