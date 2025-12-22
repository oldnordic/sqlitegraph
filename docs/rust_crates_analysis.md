# Rust Crates Analysis for SQLiteGraph Query Optimization

## Current Rust Ecosystem for Query Optimization (2025)

### 1. Query Planning and Execution Crates

#### 1.1 SQL Parser and AST

**sqlparser-rs** (v0.38.0)
```toml
[dependencies]
sqlparser = "0.38"
features = ["visitor"]
```

```rust
use sqlparser::{ast::*, parser::Parser};

// SQLiteGraph extension to support graph-specific syntax
#[derive(Debug, Clone)]
enum GraphStatement {
    Select(Select),
    GraphMatch(GraphMatchPattern),
    VectorSearch(VectorQuery),
    Explain(ExplainStatement),
}

#[derive(Debug, Clone)]
struct GraphMatchPattern {
    pattern: GraphPattern,
    where_clause: Option<Expr>,
    return_items: Vec<SelectItem>,
    order_by: Vec<OrderByExpr>,
    limit: Option<Expr>,
    offset: Option<Expr>,
}

#[derive(Debug, Clone)]
enum GraphPattern {
    Path(NodePattern, EdgePattern, NodePattern),
    Variable(String),
    Filter(Expr),
    Optional(Box<GraphPattern>),
    Named(String, Box<GraphPattern>),
}

impl GraphStatement {
    pub fn parse(sql: &str) -> Result<Self, ParseError> {
        // Extend sqlparser to handle graph patterns
        let statements = Parser::parse_sql(sql)?;

        if statements.len() != 1 {
            return Err(ParseError::TooManyStatements);
        }

        match statements.into_iter().next().unwrap() {
            Statement::Query(query) => {
                // Check for graph-specific keywords
                if sql.to_uppercase().contains("GRAPH MATCH") {
                    self.parse_graph_match(sql)
                } else if sql.to_uppercase().contains("VECTOR SEARCH") {
                    self.parse_vector_search(sql)
                } else {
                    Ok(GraphStatement::Select(*query))
                }
            },
            _ => Err(ParseError::UnsupportedStatement)
        }
    }
}
```

**datafusion** (v40.0.0)
```toml
[dependencies]
datafusion = "40.0"
features = ["simd"]
```

```rust
use datafusion::{
    arrow::datatypes::SchemaRef,
    common::{DataFusionError, Result},
    execution::{context::SessionContext, runtime_env::RuntimeEnv},
    logical_expr::{LogicalPlan, Expr},
    optimizer::{optimizer::OptimizerRule, OptimizerConfig},
};

struct SQLiteGraphOptimizer {
    graph_metadata: GraphMetadata,
    vector_index: Option<VectorIndexHandle>,
}

impl OptimizerRule for SQLiteGraphOptimizer {
    fn try_optimize(
        &self,
        plan: &LogicalPlan,
        config: &dyn OptimizerConfig,
    ) -> Result<Option<LogicalPlan>> {
        match plan {
            LogicalPlan::Filter(filter) => {
                // Check if filter can be pushed to graph index
                if let Some(graph_filter) = self.extract_graph_filter(&filter.predicate) {
                    let optimized = self.apply_graph_index_filter(plan, graph_filter)?;
                    return Ok(Some(optimized));
                }
            },
            LogicalPlan::Join(join) => {
                // Check if this is a graph traversal join
                if let Some(graph_join) = self.recognize_graph_traversal(join) {
                    let optimized = self.apply_graph_traversal_optimization(join, graph_join)?;
                    return Ok(Some(optimized));
                }
            },
            _ => {}
        }

        // Apply graph-specific optimizations recursively
        self.optimize_inputs(plan, config)
    }
}
```

#### 1.2 Cost-Based Optimization

**ordinals** (v0.3.0) - For cost estimation
```toml
[dependencies]
ordinals = "0.3"
```

**statrs** (v0.16.0) - For statistical models
```toml
[dependencies]
statrs = "0.16"
```

```rust
use statrs::distribution::{Continuous, Normal};
use statrs::statistics::Mean;

struct AdvancedCostModel {
    // Statistical models for cardinality estimation
    attribute_distributions: HashMap<String, Normal>,
    correlation_matrix: HashMap<String, HashMap<String, f64>>,
    sampling_cache: LruCache<QueryPattern, Vec<f64>>,
}

impl AdvancedCostModel {
    fn estimate_join_selectivity(&self,
                               left_attr: &str,
                               right_attr: &str) -> f64 {
        // Use correlation information to improve estimates
        let correlation = self.correlation_matrix
            .get(left_attr)
            .and_then(|m| m.get(right_attr))
            .unwrap_or(&0.0);

        // Adjust selectivity based on correlation
        let base_selectivity = 1.0 / self.get_distinct_count(left_attr) as f64;
        base_selectivity * (1.0 + correlation.abs())
    }

    fn update_distribution(&mut self, attr: &str, samples: &[f64]) {
        // Update statistical model from samples
        if let Ok(dist) = Normal::from_params(
            samples.iter().sum::<f64>() / samples.len() as f64,
            Self::calculate_std_dev(samples)
        ) {
            self.attribute_distributions.insert(attr.to_string(), dist);
        }
    }
}
```

### 2. Vector Search and ANN Libraries

#### 2.1 faiss-rs - Facebook AI Similarity Search
```toml
[dependencies]
faiss = { version = "0.12", features = ["static"] }
```

```rust
use faiss::{Index, IndexFlatIP, IndexIVFFlat, MetricType};

struct FaissVectorIndex {
    inner: Box<dyn Index>,
    nlist: usize,
    nprobe: usize,
}

impl FaissVectorIndex {
    fn new(dim: usize, nlist: usize) -> Self {
        // Create a quantizer
        let quantizer = IndexFlatIP::new(dim);

        // Create IVF index
        let ivf_index = IndexIVFFlat::new(
            quantizer,
            dim,
            nlist,
            MetricType::InnerProduct,
        );

        // Train the index
        ivf_index.train(&training_vectors);

        FaissVectorIndex {
            inner: Box::new(ivf_index),
            nlist,
            nprobe: 40,
        }
    }

    fn search(&self, query: &[f32], k: usize) -> (Vec<f64>, Vec<usize>) {
        // Set search parameters
        self.inner.set_nprobe(self.nprobe);

        // Perform search
        let (distances, labels) = self.inner.search(&[query], k);

        (distances[0].to_vec(), labels[0].to_vec())
    }
}
```

#### 2.2 hnsw-rs - Pure Rust HNSW Implementation
```toml
[dependencies]
hnsw = "0.15"
```

```rust
use hnsw::{HNSW, Params, SearchMode};

struct SQLiteGraphHNSW {
    inner: HNSW<f32>,
    dimension: usize,
}

impl SQLiteGraphHNSW {
    fn new(dimension: usize, capacity: usize) -> Self {
        let params = Params::new()
            .with_ef_construction(200)
            .with_ef(50)
            .with_m(16);

        SQLiteGraphHNSW {
            inner: HNSW::new(dimension, capacity),
            dimension,
        }
    }

    fn insert(&mut self, id: usize, vector: Vec<f32>) {
        self.inner.insert(vector, id);
    }

    fn search(&self, query: &[f32], k: usize) -> Vec<(usize, f32)> {
        let mode = SearchMode::default();
        self.inner.search(query, k, mode)
    }
}
```

#### 2.3 arrow-ord - For vector operations
```toml
[dependencies]
arrow = "50.0"
arrow-ord = "50.0"
```

```rust
use arrow::array::{Float32Array, UInt32Array};
use arrow::compute::kernels::sort::SortOptions;
use arrow_ord::sort::sort_to_indices;

struct VectorProcessor {
    batch_size: usize,
}

impl VectorProcessor {
    fn batch_cosine_similarity(&self,
                               queries: &Float32Array,
                               vectors: &Float32Array) -> Float32Array {
        // SIMD-accelerated batch cosine similarity
        let dot_products = arrow::compute::kernels::arity::binary(
            queries,
            vectors,
            |q, v| q.iter().zip(v.iter())
                .map(|(a, b)| a * b)
                .sum::<f32>()
        );

        let query_norms = Self::batch_norm(queries);
        let vector_norms = Self::batch_norm(vectors);

        arrow::compute::kernels::arity::binary(
            &dot_products,
            &arrow::compute::kernels::arity::binary(
                &query_norms,
                &vector_norms,
                |q, v| q * v
            ),
            |dot, norm| dot / norm
        )
    }
}
```

### 3. Caching and Performance Libraries

#### 3.1 dashmap - Concurrent HashMap
```toml
[dependencies]
dashmap = "6.0"
```

```rust
use dashmap::DashMap;
use std::sync::Arc;

struct ConcurrentQueryCache {
    plans: Arc<DashMap<String, CachedPlan>>,
    stats: Arc<DashMap<String, QueryStats>>,
}

impl ConcurrentQueryCache {
    fn get_or_compute<F>(&self,
                        query: &str,
                        compute: F) -> Result<PlanNode, OptimizerError>
    where
        F: FnOnce() -> Result<PlanNode, OptimizerError>
    {
        // Try to get from cache
        if let Some(cached) = self.plans.get(query) {
            cached.update_hit();
            return Ok(cached.plan.clone());
        }

        // Compute plan
        let plan = compute()?;

        // Insert into cache
        let cached_plan = CachedPlan::new(plan.clone());
        self.plans.insert(query.to_string(), cached_plan);

        Ok(plan)
    }

    fn clear_expired(&self, ttl: Duration) {
        let now = SystemTime::now();
        self.plans.retain(|_, v| now.duration_since(v.timestamp) < ttl);
    }
}
```

#### 3.2 crossbeam - Lock-free data structures
```toml
[dependencies]
crossbeam = "0.8"
```

```rust
use crossbeam::queue::SegQueue;
use crossbeam::utils::Backoff;

struct LockFreePlanCache {
    entries: SegQueue<CacheEntry>,
    head: AtomicPtr<CacheNode>,
    size: AtomicUsize,
    max_size: usize,
}

struct CacheNode {
    key: String,
    value: PlanNode,
    next: *mut CacheNode,
    timestamp: SystemTime,
    access_count: AtomicU64,
}

impl LockFreePlanCache {
    fn get(&self, key: &str) -> Option<PlanNode> {
        let backoff = Backoff::new();
        let mut current = self.head.load(Ordering::Acquire);

        while !current.is_null() {
            let node = unsafe { &*current };
            if node.key == key {
                node.access_count.fetch_add(1, Ordering::Relaxed);
                return Some(node.value.clone());
            }
            current = node.next;
            backoff.snooze();
        }

        None
    }
}
```

#### 3.3 moka - High-performance caching
```toml
[dependencies]
moka = { version = "0.12", features = ["future"] }
```

```rust
use moka::future::Cache;
use tokio::time::Instant;

struct MokaPlanCache {
    inner: Cache<String, PlanNode>,
    hit_rate_tracker: Arc<AtomicU64>,
}

impl MokaPlanCache {
    fn new(max_capacity: u64) -> Self {
        let cache = Cache::builder()
            .max_capacity(max_capacity)
            .time_to_idle(Duration::from_secs(300)) // 5 minutes
            .weigher(|_key, value: &PlanNode| -> u32 {
                // Estimate memory usage
                (value.estimated_size() / 1024) as u32
            })
            .build();

        MokaPlanCache {
            inner: cache,
            hit_rate_tracker: Arc::new(AtomicU64::new(0)),
        }
    }

    async fn get_or_compute<F, Fut>(&self, key: String, compute: F) -> Result<PlanNode, OptimizerError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<PlanNode, OptimizerError>>,
    {
        match self.inner.get(&key) {
            Some(plan) => {
                self.hit_rate_tracker.fetch_add(1, Ordering::Relaxed);
                Ok(plan)
            },
            None => {
                let plan = compute().await?;
                self.inner.insert(key, plan.clone());
                Ok(plan)
            }
        }
    }
}
```

### 4. Memory Management and Allocation

#### 4.1 bumpalo - Bump allocator
```toml
[dependencies]
bumpalo = "3.15"
```

```rust
use bumpalo::Bump;

struct QueryPlanArena {
    arena: Bump,
}

impl QueryPlanArena {
    fn new() -> Self {
        QueryPlanArena {
            arena: Bump::new(),
        }
    }

    fn allocate_plan(&self, operations: Vec<Operation>) -> &'static PlanNode {
        // Allocate plan in bump arena
        let plan = self.arena.alloc(PlanNode::new(operations));
        unsafe { std::mem::transmute(plan) }
    }

    fn reset(&mut self) {
        self.arena.reset();
    }
}
```

#### 4.2 pool - Object pooling
```toml
[dependencies]
pool = "0.1"
```

```rust
use pool::Pool;

struct PlanNodePool {
    pool: Pool<PlanNode>,
}

impl PlanNodePool {
    fn new() -> Self {
        PlanNodePool {
            pool: Pool::new(1000), // Pre-allocate 1000 nodes
        }
    }

    fn get(&mut self) -> PooledPlanNode {
        PooledPlanNode {
            node: self.pool.try_pull().unwrap_or_else(|| PlanNode::new()),
            pool: &self.pool,
        }
    }
}

struct PooledPlanNode<'a> {
    node: PlanNode,
    pool: &'a Pool<PlanNode>,
}

impl<'a> Drop for PooledPlanNode<'a> {
    fn drop(&mut self) {
        // Reset node state
        self.node.reset();
        // Return to pool
        let _ = self.pool.attach(self.node.clone());
    }
}
```

### 5. Query Hint Implementation with Crates

#### 5.1 pest - Parsing Expression Grammar
```toml
[dependencies]
pest = "2.7"
pest_derive = "2.7"
```

```rust
use pest::Parser;

#[derive(Parser)]
#[grammar = "hints.pest"]
pub struct HintParser;

#[derive(Debug)]
pub enum SQLiteGraphHint {
    Index { table: String, index: String, usage: IndexUsage },
    Join { tables: Vec<String>, method: JoinMethod },
    Order { tables: Vec<String> },
    Parallel { degree: usize },
    Cache { ttl: Option<u64> },
    Limit { count: usize },
}

impl SQLiteGraphHint {
    pub fn parse_from_comment(sql: &str) -> Result<Vec<Self>, ParseError> {
        // Extract hints from SQL comments
        let hint_comments = extract_hint_comments(sql)?;

        let mut hints = Vec::new();
        for comment in hint_comments {
            let pairs = HintParser::parse(Rule::hint_list, &comment)?;
            for pair in pairs {
                if pair.as_rule() == Rule::hint {
                    hints.push(Self::from_pair(pair)?);
                }
            }
        }

        Ok(hints)
    }
}
```

#### 5.2 once_cell - Lazy initialization
```toml
[dependencies]
once_cell = "1.19"
```

```rust
use once_cell::sync::Lazy;

static GLOBAL_OPTIMIZER_STATE: Lazy<GlobalOptimizerState> = Lazy::new(|| {
    GlobalOptimizerState::initialize()
});

struct GlobalOptimizerState {
    cost_model: AdvancedCostModel,
    hint_processor: HintProcessor,
    cache_manager: CacheManager,
}

impl GlobalOptimizerState {
    fn initialize() -> Self {
        let cost_model = AdvancedCostModel::load_from_config();
        let hint_processor = HintProcessor::new();
        let cache_manager = CacheManager::new();

        Self {
            cost_model,
            hint_processor,
            cache_manager,
        }
    }
}

// Usage in SQLiteGraph
pub fn optimize_query(query: &Query) -> Result<PlanNode, OptimizerError> {
    let state = &*GLOBAL_OPTIMIZER_STATE;
    state.hint_processor.apply_to_query(query, &state.cost_model)
}
```

### 6. Advanced Features

#### 6.1 rayon - Parallel processing
```toml
[dependencies]
rayon = "1.9"
```

```rust
use rayon::prelude::*;

struct ParallelOptimizer {
    thread_pool: ThreadPool,
}

impl ParallelOptimizer {
    fn optimize_parallel(&self, queries: &[Query]) -> Vec<Result<PlanNode, OptimizerError>> {
        queries
            .par_iter()
            .map(|query| self.optimize_single(query))
            .collect()
    }

    fn parallel_join_enumeration(&self, tables: &[Table]) -> Vec<JoinOrder> {
        // Generate join orders in parallel
        (0..tables.len())
            .into_par_iter()
            .map(|i| {
                let mut order = Vec::new();
                order.push(i);
                self.extend_join_order(&mut order, tables, i);
                order
            })
            .collect()
    }
}
```

#### 6.2 tracing - Performance tracing
```toml
[dependencies]
tracing = "0.1"
tracing-subscriber = "0.3"
```

```rust
use tracing::{instrument, span, Level};

#[instrument(skip(self))]
impl QueryOptimizer {
    pub fn optimize(&self, query: &Query) -> Result<PlanNode, OptimizerError> {
        let span = span!(Level::DEBUG, "optimize_query", query = %query);
        let _enter = span.enter();

        // Parse phase
        let parsed = self.parse_query(query)?;
        tracing::debug!("Parsed query: {:?}", parsed);

        // Logical optimization
        let logical = self.logical_optimize(parsed)?;
        tracing::debug!("Logical plan: {:?}", logical);

        // Physical optimization
        let physical = self.physical_optimize(logical)?;
        tracing::debug!("Physical plan: {:?}", physical);

        // Cost estimation
        let cost = self.estimate_cost(&physical)?;
        tracing::info!("Final cost: {}", cost);

        Ok(physical)
    }
}
```

### 7. Integration Recommendations

```rust
// Recommended dependency set for SQLiteGraph
[dependencies]
# Core SQL parsing and planning
sqlparser = { version = "0.38", features = ["visitor"] }
datafusion = { version = "40.0", features = ["simd"] }

# Vector search
faiss = { version = "0.12", features = ["static"] }
hnsw = "0.15"

# Caching
moka = { version = "0.12", features = ["future"] }
dashmap = "6.0"

# Performance
rayon = "1.9"
crossbeam = "0.8"
bumpalo = "3.15"

# Parsing hints
pest = { version = "2.7", features = ["derive"] }
once_cell = "1.19"

# Statistics and cost estimation
statrs = "0.16"
ordinals = "0.3"

# Tracing
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
bincode = "1.3"
```

This analysis shows that Rust has a mature ecosystem for implementing sophisticated query optimization and hint systems. The key is to combine these crates effectively while maintaining SQLiteGraph's constraints around determinism and 300 LOC per module.