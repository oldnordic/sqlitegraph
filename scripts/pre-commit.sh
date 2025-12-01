#!/bin/bash

# Pre-commit hook for SQLiteGraph
# Runs formatting, linting, and tests before allowing commits

set -e

echo "Running pre-commit checks..."

# Check formatting
echo "Checking code formatting..."
if ! cargo fmt --all --check; then
    echo "❌ Code formatting check failed"
    echo "Run 'cargo fmt --all' to fix formatting issues"
    exit 1
fi
echo "✅ Code formatting check passed"

# Run clippy with strict warnings
echo "Running clippy lints..."
if ! cargo clippy --workspace --all-features -- -D warnings; then
    echo "❌ Clippy found issues"
    exit 1
fi
echo "✅ Clippy checks passed"

# Run tests
echo "Running tests..."
if ! cargo test --workspace; then
    echo "❌ Tests failed"
    exit 1
fi
echo "✅ All tests passed"

echo "🎉 All pre-commit checks passed!"