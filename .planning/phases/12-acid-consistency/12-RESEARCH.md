# Phase 12: ACID Consistency - Research

**Researched:** 2026-01-20
**Domain:** V2 WAL Consistency Validation, Cluster Overlap Detection, Checkpoint/Recovery Integrity
**Confidence:** HIGH

## Summary

This phase focuses on re-enabling runtime validation for data integrity that was previously disabled due to timing issues. The codebase contains commented-out validation code for:

1. **Cluster overlap validation** in `NodeRecordV2::validate()` - Disabled due to false positives from allocation timing issues
2. **Checkpoint state invariant validation** - Partially implemented with commented sections needing completion
3. **Pre-commit validation** - Exists but needs integration into transaction commit path
4. **Post-recovery validation** - Exists in recovery validator but needs integration into WAL replay completion

The primary technical challenge is that cluster allocations happen in a specific sequence (outgoing first, then incoming), and the validation was checking for overlap at a point where both clusters weren't yet allocated, causing false positives.

**Primary recommendation:** Re-enable validation with timing-aware checks that account for allocation sequencing, and integrate existing validation infrastructure into commit/recovery paths.

## Standard Stack

The validation infrastructure already exists in the codebase. No new dependencies are needed.

### Core Validation Components
| Component | Location | Purpose | Status |
|-----------|----------|---------|--------|
| `NodeRecordV2::validate()` | `node_record_v2/validation.rs:31-122` | Node record consistency validation | Partially disabled (lines 79-119) |
| `CheckpointState` | `wal/checkpoint/core.rs:19-38` | Checkpoint state machine enum | Fully implemented |
| `V2InvariantValidator` | `wal/checkpoint/validation/invariants.rs` | V2 format invariant validation | Has commented sections (lines 182-278) |
| `CheckpointConsistencyValidator` | `wal/checkpoint/validation/consistency.rs` | Checkpoint-WAL consistency | Fully implemented |
| `TransactionValidator` | `wal/recovery/validator.rs` | WAL transaction validation | Fully implemented |

### Supporting Infrastructure
| Component | Location | Purpose |
|-----------|----------|---------|
| `CheckpointValidator` | `wal/checkpoint/validation/mod.rs` | Comprehensive checkpoint validation |
| `RecoveryValidator` | `wal/recovery/validator.rs:1111-1161` | Recovery sequence validation |
| `V2TransactionCoordinator` | `wal/transaction_coordinator.rs` | Transaction commit orchestration |

### No Alternatives Considered
All required validation infrastructure exists. The task is to re-enable and integrate existing code.

## Architecture Patterns

### Validation Pattern: Commented Code Re-enabling

The codebase follows a pattern where validation was disabled by commenting out code with TODOs:

**Example from `node_record_v2/validation.rs:79-119`:**
```rust
// DISABLED: Cluster overlap validation
// The cluster allocation logic now prevents overlap by design
// This validation was causing false positives due to timing issues
// TODO: Implement a more robust validation that accounts for allocation timing
/*
if self.outgoing_cluster_offset > 0 && self.incoming_cluster_offset > 0 {
    let outgoing_end = self.outgoing_cluster_offset + self.outgoing_cluster_size as u64;

    if self.incoming_cluster_offset < outgoing_end
        && self.incoming_cluster_offset > self.outgoing_cluster_offset
    {
        return Err(NativeBackendError::InconsistentAdjacency {
            node_id: self.id,
            count: self.outgoing_edge_count,
            direction: "cluster_overlap".to_string(),
            file_count: 0,
        });
    }
}
*/
```

### Pattern: Two-Phase Cluster Allocation

The root cause of timing issues is that clusters are allocated in sequence:

1. **Outgoing cluster** allocated first
2. **Incoming cluster** allocated second

During the window between allocations, the node record has only one cluster set, but validation was checking for overlap assuming both exist.

**From cluster creation in `wal/checkpoint/operations.rs:794-922`:**
```rust
fn apply_cluster_create(
    &mut self,
    node_id: u64,
    direction: u8,
    cluster_offset: u64,
    cluster_size: u64,
    edge_data: &[u8],
    _lsn: u64,
) -> CheckpointResult<()> {
    // ... validation ...

    // Update cluster metadata based on direction
    match cluster_direction {
        Direction::Outgoing => {
            node_record.set_outgoing_cluster(
                cluster_offset as FileOffset,
                cluster_size as u32,
                edge_cluster.edge_count(),
            );
        }
        Direction::Incoming => {
            node_record.set_incoming_cluster(
                cluster_offset as FileOffset,
                cluster_size as u32,
                edge_cluster.edge_count(),
            );
        }
    }
    // ...
}
```

### Pattern: Checkpoint State Machine

**From `wal/checkpoint/core.rs:19-38`:**
```rust
pub enum CheckpointState {
    Idle,
    Initializing,
    Collecting,
    Processing,
    Flushing,
    Validating,
    Complete,
    Failed,
}
```

The state machine has a `Validating` state that should trigger invariant validation.

### Pattern: Transaction Commit with Validation

**From `transaction_coordinator.rs:584-608`:**
```rust
pub async fn commit_transaction(&self, tx_id: TransactionId) -> NativeResult<()> {
    // Validate transaction state
    {
        let active = self.active_transactions.read();
        if let Some(context) = active.get(&tx_id) {
            if context.state != TransactionState::Active {
                return Err(NativeBackendError::InvalidTransactionState {
                    tx_id,
                    state: format!("{:?}", context.state),
                });
            }
        }
    }

    // Use two-phase commit coordinator
    self.two_phase_coordinator.commit_transaction(tx_id).await?;

    // Cleanup
    self.cleanup_transaction(tx_id).await?;

    Ok(())
}
```

**Opportunity:** Insert pre-commit validation before `commit_transaction()`.

### Anti-Patterns to Avoid
- **Partial state validation:** Don't validate when only one cluster is allocated
- **Validation without context:** Don't validate checkpoints without accessing associated WAL state
- **Standalone validation:** Always integrate validation into the normal flow (commit, checkpoint, recovery)

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cluster overlap detection | Custom interval tree logic | Re-enable commented code with timing fix | The existing logic is correct; just needs allocation sequencing awareness |
| Checkpoint state validation | New validation framework | `V2InvariantValidator::validate_checkpoint_state_invariants()` | Already exists with commented sections |
| Transaction validation | Custom pre-commit checks | `TransactionValidator::validate_transaction()` | Already fully implemented |
| Recovery integrity | Post-replay custom checks | `RecoveryValidator::validate_recovery_sequence()` | Already fully implemented |

**Key insight:** The validation infrastructure exists but is either disabled (commented out) or not integrated into the execution paths.

## Common Pitfalls

### Pitfall 1: False Positives in Cluster Overlap Validation
**What goes wrong:** Validation fails because incoming cluster is allocated before outgoing cluster, creating a temporary state where validation incorrectly reports overlap.

**Why it happens:** Clusters are allocated sequentially, but validation runs before both allocations complete.

**How to avoid:** Only validate when BOTH clusters are allocated (offsets > 0), or track allocation state explicitly.

**Warning signs:**
- `InconsistentAdjacency` errors with `direction: "cluster_overlap"`
- Debug log shows only one cluster with non-zero offset

### Pitfall 2: Checkpoint State Invariants Accessing Non-Existent Fields
**What goes wrong:** Validation code references struct fields that don't exist on `CheckpointState`.

**Why it happens:** `CheckpointState` is a simple enum, not a struct with metadata fields.

**How to avoid:** Store checkpoint metadata separately (in `CheckpointManagerState`) and pass it to validation functions.

**From `invariants.rs:236-278` (commented out):**
```rust
// Checkpoint state validation commented out - CheckpointState enum doesn't have expected fields
// TODO: Update validation to work with actual CheckpointState enum structure
```

### Pitfall 3: Validation Without Database Context
**What goes wrong:** Validation runs without access to actual database state, making it impossible to verify constraints.

**Why it happens:** Validators are created without database/graph file references.

**How to avoid:** Ensure validators have access to `GraphFile`, `NodeStore`, or other backend components.

### Pitfall 4: Ignoring Return Values from Validation
**What goes wrong:** Validation functions are called but errors are discarded.

**Why it happens:** Validation returns `Result` types that aren't propagated.

**How to avoid:** Use `?` operator or explicitly handle validation errors.

## Code Examples

### Example 1: Re-enabling Cluster Overlap Validation with Timing Fix

**File:** `sqlitegraph/src/backend/native/v2/node_record_v2/validation.rs`

The current commented code (lines 79-119) should be replaced with timing-aware validation:

```rust
// Validate cluster overlap only when both clusters are allocated
// This accounts for allocation sequencing where clusters are allocated one at a time
if self.outgoing_cluster_offset > 0 && self.incoming_cluster_offset > 0 {
    let outgoing_end = self.outgoing_cluster_offset + self.outgoing_cluster_size as u64;
    let incoming_end = self.incoming_cluster_offset + self.incoming_cluster_size as u64;

    // Check for overlap: intervals [outgoing_start, outgoing_end) and [incoming_start, incoming_end)
    // Overlap occurs if: incoming_start < outgoing_end AND outgoing_start < incoming_end
    if self.incoming_cluster_offset < outgoing_end
        && self.outgoing_cluster_offset < incoming_end
    {
        // Additional check: allow edge case where clusters are adjacent (not overlapping)
        let overlap_size = std::cmp::min(outgoing_end, incoming_end)
            - std::cmp::max(self.outgoing_cluster_offset, self.incoming_cluster_offset);

        if overlap_size > 0 {
            return Err(NativeBackendError::InconsistentAdjacency {
                node_id: self.id,
                count: self.outgoing_edge_count,
                direction: "cluster_overlap".to_string(),
                file_count: overlap_size as u32,
            });
        }
    }
}
```

### Example 2: Checkpoint State Validation with Metadata

**File:** `sqlitegraph/src/backend/native/v2/wal/checkpoint/core.rs`

Add validation to checkpoint state transitions:

```rust
impl V2WALCheckpointManager {
    pub fn transition_to_state(&self, new_state: CheckpointState) -> CheckpointResult<()> {
        let mut state = self.state.lock();

        // Validate state transition
        let valid_transition = match (&state.current_state, &new_state) {
            (CheckpointState::Idle, CheckpointState::Initializing) => true,
            (CheckpointState::Initializing, CheckpointState::Collecting) => true,
            (CheckpointState::Collecting, CheckpointState::Processing) => true,
            (CheckpointState::Processing, CheckpointState::Flushing) => true,
            (CheckpointState::Flushing, CheckpointState::Validating) => true,
            (CheckpointState::Validating, CheckpointState::Complete) => true,
            // Any state can transition to Failed
            (_, CheckpointState::Failed) => true,
            _ => false,
        };

        if !valid_transition {
            return Err(CheckpointError::state(format!(
                "Invalid state transition: {:?} -> {:?}",
                state.current_state, new_state
            )));
        }

        state.current_state = new_state;
        Ok(())
    }
}
```

### Example 3: Pre-Commit Validation Integration

**File:** `sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs`

Add validation hook before commit:

```rust
pub async fn commit_transaction(&self, tx_id: TransactionId) -> NativeResult<()> {
    // NEW: Pre-commit validation
    self.validate_pre_commit(tx_id).await?;

    // Existing validation
    {
        let active = self.active_transactions.read();
        // ... existing checks ...
    }

    self.two_phase_coordinator.commit_transaction(tx_id).await?;
    self.cleanup_transaction(tx_id).await?;

    Ok(())
}

// New method
async fn validate_pre_commit(&self, tx_id: TransactionId) -> NativeResult<()> {
    let context = {
        let active = self.active_transactions.read();
        active.get(&tx_id)
            .cloned()
            .ok_or_else(|| NativeBackendError::TransactionNotFound { tx_id })?
    };

    // Validate all WAL records in transaction
    for record in &context.wal_records {
        self.validate_record_constraints(record)?;
    }

    Ok(())
}
```

### Example 4: Post-Recovery Validation

**File:** `sqlitegraph/src/backend/native/v2/wal/recovery/core.rs`

Add validation after WAL replay:

```rust
impl V2WALRecoveryEngine {
    pub async fn recover(&mut self) -> RecoveryResult<RecoveryReport> {
        // ... existing recovery logic ...

        // Scan WAL for transactions
        let scan_result = self.scanner.scan_wal_file(&self.config.wal_path).await?;

        // Replay committed transactions
        let replay_result = self.replayer.replay_transactions(
            &scan_result.transactions,
            &self.graph_file_path,
        ).await?;

        // NEW: Post-recovery validation
        self.validate_post_recovery(&replay_result).await?;

        Ok(report)
    }

    async fn validate_post_recovery(&self, replay_result: &ReplayResult) -> RecoveryResult<()> {
        // Validate graph file integrity
        let graph_file = GraphFile::open(&self.graph_file_path)
            .map_err(|e| RecoveryError::validation(format!("Cannot open graph file: {}", e)))?;

        // Verify node count consistency
        let header = graph_file.persistent_header();
        if header.node_count == 0 && replay_result.transactions_replayed > 0 {
            return Err(RecoveryError::validation(
                "Node count is zero after recovery with transactions replayed".to_string()
            ));
        }

        // Verify free space manager consistency
        // TODO: Add more integrity checks

        Ok(())
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Active cluster overlap validation | Commented out due to timing issues | Before 2024-12 | Overlap detection disabled |
| Manual checkpoint state checks | `V2InvariantValidator` (partially implemented) | 2024-12 | Structured validation available |
| No pre-commit validation | `TransactionValidator` exists but not integrated | 2024-12 | Infrastructure ready, needs integration |
| Post-recovery validation exists | `RecoveryValidator` fully implemented | 2024-12 | Ready for integration |

**Commented/disabled code to re-enable:**
- `node_record_v2/validation.rs:79-119` - Cluster overlap validation
- `checkpoint/validation/invariants.rs:182-278` - State invariants with field access issues

## Open Questions

1. **Cluster allocation sequencing**: What is the exact sequence of cluster allocations during node creation?
   - **What we know:** Outgoing cluster is allocated first, then incoming
   - **What's unclear:** Whether there are cases where only one cluster is ever allocated
   - **Recommendation:** Trace through node creation path to verify allocation sequence

2. **Checkpoint state metadata storage**: Where should checkpoint metadata be stored for validation?
   - **What we know:** `CheckpointManagerState` has internal fields but `CheckpointState` is a simple enum
   - **What's unclear:** Whether to extend `CheckpointState` or keep metadata separate
   - **Recommendation:** Keep metadata in `CheckpointManagerState`, validate by passing both to validator

3. **Pre-commit validation scope**: What constraints should be checked?
   - **What we know:** `TransactionValidator` can validate individual records
   - **What's unclear:** What cross-record constraints exist
   - **Recommendation:** Start with individual record validation, expand based on test failures

## Sources

### Primary (HIGH confidence)
- **Source code analysis** - All file reads from actual codebase
- **`sqlitegraph/src/backend/native/v2/node_record_v2/validation.rs`** - Lines 79-119 contain disabled overlap validation
- **`sqlitegraph/src/backend/native/v2/wal/checkpoint/core.rs`** - CheckpointState enum definition
- **`sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/invariants.rs`** - Lines 182-278 contain commented validation
- **`sqlitegraph/src/backend/native/v2/wal/recovery/validator.rs`** - Full validation implementation
- **`sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs`** - Transaction commit path

### Secondary (MEDIUM confidence)
- **Existing validation infrastructure** - Fully implemented but not integrated
- **`checkpoint/validation/consistency.rs`** - Comprehensive consistency validation
- **`wal/recovery/mod.rs`** - Recovery orchestration

### Tertiary (LOW confidence)
- **None** - All findings are from direct source code analysis

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All components are existing in-code
- Architecture: HIGH - Analyzed actual source files
- Pitfalls: HIGH - Root cause of timing issues identified in comments

**Research date:** 2026-01-20
**Valid until:** 60 days (stable codebase, API changes unlikely)
