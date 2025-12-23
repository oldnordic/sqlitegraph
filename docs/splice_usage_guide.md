# Splice Usage Guide for SQLiteGraph Mock Implementation

**Date**: 2024-12-23
**Purpose**: Guide for using Splice to refactor mock implementations in SQLiteGraph
**Splice Version**: 0.1.0
**Repository**: https://github.com/oldnordic/splice

---

## What is Splice?

Splice is a span-safe refactoring kernel for Rust using SQLiteGraph. Think of it as `sed` that:
- Understands Rust syntax via tree-sitter
- Validates replacements with tree-sitter reparse
- Validates replacements with cargo check
- Performs atomic rollback on validation failures

**Key Benefit**: Byte-accurate, AST-validated replacements with automatic rollback if anything fails.

---

## Installation

Splice is already installed at `/home/feanor/.local/bin/splice` on this system.

To verify:
```bash
splice --version
# Should output: splice 0.1.0
```

For manual installation:
```bash
cd /home/feanor/Projects/splice
cargo build --release
mkdir -p ~/.local/bin
cp target/release/splice ~/.local/bin/splice
```

---

## Quick Start Example

We successfully tested Splice on the `handle_header_update` mock in `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs`.

### Step 1: Create Replacement File

Create `/tmp/test_handle_header_update.rs` with the improved implementation:

```rust
    /// Handle header update during replay
    ///
    /// Updates the graph file header with new data during WAL replay.
    /// This operation ensures that header modifications (such as metadata
    /// updates, version changes, or flag modifications) are properly applied
    /// during recovery.
    ///
    /// # Arguments
    /// * `header_offset` - Byte offset in the file where the header data starts
    /// * `new_data` - New header data to write
    /// * `old_data` - Previous header data (for rollback purposes)
    /// * `rollback_data` - Accumulator for rollback operations
    ///
    /// # Returns
    /// * `Ok(())` if header update succeeds
    /// * `Err(RecoveryError)` if the update fails
    ///
    /// # TODO
    /// This is currently a placeholder implementation. The full implementation should:
    /// 1. Validate that header_offset is within valid header region
    /// 2. Verify new_data size doesn't exceed header bounds
    /// 3. Perform atomic write to GraphFile header
    /// 4. Store rollback operation with old_data if provided
    pub fn handle_header_update(
        &self,
        header_offset: u64,
        new_data: &[u8],
        _old_data: Option<&[u8]>,
        _rollback_data: &mut Vec<super::types::RollbackOperation>,
    ) -> Result<(), RecoveryError> {
        warn!("Header update replay not yet implemented - placeholder (offset: {}, data_size: {})",
              header_offset, new_data.len());

        // TODO: Implement actual header update logic:
        // 1. Validate header_offset is within GraphFile::HEADER_SIZE
        // 2. Verify new_data won't overflow header boundaries
        // 3. Write new_data to GraphFile at header_offset
        // 4. If old_data.is_some(), add RollbackOperation::HeaderUpdate

        Ok(())
    }
```

### Step 2: Apply the Patch

```bash
splice patch \
  --file sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs \
  --symbol handle_header_update \
  --kind function \
  --with /tmp/test_handle_header_update.rs \
  --verbose
```

**Output**:
```
Patched 'handle_header_update' at bytes 68603..69004 (hash: 2751661e7abc202c634fdbd80e4000c803a4b4613e15e77a75f5c1fd44f86ee1 -> cf130ef314d01b17e14ecdff16d3e32deff6de2dd0f332b56b040f6f7483aa9e)
```

### Step 3: Verify Compilation

```bash
cargo check --package sqlitegraph
```

**Result**: ✅ Compiled successfully (272 warnings, 0 errors)

---

## Splice Command Reference

### Basic Syntax

```bash
splice patch [OPTIONS]
```

### Required Options

- `--file <FILE>` - Path to the source file containing the symbol
- `--symbol <SYMBOL>` - Symbol name to patch
- `--with <FILE>` - Path to file containing replacement content

### Optional Options

- `--kind <KIND>` - Symbol kind filter: `function`, `struct`, `enum`, `trait`, `impl`
- `--verbose` - Enable verbose logging
- `--analyzer <MODE>` - rust-analyzer validation mode: `off`, `os`, `path`

---

## Validation Gates

Every Splice patch passes:

1. **UTF-8 Boundary Validation** - Ensures valid UTF-8 at patch boundaries
2. **Tree-Sitter Reparse** - Validates Rust syntax after patch
3. **Cargo Check** - Validates compilation after patch
4. **Atomic Rollback** - Automatic rollback if ANY validation fails

---

## Using Splice for Multi-Step Refactors

Splice supports JSON plan files for orchestrating multiple patches:

### Create Plan File

`plan.json`:
```json
{
  "steps": [
    {
      "file": "sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs",
      "symbol": "handle_header_update",
      "kind": "function",
      "with": "patches/header_update.rs"
    },
    {
      "file": "sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs",
      "symbol": "handle_string_insert",
      "kind": "function",
      "with": "patches/string_insert.rs"
    }
  ]
}
```

### Execute Plan

```bash
splice plan --file plan.json
```

---

## Splice Limitations

Splice is NOT:
- ❌ An IDE (use Rust Analyzer or IntelliJ Rust)
- ❌ A semantic refactoring tool (doesn't track cross-file references)
- ❌ A complete solution (focused tool for one specific job)
- ❌ Production-hardened (it's an MVP with known limitations)

**Key Limitations**:
- No cross-file reference tracking
- No persistent database (creates graph on-the-fly for each patch)
- No resume mode for failed plans
- No dry-run mode (can't preview without applying)
- No auto-discovery of symbols (you must know exact names)
- Single-file symbol resolution only

---

## Current Mock Status (2024-12-23)

Based on `docs/mock_status_report.md`:

**Total V2WALRecord Operations**: 16 variants
**Fully Implemented**: 11 operations (68.75%)
**Still Mock**: 1 operation (6.25%) - `handle_header_update`
**N/A (Markers)**: 4 operations (25%)

### Remaining Mock

**handle_header_update** in `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs:1487-1497`

- Current status: Mock with warning
- Priority: MEDIUM
- Can use Splice to implement when ready

---

## Best Practices

### 1. Always Commit Before Using Splice

```bash
git add -A
git commit -m "Pre-splice snapshot"
```

Splice performs atomic rollback, but having a git commit ensures you can always revert.

### 2. Create Replacement Files Outside Repository

Store replacement files in `/tmp/` or a dedicated patches directory to avoid accidentally committing them.

### 3. Test Compilation After Each Patch

```bash
cargo check --package sqlitegraph
cargo test --package sqlitegraph
```

### 4. Use Symbol Kind for Disambiguation

If you have multiple symbols with the same name, use `--kind` to specify which one:
```bash
splice patch --file src/lib.rs --symbol foo --kind function --with new_foo.rs
```

### 5. Enable Verbose Mode for Debugging

```bash
splice patch --verbose --file src/lib.rs --symbol bar --with new_bar.rs
```

---

## Advanced Usage: Multi-File Refactor Plans

For complex refactors affecting multiple files, create a JSON plan:

### Example: Complete Edge Operations Refactor

`edge_operations_plan.json`:
```json
{
  "steps": [
    {
      "file": "sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs",
      "symbol": "handle_edge_insert",
      "kind": "function",
      "with": "/patches/edge_insert_v2.rs"
    },
    {
      "file": "sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs",
      "symbol": "handle_edge_update",
      "kind": "function",
      "with": "/patches/edge_update_v2.rs"
    },
    {
      "file": "sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs",
      "symbol": "handle_edge_delete",
      "kind": "function",
      "with": "/patches/edge_delete_v2.rs"
    }
  ]
}
```

Execute:
```bash
splice plan --file edge_operations_plan.json
```

---

## Troubleshooting

### Error: Symbol Not Found

**Cause**: Splice uses tree-sitter for symbol resolution, which requires the symbol to be parseable.

**Solution**:
1. Verify the symbol name is exact (case-sensitive)
2. Use `--kind` to disambiguate
3. Check that the file compiles: `cargo check --package sqlitegraph`

### Error: Validation Failed

**Cause**: Replacement code doesn't compile or fails tree-sitter validation.

**Solution**:
1. Test the replacement code in isolation
2. Verify imports are included in the replacement
3. Check for syntax errors: `rustc --crate-type lib /tmp/replacement.rs`

### Error: UTF-8 Boundary Violation

**Cause**: Patch boundary splits a multi-byte UTF-8 character.

**Solution**:
1. Ensure your replacement file starts and ends at valid UTF-8 boundaries
2. Avoid cutting through multi-byte characters

---

## Integration with Development Workflow

### Before Implementing a Mock

1. **Read the existing code**: Understand the mock's purpose and context
2. **Read related tests**: See what behavior is expected
3. **Check dependencies**: Verify what GraphFile APIs are needed
4. **Create implementation file**: Write the real implementation in `/tmp/`
5. **Apply with Splice**: Use Splice to replace the mock
6. **Run tests**: Verify all tests pass
7. **Update documentation**: Mark mock as implemented in `docs/mock_status_report.md`

### Example Workflow

```bash
# 1. Read the mock
rg "handle_header_update" sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs -A 10

# 2. Read related tests
rg "test_header_update" sqlitegraph/tests/ -A 20

# 3. Create implementation
cat > /tmp/header_update_impl.rs << 'EOF'
// Implementation here
EOF

# 4. Apply with Splice
splice patch \
  --file sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs \
  --symbol handle_header_update \
  --kind function \
  --with /tmp/header_update_impl.rs \
  --verbose

# 5. Run tests
cargo test handle_header_update

# 6. Update documentation
# Edit docs/mock_status_report.md to mark as implemented
```

---

## Documentation References

- **Splice Manual**: `docs/splice_manual.md` (complete user manual, 400+ lines)
- **Quick Start**: `QUICKSTART.md` in Splice repository
- **Splice README**: https://github.com/oldnordic/splice
- **Mock Status**: `docs/mock_status_report.md`

---

## Summary

Splice is a powerful tool for safely refactoring Rust code with validation gates and automatic rollback. It's particularly useful for:

1. Replacing mock implementations with real code
2. Adding comprehensive documentation
3. Refactoring function signatures
4. Updating multiple functions in coordinated plans
5. Maintaining code quality during refactors

**Key Benefits**:
- Byte-accurate replacements
- AST-validated changes
- Automatic rollback on failure
- Multi-step plan orchestration
- No manual span calculations

**Remember**: Always commit before using Splice, test compilation after each patch, and update documentation to track progress.
