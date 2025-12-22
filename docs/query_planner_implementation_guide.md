# SQLiteGraph Query Planner Hints: Implementation Guide

## Executive Summary

This guide provides a comprehensive roadmap for implementing query optimization and hint systems in SQLiteGraph, leveraging current research and Rust ecosystem capabilities while respecting SQLiteGraph's constraints (determinism, 300 LOC per module, no async dependencies).

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    SQLiteGraph Query Layer                  │
├─────────────────────────────────────────────────────────────┤
│  Query Parser  │  Hint Processor  │  Pattern Matcher        │
├─────────────────────────────────────────────────────────────┤
│  Query Optimizer  │  Cost Estimator  │  Plan Cache           │
├─────────────────────────────────────────────────────────────┤
│  Vector Search Engine  │  Graph Engine  │  Storage Layer     │
└─────────────────────────────────────────────────────────────┘
```

## Phase 1: Foundation (Weeks 1-2)

### 1.1 Hint Syntax Definition

**SQLiteGraph Hint Syntax (compatible with SQLite):**
```sql
-- SQLite-style pragmas
PRAGMA query_plan.index_scan(table_name, index_name);
PRAGMA query_plan.join_method(hash, table1, table2);
PRAGMA query_plan.join_order(table1, table2, table3);
PRAGMA query_plan.limit_scan(1000);
PRAGMA query_plan.cache_ttl(60);

-- Inline comments (PostgreSQL-style)
SELECT * FROM nodes /*+ IndexScan(nodes idx_type) */ WHERE type = 'User';

-- SQLiteGraph specific hints
SELECT * FROM edges /*+ GraphPath(max_depth=3, direction=outgoing) */
WHERE source = ?;
```

### 1.2 Core Data Structures

```rust
// src/query_optimizer/mod.rs (under 300 LOC)
use std::collections::HashMap;
use std::sync::Arc;
use crate::graph::SqliteGraph;

#[derive(Debug, Clone)]
pub struct QueryHints {
    // Index-related hints
    pub force_index: HashMap<String, String>,  // table -> index
    pub forbid_index: HashMap<String, String>, // table -> index

    // Join-related hints
    pub join_order: Option<Vec<String>>,
    pub join_method: HashMap<(String, String), JoinMethod>,

    // Execution hints
    pub limit_scan: Option<usize>,
    pub parallel_degree: Option<usize>,
    pub cache_ttl: Option<u64>,

    // Graph-specific hints
    pub max_depth: Option<usize>,
    pub direction: Option<GraphDirection>,
    pub algorithm: Option<TraversalAlgorithm>,
}

#[derive(Debug, Clone)]
pub enum JoinMethod {
    NestedLoop,
    HashJoin,
    MergeJoin,
}

#[derive(Debug, Clone)]
pub enum GraphDirection {
    Outgoing,
    Incoming,
    Undirected,
}

#[derive(Debug, Clone)]
pub enum TraversalAlgorithm {
    BFS,
    DFS,
    Dijkstra,
    AStar,
}

pub struct QueryOptimizer {
    cost_model: CostModel,
    stats_collector: Arc<StatsCollector>,
    plan_cache: PlanCache,
}

impl QueryOptimizer {
    pub fn new(graph: Arc<SqliteGraph>) -> Self {
        QueryOptimizer {
            cost_model: CostModel::new(),
            stats_collector: Arc::new(StatsCollector::new()),
            plan_cache: PlanCache::new(1000),
        }
    }
}
```

### 1.3 Hint Parser Implementation

```rust
// src/query_optimizer/hint_parser.rs (under 300 LOC)
use pest::Parser;
use pest_derive::Parser;
use crate::query_optimizer::QueryHints;

#[derive(Parser)]
#[grammar = "query_hints.pest"]
struct HintParser;

impl QueryHints {
    pub fn parse_from_sql(sql: &str) -> Result<Self, ParseError> {
        let mut hints = QueryHints::default();

        // Extract PRAGMA hints
        for line in sql.lines() {
            if line.trim().starts_with("PRAGMA query_plan.") {
                let pragma = line.trim();
                hints.apply_pragma(pragma)?;
            }
        }

        // Extract comment hints
        let re = regex::Regex::new(r"/\*\+(.*?)\*/").unwrap();
        for caps in re.captures_iter(sql) {
            if let Some(hint_text) = caps.get(1) {
                let parsed = HintParser::parse(Rule::hint, hint_text.as_str())
                    .map_err(|_| ParseError::InvalidHint)?;
                hints.apply_parsed_hint(parsed)?;
            }
        }

        Ok(hints)
    }
}
```

## Phase 2: Basic Optimization (Weeks 3-4)

### 2.1 Simple Cost Model

```rust
// src/query_optimizer/cost_model.rs (under 300 LOC)
use crate::storage::StorageStats;

#[derive(Debug)]
pub struct Cost {
    pub io_ops: f64,
    pub cpu_ops: f64,
    pub memory_kb: f64,
    pub total_cost: f64,
}

pub struct CostModel {
    io_cost: f64,
    cpu_cost: f64,
    memory_cost: f64,
}

impl CostModel {
    pub fn new() -> Self {
        CostModel {
            io_cost: 0.1,    // Per I/O operation
            cpu_cost: 0.001,  // Per CPU operation
            memory_cost: 0.0001, // Per KB
        }
    }

    pub fn estimate_scan_cost(&self, table_size: usize, has_index: bool) -> Cost {
        let pages = (table_size + 4095) / 4096; // 4KB pages
        let io_ops = if has_index { pages as f64 * 0.1 } else { pages as f64 };
        let cpu_ops = table_size as f64 * 0.5;

        Cost {
            io_ops,
            cpu_ops,
            memory_kb: (pages * 4) as f64,
            total_cost: io_ops * self.io_cost + cpu_ops * self.cpu_cost,
        }
    }

    pub fn estimate_join_cost(&self,
        left_size: usize,
        right_size: usize,
        method: JoinMethod) -> Cost
    {
        match method {
            JoinMethod::NestedLoop => {
                Cost {
                    io_ops: (left_size * right_size) as f64 * 0.001,
                    cpu_ops: (left_size * right_size) as f64,
                    memory_kb: 0.0,
                    total_cost: (left_size * right_size) as f64 * self.cpu_cost,
                }
            },
            JoinMethod::HashJoin => {
                Cost {
                    io_ops: (left_size + right_size) as f64 * 0.1,
                    cpu_ops: (left_size + right_size) as f64 * 1.5,
                    memory_kb: right_size as f64 * 0.064, // 64 bytes per tuple
                    total_cost: (left_size + right_size) as f64 *
                               (self.io_cost * 0.1 + self.cpu_cost * 1.5),
                }
            },
            JoinMethod::MergeJoin => {
                Cost {
                    io_ops: (left_size + right_size) as f64 * 0.1,
                    cpu_ops: (left_size + right_size) as f64 * 1.2,
                    memory_kb: 0.0,
                    total_cost: (left_size + right_size) as f64 *
                               (self.io_cost * 0.1 + self.cpu_cost * 1.2),
                }
            },
        }
    }
}
```

### 2.2 Rule-Based Optimizer

```rust
// src/query_optimizer/rules.rs (under 300 LOC)
use crate::query_optimizer::{PlanNode, QueryHints};

#[derive(Debug)]
pub enum OptimizationRule {
    PushDownSelection,
    PushDownProjection,
    IndexSelection,
    JoinReordering,
    LimitPushdown,
}

pub struct RuleBasedOptimizer {
    rules: Vec<OptimizationRule>,
}

impl RuleBasedOptimizer {
    pub fn apply_rules(&self,
        mut plan: PlanNode,
        hints: &QueryHints) -> Result<PlanNode, OptimizerError>
    {
        // Apply rules in order
        for rule in &self.rules {
            plan = match rule {
                OptimizationRule::PushDownSelection => {
                    self.push_down_selection(plan)?
                },
                OptimizationRule::IndexSelection => {
                    self.apply_index_hints(plan, hints)?
                },
                OptimizationRule::JoinReordering => {
                    self.apply_join_order_hints(plan, hints)?
                },
                _ => plan,
            };
        }
        Ok(plan)
    }

    fn push_down_selection(&self, plan: PlanNode) -> Result<PlanNode, OptimizerError> {
        // Move WHERE clauses as close to data source as possible
        match plan {
            PlanNode::Filter { input, predicate } => {
                if let PlanNode::Join { .. } = *input {
                    // Try to push filter below join
                    self.push_filter_below_join(*input, predicate)
                } else {
                    Ok(PlanNode::Filter { input, predicate })
                }
            },
            _ => Ok(plan),
        }
    }
}
```

## Phase 3: Advanced Features (Weeks 5-6)

### 3.1 Vector Search Integration

```rust
// src/query_optimizer/vector_optimizer.rs (under 300 LOC)
use crate::vector::{VectorIndex, ANNIndex};
use crate::query_optimizer::PlanNode;

pub struct VectorSearchOptimizer {
    ann_index: Box<dyn ANNIndex>,
    threshold: f64, // Minimum similarity threshold
}

impl VectorSearchOptimizer {
    pub fn optimize_vector_query(&self, query: VectorQuery) -> Result<PlanNode, OptimizerError> {
        // Determine optimal search strategy
        let strategy = self.determine_strategy(&query)?;

        match strategy {
            VectorSearchStrategy::ANN => {
                Ok(PlanNode::VectorANN {
                    query: query.vector,
                    k: query.k,
                    ef: query.ef.unwrap_or(64),
                    filters: query.filters,
                })
            },
            VectorSearchStrategy::Hybrid => {
                // Combine exact and approximate search
                let exact = self.plan_exact_search(&query)?;
                let ann = self.plan_ann_search(&query)?;
                Ok(PlanNode::VectorHybrid {
                    exact: Box::new(exact),
                    ann: Box::new(ann),
                    merge_strategy: MergeStrategy::Union,
                })
            },
            VectorSearchStrategy::Filtered => {
                // Apply metadata filters first
                Ok(PlanNode::Sequence {
                    operations: vec![
                        PlanNode::Filter {
                            input: Box::new(PlanNode::VectorScan {
                                index_id: 0
                            }),
                            predicate: query.filters.unwrap_or_default()
                        },
                        PlanNode::VectorRank {
                            query_vector: query.vector,
                            k: query.k,
                        },
                    ],
                })
            },
        }
    }

    fn determine_strategy(&self, query: &VectorQuery) -> Result<VectorSearchStrategy, OptimizerError> {
        // Based on query characteristics
        if query.k > 1000 {
            Ok(VectorSearchStrategy::ANN)
        } else if query.filters.is_some() {
            Ok(VectorSearchStrategy::Filtered)
        } else {
            Ok(VectorSearchStrategy::Hybrid)
        }
    }
}
```

### 3.2 Plan Caching Implementation

```rust
// src/query_optimizer/cache.rs (under 300 LOC)
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

type CacheKey = String;

#[derive(Clone)]
struct CachedPlan {
    plan: PlanNode,
    timestamp: SystemTime,
    hit_count: u64,
    ttl: Duration,
}

pub struct PlanCache {
    cache: Arc<DashMap<CacheKey, CachedPlan>>,
    max_size: usize,
    cleanup_interval: Duration,
}

impl PlanCache {
    pub fn new(max_size: usize) -> Self {
        let cache = Arc::new(DashMap::new());
        let instance = PlanCache {
            cache: cache.clone(),
            max_size,
            cleanup_interval: Duration::from_secs(60),
        };

        // Start cleanup task (no async, use thread)
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(instance.cleanup_interval);
                instance.cleanup_expired();
            }
        });

        instance
    }

    pub fn get(&self, key: &str) -> Option<PlanNode> {
        if let Some(entry) = self.cache.get(key) {
            // Check TTL
            if SystemTime::now().duration_since(entry.timestamp) < entry.ttl {
                // Update hit count
                entry.hit_count += 1;
                return Some(entry.plan.clone());
            } else {
                // Expired, remove
                self.cache.remove(key);
            }
        }
        None
    }

    pub fn insert(&self, key: String, plan: PlanNode, ttl: Duration) {
        // Check size limit
        if self.cache.len() >= self.max_size {
            self.evict_lru();
        }

        let cached = CachedPlan {
            plan,
            timestamp: SystemTime::now(),
            hit_count: 0,
            ttl,
        };

        self.cache.insert(key, cached);
    }

    fn evict_lru(&self) {
        let mut oldest = None;
        let mut oldest_time = SystemTime::now();

        for entry in self.cache.iter() {
            if entry.timestamp < oldest_time {
                oldest_time = entry.timestamp;
                oldest = Some(entry.key().clone());
            }
        }

        if let Some(key) = oldest {
            self.cache.remove(&key);
        }
    }
}
```

## Phase 4: Performance Gates (Week 7)

### 4.1 Benchmark Integration

```rust
// src/query_optimizer/benchmarks.rs (under 300 LOC)
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Serialize, Deserialize)]
pub struct OptimizationBenchmark {
    pub query_hash: String,
    pub parse_time_ns: u64,
    pub optimize_time_ns: u64,
    pub plan_cache_hit: bool,
    pub cost_estimate: f64,
    pub actual_cost: Option<f64>,
}

pub struct PerformanceGates {
    baselines: OptimizationBaselines,
    collected: Vec<OptimizationBenchmark>,
}

#[derive(Serialize, Deserialize)]
struct OptimizationBaselines {
    max_parse_time_ns: u64,
    max_optimize_time_ns: u64,
    min_cache_hit_rate: f64,
    max_cost_error: f64,
}

impl PerformanceGates {
    pub fn new() -> Result<Self, Error> {
        let baselines = Self::load_baselines()?;
        Ok(PerformanceGates {
            baselines,
            collected: Vec::new(),
        })
    }

    pub fn measure_optimization(&mut self,
        query: &str,
        optimizer: &QueryOptimizer) -> Result<PlanNode, OptimizerError>
    {
        // Parse time
        let start = Instant::now();
        let parsed = QueryHints::parse_from_sql(query)?;
        let parse_time = start.elapsed().as_nanos() as u64;

        // Optimization time
        let start = Instant::now();
        let plan = optimizer.optimize(parsed)?;
        let optimize_time = start.elapsed().as_nanos() as u64;

        // Record benchmark
        let benchmark = OptimizationBenchmark {
            query_hash: Self::hash_query(query),
            parse_time_ns: parse_time,
            optimize_time_ns: optimize_time,
            plan_cache_hit: optimizer.cache_hit(),
            cost_estimate: plan.estimated_cost(),
            actual_cost: None,
        };

        // Validate against gates
        self.validate_benchmark(&benchmark)?;
        self.collected.push(benchmark);

        Ok(plan)
    }

    fn validate_benchmark(&self, benchmark: &OptimizationBenchmark) -> Result<(), GateError> {
        if benchmark.parse_time_ns > self.baselines.max_parse_time_ns {
            return Err(GateError::ParseTimeout);
        }

        if benchmark.optimize_time_ns > self.baselines.max_optimize_time_ns {
            return Err(GateError::OptimizeTimeout);
        }

        Ok(())
    }
}
```

## Phase 5: Integration & Testing (Week 8)

### 5.1 CLI Integration

```rust
// src/cli/query_hints.rs (extension to existing CLI)
use clap::Args;

#[derive(Args)]
pub struct QueryHintCommands {
    #[command(subcommand)]
    pub command: QueryHintSubCommand,
}

#[derive(Subcommand)]
pub enum QueryHintSubCommand {
    /// Show current query plan for a query
    Explain {
        query: String,
        #[arg(short, long)]
        hints: Option<String>,
    },
    /// Test optimization with hints
    Optimize {
        query: String,
        #[arg(short, long)]
        hints: Option<String>,
    },
    /// Benchmark query optimization
    Benchmark {
        query_file: String,
        #[arg(long)]
        iterations: Option<usize>,
    },
}

pub fn handle_query_hint_commands(cmd: QueryHintCommands) -> Result<(), Error> {
    match cmd.command {
        QueryHintSubCommand::Explain { query, hints } => {
            let hints = hints.map(|h| QueryHints::parse_from_sql(&h)).transpose()?;
            let plan = explain_query(&query, hints)?;
            println!("Query Plan:\n{}", plan.format());
        },
        QueryHintSubCommand::Optimize { query, hints } => {
            let hints = hints.map(|h| QueryHints::parse_from_sql(&h)).transpose()?;
            let stats = execute_with_hints(&query, hints)?;
            print_execution_stats(stats);
        },
        QueryHintSubCommand::Benchmark { query_file, iterations } => {
            let results = run_optimization_benchmark(&query_file, iterations.unwrap_or(100))?;
            print_benchmark_results(results);
        },
    }
    Ok(())
}
```

### 5.2 Test Suite

```rust
// tests/query_optimizer_tests.rs (under 300 LOC)
use sqlitegraph::query_optimizer::*;
use sqlitegraph::test_utils::*;

#[test]
fn test_hint_parsing() {
    let sql = "SELECT * FROM nodes /*+ IndexScan(nodes idx_type) */ WHERE type = ?";
    let hints = QueryHints::parse_from_sql(sql).unwrap();

    assert_eq!(
        hints.force_index.get("nodes").unwrap(),
        "idx_type"
    );
}

#[test]
fn test_join_order_hint() {
    let hints = QueryHints {
        join_order: Some(vec!["users".to_string(), "posts".to_string(), "comments".to_string()]),
        ..Default::default()
    };

    let optimizer = QueryOptimizer::new(test_graph());
    let plan = optimizer.optimize_with_hints(test_join_query(), &hints).unwrap();

    // Verify join order
    assert_eq!(plan.join_order(), &["users", "posts", "comments"]);
}

#[test]
fn test_plan_cache() {
    let cache = PlanCache::new(100);
    let key = "test_query".to_string();
    let plan = create_test_plan();

    // Insert and retrieve
    cache.insert(key.clone(), plan.clone(), Duration::from_secs(60));
    let retrieved = cache.get(&key).unwrap();

    assert_eq!(retrieved, plan);
}

#[test]
fn test_vector_optimization() {
    let optimizer = VectorSearchOptimizer::new(test_vector_index());
    let query = VectorQuery {
        vector: vec![0.1, 0.2, 0.3],
        k: 10,
        filters: Some(Expr::Eq("type".to_string(), "image".to_string())),
        ..Default::default()
    };

    let plan = optimizer.optimize_vector_query(query).unwrap();

    // Should use filtered strategy
    match plan {
        PlanNode::Sequence { .. } => {}, // Expected
        _ => panic!("Expected sequence plan for filtered query"),
    }
}

#[test]
fn test_performance_gates() {
    let mut gates = PerformanceGates::new().unwrap();
    let optimizer = QueryOptimizer::new(test_graph());

    // Simple query should pass gates
    let plan = gates.measure_optimization("SELECT * FROM nodes LIMIT 10", &optimizer);
    assert!(plan.is_ok());

    // Complex query should still pass but be slower
    let complex = generate_complex_query(100);
    let plan = gates.measure_optimization(&complex, &optimizer);
    assert!(plan.is_ok());
}
```

## Implementation Checklist

### Module Structure (all under 300 LOC):
- [ ] `src/query_optimizer/mod.rs` - Main optimizer interface
- [ ] `src/query_optimizer/hint_parser.rs` - Parse SQL hints
- [ ] `src/query_optimizer/cost_model.rs` - Cost estimation
- [ ] `src/query_optimizer/rules.rs` - Optimization rules
- [ ] `src/query_optimizer/cache.rs` - Plan caching
- [ ] `src/query_optimizer/vector_optimizer.rs` - Vector query optimization
- [ ] `src/query_optimizer/benchmarks.rs` - Performance gates
- [ ] `src/cli/query_hints.rs` - CLI commands for hints

### Dependencies to add to Cargo.toml:
```toml
[dependencies]
# Query parsing and optimization
sqlparser = { version = "0.38", features = ["visitor"] }
pest = { version = "2.7", features = ["derive"] }
regex = "1.10"

# Caching
dashmap = "6.0"
once_cell = "1.19"

# Vector search (if implementing vector queries)
hnsw = "0.15"  # Pure Rust implementation

# Statistics
statrs = "0.16"

# Serialization for plan cache
serde = { version = "1.0", features = ["derive"] }
bincode = "1.3"

# CLI integration
clap = { version = "4.5", features = ["derive"] }

# Testing
proptest = "1.4"
```

## Performance Targets

Based on research and SQLiteGraph's requirements:

1. **Parse time**: < 100μs for simple queries, < 1ms for complex queries
2. **Optimization time**: < 1ms for simple queries, < 10ms for complex queries
3. **Plan cache hit rate**: > 80% for repeated queries
4. **Memory overhead**: < 1MB for 1000 cached plans
5. **Hint application overhead**: < 50μs per hint

## Monitoring and Observability

```rust
// Add to SQLiteGraph's existing metrics system
pub struct QueryOptimizerMetrics {
    pub queries_optimized: AtomicU64,
    pub hints_applied: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub optimization_time_ns: AtomicU64,
    pub plan_size_bytes: AtomicU64,
}
```

## Future Extensions

1. **Machine Learning for Cost Estimation**: Use historical execution data to improve cost models
2. **Adaptive Hint Learning**: Automatically learn effective hints from query patterns
3. **Distributed Query Optimization**: For multi-node SQLiteGraph deployments
4. **Explain AI**: Natural language explanations of query plans
5. **Visual Plan Explorer**: Interactive query plan visualization

This implementation guide provides a phased approach to adding query optimization and hint systems to SQLiteGraph while maintaining its core design principles of determinism, auditability, and performance.