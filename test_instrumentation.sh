#!/bin/bash

# Phase 1 Instrumentation Test
# Test script to verify adjacency iterator instrumentation captures infinite loop behavior

echo "=== Phase 1: Testing Adjacency Iterator Instrumentation ==="

# Set debug environment variables to enable instrumentation
export RUST_LOG=debug
export V2_SLOT_DEBUG=1

# Create a temporary test directory
TEST_DIR="/tmp/adjacency_instrumentation_test"
mkdir -p "$TEST_DIR"

echo "1. Building with debug instrumentation..."
cargo build --workspace

echo "2. Running a simple adjacency operation test..."
cd tests/v2_clustered_adjacency_tdd_tests.rs

# Run tests with debug output to capture instrumentation data
RUST_LOG=debug cargo test --test v2_clustered_adjacency_tdd_tests -- --nocapture 2>&1 | tee "$TEST_DIR/instrumentation_output.log"

echo "3. Checking for instrumentation output..."
if grep -q "DEBUG:.*adjacency" "$TEST_DIR/instrumentation_output.log"; then
    echo "✓ Adjacency instrumentation detected in logs"
else
    echo "✗ No adjacency instrumentation found in logs"
fi

if grep -q "Metrics snapshot" "$TEST_DIR/instrumentation_output.log"; then
    echo "✓ Metrics snapshots captured"
else
    echo "✗ No metrics snapshots found"
fi

if grep -q "Starting collect operation" "$TEST_DIR/instrumentation_output.log"; then
    echo "✓ Collect operation instrumentation working"
else
    echo "✗ No collect operation instrumentation found"
fi

if grep -q "V2 clustered adjacency" "$TEST_DIR/instrumentation_output.log"; then
    echo "✓ V2 clustered adjacency instrumentation working"
else
    echo "✗ No V2 clustered adjacency instrumentation found"
fi

echo "4. Checking for infinite loop detection..."
INFINITE_LOOPS=$(grep -c "POTENTIAL INFINITE LOOP DETECTED" "$TEST_DIR/instrumentation_output.log" || true)
echo "Infinite loop detections: $INFINITE_LOOPS"

if [ "$INFINITE_LOOPS" -gt 0 ]; then
    echo "✓ Infinite loop detection instrumentation working"
else
    echo "- No infinite loops detected (may be normal)"
fi

echo "5. Phase 1 Instrumentation Test Complete"
echo "Log saved to: $TEST_DIR/instrumentation_output.log"

# Show a sample of the instrumentation output
echo ""
echo "=== Sample Instrumentation Output ==="
head -50 "$TEST_DIR/instrumentation_output.log" | grep -E "(DEBUG|Metrics|V2|collect|adjacency)" || echo "No instrumentation output found in first 50 lines"