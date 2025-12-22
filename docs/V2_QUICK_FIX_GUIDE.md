# V2 Modularization Quick Fix Guide

## Critical First Fixes (Build Will Not Work Without These)

### 1. Recovery System - TransactionState Import
**Files**: `scanner.rs`, `replayer.rs`, `validator.rs`

**Replace**:
```rust
use super::{errors::RecoveryError, constants::*, TransactionState};
```

**With**:
```rust
use super::{errors::RecoveryError, constants::*};
use crate::backend::native::v2::wal::recovery::core::TransactionState;
```

### 2. RecoveryResult Generic Type Fixes
**File**: `sqlitegraph/src/backend/native/v2/wal/recovery/mod.rs`

**Replace**:
```rust
-> RecoveryResult<V2WALRecoveryEngine>
-> RecoveryResult<()>
```

**With**:
```rust
-> Result<V2WALRecoveryEngine, RecoveryError>
-> Result<(), RecoveryError>
```

### 3. Checkpoint Constants Import
**File**: `sqlitegraph/src/backend/native/v2/wal/checkpoint/strategies.rs`

**Add after line 6**:
```rust
use crate::backend::native::v2::wal::checkpoint::constants::strategies::*;
```

### 4. V2WALRecord Import
**File**: `sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs`

**Add after line 14**:
```rust
use crate::backend::native::v2::V2WALRecord;
```

### 5. Module Visibility Fixes
**File**: `sqlitegraph/src/backend/native/v2/edge_cluster/mod.rs`

**Replace**:
```rust
mod cluster_trace;
```

**With**:
```rust
pub mod cluster_trace;
```

### 6. Error Variant Updates
**File**: `sqlitegraph/src/backend/native/v2/wal/reader.rs`

**Replace**:
```rust
NativeBackendError::IoError { ... }
NativeBackendError::InvalidState { ... }
```

**With**:
```rust
NativeBackendError::Io { ... }
NativeBackendError::InvalidHeader { ... }
```

## One-Line Fixes Summary

```bash
# Fix RecoveryState imports
sed -i 's/use super::{errors::RecoveryError, constants::\*, TransactionState};/use super::{errors::RecoveryError, constants::*};\nuse crate::backend::native::v2::wal::recovery::core::TransactionState;/' sqlitegraph/src/backend/native/v2/wal/recovery/scanner.rs sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs

# Fix RecoveryResult generics
sed -i 's/RecoveryResult</Result</g' sqlitegraph/src/backend/native/v2/wal/recovery/mod.rs
sed -i 's/RecoveryResult<()>)/Result<(), RecoveryError>/' sqlitegraph/src/backend/native/v2/wal/recovery/mod.rs

# Add checkpoint constants import
sed -i '7i use crate::backend::native::v2::wal::checkpoint::constants::strategies::*;' sqlitegraph/src/backend/native/v2/wal/checkpoint/strategies.rs

# Add V2WALRecord import
sed -i '15i use crate::backend::native::v2::V2WALRecord;' sqlitegraph/src/backend/native/v2/wal/recovery/replayer.rs

# Make cluster_trace public
sed -i 's/mod cluster_trace;/pub mod cluster_trace;/' sqlitegraph/src/backend/native/v2/edge_cluster/mod.rs
```

## Verification Commands

```bash
# Test if basic fixes work
cargo check --workspace 2>&1 | head -20

# Count remaining errors
cargo check 2>&1 | grep "error\[" | wc -l

# Focus on specific error types
cargo check 2>&1 | grep "E0432"  # Unresolved imports
cargo check 2>&1 | grep "E0425"  # Not found in scope
cargo check 2>&1 | grep "E0603"  # Private module
```

## Build Order

1. Apply critical fixes above (5-10 minutes)
2. Run `cargo check` to verify major issues resolved
3. Address remaining import/export issues (30-60 minutes)
4. Fix error variant issues (60-90 minutes)
5. Clean up warnings (30 minutes)

Expected error count progression:
- Start: 306 errors
- After critical fixes: ~50-100 errors
- After import fixes: ~20-50 errors
- After error fixes: ~5-10 errors
- Final cleanup: 0 errors

This guide focuses on the 20% of fixes that will resolve 80% of build errors.