# Development Workflow

**Purpose**: Prevent code drift, database drift, and "works on my machine" syndrome.
**Status**: MANDATORY for all changes.

---

## The Golden Rule

> **NEVER write code based on assumptions. ALWAYS read the source first.**

---

## Workflow Diagram

```
┌─────────────────────────────────────────────────────────────┐
│ 1. UNDERSTAND (READ FIRST)                                 │
│    - Read source code                                       │
│    - Check database schema (.schema queries)                │
│    - Read existing docs/                                    │
└────────────┬────────────────────────────────────────────────┘
             │
             ▼
┌─────────────────────────────────────────────────────────────┐
│ 2. PLAN (STORE DECISION)                                    │
│    - Architectural decision with reasoning                  │
│    - Alternatives considered                                │
│    - Trade-offs documented                                  │
└────────────┬────────────────────────────────────────────────┘
             │
             ▼
┌─────────────────────────────────────────────────────────────┐
│ 3. PROVE (TDD)                                               │
│    - Write failing test                                     │
│    - Show expected failure                                  │
└────────────┬────────────────────────────────────────────────┘
             │
             ▼
┌─────────────────────────────────────────────────────────────┐
│ 4. IMPLEMENT                                                │
│    - Write code to pass test                                │
│    - Use proper refactoring tools                           │
└────────────┬────────────────────────────────────────────────┘
             │
             ▼
┌─────────────────────────────────────────────────────────────┐
│ 5. VERIFY                                                   │
│    - Show test passes with full output                      │
│    - Run cargo check / equivalent                           │
│    - Update documentation                                   │
└─────────────────────────────────────────────────────────────┘
```

---

## Step 1: UNDERSTAND (READ FIRST)

### Check Database Schema
```bash
# Get actual schema
sqlite3 .codemcp/operations.db ".schema"

# Check row counts
sqlite3 .codemcp/operations.db "
SELECT 'table_name', COUNT(*) FROM table_name
UNION ALL ...
"

# Verify columns exist
sqlite3 .codemcp/operations.db "PRAGMA table_info(reinjection_briefs);"
```

### Read Source Code
```bash
# Use Read tool, NOT cat
Read /path/to/file.rs

# Get specific line range
Read /path/to/file.rs (lines 100-200)
```

### Check Existing Tools
```bash
# What tools are available?
# Check project's CLAUDE.md or docs/API.md
```

---

## Step 2: PLAN (STORE DECISION)

### Decision Template

```sql
INSERT INTO architectural_decisions (
    project, decision, reasoning, alternatives, trade_offs,
    related_files, related_symbols,
    session_id, created_at_epoch, created_at
) VALUES (
    'project-name',
    'Short title of change',
    'WHY this change is needed.
     WHAT problem it solves.
     PROOF: [cite exact file:line numbers]',
    'Alt 1: [description]
     Alt 2: [description]
     Why rejected: [reason]',
    'Cost: [time/complexity]
     Risk: [what could break]
     Benefit: [what we gain]',
    '["/path/to/file1.rs", "/path/to/file2.rs"]',
    '["function_name", "StructName"]',
    'session-xyz',
    strftime('%s', 'now'),
    datetime('now')
);
```

### For Bugfixes Specifically

```sql
INSERT INTO architectural_decisions (
    project, decision, reasoning, alternatives, trade_offs,
    session_id, created_at_epoch, created_at
) VALUES (
    'project-name',
    'Fix: [Bug Description]',
    'ROOT CAUSE: [exact location, proven evidence]
     PROOF: [error message, stack trace, test output]
     FIX: [how this addresses root cause]',
    'Quick fix: [why insufficient]
     Rewrite: [why overkill]',
    'Regression risk: [what could break]
     Mitigation: [test coverage]',
    'session-xyz',
    strftime('%s', 'now'),
    datetime('now')
);
```

---

## Step 3: PROVE (TDD)

### Write Failing Test First

```rust
#[test]
fn test_reinjection_brief_retrieval() {
    // Given: A brief exists in database
    let brief = create_test_brief(...);

    // When: We query for it
    let result = get_reinjection_brief("project");

    // Then: Should return the brief
    assert_eq!(result.project, "project");
    assert!(result.current_task.len() > 0);
}
```

### Run and Show Failure

```bash
$ cargo test test_reinjection_brief

FAILURES:
---- test_reinjection_brief_retrieval stdout ----
thread panicked at 'assertion failed: `(left == right)`'
  left: `""`,
  right: `"current task value"'
```

**This proves the test catches the bug.**

---

## Step 4: IMPLEMENT

### Use Proper Tools

| Task | Tool | Why |
|------|------|-----|
| Find symbols | `find_symbols` | Exact byte spans |
| Rename | `refactor_rename` | Safe, validated |
| Delete | `refactor_delete` | Removes all refs |
| Understand | `discover_summary` | Semantic info |

### Write the Code

```rust
// Implement fix based on ROOT CAUSE analysis
pub fn get_reinjection_brief(project: &str) -> Option<Brief> {
    // PROVEN: This table exists (checked schema)
    // PROVEN: These columns exist (checked PRAGMA)
    let query = "
        SELECT current_task, active_files, explicit_goal, self_check_prompt
        FROM reinjection_briefs
        WHERE project = ?
        ORDER BY created_at_epoch DESC
        LIMIT 1
    ";
    // ...
}
```

---

## Step 5: VERIFY

### Show Test Passes (Full Output)

```bash
$ cargo test test_reinjection_brief

running 1 test
test test_reinjection_brief_retrieval ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured
```

### Run Compiler Check

```bash
$ cargo check
    Checking project v0.1.0
    Finished `dev` profile
```

### Update Documentation

- Update DATABASE_SCHEMA.md if schema changed
- Update API.md if public API changed
- Update CLAUDE.md if workflow changed

---

## Example: session-start.js Bugfix

### Step 1: UNDERSTAND
```bash
# Read claude-mem's working implementation
Read /home/feanor/.claude/plugins/marketplaces/thedotmack/src/services/context-generator.ts

# Check CodeMCP's actual schema
sqlite3 .codemcp/operations.db ".schema reinjection_briefs"
```

### Step 2: PLAN
```sql
INSERT INTO architectural_decisions (...) VALUES (
    'codemcp',
    'Fix session-start.js SQL syntax errors',
    'ROOT CAUSE: SQLite CLI does not support ?1 positional params.
     PROOF: Error "near ?1: syntax error"
     FIX: Use quoted string interpolation with proper escaping.',
    '1. Use Rust rusqlite (requires rewrite)
     2. Use prepared statements (complex in shell)',
    'Shell SQL is simpler but requires careful escaping.',
    '["plugin/scripts/session-start.js"]',
    '["get_reinjection_brief"]',
    'session-20250108',
    strftime('%s', 'now'),
    datetime('now')
);
```

### Step 3: PROVE
```bash
# Write test query, verify it fails
sqlite3 .codemcp/operations.db "SELECT ... WHERE project = 'codemcp'"
```

### Step 4: IMPLEMENT
```javascript
// Fixed: Use quoted string interpolation
const query = `
  SELECT current_task, active_files, explicit_goal, self_check_prompt
  FROM reinjection_briefs
  WHERE project = '${escapedProject}'
  ORDER BY created_at_epoch DESC
  LIMIT 1
`;
```

### Step 5: VERIFY
```bash
# Test the fixed query
node plugin/scripts/session-start.js
# Output: Correct reinjection brief displayed
```

---

## Anti-Patterns (DO NOT DO)

| ❌ Anti-Pattern | ✅ Correct Approach |
|----------------|-------------------|
| `grep "function_name"` | `find_symbols(query="function_name")` |
| `cat file.rs` | `Read /path/to/file.rs` |
| Edit without reading | Read first, then Edit |
| Assume schema | Query `.schema` first |
| "I'll fix later" | Fix now or document debt |
| Comment out broken code | Delete or fix properly |
| `#[allow(...)]` | Fix the warning |
| TODO/FIXME in prod | Do it now or create issue |

---

## Quick Reference

### Database Commands
```bash
# Check schema
sqlite3 path/to.db ".schema"

# Check specific table
sqlite3 path/to.db "PRAGMA table_info(table_name);"

# Check row counts
sqlite3 path/to.db "SELECT COUNT(*) FROM table_name;"

# Check indexes
sqlite3 path/to.db ".indexes"

# Test query
sqlite3 path/to.db "SELECT ... LIMIT 5;"
```

### CodeMCP Tools
```bash
# Find symbol (get exact locations)
find_symbols(query="symbol_name")

# Get semantic summary
discover_summary(symbol="function_name", auto_index=true)

# Get code chunks (no file I/O)
get_code_chunks(file_path="src/lib.rs")

# Check database status
semantic_stats()
memory_index_status()

# Store decision (before coding!)
store_decision(project="name", decision="...", reasoning="...")
```

---

## Remember

> **"Two days of debugging can save you ten minutes of planning."**

Plan first. Read source. Check schema. Store decision. Then code.
