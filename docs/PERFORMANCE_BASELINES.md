# SQLiteGraph Performance Baselines

**Established:** 2026-01-17
**Phase:** 07 - Performance Optimization

## Baseline Methodology

Baselines established using Criterion benchmark framework with:
- Sample size: 100 iterations
- Warm-up time: 5 seconds
- Measurement time: 15 seconds
- Regression threshold: 10%

## Benchmark Categories

### WAL Recovery Throughput

Measures database startup time after crash with varying transaction counts in WAL.

| Transactions | Mean (ms) | Std Dev (ms) | Throughput (tx/sec) |
|--------------|------------|--------------|---------------------|
| 10           | TBD        | TBD          | TBD                 |
| 50           | TBD        | TBD          | TBD                 |
| 100          | TBD        | TBD          | TBD                 |
| 500          | TBD        | TBD          | TBD                 |

*Note: Baselines will be populated after first benchmark run*

**Purpose:** Validate parallel WAL recovery optimizations from plan 07-01.

### Insert Throughput

Measures single-node insertion performance with varying batch sizes.

| Batch Size | Mean (ms) | Throughput (ops/sec) |
|------------|-----------|----------------------|
| 1          | TBD       | TBD                  |
| 10         | TBD       | TBD                  |
| 100        | TBD       | TBD                  |
| 1000       | TBD       | TBD                  |

**Purpose:** Track write path performance and detect lock contention issues from plan 07-02.

### Traversal Performance

Measures BFS traversal performance across different graph depths.

| Depth | Mean (ms) | Throughput (nodes/sec) |
|-------|-----------|--------------------------|
| 10    | TBD       | TBD                      |
| 50    | TBD       | TBD                      |
| 100   | TBD       | TBD                      |
| 500   | TBD       | TBD                      |

**Purpose:** Validate read path optimizations from Phase 3.

### Memory Efficiency

Measures memory usage for node storage with varying data sizes.

| Node Count | Memory (MB) | Bytes per Node |
|------------|-------------|----------------|
| 100        | TBD         | TBD            |
| 1000       | TBD         | TBD            |
| 10000      | TBD         | TBD            |

**Purpose:** Track memory overhead and validate compression optimizations from Phase 3.

## Running Benchmarks

### Run All Benchmarks

```bash
cd sqlitegraph
cargo bench --bench comprehensive_performance
```

### Save Baseline

```bash
cd sqlitegraph
cargo bench --bench comprehensive_performance -- --save-baseline main
```

### Compare Against Baseline

```bash
cd sqlitegraph
cargo bench --bench comprehensive_performance -- --baseline main --load-baseline main
```

### Using CI Script

```bash
./scripts/run_performance_benchmarks.sh
```

## Updating Baselines

When legitimate performance improvements are made, update baselines:

1. Run benchmarks with new code
2. Verify improvements are real and consistent
3. Save new baseline:
   ```bash
   cd sqlitegraph
   cargo bench --bench comprehensive_performance -- --save-baseline main
   ```
4. Update this documentation with new baseline values
5. Commit baseline files:
   ```bash
   git add target/benchmark/criterion/
   git commit -m "perf: update performance baselines"
   ```

## Regression Detection

Regressions are detected when performance degrades by more than 10% from baseline.

**Automatic Detection:**

The benchmark runner script (`scripts/run_performance_benchmarks.sh`) automatically:
- Runs all benchmarks
- Compares against saved baseline
- Fails (exit code 1) if regression >10% detected

**Manual Review:**

If regression is detected:
1. Open HTML report: `target/benchmark/criterion/report/index.html`
2. Compare current vs baseline for each benchmark
3. Identify which operations regressed
4. Investigate code changes causing regression
5. Fix or justify regression

## Continuous Integration

Performance benchmarks run automatically on:
- Every push to `main` branch
- Every pull request to `main` branch

CI fails if regression >10% is detected.

## Known Limitations

1. **Benchmark Variance:** Results may vary ±5% between runs due to system load
2. **Cold Start Effects:** First run after clean build is slower (warm-up mitigates this)
3. **Hardware Dependent:** Results are hardware-specific (CI provides consistent environment)
4. **WAL Recovery:** Baseline uses sequential recovery (parallel not yet implemented - plan 07-01)
5. **Lock Contention:** Single-threaded benchmarks (parallel contention testing pending plan 07-02)

## References

- Benchmark source: `sqlitegraph/benches/comprehensive_performance.rs`
- Phase 3 benchmarks: `sqlitegraph/benches/read_path_benchmarks.rs`
- Phase 7 plans: `.planning/phases/07-performance/`
- Criterion documentation: https://bheisler.github.io/criterion.rs/

## Pending Optimizations

The following optimizations from Phase 7 plans will affect these baselines:

- **Plan 07-01:** Parallel WAL recovery - should improve WAL recovery throughput
- **Plan 07-02:** Lock contention reduction - should improve insert throughput under parallelism

After these plans are implemented, re-run benchmarks and update baselines.
