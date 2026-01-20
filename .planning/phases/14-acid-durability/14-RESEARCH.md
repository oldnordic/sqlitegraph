# Phase 14: ACID Durability - Research

**Researched:** 2026-01-20
**Domain:** WAL Checkpoint Trigger Strategies (V2)
**Confidence:** HIGH

## Summary

Phase 14 completes the ACID durability property by implementing fully functional checkpoint trigger strategies. The checkpoint infrastructure already exists with a robust `CheckpointStrategy` enum and `StrategyEvaluator` in `strategies.rs`, but the actual trigger evaluation logic has TODO placeholders. The core implementation work is:

1. **Complete transaction-count checkpoint trigger** - Track transactions since last checkpoint and trigger when threshold exceeded
2. **Complete size-based checkpoint trigger** - Track WAL file size and trigger when threshold exceeded
3. **Integrate counter tracking in WAL manager** - Add `transactions_since_checkpoint` and `wal_size_at_checkpoint` fields
4. **Wire up strategy evaluation** - Replace TODO placeholders in `V2WALCheckpointManager::evaluate_checkpoint_strategy`
5. **Add counter reset logic** - Reset counters after checkpoint completion

The codebase already has:
- `CheckpointStrategy` enum with `SizeThreshold(u64)`, `TransactionCount(u64)`, `TimeInterval(Duration)`, and `Adaptive` variants
- `StrategyEvaluator` with skeleton implementations for each trigger type
- `CheckpointTrigger` struct for reporting trigger information
- `V2WALConfig` with `checkpoint_interval` field (transaction count)
- `CheckpointManagerState` with `checkpointed_lsn` tracking
- Constants for default thresholds in `checkpoint/constants.rs`

**Primary recommendation:** Complete the existing TODO implementations in `core.rs` and `strategies.rs`, add counter tracking fields to `CheckpointManagerState`, and integrate with `V2WALManager` for transaction count updates.

## Existing Implementation Summary

### Checkpoint Strategies Module (`checkpoint/strategies.rs`)

The `CheckpointStrategy` enum defines four trigger strategies (lines 14-34):

```rust
pub enum CheckpointStrategy {
    SizeThreshold(u64),           // Checkpoint when WAL exceeds specified size
    TransactionCount(u64),        // Checkpoint after N transactions
    TimeInterval(Duration),       // Checkpoint after time interval
    Adaptive {                    // Combined strategy
        min_interval: Duration,
        max_wal_size: u64,
        max_transactions: u64,
    },
}
```

The `StrategyEvaluator` provides `should_checkpoint()` method (lines 213-249) that evaluates each strategy:

- **`evaluate_size_threshold`** (lines 251-278): Already implemented - reads WAL file size via `std::fs::metadata`, returns true if `wal_size >= max_size`
- **`evaluate_transaction_count`** (lines 281-310): Partially implemented - reads WAL header to get LSN delta, but uses `committed_lsn - checkpointed_lsn` which may not accurately count transactions
- **`evaluate_time_interval`** (lines 313-340): Already implemented - checks elapsed time since last checkpoint
- **`evaluate_adaptive`** (lines 343-409): Combines all three checks

**Gap:** `evaluate_transaction_count` uses LSN delta as a proxy for transaction count, but LSN increments per record, not per transaction. Need actual transaction counter.

### Checkpoint Manager Core (`checkpoint/core.rs`)

The `V2WALCheckpointManager` in `core.rs` has:

- **`CheckpointManagerState`** (lines 77-107): Tracks checkpoint state including `checkpointed_lsn`, `last_checkpoint`, `completed_checkpoints`, `failed_attempts`
- **`evaluate_checkpoint_strategy`** (lines 663-689): Has TODO placeholders for `TransactionCount`, `SizeThreshold`, and `Adaptive` strategies

**Current implementation:**
```rust
CheckpointStrategy::TransactionCount(_threshold) => {
    Ok(false) // TODO: Implement transaction count checking
}
CheckpointStrategy::SizeThreshold(_threshold) => {
    Ok(false) // TODO: Implement size threshold checking
}
CheckpointStrategy::Adaptive { .. } => {
    Ok(false) // TODO: Implement adaptive strategy
}
```

**Gap:** These TODO placeholders need to delegate to `StrategyEvaluator` or implement inline logic.

### WAL Manager (`manager.rs`)

The `V2WALManager` has basic checkpoint triggering:

- **`requires_checkpoint`** (lines 485-492): Simple check using `wal_size > max_wal_size` or `(current_lsn - checkpointed_lsn) > checkpoint_interval`
- **`force_checkpoint`** (lines 452-468): Forces checkpoint and updates `metrics.checkpoint_count`

**Gap:** No persistent tracking of `transactions_since_checkpoint` that resets after checkpoint.

## WAL Manager Structure

### Current State Tracking

`V2WALManager` maintains:
- `WALManagerMetrics` with `total_transactions`, `committed_transactions`, `checkpoint_count`
- `active_transactions: HashMap<u64, ActiveTransaction>`
- `header: Arc<RwLock<V2WALHeader>>` with `current_lsn`, `committed_lsn`, `checkpointed_lsn`

**Missing:** A dedicated `transactions_since_checkpoint` counter that:
1. Increments on each transaction commit
2. Resets to 0 after checkpoint completes
3. Is accessible to checkpoint manager for trigger evaluation

### WAL File Size Tracking

Current `estimate_wal_size()` implementation (lines 585-595):
```rust
fn estimate_wal_size(&self) -> u64 {
    if let Ok(metadata) = std::fs::metadata(&self.config.wal_path) {
        return metadata.len();
    }
    // Fallback to writer metrics
    let metrics = self.writer.get_metrics();
    metrics.bytes_written + std::mem::size_of::<V2WALHeader>() as u64
}
```

**Status:** Already functional. Uses actual file size when available.

## Counter Tracking Gaps

### What's Missing

1. **Transaction Counter**: No persistent `transactions_since_checkpoint` field
   - `committed_transactions` is a lifetime total, not resettable
   - No counter that resets after checkpoint

2. **Counter Reset Logic**: No mechanism to reset counters after checkpoint
   - `force_checkpoint` updates `metrics.checkpoint_count` but doesn't reset transaction counter
   - Need `checkpoint_completed()` callback to reset counters

3. **Integration Points**: Missing integration between WAL manager and checkpoint manager
   - Checkpoint manager needs access to transaction counter
   - WAL manager needs notification when checkpoint completes

### Recommended Solution

Add to `CheckpointManagerState`:
```rust
pub struct CheckpointManagerState {
    // ... existing fields ...
    pub transactions_since_checkpoint: u64,
    pub wal_size_at_last_checkpoint: u64,
}
```

Add to `V2WALManager`:
```rust
fn on_checkpoint_completed(&self) {
    let mut metrics = self.metrics.write();
    metrics.transactions_since_checkpoint = 0;
    // Reset checkpointed_lsn
}
```

## Configuration Integration

### Existing Configuration Fields

`V2WALConfig` already has relevant fields (from `mod.rs`):
- `checkpoint_interval: u64` - Number of transactions (default: 1000)
- `max_wal_size: u64` - Maximum WAL size before forced checkpoint (default: 1GB)

**Status:** Configuration exists but not used for trigger evaluation by checkpoint manager.

### NativeConfig Integration (CP-03)

Requirement CP-03 asks for checkpoint triggers to be configurable via `NativeConfig`.

**Current `NativeConfig`** (from `config/native.rs`):
```rust
pub struct NativeConfig {
    pub create_if_missing: bool,
    pub reserve_node_capacity: Option<usize>,
    pub reserve_edge_capacity: Option<usize>,
    pub cpu_profile: Option<CpuProfile>,
    pub max_parallel_transactions: usize,
}
```

**Gap:** No checkpoint configuration fields in `NativeConfig`.

**Recommended addition:**
```rust
pub struct NativeConfig {
    // ... existing fields ...
    /// Checkpoint strategy configuration
    pub checkpoint_strategy: CheckpointStrategy,
    /// Or individual fields:
    pub checkpoint_transaction_threshold: Option<u64>,
    pub checkpoint_size_threshold: Option<u64>,
    pub checkpoint_time_interval: Option<Duration>,
}
```

## Key Files

| File | Current Role | Phase 14 Changes Needed |
|------|--------------|-------------------------|
| `checkpoint/strategies.rs` | Defines `CheckpointStrategy` enum and `StrategyEvaluator` | Complete `evaluate_transaction_count` to use actual counter, not LSN delta |
| `checkpoint/core.rs` | `V2WALCheckpointManager` core orchestration | Replace TODO placeholders in `evaluate_checkpoint_strategy`, add counter fields to `CheckpointManagerState`, add counter reset logic |
| `manager.rs` | `V2WALManager` coordinates WAL operations | Add `transactions_since_checkpoint` tracking, add `on_checkpoint_completed` callback |
| `config/native.rs` | `NativeConfig` for native backend | Add checkpoint threshold configuration fields |
| `wal/mod.rs` | `V2WALConfig` for WAL-specific config | Already has `checkpoint_interval` and `max_wal_size` - may want to add individual strategy thresholds |

## Dependencies

### Within This Phase

```
Transaction counter tracking (manager.rs)
    |
    v
Strategy evaluation uses counter (strategies.rs)
    |
    v
Checkpoint manager evaluates triggers (core.rs)
    |
    v
Counter reset after checkpoint (core.rs + manager.rs)
```

### External Dependencies

- **Phase 13 (Isolation)**: Complete - transaction coordinator provides commit notifications
- **Phase 12 (Consistency)**: Complete - checkpoint infrastructure in place

## Risks/Concerns

### 1. Transaction Counting Accuracy (MEDIUM)

**Risk:** Using LSN delta (`committed_lsn - checkpointed_lsn`) as a proxy for transaction count is inaccurate because LSN increments per WAL record, not per transaction.

**Mitigation:** Add explicit `transactions_since_checkpoint` counter that increments on `commit_transaction()`.

### 2. Counter Reset Race Conditions (MEDIUM)

**Risk:** Checkpoint runs in background thread (see `manager.rs:376-380`). Counter reset must be atomic with checkpoint completion to avoid missing transactions.

**Mitigation:** Use `Arc<Mutex<CheckpointManagerState>>` for counter access, reset counter under lock after checkpoint completes.

### 3. Multiple Trigger Conditions (LOW)

**Risk:** `Adaptive` strategy checks multiple conditions. Need to ensure all counters are reset consistently.

**Mitigation:** Single `reset_counters()` method called after any checkpoint completion, regardless of trigger reason.

### 4. Configuration Complexity (LOW)

**Risk:** Adding checkpoint configuration to `NativeConfig` may duplicate existing `V2WALConfig` fields.

**Mitigation:** Keep WAL-specific thresholds in `V2WALConfig`, add only high-level strategy selector to `NativeConfig`.

## Implementation Approach

### Task 1: Add Counter Tracking to WAL Manager
- Add `transactions_since_checkpoint: u64` to `WALManagerMetrics` or new field
- Increment counter in `commit_transaction()` after successful commit
- Add `get_transactions_since_checkpoint()` accessor

### Task 2: Complete Strategy Evaluation
- Implement `TransactionCount` evaluation in `core.rs::evaluate_checkpoint_strategy`
- Use actual counter from WAL manager, not LSN delta
- Implement `SizeThreshold` evaluation (delegates to file size check)

### Task 3: Add Counter Reset Logic
- Add `reset_checkpoint_counters()` method to `V2WALManager`
- Call after `force_checkpoint()` completes
- Reset `transactions_since_checkpoint` to 0
- Update `wal_size_at_last_checkpoint`

### Task 4: NativeConfig Integration
- Add checkpoint threshold fields to `NativeConfig`
- Wire through to `V2WALConfig` during WAL manager creation
- Add validation for threshold values

### Task 5: Testing
- Test transaction-count trigger fires at threshold
- Test size-based trigger fires at threshold
- Test counters reset after checkpoint
- Test adaptive strategy with combined triggers

## Standard Stack

No new dependencies required. Existing stack suffices:

| Component | Usage |
|-----------|-------|
| `parking_lot::{Mutex, RwLock}` | Counter synchronization |
| `std::sync::atomic::AtomicU64` | Lock-free counter if needed |
| `std::fs::metadata` | WAL file size checking |
| `std::time::{Duration, Instant}` | Time-based tracking |

## Don't Hand-Roll

| Problem | Don't Build | Use Instead |
|---------|-------------|-------------|
| Atomic transaction counter | `AtomicU64` with manual reset logic | Use `Mutex<u64>` for simplicity with explicit reset |
| WAL file size polling | Custom file watcher | `std::fs::metadata` is sufficient - called on-demand |
| Time interval tracking | Custom timer with `sleep` | `Instant::elapsed()` on-demand checks |

## Common Pitfalls

### Pitfall 1: Using LSN Delta as Transaction Count

**What goes wrong:** LSN increments per WAL record (node insert, edge insert, etc.), not per transaction. A single transaction may write multiple records, causing checkpoint to trigger early.

**Why it happens:** Code in `strategies.rs:291` uses `header.committed_lsn.saturating_sub(checkpointed_lsn)`.

**How to avoid:** Add explicit `transactions_since_checkpoint` counter incremented only on `commit_transaction()`.

### Pitfall 2: Not Resetting Counters After Checkpoint

**What goes wrong:** Checkpoint fires immediately again because counters still exceed threshold.

**Why it happens:** Forgetting to reset counter after `force_checkpoint()`.

**How to avoid:** Add `reset_counters()` call at end of checkpoint flow, verify with test that commits after checkpoint don't trigger immediately.

### Pitfall 3: Race Condition with Background Checkpoint

**What goes wrong:** Transaction commits during checkpoint, counter resets before transaction counted.

**Why it happens:** Checkpoint runs in background thread (manager.rs:376), counter reset not synchronized with commit.

**How to avoid:** Hold lock on counter state during reset, or use "checkpoint in progress" flag to defer counter updates.

## Code Examples

### Adding Counter to WAL Manager Metrics

```rust
// In manager.rs, WALManagerMetrics struct
pub struct WALManagerMetrics {
    // ... existing fields ...
    pub transactions_since_checkpoint: u64,
}

// In commit_transaction(), after successful commit
{
    let mut metrics = self.metrics.write();
    metrics.committed_transactions += 1;
    metrics.transactions_since_checkpoint += 1;  // NEW
}
```

### Counter Reset After Checkpoint

```rust
// In manager.rs, new method
pub fn on_checkpoint_completed(&self) -> NativeResult<()> {
    let mut metrics = self.metrics.write();
    metrics.transactions_since_checkpoint = 0;
    metrics.checkpoint_count += 1;
    Ok(())
}
```

### Strategy Evaluation in Core

```rust
// In core.rs, evaluate_checkpoint_strategy method
fn evaluate_checkpoint_strategy(
    &self,
    strategy: &CheckpointStrategy,
    _dirty_blocks: &DirtyBlockTracker,
    state: &CheckpointManagerState,
) -> CheckpointResult<bool> {
    match strategy {
        CheckpointStrategy::TransactionCount(threshold) => {
            Ok(state.transactions_since_checkpoint >= *threshold)
        }
        CheckpointStrategy::SizeThreshold(threshold) => {
            let wal_size = std::fs::metadata(&self.config.wal_path)
                .map(|m| m.len())
                .unwrap_or(0);
            Ok(wal_size >= *threshold)
        }
        // ... other strategies
    }
}
```

## Sources

### Primary (HIGH confidence)

| Source | Topics |
|--------|--------|
| `sqlitegraph/src/backend/native/v2/wal/checkpoint/strategies.rs` | CheckpointStrategy enum, StrategyEvaluator implementation |
| `sqlitegraph/src/backend/native/v2/wal/checkpoint/core.rs` | V2WALCheckpointManager, CheckpointManagerState, evaluate_checkpoint_strategy |
| `sqlitegraph/src/backend/native/v2/wal/manager.rs` | V2WALManager, WALManagerMetrics, checkpoint triggering |
| `sqlitegraph/src/backend/native/v2/wal/checkpoint/constants.rs` | Default threshold values |
| `sqlitegraph/src/backend/native/v2/wal/mod.rs` | V2WALConfig structure |
| `sqlitegraph/src/config/native.rs` | NativeConfig structure |
| `.planning/REQUIREMENTS.md` | ACID-19 through ACID-23, CP-01 through CP-04 |

### Secondary (MEDIUM confidence)

| Source | Topics |
|--------|--------|
| `sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs` | Two-phase commit flow, commit notification points |

## Metadata

**Confidence breakdown:**
- Existing implementation: HIGH - Read source code directly
- Counter tracking gaps: HIGH - Identified missing fields through code analysis
- Configuration integration: HIGH - Clear path to add fields to NativeConfig
- Pitfalls: HIGH - Based on code analysis of existing TODOs and incomplete implementations

**Research date:** 2026-01-20
**Valid until:** 30 days (stable checkpoint module, no external dependencies)
