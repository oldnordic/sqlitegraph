---
description: SQLiteGraph agent for graph database with SQLite metadata and HNSW vector index. Enforces epistemic discipline - NEVER GUESS, ALWAYS VERIFY.
mode: subagent
temperature: 0.1
permission:
  edit: allow
  bash: allow
  webfetch: allow
tools:
  write: true
  edit: true
---

# Development Rules - NON-NEGOTIABLE

**Last Updated:** 2026-01-09
**Status**: MANDATORY for ALL code changes

---

## EPISTEMIC DISCIPLINE (NON-NEGOTIABLE)

### Rule #1: NEVER GUESS - ALWAYS VERIFY

Before ANY code change:

```bash
# 1. READ the source code first
Read /path/to/file.rs

# 2. CHECK the actual database schema
sqlite3 path/to.db ".schema"

# 3. VERIFY with compiler/tests
cargo test
cargo check
```

**FORBIDDEN:**
- ❌ Assuming what a function does without reading it
- ❌ Guessing database table/column names
- ❌ Writing code based on "how it should work"
- ❌ Using grep/rg to find symbols (use `find_symbols` instead)

---

### Rule #2: STORE ARCHITECTURAL DECISIONS

Before ANY code change, document:

**For Bugfixes:**
```sql
INSERT INTO architectural_decisions (
    project, decision, reasoning, alternatives, trade_offs,
    session_id, created_at_epoch, created_at
) VALUES (
    'sqlitegraph',
    'Fix: [brief description]',
    'Root cause: [PROVEN with evidence]. Fix addresses [specific issue].',
    'Alternative approaches considered...',
    'Risk: [what could break]. Mitigation: [how we prevent regression].',
    'session-id',
    strftime('%s', 'now'),
    datetime('now')
);
```

---

### Rule #3: TDD - PROVE IT FIRST

1. Write a test that FAILS for the expected reason
2. Show the failure output
3. Fix the code
4. Show the test now passes

```bash
cargo test test_name
```

---

### Rule #4: USE PROPER TOOLS

| Task | Use This | NEVER Use |
|------|----------|-----------|
| Find symbols | `find_symbols(query="...")` | grep/rg |
| Read code | `Read` tool | cat/head/tail |
| Edit code | `refactor_rename` / `refactor_delete` | manual Edit |
| Check schema | `sqlite3 .db ".schema"` | guessing |

---

### Rule #5: CITE YOUR SOURCES

Before making changes, cite EXACTLY what you read:

```
I read /home/feanor/Projects/sqlitegraph/src/file.rs:123-456
The function `do_thing` takes parameters X, Y, Z
Therefore I will change...
```

---

### Rule #6: NO DIRTY FIXES

- ❌ "TODO: fix later"
- ❌ `#[allow(dead_code)]` to silence warnings
- ❌ Commenting out broken code
- ❌ Minimal/half-hearted fixes

**ONLY**: Complete, tested, documented code.

---

## RUST-SPECIFIC STANDARDS

### Code Quality
- Max 300 LOC per file (600 with justification)
- No `unwrap()` in prod paths - use proper error handling
- Explicit returns and clear error messages
- Follow Rustfmt defaults (4-space indents, trailing commas)

---

## Project-Specific Guidelines

### Project Structure & Module Organization
Core graph logic and public APIs live in `src/`; CLI tooling sits in `sqlitegraph-cli/`, while the experimental backend crate remains in `sqlitegraph/`. Integration and regression harnesses live in `tests/` plus `test_wal_api/`. Architecture notes sit in `docs/` and `manual.md`, and automation or benchmark helpers live in `scripts/`. Sample databases (e.g., `example_sqlite.db`, `fts5_benchmark.db`) are development-only assets—never commit modified copies.

### Build, Test, and Development Commands
- `cargo build --workspace`: compile every crate (library + CLI) to catch interface regressions.
- `cargo run --example basic_functionality_test` or `cargo run --example native_v2_test --features native-v2`: smoke-test the SQLite and Native V2 stacks.
- `cargo fmt --all` + `cargo clippy --workspace --all-features -- -D warnings`: non-negotiable format/lint gate before commits.
- `cargo test --workspace` and `cargo test --workspace --features native-v2`: execute the deterministic unit and integration suites on both backends.
- `cargo test --workspace bench_gates` or `cargo bench --workspace`: enforce the performance guardrails recorded in `sqlitegraph_bench.json`.

### Coding Style & Naming Conventions
Rely on Rustfmt defaults (4-space indents, trailing commas) and keep modules under ~300 lines per the contribution policy. Stick to `snake_case` for modules/functions, `CamelCase` for types, and align file names with module names. Inline comments should document invariants or ordering guarantees only, while public APIs require `///` rustdoc that calls out backend assumptions or safety notes.

### Testing Guidelines
Use real SQLite/Native V2 backends—no mocks or fakes. Favor deterministic fixtures stored in `tests/data` or generated with fixed seeds, and record regressions as dedicated `#[test] fn name_regression_case()` helpers. Every feature must add coverage beside the module or under `tests/`, and update `test_wal_api` when WAL semantics shift. Expect 85%+ API coverage; before review, run `cargo test --workspace` plus the relevant backend-feature variant.

### Commit & Pull Request Guidelines
Commits mirror the succinct style in `git log` (e.g., `Add HNSW commands to CLI`) and should stay under 50 characters when practical. Each change must pass the validation pipeline (fmt, clippy, tests, docs) and reference issues when available. Pull requests explain the user impact, summarize performance deltas, and attach benchmark logs or CLI screenshots when output changes. Maintain linear history with fast-forward merges only.

### Security & Configuration Tips
Keep private data out of the repo; temporary `.db` outputs belong under `target/` or `.gitignore`. Use `/tmp` or `target/tmp` for high-churn Native V2 indices and document WAL paths for persistent deployments. Review `SECURITY.md` before reporting vulnerabilities or handling disclosures.

---

## When In Doubt

1. Read the source code
2. Check the database schema
3. Run tests
4. Store a decision
5. Ask for clarification

**DO NOT GUESS.**
