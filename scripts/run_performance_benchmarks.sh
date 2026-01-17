#!/bin/bash
# Performance benchmark runner with regression detection
#
# Usage: ./scripts/run_performance_benchmarks.sh
#
# Exit codes:
#   0 - All benchmarks passed
#   1 - Regression detected (>10% performance degradation)
#   2 - Benchmark execution failed

set -e

echo "=== SQLiteGraph Performance Benchmarks ==="
echo "Date: $(date)"
echo "Commit: $(git rev-parse --short HEAD)"
echo ""

# Check if Criterion is installed
if ! command -v cargo &> /dev/null; then
    echo "Error: cargo not found"
    exit 2
fi

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo "Error: Must run from repository root (where Cargo.toml is)"
    exit 2
fi

# Change to sqlitegraph directory
cd sqlitegraph

# Run benchmarks with output
echo "Running benchmarks..."
CARGO_TARGET_DIR=target/benchmark cargo bench --bench comprehensive_performance -- --save-baseline main --nocapture

echo ""
echo "Benchmark results:"
echo "HTML report: target/benchmark/criterion/report/index.html"
echo ""

# Check for regressions using Criterion's comparison
# This will generate comparison if baseline exists
if [ -f "target/benchmark/criterion/baseline.json" ]; then
    echo "Comparing against baseline..."
    cargo bench --bench comprehensive_performance -- --baseline main --load-baseline main > /dev/null 2>&1

    if [ $? -ne 0 ]; then
        echo ""
        echo "=== REGRESSION DETECTED ==="
        echo "Performance has degraded by more than 10% in one or more benchmarks."
        echo "Review the HTML report for details."
        exit 1
    fi

    echo "No regression detected (performance within 10% of baseline)"
fi

echo ""
echo "=== Benchmarks Complete ==="
exit 0
