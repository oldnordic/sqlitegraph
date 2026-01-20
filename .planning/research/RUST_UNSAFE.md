# Rust Unsafe Code and Memory Safety Research

**Research Date:** 2026-01-20
**Focus:** Eliminating unsafe `transmute` in favor of safe patterns, specifically for GraphFile lifetime issues

## Overview

SQLiteGraph uses `std::mem::transmute` in 10+ locations to extend `GraphFile` lifetimes. This research covers safe alternatives and best practices for handling owned data with shared references.

## Key Sources

- [Toward Safe Transmutation in Rust (LWN - Oct 2024)](https://lwn.net/Articles/994334/)
- [Rust Release Notes 2025 - Unsafe validation](https://doc.rust-lang.org/beta/releases.html)
- [Rust Advanced Concurrency: Arc, Mutex, RwLock (Mar 2025)](https://www.ruststepbystep.com/rust-advanced-concurrency-using-arc-mutex-and-rwlock-safely/)
- [GitHub: Allow unguarded RwLock access (Feb 2025)](https://github.com/rust-lang/libs-team/issues/531)
- [RustSec Advisory Database](https://rustsec.org/advisories/)
- [Arc in std::sync](https://doc.rust-lang.org/std/sync/struct.Arc.html)
- [Rust docs: The Rust Book - Unsafe Rust](https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html)

---

## The Problem: Lifetime Transmutation

### SQLiteGraph Issue

**Pattern used in multiple locations:**
```rust
// checkpoint/operations.rs:449-450
// checkpoint/record/integrator.rs:40
// recovery/replayer/rollback.rs:142,179,224,524,629,716,890

let graph_file_static: &'static GraphFile = unsafe {
    std::mem::transmute::<&GraphFile, &'static GraphFile>(graph_file)
};
```

**Why this is done:**
- GraphFile has a lifetime tied to the owner (Integrator, Replayer, etc.)
- Some APIs require `'static` lifetime
- Transmute extends lifetime to `'static`

**Why it's dangerous:**
- If owner is dropped, `'static` reference becomes dangling
- Use-after-free = undefined behavior
- No compiler protection

---

## Safe Alternatives

### Option 1: Arc<RwLock<GraphFile>> (Recommended)

**Pattern:**
```rust
// Instead of:
let graph_file_static: &'static GraphFile = unsafe { transmute(graph_file) };

// Use:
let graph_file: Arc<RwLock<GraphFile>> = Arc::new(RwLock::new(GraphFile::create(path)?));
let graph_file_clone = graph_file.clone();

// In callback/task:
let graph = graph_file_clone.read().unwrap();
// Use graph here
```

**Pros:**
- Completely safe
- Shared ownership across threads/tasks
- Compile-time lifetime guarantees
- No risk of use-after-free

**Cons:**
- Runtime locking overhead
- More verbose
- Potential lock contention

**When to use:**
- Multiple concurrent readers
- Need thread safety
- Can tolerate lock overhead

### Option 2: Arc Without Lock

**If GraphFile is mostly immutable:**
```rust
let graph_file: Arc<GraphFile> = Arc::new(GraphFile::create(path)?);

// For mutation, use interior mutability or exclusive access
// For reads, just clone Arc (cheap)
```

**Pros:**
- No locking overhead for reads
- Safe shared ownership
- Cheap clone (pointer copy)

**Cons:**
- Mutation requires careful design
- May need `Arc<Mutex<_>>` or `Arc<RwLock<_>>` anyway

**When to use:**
- Mostly read-only access
- Infrequent mutations

### Option 3: Scoped Threads

**For WAL recovery with known lifetime:**
```rust
use std::thread::scope;

scope(|s| {
    s.spawn(|| {
        // graph_file is borrowed here
        // Lifetime tied to scope
        use_graph(graph_file);
    });
});
// graph_file still valid here
```

**Pros:**
- Safe borrowing
- No explicit lifetime extension
- Compiler enforces correctness

**Cons:**
- Only works within a scope
- Not suitable for storing references

**When to use:**
- Parallel operations within a function
- Known lifetime boundaries

### Option 4: Self-Referential Struct with Arc

**For complex cases:**
```rust
struct GraphOwner {
    graph_file: Arc<RwLock<GraphFile>>,
    // Can store Arc<RwLock<GraphFile>> in callbacks
}

impl GraphOwner {
    fn new() -> Self {
        let graph_file = Arc::new(RwLock::new(GraphFile::create().unwrap()));
        Self { graph_file }
    }

    fn get_callback(&self) -> impl Fn() + 'static {
        let graph = self.graph_file.clone();
        move || {
            let graph_guard = graph.read().unwrap();
            // Safe to use graph_guard here
        }
    }
}
```

**Pros:**
- Self-referential patterns work
- Safe `'static` callbacks
- Clear ownership

**Cons:**
- More boilerplate
- Need to clone Arc

---

## Audit Strategy

### Step 1: Find All Transmute Sites

```bash
cd sqlitegraph
grep -r "std::mem::transmute" --include="*.rs" .
```

**Known locations (from CONCERNS.md):**
- `checkpoint/operations.rs:449-450`
- `checkpoint/record/integrator.rs:40`
- `recovery/replayer/rollback.rs:142,179,224,524,629,716,890`

### Step 2: For Each Site

**Questionnaire:**
1. What is the actual lifetime of the referenced data?
2. Who owns it? When is it dropped?
3. Why is `'static` required?
4. Can we use `Arc<RwLock<_>>` instead?

### Step 3: Replace with Safe Pattern

**Example transformation:**

**Before (unsafe):**
```rust
struct Integrator {
    graph_file: &'graph GraphFile,
}

impl Integrator {
    fn new(graph_file: &GraphFile) -> Self {
        Self {
            graph_file: unsafe { transmute(graph_file) }
        }
    }
}
```

**After (safe):**
```rust
struct Integrator {
    graph_file: Arc<RwLock<GraphFile>>,
}

impl Integrator {
    fn new(graph_file: Arc<RwLock<GraphFile>>) -> Self {
        Self { graph_file }
    }

    fn read_graph(&self) -> RwLockReadGuard<GraphFile> {
        self.graph_file.read().unwrap()
    }
}
```

---

## Miri Testing

### What is Miri?

**Miri** is an interpreter for Rust's mid-level intermediate representation (MIR). It detects **undefined behavior** including:
- Use-after-free
- Data races
- Invalid memory access
- Uninitialized memory reads

### Running Miri

```bash
# Install miri
rustup component add miri

# Run tests with miri
cargo +nightly miri test

# Run specific test
cargo +nightly miri test test_wal_recovery

# With sanitizer flags
MIRIFLAGS="-Zmiri-disable-isolation" cargo +nightly miri test
```

### Test Cases Required

For each transmute site:
```rust
#[test]
fn test_transmute_safety() {
    // Create GraphFile
    let graph_file = GraphFile::create(temp_path).unwrap();

    // Create unsafe reference
    let graph_static: &'static GraphFile = unsafe {
        std::mem::transmute(&graph_file)
    };

    // Use the reference
    graph_static.read_header();

    // Drop original
    drop(graph_file);

    // This SHOULD fail with miri (use-after-free)
    // If it doesn't fail, we got lucky
    graph_static.read_header(); // UB!
}
```

**Expected outcome:** Miri should detect the use-after-free.

**After fix:** Test should pass without Miri errors.

---

## Unsafe Code Guidelines

### When Unsafe Is Acceptable

1. **FFI (Foreign Function Interface)**
   - Calling C functions
   - Document safety requirements

2. **Performance Critical Paths**
   - Only after profiling shows need
   - Document why safe alternative is too slow
   - Add comprehensive tests

3. **Implementing Safe Abstractions**
   - Unsafe internally, safe API
   - Examples: Vec, Arc, Mutex

### Unsafe Code Checklist

For each `unsafe` block:
- [ ] Document the invariant being relied upon
- [ ] Explain why this can't be done safely
- [ ] Add Miri test to validate safety
- [ ] Review with another Rust developer

---

## Migration Plan for SQLiteGraph

### Phase 1: Audit

1. Run `grep -r "std::mem::transmute"`
2. Document each site with:
   - Why transmute is used
   - Actual lifetime of data
   - Owner and drop point
   - Safe alternative

### Phase 2: Replace

For each site:
1. Change `GraphFile` owner to use `Arc<RwLock<GraphFile>>`
2. Update all borrows to use `.read().unwrap()` or `.write().unwrap()`
3. Remove `unsafe` and `transmute`
4. Update tests

### Phase 3: Validate

1. Run `cargo +nightly miri test`
2. Fix any Miri-detected issues
3. Run full test suite
4. Performance benchmarks (ensure no regression)

### Phase 4: Prevent Recurrence

1. Add clippy lint: `#![warn(clippy::transmute_ptr_to_ptr)]`
2. Add pre-commit hook to check for new unsafe
3. Document unsafe code policy in CONTRIBUTING.md

---

## Other Safety Considerations

### Input Sanitization

**Current concern:** User-provided JSON stored without validation

**Fix:**
```rust
const MAX_JSON_SIZE: usize = 10 * 1024 * 1024; // 10MB
const MAX_JSON_DEPTH: usize = 128;

fn validate_json(json: &Value) -> Result<()> {
    // Check size
    let json_str = serde_json::to_string(json)?;
    if json_str.len() > MAX_JSON_SIZE {
        return Err(Error::JsonTooLarge);
    }

    // Check depth
    let depth = calculate_depth(json)?;
    if depth > MAX_JSON_DEPTH {
        return Err(Error::JsonTooDeep);
    }

    Ok(())
}

fn calculate_depth(value: &Value, current: usize) -> Result<usize> {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => Ok(current),
        Value::Array(arr) => {
            let max = arr.iter()
                .map(|v| calculate_depth(v, current + 1))
                .max()
                .unwrap_or(Ok(current))?;
            Ok(max)
        }
        Value::Object(obj) => {
            let max = obj.values()
                .map(|v| calculate_depth(v, current + 1))
                .max()
                .unwrap_or(Ok(current))?;
            Ok(max)
        }
    }
}
```

### Deadlock Detection

**Current concern:** Deadlock detection incomplete

**Resources needed:**
- Track which transaction holds which lock
- Build wait-for graph
- Detect cycles
- Select victim to abort

---

## Implementation Checklist for SQLiteGraph v1.1

### Unsafe Code Audit
- [ ] Document all 10+ transmute sites
- [ ] Verify actual lifetimes for each
- [ ] Determine safe alternative for each

### Replace Transmute
- [ ] Convert checkpoint/operations.rs to Arc<RwLock<GraphFile>>
- [ ] Convert checkpoint/record/integrator.rs
- [ ] Convert recovery/replayer/rollback.rs (6 sites)
- [ ] Update all callers

### Miri Testing
- [ ] Set up Miri in CI
- [ ] Write tests for each former transmute site
- [ ] Ensure no Miri errors

### Input Validation
- [ ] Add JSON size limit
- [ ] Add JSON depth limit
- [ ] Add validation tests

### Deadlock Detection
- [ ] Implement resource-level lock tracking
- [ ] Implement wait-for graph
- [ ] Add cycle detection
- [ ] Add victim selection

---

## References

- [Rust Book: Unsafe Rust](https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html)
- [LWN: Toward Safe Transmutation](https://lwn.net/Articles/994334/)
- [Rust Release Notes 2025](https://doc.rust-lang.org/beta/releases.html)

---
*Rust Unsafe Patterns Research: 2026-01-20*
