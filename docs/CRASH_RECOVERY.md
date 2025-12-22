# Formal Crash Recovery System for SQLiteGraph V2 WAL

## Overview

This document describes SQLiteGraph's formal, first-class crash recovery system for the V2 native backend WAL (Write-Ahead Logging) system. The recovery system has been designed and implemented following strict professional development methodology with TDD principles, ensuring deterministic behavior and comprehensive crash recovery semantics.

## Architecture

### Core Components

1. **Explicit Recovery State Model** (`states.rs`)
   - Deterministic state machine using only existing signals
   - No new heuristics or assumptions introduced
   - States: CleanShutdown, DirtyShutdown, PartialCheckpoint, CorruptWAL, CorruptGraphFile, Unrecoverable

2. **Authority Resolution Contract** (implemented in states.rs)
   - Clear, explicit rules for WAL vs graph file precedence
   - Deterministic decision matrix with no ambiguity
   - Enforced in code and validated by tests

3. **Recovery Coordinator** (`coordinator.rs`)
   - Centralized orchestration using existing WAL reader + replayer
   - No duplicated recovery logic - delegates to existing components
   - Structured result reporting with comprehensive diagnostics

4. **Comprehensive Test Suite** (`test_cases.rs`)
   - All tests initially FAIL (TDD methodology)
   - Real graph files + WALs with actual crash simulation
   - Validates recovery decisions, authority resolution, and replay behavior

### Recovery State Machine

```
┌─────────────────────────┐
│        File Analysis        │
└─────────────┬─────────────┘
              │
              ▼
    ┌─────────────────────────┐
    │  Determine Recovery State │
    │  (using existing signals) │
    └─────────────┬─────────────┘
              │
              ▼
    ┌─────────────────────────┐
    │  Authority Resolution     │
    │  (explicit decision rules)│
    └─────────────┬─────────────┘
              │
              ▼
    ┌─────────────────────────┐
    │ Recovery Orchestration   │
    │ (existing WAL + replayer) │
    └─────────────┬─────────────┘
              │
              ▼
    ┌─────────────────────────┐
    │   Validation & Reporting   │
    └─────────────────────────┘
```

## Recovery States

### CleanShutdown
**Detection**: `committed_lsn == current_lsn` AND `active_transactions == 0`
- **Authority**: GraphFile
- **Action**: No recovery needed
- **Severity**: None

### DirtyShutdown
**Detection**: `active_transactions > 0` OR `committed_lsn < current_lsn`
- **Authority**: WAL
- **Action**: Replay uncommitted transactions, rollback incomplete ones
- **Severity**: Low

### PartialCheckpoint
**Detection**: `checkpoint_exists` AND `checkpointed_lsn < committed_lsn`
- **Authority**: WAL
- **Action**: Resume checkpoint completion, replay pending transactions
- **Severity**: Medium

### CorruptWAL
**Detection**: Invalid WAL magic bytes, version mismatch, or header validation failure
- **Authority**: Unrecoverable
- **Action**: System cannot be recovered
- **Severity**: Critical

### CorruptGraphFile
**Detection**: Graph file missing or header validation failure
- **Authority**: Unrecoverable
- **Action**: System cannot be recovered
- **Severity**: Critical

### Unrecoverable
**Detection**: Both WAL and graph file corrupted or missing
- **Authority**: Unrecoverable
- **Action**: System cannot be recovered
- **Severity**: Critical

## Authority Resolution Contract

### Decision Rules

The authority resolution follows these explicit, deterministic rules:

| Recovery State | Authority | Recovery Required | Action |
|----------------|------------|-------------------|--------|
| CleanShutdown | GraphFile | ❌ No | Use graph file as-is |
| DirtyShutdown | WAL | ✅ Yes | Replay WAL to graph file |
| PartialCheckpoint | WAL | ✅ Yes | Resume checkpoint |
| CorruptWAL | Unrecoverable | ❌ No | System unrecoverable |
| CorruptGraphFile | Unrecoverable | ❌ No | System unrecoverable |
| Unrecoverable | Unrecoverable | ❌ No | System unrecoverable |

### Authority Implementation

```rust
pub enum Authority {
    /// WAL file has authority (replay WAL to graph file)
    WAL,

    /// Graph file has authority (ignore WAL, use graph file as-is)
    GraphFile,

    /// Both are corrupt - unrecoverable
    Unrecoverable,
}
```

### Authority Resolution Logic

```rust
pub fn determine_from_recovery_state(state: RecoveryState) -> Authority {
    match state {
        RecoveryState::CleanShutdown => Authority::GraphFile,
        RecoveryState::DirtyShutdown => Authority::WAL,
        RecoveryState::PartialCheckpoint => Authority::WAL,
        RecoveryState::CorruptWAL => Authority::Unrecoverable,
        RecoveryState::CorruptGraphFile => Authority::Unrecoverable,
        RecoveryState::Unrecoverable => Authority::Unrecoverable,
    }
}
```

## Recovery Signal Detection

### WAL Header Analysis

The system uses ONLY existing signals from `V2WALHeader`:

```rust
pub struct V2WALHeader {
    pub magic: [u8; 8],           // Format validation
    pub version: u32,            // Version compatibility
    pub current_lsn: u64,         // Current LSN in WAL
    pub committed_lsn: u64,       // Highest committed LSN
    pub checkpointed_lsn: u64,    // Highest checkpointed LSN
    pub active_transactions: u32,  // Active transaction count
    pub flags: u32,              // Feature flags
}
```

### State Detection Logic

```rust
// Active transactions indicate dirty shutdown
if header.active_transactions > 0 {
    return RecoveryState::DirtyShutdown;
}

// Uncommitted records indicate dirty shutdown
if header.committed_lsn < header.current_lsn {
    return RecoveryState::DirtyShutdown;
}

// Partial checkpoint detection
if checkpoint_exists && header.checkpointed_lsn < header.committed_lsn {
    return RecoveryState::PartialCheckpoint;
}

// Clean shutdown detection
if header.checkpointed_lsn == header.committed_lsn && header.active_transactions == 0 {
    return RecoveryState::CleanShutdown;
}
```

## Recovery Orchestration

### Recovery Coordinator API

```rust
let coordinator = RecoveryCoordinator::new(config, database_path, checkpoint_path);
let result = coordinator.orchestrate_recovery()?;

match result.decision {
    RecoveryDecision::NoRecoveryNeeded => {
        println!("System is clean - no recovery needed");
    }
    RecoveryDecision::RecoveryPerformed => {
        println!("Recovery completed in {:?}", result.duration);
        println!("Recovered {} transactions", result.metrics?.committed_transactions_replayed);
    }
    RecoveryDecision::Unrecoverable => {
        println!("System is unrecoverable - manual intervention required");
    }
}
```

### Recovery Workflow

1. **Analysis Phase** (No side effects)
   - Read WAL header and validate integrity
   - Analyze LSN relationships
   - Check file existence and accessibility

2. **Decision Phase** (Deterministic rules)
   - Apply explicit authority resolution contract
   - Make recovery decision based on state
   - Validate decision consistency

3. **Orchestration Phase** (Existing components)
   - Use existing `V2WALRecoveryEngine` for recovery
   - Delegate to existing `V2WALReader` for WAL access
   - Utilize existing replayer for transaction replay

4. **Validation Phase** (Post-recovery checks)
   - Verify recovery completion
   - Validate system consistency
   - Generate structured diagnostics

## Test Coverage

### Required Test Scenarios

1. **test_recovery_clean_shutdown_no_replay**
   - Validates clean shutdown detection
   - Ensures no recovery is performed
   - Verifies graph file authority

2. **test_recovery_dirty_wal_replay**
   - Simulates crash with uncommitted transactions
   - Validates WAL authority and replay
   - Verifies transaction rollback

3. **test_recovery_partial_checkpoint_resume**
   - Simulates interrupted checkpoint
   - Validates checkpoint resumption
   - Verifies WAL authority

4. **test_recovery_uncommitted_transaction_rollback**
   - Tests explicit transaction rollback
   - Validates rollback during recovery
   - Verifies consistency

5. **test_recovery_corrupt_wal_detection**
   - Simulates WAL corruption
   - Validates corruption detection
   - Verifies unrecoverable state

6. **test_recovery_authority_resolution**
   - Tests all authority resolution scenarios
   - Validates decision matrix
   - Ensures deterministic behavior

### Test Implementation Approach

All tests follow this pattern:

```rust
// Create real graph file + WAL
let _graph_file = GraphFile::create(&graph_path)?;
let manager = V2WALManager::create(config)?;

// Perform real operations
let tx_id = manager.begin_transaction(TransactionIsolation::ReadCommitted)?;
manager.write_transaction_record(tx_id, record)?;

// Simulate crash (drop without commit)
drop(manager);

// Analyze recovery context
let context = RecoveryContext::analyze_files(&wal_path, &graph_path, &checkpoint_path)?;

// Assert recovery decision
assert_eq!(context.state, ExpectedState);
assert_eq!(context.authority, ExpectedAuthority);
```

## Guarantees

### Deterministic Behavior
- **Same inputs → Same outputs**: Identical file states always produce identical recovery decisions
- **No random factors**: All decisions based on deterministic signal analysis
- **Reproducible results**: Recovery can be safely repeated

### No Side Effects During Inspection
- **Read-only analysis**: Recovery state detection never modifies files
- **Non-destructive**: Authority resolution uses only read operations
- **Safe inspection**: File analysis cannot corrupt data

### Idempotent Recovery
- **Safe replay**: Recovery can be repeated without adverse effects
- **Idempotent operations**: Multiple recovery attempts yield same final state
- **Rollback support**: Incomplete operations can be safely rolled back

### Complete Crash Recovery
- **WAL replay**: All committed transactions are recovered
- **Transaction integrity**: Partial transactions are properly rolled back
- **Consistency validation**: System consistency is verified post-recovery

## Performance Characteristics

### Recovery Time Complexity
- **Analysis**: O(1) - Header validation only
- **Decision**: O(1) - Simple state machine lookup
- **Recovery**: O(n) - Proportional to WAL size and transaction count
- **Validation**: O(1) - Post-recovery consistency checks

### Memory Usage
- **Minimal overhead**: Coordinator maintains only state information
- **No buffering**: Uses existing WAL reader and replayer components
- **Scalable**: Memory usage scales with transaction complexity

### I/O Patterns
- **Sequential access**: Leverages existing WAL sequential reads
- **Batch processing**: Uses existing replayer batch optimization
- **Minimal seeks**: Optimized for typical recovery workloads

## Non-Goals

### What Recovery Never Does

- **Speculative recovery**: Never attempts recovery on ambiguous states
- **Data reconstruction**: Does not attempt to recover corrupted data
- **Version migration**: Does not handle schema version upgrades
- **Performance optimization**: Does not optimize recovery performance beyond correctness
- **Partial recovery**: Never performs partial or incomplete recovery

### Recovery Limitations

- **Magic byte dependency**: Requires valid WAL magic bytes
- **Header validation**: Depends on intact WAL headers
- **File system integrity**: Assumes underlying file system is reliable
- **Transaction boundaries**: Cannot recover transactions without proper begin/commit markers

## Integration Points

### Existing V2 WAL Components

The recovery system integrates seamlessly with existing V2 WAL infrastructure:

- **V2WALManager**: Used for creating test scenarios and normal operations
- **V2WALReader**: Leveraged for WAL file analysis during recovery
- **V2WALRecoveryEngine**: Used for actual recovery orchestration
- **V2GraphFileReplayer**: Used for transaction replay to graph file
- **V2WALCheckpointManager**: Used for checkpoint consistency validation

### Graph File Integration

- **GraphFile**: Target for recovery operations
- **NodeRecordV2**: Native V2 node format support
- **EdgeCluster**: V2 clustered edge format integration
- **StringTable**: V2 string table management
- **FreeSpaceManager**: V2 free space management

## Implementation Files

### Core Implementation (≤300 LOC each)

- **`states.rs`** (280 lines) - Explicit recovery state model and authority resolution
- **`coordinator.rs` (298 lines) - Recovery orchestration coordinator
- **`test_cases.rs` (293 lines) - Comprehensive TDD test suite

### Module Integration

- **`mod.rs`** - Module exports and type re-exports
- **Integration with existing** `core.rs`, `replayer.rs`, `scanner.rs`, `validator.rs`

## Usage Examples

### Basic Recovery

```rust
use sqlitegraph::backend::native::v2::wal::recovery::{RecoveryCoordinator};

// Create coordinator
let config = V2WALConfig::for_graph_file(&database_path);
let coordinator = RecoveryCoordinator::new(config, database_path, checkpoint_path);

// Perform recovery
let result = coordinator.orchestrate_recovery()?;

match result.decision {
    RecoveryDecision::NoRecoveryNeeded => {
        println!("System is clean");
    }
    RecoveryDecision::RecoveryPerformed => {
        println!("Recovery completed successfully");
        println!("Duration: {:?}", result.duration);
    }
    RecoveryDecision::Unrecoverable => {
        println!("System recovery failed");
    }
}
```

### Custom Recovery Analysis

```rust
// Analyze recovery context without performing recovery
let context = RecoveryContext::analyze_files(&wal_path, &graph_path, &checkpoint_path)?;

println!("Recovery State: {}", context.state);
println!("Authority: {}", context.authority);
println!("Severity: {}", context.state.severity());

for diagnostic in context.diagnostics {
    println!("Diagnostic: {}", diagnostic);
}
```

## Summary

The SQLiteGraph V2 WAL crash recovery system provides:

✅ **Explicit Recovery State Model** - Deterministic states with clear definitions
✅ **Authority Resolution Contract** - Explicit rules for WAL vs graph file precedence
✅ **Comprehensive TDD Test Suite** - All required scenarios with failing-first approach
✅ **Production-Ready Orchestration** - Centralized coordinator using existing components
✅ **Deterministic Behavior** - Same inputs always produce same outputs
✅ **Complete Crash Recovery** - Full transaction recovery with rollback support

The system transforms crash recovery from "implicitly tested behavior" to a **formal, first-class subsystem** with explicit states, deterministic decision rules, and comprehensive validation.