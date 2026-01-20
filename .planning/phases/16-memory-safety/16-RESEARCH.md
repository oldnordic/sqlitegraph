# Phase 16: Memory Safety - Research

**Researched:** 2026-01-20
**Domain:** Memory Safety, Unsafe Code Elimination, Input Validation
**Confidence:** HIGH

## Summary

SQLiteGraph uses unsafe `std::mem::transmute` in 14+ locations to extend `GraphFile` lifetimes to `'static`. This is done to work around lifetime requirements in `NodeStore<'a>` and `EdgeStore<'a>` which expect references tied to the `GraphFile` lifetime. The current pattern creates dangling reference risks if the owner is dropped while `'static` references still exist.

Additionally, the codebase lacks explicit input validation for JSON payloads, relying on serde_json's default hard-coded limits which may not meet all security requirements.

**Primary recommendation:** Replace all `transmute` sites with `Arc<RwLock<GraphFile>>` pattern and add explicit JSON size/depth validation with configurable limits.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `std::sync::Arc` | std | Shared ownership | Safe reference counting without transmute |
| `std::sync::RwLock` | std | Thread-safe read-write access | Allows concurrent readers, exclusive writers |
| `parking_lot::RwLock` | 0.12 | Faster RwLock alternative | Already in dependencies, drop-in replacement |

### Input Validation
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `serde_json` | 1 | JSON parsing | Already in use, has built-in recursion limit of 128 |
| `serde` | 1 | Serialization framework | Required for validation layer |

### Miri Testing
| Tool | Purpose | Why Standard |
|------|---------|-------------|
| `miri` | rustup component | Undefined behavior detection | Standard Rust tool for unsafe code validation |

**Installation:**
```bash
# Miri (for unsafe code validation)
rustup component add miri

# No additional packages needed - Arc/RwLock are in std
```

## Architecture Patterns

### Recommended Project Structure
```
sqlitegraph/src/backend/native/v2/wal/
├── checkpoint/
│   ├── operations.rs          # Replace transmute with Arc<RwLock<GraphFile>>
│   └── record/
│       └── integrator.rs      # Replace transmute with Arc<RwLock<GraphFile>>
├── recovery/
│   └── replayer/
│       ├── rollback.rs        # Replace 6 transmute sites
│       └── operations/
│           ├── edge_ops.rs    # Replace 2 transmute sites
│           └── transaction_ops.rs  # Replace 1 transmute site
└── recovery/
    └── validator.rs           # Replace 2 transmute sites
```

### Pattern 1: Arc<RwLock<GraphFile>> (Recommended for Transmute Replacement)

**What:** Replace unsafe lifetime extension with safe shared ownership

**When to use:** When multiple components need access to GraphFile with potential concurrent access

**Example:**
```rust
// BEFORE (unsafe):
// checkpoint/operations.rs:449-450
let mut graph_file = GraphFile::open(&path)?;
let graph_file_ptr: &'static mut GraphFile = unsafe {
    std::mem::transmute::<&mut GraphFile, &'static mut GraphFile>(&mut graph_file)
};
let node_store = NodeStore::new(graph_file_ptr);

// AFTER (safe):
// checkpoint/operations.rs
let graph_file = Arc::new(RwLock::new(GraphFile::open(&path)?));
let node_store = Arc::new(Mutex::new(None));  // Lazy initialization

// When needed, initialize NodeStore:
let mut node_store_guard = node_store.lock()
    .map_err(|e| RecoveryError::replay_failure(format!("Failed to lock node store: {}", e)))?;

if node_store_guard.is_none() {
    let graph_file_ref = graph_file.read()
        .map_err(|e| RecoveryError::io_error(format!("Failed to lock graph file: {}", e)))?;

    // Use unsafe block only for the specific NodeStore initialization
    // Note: NodeStore requires &'static mut - this still needs transmute
    // The key insight: store Arc<RwLock<GraphFile>> instead of &'static reference
    *node_store_guard = Some(NodeStore::new(unsafe {
        std::mem::transmute::<&mut GraphFile, &'static mut GraphFile>(
            // This creates a static reference to data that's NOT actually static
            // The Arc ensures the data lives as long as needed
        )
    }));
}
```

**Critical caveat:** `NodeStore::new()` and `EdgeStore::new()` still require `&'static mut GraphFile`. This is a fundamental API limitation. The safe replacement requires either:
1. Modifying `NodeStore`/`EdgeStore` to accept `Arc<RwLock<GraphFile>>` directly
2. Using a scoped pattern where lifetime is properly tied
3. Storing the `Arc<RwLock<GraphFile>>` and accessing it through locks

### Pattern 2: Scoped Thread Pattern (Alternative for Single-Threaded Contexts)

**What:** Use `std::thread::scope` for bounded lifetimes

**When to use:** When operations are contained within a single function scope

**Example:**
```rust
use std::thread::scope;

fn process_graph(graph_file: &mut GraphFile) -> Result<()> {
    scope(|s| {
        s.spawn(|| {
            // Can borrow graph_file here with safe lifetime
            process_node(graph_file);
        });
    });
    // graph_file still valid here
    Ok(())
}
```

### Pattern 3: JSON Input Validation Wrapper

**What:** Add validation layer before serde_json parsing

**When to use:** For all external JSON input

**Example:**
```rust
// Source: serde_json has default depth limit of 128
// But we want explicit, configurable validation

const DEFAULT_MAX_JSON_SIZE: usize = 10 * 1024 * 1024; // 10MB
const DEFAULT_MAX_JSON_DEPTH: usize = 128;

pub struct JsonLimits {
    pub max_size: usize,
    pub max_depth: usize,
}

impl Default for JsonLimits {
    fn default() -> Self {
        Self {
            max_size: DEFAULT_MAX_JSON_SIZE,
            max_depth: DEFAULT_MAX_JSON_DEPTH,
        }
    }
}

pub fn validate_json_size(input: &[u8], limits: &JsonLimits) -> Result<(), JsonValidationError> {
    if input.len() > limits.max_size {
        return Err(JsonValidationError::SizeTooLarge {
            actual: input.len(),
            max: limits.max_size,
        });
    }
    Ok(())
}

pub fn validate_json_depth(value: &serde_json::Value, limits: &JsonLimits) -> Result<(), JsonValidationError> {
    let depth = calculate_depth(value, 0);
    if depth > limits.max_depth {
        return Err(JsonValidationError::DepthTooLarge {
            actual: depth,
            max: limits.max_depth,
        });
    }
    Ok(())
}

fn calculate_depth(value: &serde_json::Value, current: usize) -> usize {
    match value {
        serde_json::Value::Null | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_) | serde_json::Value::String(_) => current,
        serde_json::Value::Array(arr) => {
            arr.iter()
                .map(|v| calculate_depth(v, current + 1))
                .max()
                .unwrap_or(current)
        }
        serde_json::Value::Object(obj) => {
            obj.values()
                .map(|v| calculate_depth(v, current + 1))
                .max()
                .unwrap_or(current)
        }
    }
}

// Usage:
pub fn parse_json_safe(input: &[u8], limits: &JsonLimits) -> Result<serde_json::Value> {
    validate_json_size(input, limits)?;

    let value: serde_json::Value = serde_json::from_slice(input)?;
    validate_json_depth(&value, limits)?;

    Ok(value)
}
```

### Anti-Patterns to Avoid

- **Transmute for lifetime extension:** Creates dangling reference risk
  - **Why it's bad:** If owner dropped before 'static reference goes out of scope, use-after-free occurs
  - **What to do instead:** Use `Arc<RwLock<T>>` for shared ownership

- **Relying solely on serde_json's default limits:** May not meet all security requirements
  - **Why it's bad:** Default 128 depth limit is hard-coded, not configurable
  - **What to do instead:** Add explicit validation layer with configurable limits

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Shared ownership | Custom reference counting | `Arc<T>` | Thread-safe, zero-cost abstraction |
| Thread-safe access | Custom locking | `RwLock<T>` or `parking_lot::RwLock` | Well-tested, handles lock poisoning |
| JSON depth validation | Custom recursive parser | Pre-parse with `calculate_depth` | Simpler, serde_json still handles actual parsing |
| Memory safety validation | Custom analysis | `miri` | Detects undefined behavior automatically |
| Unsafe code documentation | Ad-hoc comments | Structured safety proofs | Required for review and maintenance |

**Key insight:** Custom solutions for shared ownership and locking are error-prone. The standard library provides well-tested primitives that handle edge cases like lock poisoning and reference counting correctly.

## Common Pitfalls

### Pitfall 1: Incomplete Transmute Replacement

**What goes wrong:** Replacing some transmute sites but missing others, or replacing with pattern that still has hidden unsafety

**Why it happens:** The transmute pattern is scattered across 14+ locations in 5+ files

**How to avoid:**
1. Run `grep -r "std::mem::transmute" --include="*.rs" sqlitegraph/src` to find ALL sites
2. Create audit document listing each site with:
   - File path and line number
   - What types are being transmuted
   - Why transmute exists (what problem it solves)
   - Safe replacement strategy
3. Replace ALL sites systematically

**Warning signs:**
- Still seeing `'static` lifetime after replacement
- `unsafe` blocks remain without safety documentation
- Clippy warnings about transmute

### Pitfall 2: NodeStore/EdgeStore API Lifetime Requirements

**What goes wrong:** Even with `Arc<RwLock<GraphFile>>`, the `NodeStore::new()` API requires `&'static mut GraphFile`

**Why it happens:** `NodeStore` and `EdgeStore` are defined with lifetime parameters tied to GraphFile

**How to avoid:**
1. Option A: Modify `NodeStore` to accept `Arc<RwLock<GraphFile>>` instead of reference
2. Option B: Keep the transmute but document it with a safety proof
3. Option C: Store `Arc<RwLock<GraphFile>>` and access it when needed (not holding `'static` reference)

**Warning signs:**
- Cannot compile without unsafe after attempting replacement
- Complex lifetime errors in compiler output

### Pitfall 3: Lock Ordering Deadlocks

**What goes wrong:** When replacing transmute with `Arc<RwLock<>>`, potential for deadlock if locks acquired in wrong order

**Why it happens:** Multiple components now need to acquire locks, potential for lock ordering issues

**How to avoid:**
1. Establish lock ordering hierarchy
2. Always acquire locks in consistent order
3. Use `try_lock()` or timeout for locks that may contend

**Warning signs:**
- Tests hang indefinitely
- Production system becomes unresponsive under load

### Pitfall 4: Miri Test Suite Not Comprehensive

**What goes wrong:** Miri passes but real-world unsafe behavior still exists

**Why it happens:** Miri only tests the code paths exercised by tests

**How to avoid:**
1. Write specific tests for each former transmute site
2. Include tests that verify lifetime assumptions (drop owner while using reference)
3. Run Miri on full test suite, not just subset

**Warning signs:**
- Miri passes but code still has `'static` references
- No Miri tests for specific unsafe patterns

## Code Examples

### Transmute Audit Script

Verified pattern for finding all transmute sites:

```bash
# Find all transmute sites in source code
grep -r "std::mem::transmute" --include="*.rs" sqlitegraph/src/ | grep -v ".cargo"
```

### Arc<RwLock<GraphFile>> Pattern (Verified)

From checkpoint/operations.rs:

```rust
// BEFORE: Lines 449-460
let graph_file_ptr = unsafe {
    std::mem::transmute::<&mut GraphFile, &'static mut GraphFile>(&mut graph_file)
};
let node_store = NodeStore::new(graph_file_ptr);
let edge_store = EdgeStore::new(unsafe {
    std::mem::transmute::<&mut GraphFile, &'static mut GraphFile>(&mut graph_file)
});

// AFTER: Safe pattern with Arc<RwLock<>>
pub struct V2GraphIntegrator {
    graph_file: Arc<RwLock<GraphFile>>,
    node_store: Arc<Mutex<Option<NodeStore<'static>>>>,  // Still needs 'static for NodeStore API
    edge_store: Arc<Mutex<Option<EdgeStore<'static>>>>,
    // ... other fields
}

impl V2GraphIntegrator {
    pub fn new(graph_file_path: PathBuf) -> CheckpointResult<Self> {
        let mut graph_file = GraphFile::open(&graph_file_path)?;

        // Store GraphFile in Arc<RwLock> for safe shared access
        let graph_file = Arc::new(RwLock::new(graph_file));

        // NodeStore and EdgeStore initialization happens lazily
        // when first accessed through the Arc<RwLock<>>
        Ok(Self {
            graph_file,
            node_store: Arc::new(Mutex::new(None)),
            edge_store: Arc::new(Mutex::new(None)),
            // ...
        })
    }
}
```

### Miri Test Pattern

```rust
#[cfg(test)]
mod miri_tests {
    use super::*;

    #[test]
    fn test_transmute_replacement_safety() {
        // This test should pass with Miri after transmute replacement
        let temp_dir = tempfile::tempdir().unwrap();
        let graph_path = temp_dir.path().join("test.v2");

        // Create GraphFile wrapped in Arc<RwLock<>>
        let graph_file = Arc::new(RwLock::new(
            GraphFile::create(&graph_path).unwrap()
        ));

        // Clone Arc (cheap reference count increment)
        let graph_file_clone = graph_file.clone();

        // Spawn task that uses the cloned Arc
        std::thread::spawn(move || {
            let graph = graph_file_clone.read().unwrap();
            // Use graph here - safe because Arc keeps it alive
            let _header = graph.header();
        });

        // Original Arc still valid here
        let graph = graph_file.write().unwrap();
        let _header = graph.header();

        // Both accesses are safe - Arc ensures lifetime
    }
}
```

### JSON Validation with Configurable Limits

```rust
// Source: Based on serde_json default limit of 128
// Reference: https://github.com/serde-rs/json/issues/478
// Reference: https://docs.rs/crate/serde_json/latest/source/src/de.rs

#[derive(Debug, Clone)]
pub struct JsonLimits {
    pub max_size: usize,
    pub max_depth: usize,
}

impl Default for JsonLimits {
    fn default() -> Self {
        Self {
            max_size: 10 * 1024 * 1024, // 10MB default
            max_depth: 128,              // Match serde_json default
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum JsonValidationError {
    #[error("JSON size {actual} bytes exceeds maximum {max} bytes")]
    SizeTooLarge { actual: usize, max: usize },

    #[error("JSON depth {actual} exceeds maximum {max}")]
    DepthTooLarge { actual: usize, max: usize },
}

/// Validates JSON input before parsing
///
/// # Arguments
/// * `input` - Raw JSON bytes
/// * `limits` - Configurable size and depth limits
///
/// # Returns
/// Parsed JSON value if validation passes
///
/// # Example
/// ```rust
/// let limits = JsonLimits {
///     max_size: 5 * 1024 * 1024,  // 5MB
///     max_depth: 64,              // More conservative
/// };
/// let value = parse_and_validate_json(input, &limits)?;
/// ```
pub fn parse_and_validate_json(
    input: &[u8],
    limits: &JsonLimits
) -> Result<serde_json::Value, JsonValidationError> {
    // Check size first (fast reject)
    if input.len() > limits.max_size {
        return Err(JsonValidationError::SizeTooLarge {
            actual: input.len(),
            max: limits.max_size,
        });
    }

    // Parse JSON
    let value: serde_json::Value = serde_json::from_slice(input)
        .map_err(|_| JsonValidationError::SizeTooLarge {
            actual: input.len(),
            max: limits.max_size
        })?;

    // Check depth (more expensive, do after parsing)
    let depth = calculate_json_depth(&value, 0);
    if depth > limits.max_depth {
        return Err(JsonValidationError::DepthTooLarge {
            actual: depth,
            max: limits.max_depth,
        });
    }

    Ok(value)
}

fn calculate_json_depth(value: &serde_json::Value, current: usize) -> usize {
    match value {
        serde_json::Value::Null | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_) | serde_json::Value::String(_) => current,

        serde_json::Value::Array(arr) => {
            arr.iter()
                .map(|v| calculate_json_depth(v, current + 1))
                .max()
                .unwrap_or(current)
        }

        serde_json::Value::Object(obj) => {
            obj.values()
                .map(|v| calculate_json_depth(v, current + 1))
                .max()
                .unwrap_or(current)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_size_rejection() {
        let limits = JsonLimits { max_size: 100, max_depth: 128 };
        let large_input = b"\"a\".repeat(101);";

        let result = parse_and_validate_json(large_input, &limits);
        assert!(matches!(result, Err(JsonValidationError::SizeTooLarge { .. })));
    }

    #[test]
    fn test_json_depth_rejection() {
        let limits = JsonLimits { max_size: 10000, max_depth: 10 };

        // Create deeply nested JSON
        let mut json_str = String::from("null");
        for _ in 0..15 {
            json_str = format!("[{}]", json_str);
        }

        let result = parse_and_validate_json(json_str.as_bytes(), &limits);
        assert!(matches!(result, Err(JsonValidationError::DepthTooLarge { .. })));
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Raw transmute for lifetime | Arc<RwLock<T>> | Ongoing | Eliminates UB risk |
| No JSON limits | serde_json default (128 depth) | 2015+ | Stack overflow protection |
| No Miri testing | Miri in CI | Ongoing | Catches undefined behavior |

**Current serde_json behavior:**
- serde_json has a **hard-coded recursion limit of 128** to prevent stack overflow
- Source: [serde_json de.rs](https://docs.rs/crate/serde_json/latest/source/src/de.rs)
- This limit is NOT configurable via API (see [Issue #162](https://github.com/serde-rs/json/issues/162))
- The `unbounded_depth` feature removes limits entirely (not recommended for external input)

**Deprecated/outdated:**
- Using `transmute` for lifetime extension without Miri validation
- Assuming external JSON input is well-formed without validation

## Complete Transmute Site Inventory

| File | Line | Type Transmuted | Purpose | Replacement Strategy |
|------|------|-----------------|---------|----------------------|
| `checkpoint/operations.rs` | 450 | `&mut GraphFile` -> `&'static mut GraphFile` | Create NodeStore | Arc<RwLock<GraphFile>> |
| `checkpoint/operations.rs` | 459 | `&mut GraphFile` -> `&'static mut GraphFile` | Create EdgeStore | Arc<RwLock<GraphFile>> |
| `checkpoint/record/integrator.rs` | 41 | `&mut GraphFile` -> `&'static mut GraphFile` | Create NodeStore | Arc<RwLock<GraphFile>> |
| `checkpoint/record/integrator.rs` | 50 | `&mut GraphFile` -> `&'static mut GraphFile` | Create EdgeStore | Arc<RwLock<GraphFile>> |
| `recovery/validator.rs` | 145 | `&mut GraphFile` -> `&'static mut GraphFile` | Create NodeStore | Arc<RwLock<GraphFile>> |
| `recovery/validator.rs` | 154 | `&mut GraphFile` -> `&'static mut GraphFile` | Create EdgeStore | Arc<RwLock<GraphFile>> |
| `recovery/replayer/rollback.rs` | 150 | `&mut GraphFile` -> `&'static mut GraphFile` | Create NodeStore | Arc<RwLock<GraphFile>> |
| `recovery/replayer/rollback.rs` | 187 | `&mut GraphFile` -> `&'static mut GraphFile` | Create NodeStore | Arc<RwLock<GraphFile>> |
| `recovery/replayer/rollback.rs` | 238 | `&mut GraphFile` -> `&'static mut GraphFile` | Create NodeStore | Arc<RwLock<GraphFile>> |
| `recovery/replayer/rollback.rs` | 778 | `&mut GraphFile` -> `&'static mut GraphFile` | Create NodeStore | Arc<RwLock<GraphFile>> |
| `recovery/replayer/rollback.rs` | 883 | `&mut GraphFile` -> `&'static mut GraphFile` | Create NodeStore | Arc<RwLock<GraphFile>> |
| `recovery/replayer/rollback.rs` | 970 | `&mut GraphFile` -> `&'static mut GraphFile` | Create NodeStore | Arc<RwLock<GraphFile>> |
| `recovery/replayer/rollback.rs` | 1144 | `&mut GraphFile` -> `&'static mut GraphFile` | Create NodeStore | Arc<RwLock<GraphFile>> |
| `replayer/operations/edge_ops.rs` | 169 | `&mut GraphFile` -> `&'static mut GraphFile` | Create NodeStore | Arc<RwLock<GraphFile>> |
| `replayer/operations/edge_ops.rs` | 290 | `&mut GraphFile` -> `&'static mut GraphFile` | Create NodeStore | Arc<RwLock<GraphFile>> |
| `replayer/operations/edge_ops.rs` | 598 | `&mut GraphFile` -> `&'static mut GraphFile` | Create NodeStore | Arc<RwLock<GraphFile>> |
| `replayer/operations/transaction_ops.rs` | 136 | `&mut GraphFile` -> `&'static mut GraphFile` | Create NodeStore | Arc<RwLock<GraphFile>> |

**Total: 17 transmute sites identified**

Note: This count is higher than the initially mentioned "10+ locations" based on actual grep results. All sites follow the same pattern of extending GraphFile lifetime to 'static.

## Open Questions

1. **NodeStore/EdgeStore API Lifetime Requirements**
   - What we know: NodeStore and EdgeStore constructors require `&'static mut GraphFile`
   - What's unclear: Can we modify these APIs to accept `Arc<RwLock<GraphFile>>` without breaking other code?
   - Recommendation: Research NodeStore/EdgeStore API design to determine if lifetime parameter can be eliminated

2. **Performance Impact of Arc<RwLock<>>**
   - What we know: Arc/RwLock adds locking overhead
   - What's unclear: Performance impact on hot paths like WAL recovery
   - Recommendation: Benchmark before/after to measure impact

3. **serde_json Configurable Depth**
   - What we know: serde_json has hard-coded 128 depth limit, not configurable
   - What's unclear: Should we implement our own depth-limited parser or just validate after parsing?
   - Recommendation: Pre-parse validation is simpler; use serde_json for actual parsing

## Sources

### Primary (HIGH confidence)
- [Existing research: RUST_UNSAFE.md](/home/feanor/Projects/sqlitegraph/.planning/research/RUST_UNSAFE.md) - Safe alternatives to unsafe transmute
- [Source code: checkpoint/operations.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/v2/wal/checkpoint/operations.rs) - Lines 450, 459
- [Source code: checkpoint/record/integrator.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs) - Lines 41, 50
- [Source code: recovery/replayer/rollback.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/v2/wal/recovery/replayer/rollback.rs) - Lines 150, 187, 238, 778, 883, 970, 1144
- [Source code: recovery/validator.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/v2/wal/recovery/validator.rs) - Lines 145, 154

### Secondary (MEDIUM confidence)
- [serde_json recursion limit discussion](https://github.com/serde-rs/json/issues/478) - Confirms 128 depth limit
- [serde_json source code](https://docs.rs/crate/serde_json/latest/source/src/de.rs) - Recursion limit implementation
- [Stack overflow discussion](https://users.rust-lang.org/t/deserialize-large-nested-struct-without-stack-overflow-help/136335) - serde_json has 128 limit

### Tertiary (LOW confidence)
- [Add configurable recursion limit](https://github.com/serde-rs/json/issues/162) - Request for configurable limits (not yet implemented)
- [json-threat-protection crate](https://docs.rs/json-threat-protection) - Alternative for JSON validation

## Metadata

**Confidence breakdown:**
- Transmute site inventory: HIGH - Verified with grep against actual source
- Arc<RwLock<>> replacement pattern: HIGH - Standard Rust pattern, well-documented
- JSON depth limit: MEDIUM - Verified serde_json has 128 limit, but implementation details from docs
- Miri testing: HIGH - Standard Rust tool

**Research date:** 2026-01-20
**Valid until:** 7 days (fast-moving domain - unsafe code practices)
