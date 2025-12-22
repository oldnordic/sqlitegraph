# Recovery Errors Modularization - Developer Migration Guide

## Overview

This guide helps developers migrate their code to work with the new modularized `recovery/errors` system. The modularization maintains 100% backward compatibility while providing enhanced features and improved code organization.

## Quick Start

### Your Existing Code Still Works
```rust
// This continues to work without any changes
use crate::backend::native::v2::wal::recovery::errors::RecoveryError;

fn handle_recovery_error() -> RecoveryResult<()> {
    Err(RecoveryError::configuration("Invalid configuration"))
}
```

### Recommended New Import Patterns
```rust
// Preferred modern approach - more explicit about what you're using
use crate::backend::native::v2::wal::recovery::errors::{
    RecoveryError, RecoveryErrorKind, ErrorContext, RecoverySuggestion
};

// For specific functionality
use crate::backend::native::v2::wal::recovery::errors::core::RecoveryError;
use crate::backend::native::v2::wal::recovery::errors::context::ErrorContext;
```

## Migration Scenarios

### Scenario 1: Basic Error Creation

#### Before (Still Works)
```rust
use crate::backend::native::v2::wal::recovery::errors::RecoveryError;

fn create_error() -> RecoveryError {
    RecoveryError::io("File read failed")
}
```

#### After (Recommended)
```rust
use crate::backend::native::v2::wal::recovery::errors::RecoveryError;

fn create_error() -> RecoveryError {
    // Same API, but now you can be more specific about imports
    RecoveryError::io("File read failed")
}
```

#### Enhanced New Pattern
```rust
use crate::backend::native::v2::wal::recovery::errors::{
    RecoveryError, ErrorContext, RecoverySuggestion
};

fn create_rich_error() -> RecoveryError {
    RecoveryError::io("File read failed")
        .with_context(ErrorContext {
            wal_path: Some("/path/to/wal".to_string()),
            lsn_range: Some((1000, 2000)),
            ..Default::default()
        })
        .with_recovery(RecoverySuggestion::Retry { max_attempts: 3, backoff_ms: 1000 })
}
```

### Scenario 2: Error Handling with Context

#### Before (Still Works)
```rust
use crate::backend::native::v2::wal::recovery::errors::{RecoveryError, ErrorContext};

fn handle_wal_error(lsn: u64) -> RecoveryError {
    let mut context = ErrorContext::default();
    context.lsn_range = Some((lsn, lsn));
    context.recovery_state = Some("reading".to_string());

    RecoveryError::wal_file("Read failed").with_context(context)
}
```

#### After (Enhanced)
```rust
use crate::backend::native::v2::wal::recovery::errors::{
    RecoveryError, ErrorContext
};

fn handle_wal_error(lsn: u64) -> RecoveryError {
    // Same API, but with better organization and documentation
    RecoveryError::wal_file("Read failed")
        .with_context(
            ErrorContext::default()
                .with_lsn_range(lsn, lsn)
                .with_recovery_state("reading")
        )
}

// Or use the new builder pattern
impl ErrorContext {
    pub fn with_lsn_range(mut self, start: u64, end: u64) -> Self {
        self.lsn_range = Some((start, end));
        self
    }

    pub fn with_recovery_state(mut self, state: &str) -> Self {
        self.recovery_state = Some(state.to_string());
        self
    }
}
```

### Scenario 3: Working with Error Collections

#### Before (Still Works)
```rust
use crate::backend::native::v2::wal::recovery::errors::{RecoveryErrorCollection, RecoveryError};

fn aggregate_errors() -> RecoveryErrorCollection {
    let mut collection = RecoveryErrorCollection::new();
    collection.add_error(RecoveryError::configuration("Bad config"));
    collection.add_error(RecoveryError::io("Disk error"));
    collection
}
```

#### After (Enhanced with New Methods)
```rust
use crate::backend::native::v2::wal::recovery::errors::{
    RecoveryErrorCollection, RecoveryError, RecoveryAction
};

fn aggregate_errors() -> RecoveryErrorCollection {
    let mut collection = RecoveryErrorCollection::new();

    // Same old API
    collection.add_error(RecoveryError::configuration("Bad config"));
    collection.add_error(RecoveryError::io("Disk error"));

    // New enhanced API
    collection.add_errors(vec![
        RecoveryError::validation("Invalid data"),
        RecoveryError::corruption("Checksum mismatch")
    ]);

    // Use new analysis methods
    match collection.recommended_action() {
        RecoveryAction::Continue => println!("Recovery can continue"),
        RecoveryAction::RetryWithDelay => println!("Schedule retry"),
        RecoveryAction::ManualIntervention => println!("Escalate to operator"),
        _ => println!("Other action required"),
    }

    collection
}
```

### Scenario 4: V2-Specific Error Context

#### Before (Still Works)
```rust
use crate::backend::native::v2::wal::recovery::errors::RecoveryError;

fn create_v2_error() -> RecoveryError {
    RecoveryError::v2_integration("Cluster format error")
        .with_v2_context(
            Some(123),                    // transaction_id
            Some((456, 789)),             // cluster_key
            Some(1000),                   // node_count
            Some(2000)                    // edge_count
        )
}
```

#### After (Enhanced with Type Safety)
```rust
use crate::backend::native::v2::wal::recovery::errors::{
    RecoveryError, ErrorContext, context::V2Context
};

fn create_v2_error() -> RecoveryError {
    // Same method, but with enhanced documentation
    RecoveryError::v2_integration("Cluster format error")
        .with_v2_context(
            Some(123),                    // transaction_id
            Some((456, 789)),             // cluster_key (node_a, node_b)
            Some(1000),                   // node_count
            Some(2000)                    // edge_count
        )
}

// New typed approach (recommended for V2-specific code)
impl ErrorContext {
    pub fn with_v2_cluster_info(
        mut self,
        transaction_id: u64,
        cluster_key: (u64, u64),
        stats: (Option<u64>, Option<u64>), // (node_count, edge_count)
    ) -> Self {
        self.transaction_id = Some(transaction_id);
        self.metadata.insert("cluster_key".to_string(),
            format!("{}-{}", cluster_key.0, cluster_key.1));

        if let Some(node_count) = stats.0 {
            self.metadata.insert("node_count".to_string(),
                node_count.to_string());
        }

        if let Some(edge_count) = stats.1 {
            self.metadata.insert("edge_count".to_string(),
                edge_count.to_string());
        }

        self
    }
}
```

## New Enhanced Features

### 1. Intelligent Recovery Suggestions

```rust
use crate::backend::native::v2::wal::recovery::errors::{
    RecoveryError, RecoverySuggestion
};

fn create_smart_error() -> RecoveryError {
    let error = RecoveryError::timeout("WAL operation timed out");

    // New: Automatic retry delay calculation
    if let Some(delay_ms) = error.retry_delay_ms() {
        println!("Suggested retry delay: {}ms", delay_ms);
    }

    // New: Check if manual intervention is needed
    if error.requires_manual_intervention() {
        escalate_to_operator(&error);
    }

    error
}
```

### 2. Enhanced Error Diagnostics

```rust
use crate::backend::native::v2::wal::recovery::errors::RecoveryError;

fn enhanced_diagnostics(error: &RecoveryError) {
    // New: Rich diagnostic reporting
    let report = error.diagnostic_report();
    println!("{}", report);

    // New: Structured context access
    if let Some((start_lsn, end_lsn)) = error.context.lsn_range {
        println!("Error occurred in LSN range: {}-{}", start_lsn, end_lsn);
    }

    if let Some(progress) = error.context.recovery_progress_percentage {
        println!("Recovery was {:.1}% complete when error occurred", progress);
    }

    // New: Error severity classification
    match error.severity() {
        ErrorSeverity::Warning => println!("Warning level error"),
        ErrorSeverity::Error => println!("Error level error"),
        ErrorSeverity::Critical => println!("Critical error - immediate attention required"),
    }
}
```

### 3. Collection Intelligence

```rust
use crate::backend::native::v2::wal::recovery::errors::{
    RecoveryErrorCollection, RecoveryAction, ErrorSeverity
};

fn intelligent_error_handling(collection: &RecoveryErrorCollection) {
    // New: Automatic action recommendation
    let action = collection.recommended_action();

    match action {
        RecoveryAction::Continue => {
            println!("All errors are recoverable - continuing recovery");
        },
        RecoveryAction::RetryWithDelay => {
            let delay = collection.retry_delay_ms().unwrap_or(5000);
            println!("Retrying recovery in {}ms", delay);
        },
        RecoveryAction::ManualIntervention => {
            println!("Manual intervention required");
            println!("{}", collection.detailed_report());
        },
        RecoveryAction::Abort => {
            println!("Recovery must be aborted due to critical errors");
        },
        _ => {}
    }

    // New: Error analysis
    let (warnings, errors, critical) = collection.count_by_severity();
    println!("Error breakdown: {} warnings, {} errors, {} critical",
             warnings, errors, critical);

    // New: Recovery estimation
    if collection.has_unrecoverable_errors() {
        println!("Some errors cannot be automatically recovered");
    }
}
```

## Testing Your Migration

### 1. Compatibility Testing

```rust
// Test that your existing code still works
#[cfg(test)]
mod compatibility_tests {
    use super::*;
    use crate::backend::native::v2::wal::recovery::errors::{
        RecoveryError, RecoveryErrorKind, ErrorContext
    };

    #[test]
    fn test_existing_api_still_works() {
        // All existing constructors should work
        let error = RecoveryError::configuration("test");
        assert_eq!(error.kind, RecoveryErrorKind::Configuration);

        // All existing methods should work
        let error = RecoveryError::io("test")
            .with_context(ErrorContext::default())
            .with_recovery(RecoverySuggestion::None);

        assert!(error.is_recoverable());
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_existing_imports_still_work() {
        // Original import path should still work
        use crate::backend::native::v2::wal::recovery::errors::RecoveryError as LegacyError;

        let error = LegacyError::corruption("test");
        assert_eq!(error.kind, RecoveryErrorKind::Corruption);
    }
}
```

### 2. Performance Testing

```rust
// Test that performance hasn't degraded
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_error_creation_performance() {
        let start = Instant::now();

        for _ in 0..10_000 {
            let _ = RecoveryError::io("benchmark test");
        }

        let duration = start.elapsed();
        println!("10,000 error creations took: {:?}", duration);

        // Should be under 100ms (adjust threshold as needed)
        assert!(duration.as_millis() < 100);
    }

    #[test]
    fn test_collection_performance() {
        let mut collection = RecoveryErrorCollection::new();
        let start = Instant::now();

        for i in 0..1_000 {
            collection.add_error(RecoveryError::io(format!("Error {}", i)));
        }

        let duration = start.elapsed();
        println!("1,000 errors added to collection in: {:?}", duration);

        assert!(duration.as_millis() < 50);
    }
}
```

## Common Migration Patterns

### Pattern 1: Gradual Import Migration

```rust
// Phase 1: Keep existing imports, start using new features
mod my_module {
    use crate::backend::native::v2::wal::recovery::errors::RecoveryError;

    fn my_function() -> RecoveryError {
        // Use existing API
        RecoveryError::io("test")
    }
}

// Phase 2: Add new imports for enhanced features
mod my_module {
    use crate::backend::native::v2::wal::recovery::errors::{
        RecoveryError, ErrorContext, RecoverySuggestion
    };

    fn my_function() -> RecoveryError {
        // Use enhanced API
        RecoveryError::io("test")
            .with_context(ErrorContext::default())
            .with_recovery(RecoverySuggestion::Retry { max_attempts: 3, backoff_ms: 1000 })
    }
}

// Phase 3: Use module-specific imports (optional)
mod my_module {
    use crate::backend::native::v2::wal::recovery::errors::core::RecoveryError;
    use crate::backend::native::v2::wal::recovery::errors::context::ErrorContext;
    use crate::backend::native::v2::wal::recovery::errors::recovery::RecoverySuggestion;

    fn my_function() -> RecoveryError {
        // Same API, more explicit imports
        RecoveryError::io("test")
            .with_context(ErrorContext::default())
            .with_recovery(RecoverySuggestion::Retry { max_attempts: 3, backoff_ms: 1000 })
    }
}
```

### Pattern 2: Error Handling Enhancement

```rust
// Before: Basic error handling
fn handle_wal_read_error(path: &str, lsn: u64) -> Result<(), RecoveryError> {
    // ... WAL read logic ...
    Err(RecoveryError::io("Failed to read WAL"))
}

// After: Enhanced error handling with context
fn handle_wal_read_error(path: &str, lsn: u64) -> Result<(), RecoveryError> {
    // ... WAL read logic ...
    Err(RecoveryError::io("Failed to read WAL")
        .with_context(
            ErrorContext::default()
                .with_wal_path(path.to_string())
                .with_lsn_range(lsn, lsn)
                .with_recovery_state("reading_wal")
        )
        .with_recovery(RecoverySuggestion::Retry { max_attempts: 3, backoff_ms: 1000 })
    )
}
```

### Pattern 3: Collection-Based Error Aggregation

```rust
// Before: Individual error handling
fn process_multiple_operations() -> Result<(), RecoveryError> {
    let result1 = operation1();
    let result2 = operation2();
    let result3 = operation3();

    // Return first error
    result1.or(result2).or(result3)
}

// After: Comprehensive error collection
fn process_multiple_operations() -> Result<(), RecoveryErrorCollection> {
    let mut errors = RecoveryErrorCollection::new();

    if let Err(error) = operation1() {
        errors.add_error(error);
    }

    if let Err(error) = operation2() {
        errors.add_error(error);
    }

    if let Err(error) = operation3() {
        errors.add_error(error);
    }

    if errors.has_errors() {
        Err(errors)
    } else {
        Ok(())
    }
}
```

## Troubleshooting

### Common Issues and Solutions

#### Issue 1: Import Path Changes

**Problem:** Compilation fails with "module not found" errors
**Solution:** Use the compatibility re-exports:

```rust
// This should still work
use crate::backend::native::v2::wal::recovery::errors::RecoveryError;

// If it doesn't, try this
use crate::backend::native::v2::wal::recovery::errors::core::RecoveryError;
```

#### Issue 2: Missing Methods

**Problem:** A method that used to exist is no longer available
**Solution:** Check if it moved to a different module:

```rust
// Methods that might have moved:
// - with_v2_context() -> Still on RecoveryError
// - diagnostic_report() -> Still on RecoveryError
// - retry_delay_ms() -> Still on RecoveryError
// - recommended_action() -> Now on RecoveryErrorCollection
```

#### Issue 3: Performance Regression

**Problem:** Code seems slower after migration
**Solution:** Verify you're not importing unnecessary modules:

```rust
// Good: Import only what you need
use crate::backend::native::v2::wal::recovery::errors::RecoveryError;

// Avoid: Wildcard imports unless needed
// use crate::backend::native::v2::wal::recovery::errors::*;
```

#### Issue 4: Test Failures

**Problem:** Tests fail after migration
**Solution:** Update test imports and check for API changes:

```rust
#[cfg(test)]
mod tests {
    // Update imports if needed
    use crate::backend::native::v2::wal::recovery::errors::RecoveryError;

    #[test]
    fn test_error_creation() {
        // Your test code should work unchanged
        let error = RecoveryError::io("test");
        assert!(error.is_recoverable());
    }
}
```

## Getting Help

### Resources

1. **Documentation:** Read the comprehensive modularization report
2. **Examples:** Check the test files for usage patterns
3. **Code Review:** Look at other modules that have been migrated
4. **Team Communication:** Ask questions in team channels

### Support Channels

- **Technical Questions:** Engineering team
- **Migration Issues:** Code review process
- **Performance Concerns:** Performance engineering team
- **Documentation Issues:** Technical writing team

### Best Practices

1. **Start Small:** Migrate one module at a time
2. **Test Thoroughly:** Ensure all functionality still works
3. **Use New Features:** Take advantage of enhanced APIs
4. **Monitor Performance:** Watch for regressions
5. **Ask Questions:** Don't hesitate to seek help

---

**Migration Timeline:** 2-3 weeks for complete codebase
**Support Level:** Full backward compatibility maintained
**Risk Level:** Low - No breaking changes expected

For specific questions or issues, refer to the comprehensive modularization report or contact the engineering team.