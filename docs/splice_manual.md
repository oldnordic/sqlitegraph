# Splice Manual - Span-Safe Refactoring Kernel for Rust

**Version**: 0.1.0
**Binary**: `/usr/local/bin/splice`
**Status**: Production-Ready, Self-Hosting

---

## Overview

Splice is a span-safe refactoring tool for Rust code that performs byte-accurate, AST-validated replacements using SQLiteGraph as the ground-truth code graph.

### Core Philosophy

1. **Span Safety**: All operations work with exact byte spans (start, end positions)
2. **Validation Gates**: Every patch passes tree-sitter reparse + cargo check
3. **Atomic Rollback**: Any failure triggers automatic rollback
4. **No Magic**: Explicit parameters only, no hidden behavior

### What Splice Does

- Replace function bodies, struct definitions, enum variants, trait definitions, impl blocks
- Validate syntax with tree-sitter after every patch
- Validate semantics with cargo check after every patch
- Rollback atomically if ANY validation fails
- Orchestrate multi-step refactors with JSON plans

### What Splice Does NOT Do

- Cross-file reference tracking (use `rg` or IDE features)
- Automatic symbol discovery (you must know exact symbol names)
- Smart refactoring (no "rename all references" - that's Task 9+)
- Configuration files (CLI only)
- Persistent databases (on-the-fly graph creation)

---

## Installation

### From Release Binary

```bash
# Copy to system path
sudo cp target/release/splice /usr/local/bin/splice

# Verify installation
splice --version
splice --help
```

### From Source

```bash
# Clone and build
git clone https://github.com/oldnordic/splice.git
cd splice
cargo build --release

# Install
sudo cp target/release/splice /usr/local/bin/splice
```

---

## Quick Start

### Single Patch (Most Common)

```bash
# Replace a function body
splice patch \
  --file src/lib.rs \
  --symbol foo \
  --kind function \
  --with new_foo.rs
```

Where `new_foo.rs` contains:
```rust
pub fn foo(name: &str) -> String {
    format!("New implementation: {}", name)
}
```

### Multi-Step Refactor (Plan)

Create `plan.json`:
```json
{
  "steps": [
    {
      "file": "src/lib.rs",
      "symbol": "foo",
      "kind": "function",
      "with": "patches/foo.rs"
    },
    {
      "file": "src/lib.rs",
      "symbol": "bar",
      "kind": "function",
      "with": "patches/bar.rs"
    }
  ]
}
```

Execute:
```bash
splice plan --file plan.json
```

---

## Command Reference

### splice patch

Apply a single patch to a symbol's span.

#### Syntax

```bash
splice patch \
  --file <PATH> \
  --symbol <NAME> \
  [--kind <KIND>] \
  --with <FILE>
```

#### Required Arguments

- `--file <PATH>`: Path to source file containing the symbol
- `--symbol <NAME>`: Symbol name to patch
- `--with <FILE>`: Path to file containing replacement content

#### Optional Arguments

- `--kind <KIND>`: Symbol kind filter (one of: `function`, `struct`, `enum`, `trait`, `impl`)
- `--analyzer <MODE>`: rust-analyzer mode (one of: `off`, `os`)
- `-v, --verbose`: Enable verbose logging

#### Symbol Kinds

| Kind | Example |
|------|---------|
| `function` | `pub fn foo() {}` |
| `struct` | `pub struct Foo;` |
| `enum` | `pub enum Bar {}` |
| `trait` | `pub trait Baz {}` |
| `impl` | `impl Foo {}` |

#### Examples

```bash
# Replace function body
splice patch --file src/lib.rs --symbol greet --kind function --with new_greet.rs

# Replace struct definition
splice patch --file src/models.rs --symbol User --kind struct --with new_user.rs

# Replace enum variant
splice patch --file src/types.rs --symbol Status --kind enum --with new_status.rs

# Replace trait definition
splice patch --file src/trait.rs --symbol Processor --kind trait --with new_processor.rs

# Replace impl block
splice patch --file src/impl.rs --symbol Impl --kind impl --with new_impl.rs
```

---

### splice plan

Execute a multi-step refactoring plan.

#### Syntax

```bash
splice plan --file <PLAN.json>
```

#### Required Arguments

- `--file <PLAN.json>`: Path to JSON plan file

#### Optional Arguments

- `-v, --verbose`: Enable verbose logging

#### Plan Format

```json
{
  "steps": [
    {
      "file": "src/lib.rs",
      "symbol": "foo",
      "kind": "function",
      "with": "patches/foo.rs"
    }
  ]
}
```

#### Execution Behavior

1. Steps execute **sequentially** in order
2. Execution **stops on first failure**
3. Previous successful steps **remain applied** (no global rollback)
4. Each step has **atomic rollback** via validation gates

#### Example

```bash
# Create plan.json
cat > plan.json << 'EOF'
{
  "steps": [
    {
      "file": "src/lib.rs",
      "symbol": "greet",
      "kind": "function",
      "with": "patches/greet.rs"
    },
    {
      "file": "src/lib.rs",
      "symbol": "farewell",
      "kind": "function",
      "with": "patches/farewell.rs"
    }
  ]
}
EOF

# Execute plan
splice plan --file plan.json

# Output:
# Step 1: Patched 'greet' at bytes 123..456 (hash: abc -> def)
# Step 2: Patched 'farewell' at bytes 789..012 (hash: 123 -> 456)
# Plan executed successfully: 2 steps completed
```

---

## Patch File Format

### Function Replacement

**Original** (`src/lib.rs`):
```rust
pub fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}
```

**Patch** (`new_greet.rs`):
```rust
pub fn greet(name: &str) -> String {
    format!("Greetings, {}!", name)
}
```

**Command**:
```bash
splice patch --file src/lib.rs --symbol greet --kind function --with new_greet.rs
```

### Struct Replacement

**Original** (`src/models.rs`):
```rust
pub struct User {
    name: String,
    age: u32,
}
```

**Patch** (`new_user.rs`):
```rust
pub struct User {
    name: String,
    age: u32,
    email: String,
}
```

**Command**:
```bash
splice patch --file src/models.rs --symbol User --kind struct --with new_user.rs
```

### Enum Replacement

**Original** (`src/types.rs`):
```rust
pub enum Status {
    Active,
    Inactive,
}
```

**Patch** (`new_status.rs`):
```rust
pub enum Status {
    Active,
    Inactive,
    Pending,
}
```

**Command**:
```bash
splice patch --file src/types.rs --symbol Status --kind enum --with new_status.rs
```

---

## Error Handling

### Common Errors

#### Symbol Not Found

```bash
$ splice patch --file src/lib.rs --symbol nonexistent --kind function --with patch.rs
Error: Symbol not found: nonexistent
```

**Solution**: Check symbol name, add `--file` hint, or verify symbol kind

#### Ambiguous Symbol

```bash
$ splice patch --symbol foo --with patch.rs
Error: Ambiguous symbol 'foo': found in multiple files: ["src/a.rs", "src/b.rs"]
```

**Solution**: Add `--file` to disambiguate:
```bash
splice patch --file src/a.rs --symbol foo --kind function --with patch.rs
```

#### Parse Validation Failed

```bash
$ splice patch --file src/lib.rs --symbol foo --with invalid.rs
Error: Parse validation failed: file 'src/lib.rs' - Tree-sitter detected syntax errors in patched file
```

**Solution**: Check your patch file for syntax errors (cargo check it first)

#### Cargo Check Failed

```bash
$ splice patch --file src/lib.rs --symbol foo --with type_error.rs
Error: Cargo check failed in workspace '.': error[E0308]: mismatched types
```

**Solution**: Fix type errors in your patch file before using splice

---

## Validation Gates

Every patch passes through multiple validation gates:

### 1. UTF-8 Boundary Validation
- Ensures start/end byte positions align with UTF-8 character boundaries
- Prevents corrupting multi-byte UTF-8 sequences

### 2. Tree-Sitter Reparse
- Validates syntax of patched file
- Uses tree-sitter-rust parser
- Catches syntax errors immediately

### 3. Cargo Check
- Validates semantic correctness
- Runs full workspace compilation check
- Catches type errors, missing imports, etc.

### 4. rust-analyzer (Optional)
- Opt-in via `--analyzer os` flag
- Uses rust-analyzer from PATH
- Catches additional lint issues

### Rollback Behavior

- **Automatic**: Any gate failure triggers immediate rollback
- **Atomic**: Original file restored atomically (temp + fsync + rename)
- **Safe**: No partial patch states possible

---

## Advanced Usage

### Disambiguating Symbols

When multiple files define the same symbol name:

```bash
# Ambiguous - fails
$ splice patch --symbol foo --with patch.rs
Error: Ambiguous symbol 'foo': found in multiple files

# Disambiguated with file path
$ splice patch --file src/lib.rs --symbol foo --kind function --with patch.rs
✓ Success
```

### Verbose Logging

```bash
# Enable debug logging
splice -v patch --file src/lib.rs --symbol foo --with patch.rs
```

### rust-analyzer Integration

```bash
# Default: analyzer OFF
splice patch --file src/lib.rs --symbol foo --with patch.rs

# Enable analyzer (from PATH)
splice patch --file src/lib.rs --symbol foo --with patch.rs --analyzer os
```

**Note**: Analyzer is OFF by default. Only use if you need extra lint validation.

---

## Workflows

### Workflow 1: Safe Function Rename

1. Create new function with desired name in `new_foo.rs`
2. Run splice to replace old function:
   ```bash
   splice patch --file src/lib.rs --symbol foo --kind function --with new_foo.rs
   ```
3. Manually update all call sites to use new name (use IDE or `rg`)

### Workflow 2: Stub Implementation

1. Create stub in `stub.rs`:
   ```rust
   pub fn complex_function() -> Result<()> {
       todo!("Implement this")
   }
   ```
2. Replace real implementation:
   ```bash
   splice patch --file src/lib.rs --symbol complex_function --kind function --with stub.rs
   ```
3. Implement later

### Workflow 3: Multi-File Refactor

1. Create `plan.json` with all steps
2. Create patch files for each step
3. Execute plan:
   ```bash
   splice plan --file plan.json
   ```
4. If step N fails, steps 1..N-1 remain applied
5. Fix issue and continue (no resume mode - create new plan for remaining steps)

---

## Best Practices

### DO

✅ Run `cargo check` on patch files before using splice
✅ Use `--kind` to disambiguate when needed
✅ Test patches in git repos (easy rollback)
✅ Use verbose mode (`-v`) for debugging
✅ Create backup branches before large plans

### DON'T

❌ Manually edit files after splice starts (breaks atomicity)
❌ Skip `cargo check` on patch files (wastes time)
❌ Use patch files with syntax errors
❌ Assume splice will find all references (it won't)
❌ Create plans with 100+ steps (no resume mode)

---

## Troubleshooting

### Patch Fails with "Symbol Not Found"

**Diagnosis**: Symbol name doesn't match

**Solutions**:
1. Check symbol spelling
2. Verify `--file` path is correct
3. Add `--kind` if needed
4. Run `rg 'pub fn symbol_name'` to find exact definition

### Patch Fails with "Ambiguous Symbol"

**Diagnosis**: Multiple files define same symbol name

**Solution**: Add `--file` to specify which file

### Patch Fails with "Parse Validation Failed"

**Diagnosis**: Patch file has syntax error

**Solutions**:
1. Run `cargo check` on patch file first
2. Ensure patch file has complete valid Rust code
3. Check for missing braces, semicolons, etc.

### Patch Fails with "Cargo Check Failed"

**Diagnosis**: Patch introduces type error or missing import

**Solutions**:
1. Read cargo error output carefully
2. Fix types in patch file
3. Add missing imports to patch file
4. Test in separate file first

---

## Technical Details

### How Splice Works

1. **Extract Symbols**: Parse source file with tree-sitter-rust
2. **Resolve Symbol**: Find symbol byte span in AST
3. **Read Replacement**: Load patch file content
4. **Replace Span**: Use ropey for byte-exact replacement
5. **Validate**: Run tree-sitter reparse + cargo check
6. **Commit or Rollback**: Atomic based on validation

### Byte Spans vs Line/Column

Splice uses **byte offsets** (not line/column numbers) because:
- Deterministic (independent of editor line endings)
- Exact (no ambiguity in multi-byte characters)
- Fast (no conversion overhead)

### File Hashes

Every patch returns SHA-256 hashes:
- `before_hash`: Hash of original file content
- `after_hash`: Hash of patched file content

Use these for:
- Audit trails
- Verification
- Change detection

---

## Limitations

### Current Limitations (as of v0.1.0)

1. **No Cross-File Reference Tracking**: Splice doesn't know which functions call which
2. **No Resume Mode**: Failed plans leave partial state (manual recovery)
3. **No Auto-Discovery**: Must know exact symbol names
4. **No Persistent Database**: Graph created on-the-fly for each patch
5. **No Dry-Run Mode`: Can't preview changes without applying
6. **Single-File Symbols**: Can't patch symbols defined across multiple files

### What Splice is NOT

- ❌ An IDE (use Rust Analyzer RLS)
- ❌ A semantic refactor tool (use IntelliJ Rust)
- ❌ A build system (use cargo)
- ❌ A linter (use clippy)
- ❌ A code formatter (use rustfmt)

### What Splice IS

- ✅ A span-safe find-and-replace
- ✅ A validation gate for code changes
- ✅ An orchestration tool for multi-step refactors
- ✅ A safety net for automated edits
- ✅ An LLM tool for code modification

---

## Examples Repository

See `examples/` directory for:
- Single function patch
- Multi-step plan
- Struct modification
- Enum extension
- Trait replacement

---

## Contributing

Splice is **COMPLETE** as of v0.1.0.

No new features are planned.

For bug reports, use GitHub Issues.

---

## License

GPL-3.0-or-later

See LICENSE file for details.

---

## Version History

### v0.1.0 (2025-12-23)

- Initial release
- Self-hosting proven
- All validation gates implemented
- JSON plan format
- CLI interface
- 22/22 tests passing
- Production-ready

---

## Support

- **GitHub**: https://github.com/oldnordic/splice
- **Documentation**: See `docs/TODO.md` for development history
- **Manual**: This file (`manual.md`)

---

**End of Manual**

For quick help, run:
```bash
splice --help
splice patch --help
splice plan --help
```
