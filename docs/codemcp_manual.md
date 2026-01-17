# CodeMCP User Manual - Part 1

**Version:** 0.1.0
**Last Updated:** 2025-01-05
**Status:** Comprehensive Guide

---

## Table of Contents

1. [Overview and Introduction](#1-overview-and-introduction)
2. [Quick Start Guide](#2-quick-start-guide)
3. [Configuration](#3-configuration)

---

## 1. Overview and Introduction

### 1.1 What is CodeMCP?

**CodeMCP** is a Model Context Protocol (MCP) server that provides **safe, automated code refactoring tools** through an enforced workflow: **Magellan → Splice → LSP**.

#### The Problem It Solves

After context compaction, LLMs often forget sophisticated tool workflows and fall back to primitive operations that are dangerous:

| Wrong Approach | Why It Fails |
|----------------|--------------|
| `grep`/`regex` for symbols | Inexact matching, false positives, misses references |
| Manual file editing | Breaks references, misses cross-file usages |
| `sed` find+replace | Dangerous, no validation, broken code |
| Skipping LSP checks | Compilation errors get committed |

#### The CodeMCP Solution

CodeMCP enforces a strict workflow that guarantees correctness:

```
Magellan (exact byte spans) → Splice (safe edits) → LSP (validation) → Database (audit)
```

Every refactoring operation is:
- **Exact**: Uses byte spans from code graph, not text matching
- **Atomic**: All files succeed or all rollback together
- **Validated**: Compilation must pass, or changes are rejected
- **Audited**: Every operation recorded to SQLite database for provenance

### 1.2 Key Features and Capabilities

#### Exact Symbol Discovery

CodeMCP uses **Magellan** integration for byte-level precision:

- **Precise locations**: Byte-level accuracy for symbol positions
- **Cross-file reference tracking**: No broken references, ever
- **Call graph analysis**: Understand dependencies before refactoring
- **Code chunk storage**: Source code cached in database (no re-reading files)

#### Safe Refactoring

CodeMCP uses the **Splice kernel** for reliable code changes:

- **AST-aware edits**: Respects code structure, not text manipulation
- **LSP validation gates**: Compilation must pass or changes are rejected
- **Atomic operations**: All files succeed or all fail together
- **Audit trail**: Every operation logged to SQLite database

#### Lazy Semantic Indexing

CodeMCP uses on-demand semantic analysis for efficiency:

- **On-demand analysis**: Index only what you query
- **First query**: Slower (~500 tokens, LLM call) but indexes for future
- **Subsequent queries**: Instant (~50 tokens, cached from database)
- **No marathon runs**: No 79-day full-corpus indexing

The `semantic.db` starts **empty** and grows as you query symbols. Hash-based staleness detection ensures symbols are only re-indexed when code changes.

#### Documentation Intelligence

CodeMCP provides intelligent documentation management:

- **Semantic search**: Query docs by meaning, not keywords
- **Link validation**: Detect broken doc references
- **Staleness tracking**: Know when docs are out of sync with code
- **Staging workflow**: Changes are validated before promotion

### 1.3 Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         LLM (Claude)                            │
└────────────────────────────┬────────────────────────────────────┘
                             │ MCP Protocol
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                      CodeMCP Server (Rust)                      │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  MCP Tool Layer (Exposed to LLM)                         │  │
│  │  - refactor_rename, find_symbols, discover_summary, etc. │  │
│  └──────────────────────────────────────────────────────────┘  │
│                             │                                    │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  Enforced Workflow Engine                                │  │
│  │  1. Magellan: Exact byte spans from code graph          │  │
│  │  2. Splice: AST-safe edits with rollback                │  │
│  │  3. LSP: Compilation validation                          │  │
│  │  4. SQLiteGraph: Dependency tracking                    │  │
│  │  5. Report generation: Audit trail                      │  │
│  └──────────────────────────────────────────────────────────┘  │
│                             │                                    │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  Persistence Layer (.codemcp/)                           │  │
│  │  - codegraph.db (symbol locations, code chunks)          │  │
│  │  - semantic.db (lazy LLM analysis, cached)               │  │
│  │  - operations.db (refactor history)                      │  │
│  │  - hopgraph.db (docs embeddings, links)                  │  │
│  │  - raggraph.db (semantic search)                         │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        ▼                    ▼                    ▼
┌───────────────┐    ┌───────────────┐    ┌───────────────┐
│   Magellan    │    │    Splice     │    │   cargo/LSP   │
│ (code graph)  │    │ (refactoring) │    │  (validation) │
└───────────────┘    └───────────────┘    └───────────────┘
```

#### Database Schema

CodeMCP stores all data in the `.codemcp/` folder:

| Database | Purpose |
|----------|---------|
| `codegraph.db` | Symbol locations with exact byte spans, call graph edges, cached source code |
| `semantic.db` | Lazy-indexed semantic summaries, purpose tags, complexity analysis |
| `operations.db` | Refactor operation history, dependencies, audit trail |
| `hopgraph.db` | Documentation structure, internal links, code references |
| `raggraph.db` | Documentation embeddings for semantic search |

### 1.4 Use Cases

#### Use Case 1: Safe Symbol Renaming

**Scenario**: You need to rename a function that's used across 50 files.

**Without CodeMCP**:
1. Use grep to find all occurrences (misses some, finds false positives)
2. Manually edit each file (error-prone, slow)
3. Hope you didn't break anything (no validation)
4. Discover broken references weeks later

**With CodeMCP**:
```javascript
// Step 1: Find the symbol (exact byte spans)
find_symbols("my_function", { showCode: true })

// Step 2: Rename with validation
refactor_rename({
  symbol_name: "my_function",
  new_name: "new_function_name",
  kind: "function",
  workspace_root: "/path/to/project",
  preview_only: true  // See what will change first
})

// Step 3: Apply the rename
refactor_rename({
  symbol_name: "my_function",
  new_name: "new_function_name",
  kind: "function",
  workspace_root: "/path/to/project"
})
// ✓ All files updated atomically
// ✓ Compilation validated automatically
// ✓ Audit trail recorded
```

#### Use Case 2: Understanding Large Codebases

**Scenario**: You're working with a large, unfamiliar codebase and need to understand authentication logic.

**Without CodeMCP**:
1. Grep for "auth" (hundreds of results)
2. Read through countless files manually (token-intensive)
3. Still miss important pieces
4. Waste hours

**With CodeMCP**:
```javascript
// Step 1: Find all auth functions by purpose (token-efficient!)
discover_by_purpose("authentication", { limit: 20 })

// Step 2: Get semantic summary of key functions
discover_summary("authenticate_user", { auto_index: true })
// → "Validates JWT tokens and returns user session"

// Step 3: See the actual code without file I/O
get_code_chunks({
  file_path: "src/auth.rs",
  symbol_name: "authenticate_user"
})

// Result: Understanding in ~500 tokens instead of ~50,000
```

#### Use Case 3: Impact Analysis Before Refactoring

**Scenario**: You want to refactor a core utility function but need to know what will break.

**Without CodeMCP**:
1. Grep for function name (incomplete)
2. Manually trace call chains (error-prone)
3. Miss downstream dependencies
4. Break production code

**With CodeMCP**:
```javascript
// Step 1: Get full impact analysis
get_impact_analysis({
  symbol_name: "process_request",
  depth: 3  // See 3 levels of dependencies
})

// Returns:
// {
//   total_impacted: 47,
//   by_hop: {
//     "1": [{ name: "handle_http", file: "src/api.rs" }, ...],
//     "2": [{ name: "serve_client", file: "src/server.rs" }, ...],
//     "3": [{ name: "main", file: "src/main.rs" }, ...]
//   }
// }

// Now you know EXACTLY what will be affected
```

#### Use Case 4: Documentation Maintenance

**Scenario**: You've renamed symbols and need to update documentation references.

**Without CodeMCP**:
1. Manually search docs for old names
2. Miss references in multiple files
3. Dead links accumulate
4. Documentation becomes unreliable

**With CodeMCP**:
```javascript
// Step 1: Check documentation health
docs_status()

// Step 2: Validate links and code references
docs_validate()

// Step 3: Scan for new documentation files
docs_scan()

// Step 4: Stage changes and validate before promotion
docs_validate({ staged: true })
docs_promote({ doc_path: "docs/api.md" })

// Result: Documentation stays synchronized with code
```

---

## 2. Quick Start Guide

### 2.1 Installation

#### Prerequisites

CodeMCP requires **Rust 1.83+** to be installed on your system.

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### Install CodeMCP

```bash
# Clone the repository
git clone https://github.com/modelcontextprotocol/servers.git
cd servers/src/codemcp

# Install from source
cargo install --path .

# Verify installation
codemcp --version
# Output: CodeMCP 0.1.0
```

#### Install Required Dependencies

CodeMCP depends on **Magellan** (code indexing) and **Splice** (refactoring):

```bash
# Install Magellan and Splice
cargo install magellan splice

# Verify installation
magellan --version
splice --version
```

### 2.2 Initial Setup

#### Step 1: Initialize Your Workspace

Navigate to your project directory and initialize CodeMCP:

```bash
cd /path/to/your/project

# Initialize CodeMCP (creates .codemcp/ folder and databases)
codemcp init

# Output:
# === CodeMCP Workspace Initialization ===
#
# Workspace: /path/to/your/project
# Concurrent indexing: true
# Model: glm-4.7
# Endpoint: https://api.z.ai/api/anthropic
#
# [1/4] Creating .codemcp folder structure...
# Created: /path/to/your/project/.codemcp
#
# [2/4] Checking configuration...
# Created: /path/to/your/project/.codemcp/config.toml
#
# [3/4] Running Magellan code graph scan...
# Files scanned: 142
# Code graph: /path/to/your/project/.codemcp/codegraph.db
#
# [4/4] Running semantic indexing...
# === Semantic Indexing Complete ===
# Total symbols:  850
# Indexed:       850
# Skipped:       0 (unchanged)
# Failed:        0
#
# === Initialization Complete ===
```

#### Step 2: Configure Claude Desktop

Add CodeMCP to your Claude Desktop configuration file:

**macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
**Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "codemcp": {
      "command": "codemcp",
      "env": {
        "CODEMCP_AUTO_WATCH": "1",
        "CODEMCP_AUTO_INDEX": "1",
        "ANTHROPIC_AUTH_TOKEN": "your-api-key-here"
      }
    }
  }
}
```

**Restart Claude Desktop** after editing the configuration file.

### 2.3 Basic Commands

#### CLI Commands

CodeMCP provides several CLI commands for workspace management:

```bash
# Initialize workspace (creates .codemcp/ and databases)
codemcp init

# Start the MCP server (for use with Claude Desktop)
codemcp serve

# Migrate old databases to .codemcp/ folder
codemcp migrate

# Semantic indexing commands
codemcp semantic index      # Index all symbols with LLM
codemcp semantic stats      # Show semantic database statistics

# Documentation commands
codemcp docs scan           # Scan and stage documentation files
```

#### MCP Tools (Available in Claude Chat)

Once connected to Claude Desktop, you can use these tools directly in conversations:

```javascript
// Initialize code graph (one-time setup)
magellan_init({ workspace_root: "." })

// Start background watcher (keeps database fresh)
magellan_watch({ action: "start", workspace_root: "." })

// Find symbol locations (exact byte spans)
find_symbols({ query: "my_function", showCode: true })

// Semantic discovery
discover_summary({ symbol: "authenticate_user" })
discover_by_purpose({ purpose: "authentication" })

// Refactoring
refactor_rename({
  symbol_name: "old_func",
  new_name: "new_func",
  kind: "function",
  workspace_root: ".",
  preview_only: true  // Preview first!
})

// Documentation
docs_status()
docs_validate()
```

### 2.4 First-Time Workflow

Let's walk through your first refactoring operation with CodeMCP.

#### Scenario: Rename a Function

You have a function called `process_data` that you want to rename to `transform_data`.

##### Step 1: Find the Symbol

First, locate the function and see its current usage:

```javascript
find_symbols({
  query: "process_data",
  showCode: true
})
```

**Response**:
```json
{
  "definition": {
    "file": "src/lib.rs",
    "byte_start": 1234,
    "byte_end": 1456,
    "kind": "Function"
  },
  "references": [
    {
      "file": "src/main.rs",
      "byte_start": 567,
      "byte_end": 579
    },
    {
      "file": "src/tests.rs",
      "byte_start": 890,
      "byte_end": 902
    }
  ],
  "code": "fn process_data(input: &str) -> Result<String> { ... }"
}
```

##### Step 2: Preview the Rename

Before applying changes, see what will happen:

```javascript
refactor_rename({
  symbol_name: "process_data",
  new_name: "transform_data",
  kind: "function",
  workspace_root: ".",
  preview_only: true
})
```

**Response**:
```json
{
  "preview": true,
  "files_affected": 3,
  "references": [
    {
      "file": "src/lib.rs",
      "byte_start": 1234,
      "byte_end": 1246,
      "context": "fn process_data(...)"
    },
    {
      "file": "src/main.rs",
      "byte_start": 567,
      "byte_end": 579,
      "context": "process_data(input)"
    },
    {
      "file": "src/tests.rs",
      "byte_start": 890,
      "byte_end": 902,
      "context": "process_data(test)"
    }
  ],
  "message": "Preview: 3 files will be modified. Set preview_only=false to apply changes."
}
```

##### Step 3: Apply the Rename

If the preview looks correct, apply the changes:

```javascript
refactor_rename({
  symbol_name: "process_data",
  new_name: "transform_data",
  kind: "function",
  workspace_root: "."
})
```

**Response**:
```json
{
  "success": true,
  "files_modified": 3,
  "operation_id": "2025-01-05_123456abcdef",
  "validation": {
    "passed": true,
    "compiler": "cargo check",
    "errors": 0,
    "warnings": 0
  },
  "report_path": ".codemcp/reports/2025-01-05_123456.md"
}
```

##### Step 4: Verify the Changes

Check the report file to see exactly what changed:

```bash
cat .codemcp/reports/2025-01-05_123456.md
```

**Report Contents**:
```markdown
# Refactor Operation Report

**Operation ID**: 2025-01-05_123456abcdef
**Type**: refactor_rename
**Timestamp**: 2025-01-05T12:34:56Z
**Status**: SUCCESS

## Changes

### src/lib.rs
- Line 42: `process_data` → `transform_data` (definition)

### src/main.rs
- Line 15: `process_data(input)` → `transform_data(input)`

### src/tests.rs
- Line 78: `process_data(test)` → `transform_data(test)`

## Validation
- Compiler: `cargo check`
- Status: PASSED (0 errors, 0 warnings)

## Audit Trail
Operation recorded to `.codemcp/operations.db`
```

#### Summary

You've successfully:
1. ✅ Found exact locations of the symbol (byte-level precision)
2. ✅ Previewed changes before applying (safe exploration)
3. ✅ Applied rename across all files atomically
4. ✅ Validated compilation automatically
5. ✅ Recorded operation to audit trail

**No manual editing, no broken references, no compilation errors.**

---

## 3. Configuration

### 3.1 Config File Structure

CodeMCP uses a **TOML configuration file** located at `.codemcp/config.toml` in your workspace root.

#### Default Configuration

When you run `codemcp init`, a default configuration file is created:

```toml
[indexing]
exclude_folders = [
    "target/**",
    "crates/*/target/**",
    "node_modules/**",
    ".git/**",
    ".hg/**",
    ".svn/**",
    "vendor/**",
    "third_party/**",
    ".venv/**",
    "venv/**",
    "__pycache__/**",
    "*.egg-info/**",
    ".idea/**",
    ".vscode/**",
    ".vs/**",
    "dist/**",
    "build/**",
    "coverage/**"
]
include_folders = []
include_extensions = []
max_file_size_bytes = 1000000

[semantic]
auto_index = false
llm_endpoint = "https://api.z.ai/api/anthropic"
llm_model = "glm-4.7"

[magellan]
auto_watch = false
debounce_ms = 500
semantic_watch = false
```

### 3.2 Configuration Sections

#### Indexing Configuration

Controls which files are included/excluded from indexing:

```toml
[indexing]
# Folder patterns to include (glob syntax)
# If specified, only matching folders are indexed
include_folders = ["src/**", "lib/**"]

# Folder patterns to exclude (glob syntax)
# Files matching these patterns are excluded from indexing
exclude_folders = [
    "target/**",      # Rust build artifacts
    "node_modules/**", # Node.js dependencies
    ".git/**",        # Version control
    "vendor/**"       # Vendored dependencies
]

# File extensions to include
# If specified, only files with these extensions are indexed
include_extensions = [".rs", ".py", ".js", ".ts"]

# Maximum file size to index (in bytes)
# Files larger than this are skipped during indexing
max_file_size_bytes = 1000000  # 1 MB default
```

#### Semantic Configuration

Controls LLM-based semantic analysis behavior:

```toml
[semantic]
# Auto-index on discovery
# When true, automatically runs semantic indexing after discovery
auto_index = false  # Disabled by default (opt-in)

# LLM API endpoint
# HTTP endpoint for LLM API calls
llm_endpoint = "https://api.z.ai/api/anthropic"

# LLM model name
# Model identifier for semantic analysis
llm_model = "glm-4.7"
```

#### Magellan Configuration

Controls file watching behavior:

```toml
[magellan]
# Enable background watching
# When true, watches files for changes and auto-updates index
auto_watch = false  # Disabled by default

# Watcher debounce time (milliseconds)
# Wait time after last file change before re-indexing
debounce_ms = 500

# Auto-update semantic database
# When true, automatically updates semantic.db for changed files
# Requires ANTHROPIC_AUTH_TOKEN to be set
semantic_watch = false
```

### 3.3 Environment Variables

Environment variables **override** config file values:

#### Core Settings

| Variable | Purpose | Default |
|----------|---------|---------|
| `CODEMCP_AUTO_INDEX` | Enable auto-indexing | `0` (disabled) |
| `CODEMCP_AUTO_WATCH` | Enable auto-watching | `0` (disabled) |
| `CODEMCP_SEMANTIC_WATCH` | Enable semantic watching | `0` (disabled) |

#### LLM Settings

| Variable | Purpose | Default |
|----------|---------|---------|
| `CODEMCP_LLM_ENDPOINT` | LLM API endpoint | `https://api.z.ai/api/anthropic` |
| `CODEMCP_LLM_MODEL` | LLM model name | `glm-4.7` |
| `ANTHROPIC_AUTH_TOKEN` | API key for LLM calls | *Required* |

#### Indexing Filters

| Variable | Purpose | Format |
|----------|---------|--------|
| `CODEMCP_EXCLUDE_FOLDERS` | Comma-separated exclude patterns | `"target/**,node_modules/**"` |
| `CODEMCP_INCLUDE_FOLDERS` | Comma-separated include patterns | `"src/**,lib/**"` |

#### Example: Setting Environment Variables

```bash
# Enable auto-indexing and watching
export CODEMCP_AUTO_INDEX=1
export CODEMCP_AUTO_WATCH=1

# Use custom LLM endpoint
export CODEMCP_LLM_ENDPOINT="https://api.anthropic.com/v1"
export CODEMCP_LLM_MODEL="claude-3-5-sonnet-20241022"

# Set API key (required for semantic indexing)
export ANTHROPIC_AUTH_TOKEN="your-api-key-here"

# Custom indexing filters
export CODEMCP_EXCLUDE_FOLDERS="target/**,build/**,dist/**"
export CODEMCP_INCLUDE_FOLDERS="src/**,lib/**"
```

### 3.4 Workspace Setup

#### Workspace Discovery

CodeMCP automatically discovers your workspace root by searching for common markers:

- `.git` directory
- `Cargo.toml` (Rust projects)
- `package.json` (Node.js projects)
- `go.mod` (Go projects)
- `pom.xml` (Maven projects)
- `pyproject.toml` (Python projects)
- `.codemcp` directory (already initialized)

#### Folder Structure

CodeMCP creates the following folder structure:

```
your-project/
├── .codemcp/
│   ├── config.toml           # Configuration file
│   ├── codegraph.db          # Symbol locations and code chunks
│   ├── semantic.db           # Lazy semantic index
│   ├── operations.db         # Refactor operation history
│   ├── hopgraph.db           # Documentation structure
│   ├── raggraph.db           # Documentation embeddings
│   ├── staging.db            # Staged documentation changes
│   ├── docs_changes.db       # Documentation change history
│   ├── backups/              # Backup files before refactor
│   └── reports/              # Refactor operation reports
├── src/
├── tests/
└── Cargo.toml
```

#### Migration from Old Structure

If you have databases in the old location (workspace root), migrate them:

```bash
# Migrate old databases to .codemcp/ folder
codemcp migrate

# Output:
# Migrating databases to .codemcp folder...
# Successfully migrated 2 database(s)
```

**Migration mappings**:
- `./codegraph.db` → `.codemcp/codegraph.db`
- `./.magellan/codegraph.db` → `.codemcp/codegraph.db`
- `./semantic.db` → `.codemcp/semantic.db`

Original files are backed up with `.migrated` suffix.

---

## Next Steps

This concludes **Part 1** of the CodeMCP manual. You should now be able to:

- ✅ Understand what CodeMCP is and how it works
- ✅ Install and initialize CodeMCP in your workspace
- ✅ Configure CodeMCP for your needs
- ✅ Perform basic refactoring operations safely

**Part 2** will cover:
- Advanced tool usage and workflows
- Language-specific features and limitations
- Performance optimization tips
- Troubleshooting common issues

For more information, see:
- [README.md](README.md) - Project overview and quick reference
- [CLAUDE.md](CLAUDE.md) - Usage guide for Claude Code
- [docs/](docs/) - Additional documentation
# CodeMCP User Manual - Part 2: Core Tools

**Version:** 0.1.0
**Last Updated:** 2025-01-05
**Status:** Comprehensive Guide

---

## Table of Contents

1. [Magellan Tools](#1-magellan-tools)
   - [magellan_init](#magellan_init---code-graph-initialization)
   - [magellan_watch](#magellan_watch---continuous-indexing)
   - [magellan_cache_stats](#magellan_cache_stats---performance-monitoring)
2. [Query Tools](#2-query-tools)
   - [find_symbols](#find_symbols---symbol-discovery)
3. [Refactoring Tools](#3-refactoring-tools)
   - [refactor_rename](#refactor_rename---safe-rename-operations)
   - [refactor_delete](#refactor_delete---safe-delete-operations)
   - [refactor_pattern](#refactor_pattern---pattern-based-replacement)
4. [Audit Tools](#4-audit-tools)
   - [query_executions](#query_executions---operation-history)
   - [get_execution](#get_execution---operation-details)

---

## 1. Magellan Tools

### magellan_init - Code Graph Initialization

**Description**: Performs a one-time scan of your codebase to build the Magellan code graph database. This database stores exact symbol locations, call graph edges, and cached source code.

**Prerequisites**: None (first tool to run)

**Parameters**:

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `workspace_root` | string | No | `.` | Path to project directory. Use `.` for auto-discovery |
| `force_recreate` | boolean | No | `false` | Force database recreation even if healthy |
| `filter` | array | No | `null` | List of relative paths to scan (e.g., `["src", "tests"]`) |

**Usage Examples**:

```javascript
// Auto-discover workspace and initialize
magellan_init({ workspace_root: "." })

// Force recreate database
magellan_init({
  workspace_root: "/path/to/project",
  force_recreate: true
})

// Scan only specific directories
magellan_init({
  workspace_root: ".",
  filter: ["src", "lib", "tests"]
})
```

**Return Values**:

```json
{
  "workspace_root": "/path/to/project",
  "db_path": "/path/to/project/.codemcp/codegraph.db",
  "files_scanned": 142,
  "symbols_indexed": 1850
}
```

**Common Use Cases**:

1. **Initial Setup**: Run once when setting up CodeMCP in a new project
2. **Database Refresh**: Use `force_recreate: true` to refresh stale data
3. **Partial Indexing**: Use `filter` to index only specific directories

**Tips and Gotchas**:

- **Auto-discovery**: Using `workspace_root: "."` automatically finds the project root by searching for markers like `Cargo.toml`, `package.json`, etc.
- **Database Location**: Database is always created at `.codemcp/codegraph.db` relative to workspace root
- **Incremental Scans**: If database is healthy (has symbols), `magellan_init` skips scanning unless `force_recreate: true`
- **Filter Security**: Filter paths are validated to prevent directory traversal attacks (`../etc` is rejected)
- **Path Normalization**: Leading `./` in filter paths is automatically stripped
- **Excluded Paths**: Respects `.codemcp/config.toml` exclude patterns (default excludes `target/**`, `node_modules/**`, etc.)

**Database Health Check**:

`magellan_init` automatically checks database health before scanning:

- **Healthy**: Database exists with symbols → Skips scan (unless `force_recreate: true`)
- **Stale**: Database exists but has 0 symbols → Recreates database
- **Missing**: Database doesn't exist → Creates new database

**Error Messages**:

```
# Invalid workspace root
"Invalid workspace root: Path '/nonexistent' does not exist"

# Auto-discovery failed
"Failed to discover workspace: Could not find project markers (Cargo.toml, package.json, etc.)"

# Filter path escapes workspace
"Path '../../etc' escapes workspace root, rejecting"
```

---

### magellan_watch - Continuous Indexing

**Description**: Starts or stops a background file watcher that keeps the Magellan database fresh as you edit code. Automatically indexes changed files.

**Prerequisites**: `magellan_init` must have been run at least once

**Parameters**:

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `action` | string | No | `"status"` | One of: `"start"`, `"stop"`, `"status"` |
| `workspace_root` | string | No | `.` | Path to project directory |
| `debounce_ms` | integer | No | `500` | Wait time after last change before re-indexing |

**Usage Examples**:

```javascript
// Start watching (auto-discovers workspace)
magellan_watch({ action: "start", workspace_root: "." })

// Check if watcher is running
magellan_watch({ action: "status" })

// Stop watching
magellan_watch({ action: "stop" })

// Custom debounce time (2 seconds)
magellan_watch({
  action: "start",
  workspace_root: ".",
  debounce_ms: 2000
})
```

**Return Values**:

```json
// Start action
{
  "status": "started",
  "workspace_root": "/path/to/project",
  "db_path": "/path/to/project/.codemcp/codegraph.db",
  "debounce_ms": 500
}

// Status action
{
  "running": true,
  "workspace_root": "/path/to/project",
  "db_path": "/path/to/project/.codemcp/codegraph.db"
}

// Stop action
{
  "status": "stopped"
}

// Already running
{
  "status": "already_running"
}
```

**Common Use Cases**:

1. **Active Development**: Keep watcher running during coding sessions for real-time updates
2. **Long-running Projects**: Start watcher once and let it run indefinitely
3. **CI/CD Integration**: Use `debounce_ms` to optimize for automated workflows

**Tips and Gotchas**:

- **Single Instance**: Only one watcher can run per workspace. Starting a second returns `"already_running"`
- **File Change Detection**: Uses native filesystem events (inotify on Linux, FSEvents on macOS)
- **Debouncing**: Waits `debounce_ms` milliseconds after the last change before re-indexing to avoid redundant scans
- **Excluded Paths**: Watcher respects `.codemcp/config.toml` exclude patterns (changes to excluded paths are ignored)
- **Language Detection**: Only re-indexes files with supported extensions (`.rs`, `.py`, `.js`, `.ts`, `.go`, etc.)
- **Database Locks**: Watcher opens database in read-write mode. Other tools can read concurrently
- **Process Lifetime**: Watcher runs in background thread. Stopping the MCP server stops the watcher

**Event Types**:

| Event | Action |
|-------|--------|
| `Create` | Index new file |
| `Modify` | Delete old data, re-index file |
| `Delete` | Remove all data for file from database |

**Configuration**:

Watcher behavior can be configured via `.codemcp/config.toml`:

```toml
[magellan]
auto_watch = true      # Start watcher automatically on server startup
debounce_ms = 500      # Default debounce time
semantic_watch = false # Auto-update semantic.db (requires ANTHROPIC_AUTH_TOKEN)
```

**Auto-start**:

If `CODEMCP_AUTO_WATCH=1` environment variable is set, the watcher starts automatically when the MCP server initializes.

---

### magellan_cache_stats - Performance Monitoring

**Description**: Returns cache performance statistics from the Magellan database. Useful for monitoring database efficiency and tuning performance.

**Prerequisites**: Database must exist (run `magellan_init` first)

**Parameters**: None

**Usage Examples**:

```javascript
// Get cache statistics
magellan_cache_stats()
```

**Return Values**:

```json
{
  "query_cache_hits": 1250,
  "query_cache_misses": 85,
  "query_cache_hit_rate": 0.9363,
  "page_cache_size": 40960,
  "page_cache_count": 10,
  "page_cache_hits": 4200,
  "page_cache_misses": 180,
  "page_cache_hit_rate": 0.9589
}
```

**Metrics Explained**:

| Metric | Description |
|--------|-------------|
| `query_cache_hits` | Number of cached query plan executions |
| `query_cache_misses` | Number of query plan compilations |
| `query_cache_hit_rate` | Ratio of hits to total queries (0.0 to 1.0) |
| `page_cache_size` | Total page cache size in bytes |
| `page_cache_count` | Number of pages currently cached |
| `page_cache_hits` | Number of cache page reads |
| `page_cache_misses` | Number of disk reads for pages |
| `page_cache_hit_rate` | Ratio of page hits to total reads (0.0 to 1.0) |

**Common Use Cases**:

1. **Performance Monitoring**: Track cache efficiency over time
2. **Capacity Planning**: Monitor page cache size for memory usage
3. **Troubleshooting**: Identify performance bottlenecks

**Tips and Gotchas**:

- **Hit Rate Targets**: Aim for >90% hit rate for both query and page cache
- **Cache Size**: Page cache size = `page_cache_count × 4096` bytes (default SQLite page size)
- **Auto-discovery**: Automatically finds workspace root and database path
- **Read-only Access**: Opens database in read-only mode (no locks)

**Low Hit Rate Causes**:

| Symptom | Cause | Solution |
|---------|-------|----------|
| Low query cache hit rate | Ad-hoc queries with different parameters | Use parameterized queries |
| Low page cache hit rate | Database larger than memory | Increase SQLite cache size |
| Consistently low hit rates | Database is too small | Not a problem, cache is warming up |

---

## 2. Query Tools

### find_symbols - Symbol Discovery

**Description**: Finds symbol locations (definition + references) using exact byte spans from the code graph. Supports multiple query formats for different use cases.

**Prerequisites**: `magellan_init` must have been run

**Parameters**:

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `query` | string | Yes | - | Search query (symbol name, references, or file pattern) |
| `show_code` | boolean | No | `false` | Include source code in results |
| `explain` | boolean | No | `false` | Show query parameter guide |

**Query Formats**:

1. **Symbol Name Search**: `"my_function"`
   - Finds definition + all references
   - Returns exact byte spans

2. **References Only**: `"references:symbol_name"`
   - Finds only reference locations (not definition)
   - Useful for seeing where a symbol is used

3. **File Pattern**: `"file:/absolute/path/to/lib.rs"`
   - Lists all symbols in a specific file
   - **Requires absolute path** (database stores absolute paths)
   - SQL LIKE wildcards (`%`, `_`) are escaped for security
   - Example: `"file:/home/user/project/src/lib.rs"` ✅ works
   - Example: `"file:src/lib.rs"` ❌ doesn't work (relative path)

4. **With Code**: Set `show_code: true`
   - Includes source code in results
   - Eliminates need to read files separately

**Usage Examples**:

```javascript
// Find symbol definition and all references
find_symbols({ query: "process_request" })

// Find only references (not definition)
find_symbols({ query: "references:main" })

// List all symbols in a file (requires absolute path)
find_symbols({ query: "file:/home/user/project/src/lib.rs" })

// Find symbol with source code included
find_symbols({
  query: "my_function",
  show_code: true
})

// Show query parameter guide
find_symbols({ query: "", explain: true })
```

**Return Values**:

```json
// Symbol name search
{
  "query": "process_request",
  "show_code": false,
  "count": 5,
  "results": [
    {
      "name": "process_request",
      "kind": "Function",
      "file_path": "src/lib.rs",
      "byte_start": 1234,
      "byte_end": 1456,
      "type": "definition"
    },
    {
      "file_path": "src/main.rs",
      "byte_start": 567,
      "byte_end": 583,
      "symbol_name": "process_request",
      "type": "reference"
    }
  ]
}

// With show_code: true
{
  "query": "my_function",
  "show_code": true,
  "count": 3,
  "results": [
    {
      "name": "my_function",
      "kind": "Function",
      "file_path": "src/lib.rs",
      "byte_start": 1234,
      "byte_end": 1456,
      "start_line": 42,
      "start_col": 0,
      "end_line": 45,
      "end_col": 1,
      "code": "fn my_function(input: &str) -> Result<String> {\n    ...\n}",
      "type": "definition"
    }
  ]
}
```

**Common Use Cases**:

1. **Pre-refactoring Discovery**: Find all locations before renaming
2. **Impact Analysis**: See which files use a symbol
3. **Code Navigation**: Jump to symbol definitions
4. **Token Efficiency**: Use `show_code: true` to avoid file reads

**Tips and Gotchas**:

- **Exact Matching**: Query is exact (no regex, no fuzzy matching). Symbol names must match exactly
- **Case Sensitivity**: Symbol names are case-sensitive
- **Byte Spans**: Returns exact byte offsets, not line numbers (use `show_code: true` for line/column info)
- **File Pattern Security**: LIKE wildcards (`%`, `_`) are escaped to prevent SQL injection
- **Empty Query**: Returns error if query is empty or whitespace-only
- **Multiple Definitions**: If multiple symbols have the same name, returns all of them
- **No Results**: Returns empty array if symbol not found (not an error)

**Query Explain Output**:

```text
The `query` parameter supports multiple formats for different use cases:

1. **Symbol Name Search**: "my_function"
   - Finds definition + all references for symbol named "my_function"
   - Returns exact byte spans from code graph

2. **References Only**: "references:symbol_name"
   - Finds only reference locations (not definition)
   - Useful for seeing where a symbol is used

3. **File Pattern**: "file:src/lib.rs"
   - Lists all symbols in a specific file
   - Use SQL LIKE pattern matching (% for wildcards)

4. **With Code**: Set show_code=true
   - Includes source code in results
   - Eliminates need to read files separately

EXAMPLES:
- find_symbols(query="process_request") → Definition + all uses
- find_symbols(query="references:main") → Only where main is called
- find_symbols(query="file:src/lib.rs") → All symbols in file
- find_symbols(query="my_function", show_code=true) → With source code

The query is EXACT - no regex, no fuzzy matching. Results are from the indexed code graph.
```

**Error Messages**:

```
# Empty query
"Invalid or missing query parameter: The 'query' parameter is required and must be non-empty"

# Database not available
"codegraph.db not available. Please run: magellan_init with db_path=.codemcp/codegraph.db"
```

---

## 3. Refactoring Tools

### refactor_rename - Safe Rename Operations

**Description**: Performs safe rename across all files using Magellan (exact byte spans) → Splice (AST-safe edits) → LSP (validation) workflow. All changes are atomic and validated before committing.

**Prerequisites**: `magellan_init` must have been run first

**Parameters**:

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `symbol_name` | string | Yes | - | Current symbol name (exact match required) |
| `new_name` | string | Yes | - | New symbol name (must differ from symbol_name) |
| `kind` | string | Yes | - | Symbol kind (`function`, `struct`, `enum`, etc.) |
| `workspace_root` | string | Yes | - | Path to project directory |
| `preview_only` | boolean | No | `false` | Preview changes without applying them |

**Usage Examples**:

```javascript
// Preview rename first (recommended!)
refactor_rename({
  symbol_name: "process_data",
  new_name: "transform_data",
  kind: "function",
  workspace_root: ".",
  preview_only: true
})

// Apply the rename
refactor_rename({
  symbol_name: "process_data",
  new_name: "transform_data",
  kind: "function",
  workspace_root: "."
})

// Rename a struct
refactor_rename({
  symbol_name: "MyStruct",
  new_name: "MyNewStruct",
  kind: "struct",
  workspace_root: "/path/to/project"
})
```

**Return Values**:

```json
// Preview mode
{
  "preview": true,
  "operation": "refactor_rename",
  "symbol_name": "process_data",
  "new_name": "transform_data",
  "kind": "function",
  "total_references": 5,
  "total_files": 3,
  "files": [
    {
      "file": "src/lib.rs",
      "reference_count": 2
    },
    {
      "file": "src/main.rs",
      "reference_count": 2
    },
    {
      "file": "src/tests.rs",
      "reference_count": 1
    }
  ]
}

// Applied successfully
{
  "operation": "verified_span_replace",
  "operation_id": "2025-01-05_123456abcdef",
  "symbol_name": "process_data",
  "new_name": "transform_data",
  "kind": "function",
  "spans_replaced": 5,
  "files_affected": 3,
  "files": ["src/lib.rs", "src/main.rs", "src/tests.rs"],
  "audit_trail": [
    {
      "file": "src/lib.rs",
      "before_hash": "abc123",
      "after_hash": "def456"
    }
  ],
  "database_backup": {
    "backup_id": "2025-01-05_123456",
    "backup_dir": "/path/to/project/.codemcp/backups/2025-01-05_123456",
    "timestamp": "2025-01-05T12:34:56Z"
  }
}
```

**Common Use Cases**:

1. **Function Renaming**: Rename functions across all call sites
2. **Struct Renaming**: Rename types and their constructors
3. **Variable Renaming**: Rename variables (use `kind: "variable"`)
4. **Constant Renaming**: Rename constants and their references

**Tips and Gotchas**:

- **Preview First**: Always use `preview_only: true` before applying changes
- **Exact Spans**: Uses byte-level precision from Magellan (not text search)
- **Atomic Changes**: All files succeed or all fail together
- **LSP Validation**: Compilation must pass or changes are rejected
- **Database Backup**: Automatic backup created before applying changes (stored in `.codemcp/backups/`)
- **Language Detection**: Automatically detects language and uses appropriate validation (Rust uses `cargo check`, Python uses `python -m py_compile`, etc.)
- **Symbol Kind**: Must match the actual symbol type (wrong kind = no results found)
- **Use Statements**: Automatically finds and updates `use` statements in Rust
- **Impl Blocks**: Handles Rust `impl` blocks correctly
- **Case Sensitivity**: Symbol names are case-sensitive

**Workflow**:

```
1. Find all references (definition + references + impl blocks + use statements)
2. Detect language from file extensions
3. If Rust:
   - Use Splice's rename with cargo check validation
4. If non-Rust:
   - Use symbol-search based rename with language-specific compiler
5. Create database backup
6. Apply changes atomically
7. Validate compilation
8. Rollback on failure
9. Record operation to audit trail
```

**Validation Gates**:

| Language | Validation Command |
|----------|-------------------|
| Rust | `cargo check` |
| Python | `python -m py_compile` |
| JavaScript | `node --check` |
| TypeScript | `tsc --noEmit` |
| Go | `go build` |

**Error Messages**:

```
# Symbol not found
"Symbol not found in code graph: No references found for symbol 'my_function'"

# Same name
"symbol_name and new_name must be different: Cannot rename 'foo' to 'foo' - they are the same"

# Validation failed
"Verified span replace failed: cargo check returned errors"

# LSP validation failed
"LSP validation failed after delete: error: unused variable: `old_name`"
```

**Rollback on Failure**:

If validation fails, all changes are automatically rolled back using the database backup. The backup ID is included in the error response for manual recovery if needed.

---

### refactor_delete - Safe Delete Operations

**Description**: Safely deletes a symbol and all its references using Splice delete with LSP validation. Automatically removes dead code and unused imports.

**Prerequisites**: `magellan_init` must have been run first

**Parameters**:

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `symbol_name` | string | Yes | - | Symbol name to delete |
| `file` | string | Yes | - | File containing the symbol definition |
| `kind` | string | No | `null` | Symbol kind (optional hint) |
| `workspace_root` | string | Yes | - | Path to project directory |

**Usage Examples**:

```javascript
// Delete a function
refactor_delete({
  symbol_name: "deprecated_function",
  file: "src/lib.rs",
  kind: "function",
  workspace_root: "."
})

// Delete a struct
refactor_delete({
  symbol_name: "OldStruct",
  file: "src/types.rs",
  workspace_root: "/path/to/project"
})
```

**Return Values**:

```json
{
  "operation": "delete",
  "symbol_name": "deprecated_function",
  "file": "src/lib.rs",
  "kind": "function",
  "references_found": 5,
  "splice_output": "Deleted 5 references to 'deprecated_function'",
  "database_backup": {
    "backup_id": "2025-01-05_123456",
    "backup_dir": "/path/to/project/.codemcp/backups/2025-01-05_123456",
    "timestamp": "2025-01-05T12:34:56Z"
  }
}
```

**Common Use Cases**:

1. **Dead Code Removal**: Delete unused functions and types
2. **Deprecation Cleanup**: Remove deprecated APIs
3. **Refactoring**: Delete intermediate abstractions

**Tips and Gotchas**:

- **Definition File**: Must specify the file containing the symbol definition (not just a reference)
- **Cascading Deletes**: Deletes all references, use statements, and dead code
- **LSP Validation**: Compilation must pass or changes are rejected
- **Database Backup**: Automatic backup created before deletion
- **Kind Hint**: Providing `kind` helps Splice find the symbol faster
- **Safe Deletes**: Uses Splice's AST-aware deletion (not text search)
- **Unused Imports**: Automatically removes unused imports after deletion

**Workflow**:

```
1. Find all references using Magellan
2. Create database backup
3. Execute Splice delete (AST-aware)
4. Validate with LSP (cargo check, etc.)
5. Rollback on failure
6. Report references removed
```

**Error Messages**:

```
# File not found
"Invalid or missing file parameter: The 'file' parameter must be a valid file path"

# Validation failed
"LSP validation failed after delete: error: use of undeclared crate"
```

---

### refactor_pattern - Pattern-Based Replacement

**Description**: Performs AST-aware search and replace across multiple files using tree-sitter patterns. More sophisticated than regex but less precise than symbol-based refactoring.

**Prerequisites**: `magellan_init` must have been run first

**Parameters**:

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `pattern` | string | Yes | - | Tree-sitter search pattern |
| `replacement` | string | Yes | - | Replacement pattern |
| `workspace_root` | string | Yes | - | Path to project directory |
| `file_pattern` | string | No | `**/*.rs` | Glob pattern for files to search |
| `preview_only` | boolean | No | `false` | Preview changes without applying |

**Usage Examples**:

```javascript
// Replace error handling pattern
refactor_pattern({
  pattern: "unwrap()",
  replacement: "expect(\"context\")",
  workspace_root: ".",
  file_pattern: "src/**/*.rs",
  preview_only: true
})

// Apply the replacement
refactor_pattern({
  pattern: "old_function_name(",
  replacement: "new_function_name(",
  workspace_root: ".",
  file_pattern: "**/*.py"
})
```

**Return Values**:

```json
{
  "matches_found": 15,
  "files_affected": 5,
  "preview": true,
  "matches": [
    {
      "file": "src/lib.rs",
      "line": 42,
      "column": 10,
      "context": "let x = value.unwrap()"
    }
  ]
}
```

**Common Use Cases**:

1. **API Migration**: Replace old API calls with new ones
2. **Error Handling**: Update error handling patterns
3. **Code Hygiene**: Replace unsafe patterns with safe alternatives

**Tips and Gotchas**:

- **Tree-sitter Patterns**: Uses tree-sitter query syntax (not regex)
- **AST-Aware**: Respects code structure (won't match strings/comments)
- **Less Precise**: Not as precise as `refactor_rename` (use with caution)
- **Preview First**: Always use `preview_only: true` before applying
- **File Pattern**: Glob syntax (`**/*.rs` for all Rust files)

**Error Messages**:

```
# Invalid pattern
"Failed to parse tree-sitter pattern: syntax error at position 5"

# No matches
"Pattern not found: No matches found for the given pattern"
```

---

## 4. Audit Tools

### query_executions - Operation History

**Description**: Lists all refactoring operations recorded in the operations database. Provides a complete audit trail of all changes made to the codebase.

**Prerequisites**: Operations database must exist (auto-created on first refactor)

**Parameters**:

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `limit` | integer | No | `100` | Maximum number of operations to return |
| `offset` | integer | No | `0` | Skip N operations (for pagination) |
| `operation_type` | string | No | `null` | Filter by operation type (e.g., `refactor_rename`) |

**Usage Examples**:

```javascript
// List all operations
query_executions()

// Get last 50 operations
query_executions({ limit: 50 })

// Get rename operations only
query_executions({
  operation_type: "refactor_rename",
  limit: 20
})

// Pagination
query_executions({
  limit: 50,
  offset: 50  // Skip first 50
})
```

**Return Values**:

```json
{
  "total_operations": 125,
  "limit": 100,
  "offset": 0,
  "operations": [
    {
      "id": "2025-01-05_123456abcdef",
      "operation_type": "refactor_rename",
      "timestamp": "2025-01-05T12:34:56Z",
      "arguments": {
        "symbol_name": "process_data",
        "new_name": "transform_data",
        "kind": "function",
        "workspace_root": "/path/to/project"
      },
      "result": {
        "success": true,
        "spans_replaced": 5,
        "files_affected": 3
      }
    },
    {
      "id": "2025-01-05_120000123456",
      "operation_type": "refactor_delete",
      "timestamp": "2025-01-05T12:00:00Z",
      "arguments": {
        "symbol_name": "deprecated_function",
        "file": "src/lib.rs"
      },
      "result": {
        "success": true,
        "references_removed": 5
      }
    }
  ]
}
```

**Common Use Cases**:

1. **Change Tracking**: See what refactoring operations were performed
2. **Debugging**: Investigate recent changes
3. **Rollback**: Find operation IDs for manual rollback
4. **Compliance**: Audit trail for code changes

**Tips and Gotchas**:

- **Auto-Recording**: All refactoring operations are automatically recorded
- **Persistent Storage**: Operations stored in `.codemcp/operations.db`
- **Pagination**: Use `offset` for large datasets
- **Filtering**: Filter by `operation_type` to find specific operations
- **Timestamp**: ISO 8601 format (`2025-01-05T12:34:56Z`)
- **Operation ID**: Unique ID for each operation (used in `get_execution`)

**Operation Types**:

| Type | Description |
|------|-------------|
| `refactor_rename` | Symbol rename operation |
| `refactor_delete` | Symbol delete operation |
| `refactor_pattern` | Pattern-based replacement |
| `verified_span_replace` | Splice-based edit with validation |

---

### get_execution - Operation Details

**Description**: Retrieves detailed information about a specific refactoring operation, including all changes made and validation results.

**Prerequisites**: Operation ID must exist (from `query_executions`)

**Parameters**:

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `operation_id` | string | Yes | - | Unique operation ID |

**Usage Examples**:

```javascript
// Get operation details
get_execution({
  operation_id: "2025-01-05_123456abcdef"
})
```

**Return Values**:

```json
{
  "operation": {
    "id": "2025-01-05_123456abcdef",
    "operation_type": "refactor_rename",
    "timestamp": "2025-01-05T12:34:56Z",
    "arguments": {
      "symbol_name": "process_data",
      "new_name": "transform_data",
      "kind": "function",
      "workspace_root": "/path/to/project"
    },
    "result": {
      "success": true,
      "spans_replaced": 5,
      "files_affected": 3,
      "files": ["src/lib.rs", "src/main.rs", "src/tests.rs"],
      "audit_trail": [
        {
          "file": "src/lib.rs",
          "before_hash": "abc123",
          "after_hash": "def456"
        },
        {
          "file": "src/main.rs",
          "before_hash": "789ghi",
          "after_hash": "jkl012"
        },
        {
          "file": "src/tests.rs",
          "before_hash": "mno345",
          "after_hash": "pqr678"
        }
      ],
      "database_backup": {
        "backup_id": "2025-01-05_123456",
        "backup_dir": "/path/to/project/.codemcp/backups/2025-01-05_123456",
        "timestamp": "2025-01-05T12:34:56Z"
      }
    },
    "span_replacements": [
      {
        "id": 1,
        "operation_id": "2025-01-05_123456abcdef",
        "symbol_name": "process_data",
        "file_path": "src/lib.rs",
        "byte_start": 1234,
        "byte_end": 1246,
        "before_hash": "abc123",
        "after_hash": "def456"
      }
    ]
  }
}
```

**Common Use Cases**:

1. **Change Inspection**: See exactly what changed in an operation
2. **Verification**: Confirm operation success and validation results
3. **Rollback**: Get backup information for manual recovery
4. **Debugging**: Investigate failed operations

**Tips and Gotchas**:

- **Operation ID**: Use `query_executions` to find operation IDs
- **Hash Verification**: `before_hash` and `after_hash` allow verification of file changes
- **Backup Location**: `database_backup.backup_dir` contains pre-operation database state
- **Span Replacements**: Detailed list of all byte span replacements
- **Timestamp**: ISO 8601 format

**Error Messages**:

```
# Operation not found
"Operation not found: No operation with ID 'invalid-id' exists in the database"
```

---

## Part 2 Summary

This concludes **Part 2** of the CodeMCP manual. You should now be able to:

- Initialize and maintain the code graph database with `magellan_init` and `magellan_watch`
- Find symbol locations with `find_symbols`
- Perform safe refactoring with `refactor_rename` and `refactor_delete`
- Query operation history with `query_executions` and `get_execution`

**Key Takeaways**:

1. **Always run `magellan_init` first** before using any refactoring tools
2. **Use `preview_only: true`** before applying refactoring changes
3. **Enable `magellan_watch`** during active development for real-time updates
4. **Check cache stats** to monitor database performance
5. **Query operation history** for audit trails and rollback information

**Part 3** will cover:
- Semantic discovery tools (`discover_summary`, `discover_by_purpose`)
- Symbol metadata tools (`label_symbols`, `add_symbol_property`)
- Documentation tools (`docs_status`, `docs_validate`)
- Memory tools (`memory_similar`, `memory_record_operation`)

For more information, see:
- [manual.md](manual.md) - Part 1: Overview and Quick Start
- [CLAUDE.md](CLAUDE.md) - Usage guide for Claude Code
- [docs/](docs/) - Additional documentation

---

**Next Steps**:

1. Initialize your workspace: `magellan_init({ workspace_root: "." })`
2. Start the watcher: `magellan_watch({ action: "start", workspace_root: "." })`
3. Practice safe refactoring: Use `preview_only: true` before applying changes
4. Monitor performance: Check `magellan_cache_stats()` periodically
5. Review operations: Use `query_executions()` to see what changed

**Support**:

For issues or questions:
- Check error messages carefully (most include context and suggestions)
- Run `magellan_cache_stats()` to check database health
- Review operation history for recent changes
- See [docs/](docs/) for additional documentation
# CodeMCP User Manual - Part 3: Advanced Tools

**Version:** 0.1.0
**Last Updated:** 2026-01-05
**Status:** Comprehensive Guide

---

## Table of Contents

1. [Semantic Discovery Tools](#1-semantic-discovery-tools)
2. [Symbol Metadata Tools](#2-symbol-metadata-tools)
3. [Memory Tools](#3-memory-tools)
4. [Impact Analysis](#4-impact-analysis)
5. [Documentation Tools](#5-documentation-tools)
6. [Database Management](#6-database-management)

---

## 1. Semantic Discovery Tools

Semantic discovery tools provide token-efficient code understanding by querying `semantic.db` instead of reading source files. These tools use **lazy indexing** - symbols are only indexed when you actually query them.

### 1.1 semantic_stats

**Description:**
Get statistics about the semantic database, including symbol count, file count, and available tools.

**Parameters:**
None

**Usage Examples:**

```javascript
// Check semantic database status
semantic_stats()
```

**Return Values:**

```json
{
  "symbol_count": 850,
  "file_count": 142,
  "status": "ready",
  "available_tools": [
    "discover_summary - Get summary for a symbol",
    "discover_by_purpose - Find symbols by purpose tag",
    "discover_module - List all symbols in a file",
    "semantic_stats - Show database statistics"
  ]
}
```

**Common Use Cases:**
- Check if semantic indexing has been run
- Verify database health before querying
- Understand scope of indexed codebase

**Prerequisites:**
- `semantic.db` must exist (created automatically on first use)
- No LLM required (fast, metadata-only query)

**Token Cost:** ~50 (very cheap)

---

### 1.2 discover_summary

**Description:**
Get semantic summary for a specific symbol, including purpose tags, complexity analysis, and called functions.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `symbol` | string | Yes | Symbol name to query |
| `file_path` | string | No | Disambiguate if symbol exists in multiple files |
| `auto_index` | boolean | No | Automatically index if not found (default: from `CODEMCP_AUTO_INDEX` env var) |

**Usage Examples:**

```javascript
// Get summary with auto-indexing enabled
discover_summary({
  symbol: "authenticate_user",
  auto_index: true
})

// Get summary for specific file
discover_summary({
  symbol: "process",
  file_path: "src/api.rs"
})

// Query without auto-indexing
discover_summary({
  symbol: "MyStruct",
  auto_index: false
})
```

**Return Values:**

```json
{
  "name": "authenticate_user",
  "kind": "function",
  "file_path": "src/auth.rs",
  "summary": "Validates JWT tokens and returns user session",
  "purposes": ["authentication", "validation"],
  "complexity": "medium",
  "callees": ["validate_token", "get_user_session"],
  "cached": true,
  "suggested_next": [
    "Use get_code_chunks(file_path='src/auth.rs') when ready to edit"
  ]
}
```

**Common Use Cases:**
- Understand what a function does without reading the file
- Check complexity before refactoring
- Find related functions through callees

**Token Cost:**
- Cached: ~50 (very cheap!)
- Not cached with `auto_index=true`: ~500 (one-time LLM call)
- Not cached with `auto_index=false`: Error with hint

**Lazy Indexing Behavior:**
With `auto_index=true`, missing symbols are automatically indexed on-demand. First query is slower but subsequent queries are instant.

**Error Cases:**

```json
// Symbol not found and auto_index disabled
{
  "error": "Symbol 'my_func' not found in semantic index",
  "hint": "Set auto_index=true to index on-demand, or use index_symbol_demand tool"
}

// Symbol not in codegraph.db
{
  "error": "Symbol 'my_func' not found in codegraph.db",
  "hint": "Run: magellan_init(workspace_root='.')"
}
```

---

### 1.3 discover_by_purpose

**Description:**
Find all symbols serving a specific purpose (authentication, validation, parsing, etc.) without reading source files.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `purpose` | string | Yes | Purpose category to filter by |
| `limit` | integer | No | Maximum results (default: 50) |

**Valid Purpose Tags:**
- `authentication` - Auth, login, session management
- `validation` - Input validation, verification
- `parsing` - Parsing, deserialization
- `formatting` - Serialization, display formatting
- `storage` - Database, file writes
- `retrieval` - Database queries, file reads
- `logging` - Logging, debugging output
- `network` - HTTP, API calls
- `other` - Everything else

**Usage Examples:**

```javascript
// Find all authentication functions
discover_by_purpose({
  purpose: "authentication",
  limit: 20
})

// Find validation logic
discover_by_purpose({
  purpose: "validation"
})

// Find network-related code
discover_by_purpose({
  purpose: "network",
  limit: 100
})
```

**Return Values:**

```json
{
  "purpose": "authentication",
  "count": 15,
  "symbols": [
    {
      "name": "authenticate_user",
      "kind": "function",
      "file_path": "src/auth.rs",
      "summary": "Validates JWT tokens and returns user session",
      "complexity": "medium"
    },
    {
      "name": "login",
      "kind": "function",
      "file_path": "src/auth.rs",
      "summary": "Handles user login with credential verification",
      "complexity": "high"
    }
  ]
}
```

**Common Use Cases:**
- Find all functions for a specific domain (e.g., all auth logic)
- Understand codebase by functional categories
- Identify related functions for refactoring

**Token Cost:** ~200 (very efficient compared to grep + file reading)

**Error Cases:**

```json
// Invalid purpose tag
{
  "error": "Unknown purpose: 'auth'",
  "valid_purposes": [
    "authentication", "validation", "parsing", "formatting",
    "storage", "retrieval", "logging", "network", "other"
  ]
}
```

---

### 1.4 discover_module

**Description:**
List all symbols in a file with their semantic summaries, without reading the source file.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file_path` | string | Yes | Path to source file |

**Usage Examples:**

```javascript
// Get all symbols in a file
discover_module({
  file_path: "src/lib.rs"
})

// Get symbols for specific module
discover_module({
  file_path: "src/api/handler.rs"
})
```

**Return Values:**

```json
{
  "file_path": "src/lib.rs",
  "symbol_count": 8,
  "symbols": [
    {
      "name": "process_request",
      "kind": "function",
      "summary": "Main request handler with validation and routing",
      "complexity": "high",
      "purposes": ["network", "validation"]
    },
    {
      "name": "Config",
      "kind": "struct",
      "summary": "Configuration structure for application settings",
      "complexity": "low",
      "purposes": ["storage"]
    }
  ]
}
```

**Common Use Cases:**
- Understand file structure before editing
- Get overview of all functions in a module
- Quick reconnaissance without file I/O

**Token Cost:** ~300 (much cheaper than reading entire file)

**Error Cases:**

```json
// No symbols found
{
  "file_path": "src/empty.rs",
  "symbols": [],
  "note": "No symbols found. Ensure semantic_index() has been run."
}
```

---

### 1.5 index_symbol_demand

**Description:**
Explicitly index a single symbol on-demand using LLM. Use this when you need fine-grained control over indexing.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `symbol` | string | Yes | Symbol name to index |
| `file_path` | string | No | Disambiguate if symbol exists in multiple files |

**Usage Examples:**

```javascript
// Index a specific symbol
index_symbol_demand({
  symbol: "my_function"
})

// Index with file disambiguation
index_symbol_demand({
  symbol: "process",
  file_path: "src/api.rs"
})
```

**Return Values:**

```json
{
  "success": true,
  "message": "Symbol indexed successfully",
  "name": "my_function",
  "kind": "function",
  "file_path": "src/lib.rs",
  "summary": "Processes user input with validation",
  "purposes": ["validation", "parsing"],
  "complexity": "medium",
  "callees": ["validate_input", "parse_data"],
  "note": "Cached in semantic.db for future queries"
}
```

**Common Use Cases:**
- Explicitly index symbols before refactoring
- Build semantic index for specific domain
- Force re-indexing after code changes

**Token Cost:** ~500 (one-time LLM call)

**Environment Variables Required:**
- `ANTHROPIC_AUTH_TOKEN` - API key for LLM calls

**Error Cases:**

```json
// Symbol not found in codegraph.db
{
  "error": "Symbol 'my_func' not found in codegraph.db",
  "hint": "Run magellan_init first to build the code graph database"
}

// Multiple symbols with same name
{
  "disambiguation_required": "Found 3 symbols named 'process'. Please specify file_path.",
  "options": [
    {"symbol": "process", "kind": "function", "file_path": "src/api.rs"},
    {"symbol": "process", "kind": "function", "file_path": "src/handler.rs"},
    {"symbol": "process", "kind": "function", "file_path": "src/utils.rs"}
  ],
  "example": "index_symbol_demand(symbol='process', file_path='src/api.rs')"
}
```

---

### 1.6 index_by_purpose_demand

**Description:**
Bulk indexing for specific purposes. Uses heuristic keyword matching to find candidates, then indexes with LLM to assign proper purpose tags.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `purpose` | string | Yes | Purpose category to search for |
| `limit` | integer | No | Maximum symbols to index (default: 10) |

**Usage Examples:**

```javascript
// Index authentication functions
index_by_purpose_demand({
  purpose: "authentication",
  limit: 20
})

// Index validation logic
index_by_purpose_demand({
  purpose: "validation",
  limit: 50
})
```

**Return Values:**

```json
{
  "purpose": "authentication",
  "candidates_found": 45,
  "indexed": 20,
  "skipped": 15,
  "failed": 0,
  "matched": [
    {
      "symbol": "authenticate_user",
      "file": "src/auth.rs",
      "summary": "Validates JWT tokens and returns user session",
      "complexity": "medium"
    },
    {
      "symbol": "login",
      "file": "src/auth.rs",
      "summary": "Handles user login with credential verification",
      "complexity": "high"
    }
  ],
  "note": "Only symbols that actually match the purpose are shown in 'matched'"
}
```

**Common Use Cases:**
- Build semantic index for specific domain
- Discover and index all functions of a type
- Populate semantic.db efficiently

**Token Cost:** ~500 per symbol indexed (LLM calls)

**Behavior:**
1. Uses keyword heuristics to find candidates
2. Indexes candidates with LLM
3. Filters by actual purpose tags
4. Returns only symbols that match the purpose

---

## 2. Symbol Metadata Tools

Symbol metadata tools allow you to tag and query symbols with labels (categories) and properties (key-value pairs).

### 2.1 label_symbols

**Description:**
Discover symbols by label/kind with optional code display (Magellan 0.5.0 feature). Eliminates file reading during code discovery.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `labels` | array | No | Array of kind filters (e.g., ["Function", "Struct"]) |
| `show_code` | boolean | No | Include source code in results (default: false) |
| `limit` | integer | No | Maximum results (default: 100) |
| `list_labels` | boolean | No | List all available labels with counts |

**Usage Examples:**

```javascript
// List all available labels
label_symbols({
  list_labels: true
})

// Find all functions
label_symbols({
  labels: ["Function"],
  show_code: true,
  limit: 50
})

// Find structs and classes
label_symbols({
  labels: ["Struct", "Class"],
  limit: 100
})
```

**Return Values:**

```json
// list_labels=true
{
  "labels": [
    {"label": "Function", "count": 542},
    {"label": "Struct", "count": 128},
    {"label": "Method", "count": 315},
    {"label": "Class", "count": 45}
  ],
  "total_kinds": 4
}

// Query with labels
{
  "labels": ["Function"],
  "show_code": true,
  "limit": 50,
  "results": [
    {
      "name": "process_request",
      "kind": "Function",
      "file_path": "src/api.rs",
      "byte_start": 1234,
      "byte_end": 1456,
      "start_line": 42,
      "start_col": 0,
      "end_line": 58,
      "end_col": 1,
      "code": "fn process_request(input: &str) -> Result<String> { ... }"
    }
  ],
  "count": 50
}
```

**Common Use Cases:**
- Discover all functions in codebase
- Find all structs/classes
- Read code without file I/O (with `show_code=true`)

**Token Efficiency:**
- Old: Read file (~10K tokens)
- New: Query database (~100 tokens)
- Result: 100x more efficient

---

### 2.2 get_code_chunks

**Description:**
Get source code for symbols or files directly from the database (Magellan 0.5.0 feature). No file I/O needed.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file_path` | string | Yes | Path to source file |
| `symbol_name` | string | No | Get code for specific symbol only |

**Usage Examples:**

```javascript
// Get all code chunks from a file
get_code_chunks({
  file_path: "src/lib.rs"
})

// Get code for specific function
get_code_chunks({
  file_path: "src/lib.rs",
  symbol_name: "process_request"
})
```

**Return Values:**

```json
{
  "file_path": "src/lib.rs",
  "symbol_name": "process_request",
  "chunks": [
    {
      "content": "fn process_request(input: &str) -> Result<String> { ... }",
      "file_path": "src/lib.rs",
      "byte_start": 1234,
      "byte_end": 1456,
      "symbol_name": "process_request",
      "symbol_kind": "function"
    }
  ],
  "count": 1
}
```

**Common Use Cases:**
- Read code before refactoring
- See function implementation without file I/O
- Quick code inspection during discovery

**Prerequisites:**
- `magellan_init` must be run first
- Magellan 0.5.0+ with code chunk storage

---

### 2.3 add_symbol_label

**Description:**
Add a label tag to a symbol in the codegraph for categorization and filtering.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `symbol_name` | string | Yes | Name of the symbol to label |
| `label` | string | Yes | Label tag to add |

**Standard Labels:**
- `deprecated` - Mark deprecated symbols
- `unsafe` - Mark symbols containing unsafe code
- `async` - Mark async functions
- `public_api` - Mark public API items

**Usage Examples:**

```javascript
// Mark function as deprecated
add_symbol_label({
  symbol_name: "old_func",
  label: "deprecated"
})

// Mark unsafe code
add_symbol_label({
  symbol_name: "unsafe_operation",
  label: "unsafe"
})

// Mark public API
add_symbol_label({
  symbol_name: "api_handler",
  label: "public_api"
})
```

**Return Values:**

```json
{
  "symbol_name": "old_func",
  "label": "deprecated",
  "status": "added"
}
```

**Common Use Cases:**
- Mark deprecated functions before refactoring
- Tag unsafe code for security review
- Identify public API for documentation

**Validation Rules:**
- `symbol_name` must be non-empty
- `label` must be non-empty
- Symbol must exist in codegraph.db

---

### 2.4 get_symbols_by_label

**Description:**
Query all symbols that have been tagged with a specific label.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `label` | string | Yes | Label tag to filter by |

**Usage Examples:**

```javascript
// Find all deprecated symbols
get_symbols_by_label({
  label: "deprecated"
})

// Find all unsafe code
get_symbols_by_label({
  label: "unsafe"
})

// Find public API
get_symbols_by_label({
  label: "public_api"
})
```

**Return Values:**

```json
{
  "label": "deprecated",
  "count": 15,
  "symbols": [
    {
      "symbol_id": 123,
      "name": "old_func",
      "kind": "function",
      "file_path": "src/lib.rs",
      "byte_start": 1234,
      "byte_end": 1456
    },
    {
      "symbol_id": 456,
      "name": "deprecated_api",
      "kind": "function",
      "file_path": "src/api.rs",
      "byte_start": 5678,
      "byte_end": 5890
    }
  ]
}
```

**Common Use Cases:**
- Find all deprecated code for removal
- Identify unsafe code for security audit
- List public API for documentation generation

---

### 2.5 add_symbol_property

**Description:**
Add a key-value property to a symbol for storing metadata (complexity, test coverage, etc.).

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `symbol_name` | string | Yes | Name of the symbol |
| `key` | string | Yes | Property name |
| `value` | string | Yes | Property value |

**Common Property Keys:**
- `complexity` - Cyclomatic complexity (e.g., "15")
- `test_coverage` - Test coverage percentage (e.g., "85%")
- `deprecation_reason` - Why symbol is deprecated (e.g., "Use new_func instead")
- `perf_score` - Performance rating (e.g., "A")

**Usage Examples:**

```javascript
// Add complexity metric
add_symbol_property({
  symbol_name: "complex_func",
  key: "complexity",
  value: "15"
})

// Add test coverage
add_symbol_property({
  symbol_name: "api_handler",
  key: "test_coverage",
  value: "85%"
})

// Add deprecation reason
add_symbol_property({
  symbol_name: "old_func",
  key: "deprecation_reason",
  value: "Use new_func instead - more efficient"
})
```

**Return Values:**

```json
{
  "symbol_name": "complex_func",
  "key": "complexity",
  "value": "15",
  "status": "added"
}
```

**Common Use Cases:**
- Store complexity analysis results
- Track test coverage metrics
- Document deprecation reasons

---

### 2.6 get_symbols_by_property

**Description:**
Query symbols that have a specific property key-value pair.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `key` | string | Yes | Property name to filter by |
| `value` | string | Yes | Property value to match |

**Usage Examples:**

```javascript
// Find high-complexity functions
get_symbols_by_property({
  key: "complexity",
  value: "15"
})

// Find well-tested code
get_symbols_by_property({
  key: "test_coverage",
  value: "85%"
})

// Find deprecated functions
get_symbols_by_property({
  key: "deprecation_reason",
  value: "Use new_func instead"
})
```

**Return Values:**

```json
{
  "key": "complexity",
  "value": "15",
  "count": 8,
  "symbols": [
    {
      "symbol_id": 234,
      "name": "complex_func",
      "kind": "function",
      "file_path": "src/lib.rs",
      "byte_start": 3456,
      "byte_end": 3789
    }
  ]
}
```

**Common Use Cases:**
- Find high-complexity code for refactoring
- Identify low-test-coverage code
- Query by custom metadata

---

### 2.7 get_symbol_metadata

**Description:**
Get all labels and properties for a specific symbol.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `symbol_name` | string | Yes | Name of the symbol |

**Usage Examples:**

```javascript
// Get all metadata for a symbol
get_symbol_metadata({
  symbol_name: "my_function"
})
```

**Return Values:**

```json
{
  "symbol_name": "my_function",
  "labels": ["deprecated", "unsafe"],
  "properties": {
    "complexity": "15",
    "test_coverage": "45%",
    "deprecation_reason": "Use new_function instead"
  },
  "label_count": 2,
  "property_count": 3
}
```

**Common Use Cases:**
- Check symbol metadata before refactoring
- Review deprecation status
- Understand symbol annotations

---

## 3. Memory Tools

Memory tools provide semantic search for refactor operations using local embeddings and HNSW vector index.

### 3.1 memory_index_status

**Description:**
Get status and statistics of the HNSW memory index used for semantic search.

**Parameters:**
None

**Usage Examples:**

```javascript
// Check memory index status
memory_index_status()
```

**Return Values:**

```json
{
  "index_name": "refactor_memory",
  "status": "initialized",
  "embedding_dimension": 384,
  "distance_metric": "cosine",
  "message": "HNSW memory index is active"
}
```

**Common Use Cases:**
- Verify memory system is initialized
- Check embedding configuration
- Debug search issues

**Technical Details:**
- Uses local BGE-small-en-v1.5 embeddings (384 dimensions)
- HNSW vector index for O(log N) similarity search
- In-memory only (persists only while codemcp process runs)
- No external API calls for embeddings

---

### 3.2 memory_similar

**Description:**
Search for refactor operations similar to a query using semantic embeddings.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | Search query for similar operations |
| `limit` | integer | No | Maximum results (default: 5) |

**Usage Examples:**

```javascript
// Find similar rename operations
memory_similar({
  query: "rename function to new name",
  limit: 5
})

// Find similar deletions
memory_similar({
  query: "delete unused function"
})

// Find extraction operations
memory_similar({
  query: "extract method",
  limit: 10
})
```

**Return Values:**

```json
{
  "query": "rename function to new name",
  "results": [
    {
      "vector_id": 1,
      "distance": 0.23,
      "similarity": 0.77
    },
    {
      "vector_id": 5,
      "distance": 0.31,
      "similarity": 0.69
    }
  ],
  "count": 2
}
```

**Common Use Cases:**
- Learn from previous refactor patterns
- Check if similar changes have been made before
- Understand refactor history

**Similarity Scoring:**
- Distance: 0 = identical, 2 = opposite (cosine distance)
- Similarity: 1.0 = identical, 0.0 = opposite (derived from distance)

**Token Cost:** ~50 (very cheap, local embeddings)

---

### 3.3 memory_record_operation

**Description:**
Record a refactor operation to memory with embedding for future similarity search.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `operation_type` | string | Yes | Type of operation (e.g., "rename", "delete", "extract") |
| `symbol_name` | string | Yes | Name of the symbol being operated on |
| `symbol_kind` | string | No | Kind of symbol (function, struct, etc.) |
| `new_name` | string | No | New name for renames |
| `files_modified` | array | No | List of files affected |
| `success` | boolean | No | Whether operation succeeded (default: true) |
| `metadata` | object | No | Additional JSON metadata |

**Usage Examples:**

```javascript
// Record a rename operation
memory_record_operation({
  operation_type: "rename",
  symbol_name: "old_func",
  symbol_kind: "function",
  new_name: "new_func",
  success: true
})

// Record a delete operation
memory_record_operation({
  operation_type: "delete",
  symbol_name: "unused_func",
  symbol_kind: "function"
})

// Record with custom metadata
memory_record_operation({
  operation_type: "extract",
  symbol_name: "new_method",
  symbol_kind: "method",
  files_modified: ["src/lib.rs", "src/api.rs"],
  metadata: {
    "extraction_reason": "reduce complexity",
    "parent_function": "process_request"
  }
})
```

**Return Values:**

```json
{
  "operation_id": 1704451200,
  "vector_id": 42,
  "operation_type": "rename",
  "symbol_name": "old_func",
  "success": true,
  "message": "Operation recorded to memory"
}
```

**Common Use Cases:**
- Build semantic history of changes
- Enable future similarity searches
- Track refactor patterns over time

**Automatic Recording:**
Most refactor tools (refactor_rename, refactor_delete) automatically call this tool. Manual recording is optional.

---

## 4. Impact Analysis

### 4.1 get_impact_analysis

**Description:**
Perform k-hop graph traversal to find all symbols impacted by changing a given symbol. Essential for understanding refactoring scope.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `symbol_name` | string | Yes | Name of the symbol to analyze |
| `depth` | integer | No | Maximum hops to traverse (default: 2, max: 10) |

**Usage Examples:**

```javascript
// Analyze impact with default depth (2)
get_impact_analysis({
  symbol_name: "process_request"
})

// Deep impact analysis
get_impact_analysis({
  symbol_name: "core_utility",
  depth: 5
})

// Direct references only
get_impact_analysis({
  symbol_name: "MyStruct",
  depth: 1
})
```

**Return Values:**

```json
{
  "symbol_name": "process_request",
  "depth": 2,
  "total_impacted": 47,
  "impacted_symbols": [
    {
      "symbol_id": 123,
      "name": "handle_http",
      "kind": "function",
      "file_path": "src/api.rs",
      "hop_distance": 1,
      "edge_type": "CALLS"
    },
    {
      "symbol_id": 456,
      "name": "serve_client",
      "kind": "function",
      "file_path": "src/server.rs",
      "hop_distance": 2,
      "edge_type": "CALLS"
    }
  ]
}
```

**Hop Distance Explanation:**
- **Hop 1**: Direct references (symbols that directly reference the target)
- **Hop 2**: Indirect references (symbols that reference hop 1 symbols)
- **Hop N**: N levels of indirection from the target

**Common Use Cases:**
- Before refactoring to understand impact scope
- Risk assessment for proposed changes
- Understanding code coupling
- Identifying downstream dependencies

**Depth Selection Guide:**
- `depth=1`: Direct references only (fast, ~100 tokens)
- `depth=2`: Direct + indirect (recommended, ~200 tokens)
- `depth=3-5`: Deep analysis (larger codebases, ~500+ tokens)
- `depth>5`: Usually too broad, consider refining query

**Traversal Direction:**
Follows **incoming** edges (who references this symbol). Shows impact:
- If function A calls function B, changing B impacts A
- If struct X uses trait Y, changing Y impacts X
- If module M imports function F, changing F impacts M

**Prerequisites:**
- `magellan_init` must be run first
- `codegraph.db` must exist at `.codemcp/codegraph.db`

---

## 5. Documentation Tools

Documentation tools provide intelligent documentation management with semantic search, link validation, and governance workflow.

### 5.1 docs_status

**Description:**
Get health status of all documentation in workspace, including staleness tracking and staging information.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `category` | string | No | Filter by documentation category |

**Valid Categories:**
- `api-reference` - API and technical reference
- `architecture` - System architecture and design
- `implementation` - Implementation plans and progress
- `user-guide` - User manuals and tutorials
- `changelog` - Change history
- `reports` - Investigation and analysis reports

**Usage Examples:**

```javascript
// Check all documentation status
docs_status()

// Check specific category
docs_status({
  category: "user-guide"
})
```

**Return Values:**

```json
{
  "healthy": 45,
  "stale": [
    "docs/api.md",
    "README.md"
  ],
  "staged": [
    "docs/tutorial.md"
  ],
  "missing": [],
  "total": 48
}
```

**Health States:**
- **healthy**: Up-to-date in main index
- **stale**: File changed since last indexing
- **staged**: Pending changes in staging area (awaiting promotion)
- **missing**: File not found or not indexed

**Common Use Cases:**
- Check documentation freshness before publishing
- Identify stale docs that need re-indexing
- Verify documentation coverage

**File Types Scanned:**
- Markdown files (`.md`, `.markdown`)
- Text files (`.txt`)
- reStructuredText files (`.rst`)

**Directories Skipped:**
- `.codemcp/` - Internal CodeMCP directory
- `target/` - Rust build artifacts
- `node_modules/` - Node.js dependencies
- All hidden directories (starting with `.`)

---

### 5.2 docs_changes

**Description:**
Get recent documentation changes from the operations history.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `hours` | integer | No | Time window in hours (default: 24) |

**Usage Examples:**

```javascript
// Get changes in last 24 hours
docs_changes()

// Get changes in last week
docs_changes({
  hours: 168
})

// Get very recent changes
docs_changes({
  hours: 1
})
```

**Return Values:**

```json
{
  "changes": [
    {
      "timestamp": 1704451200,
      "file_path": "docs/api.md",
      "operation": "index",
      "details": "Indexed 5 chunks"
    },
    {
      "timestamp": 1704454800,
      "file_path": "docs/tutorial.md",
      "operation": "validate",
      "details": "Validation passed"
    }
  ],
  "count": 2,
  "hours": 24
}
```

**Common Use Cases:**
- Track documentation activity over time
- Review recent changes
- Audit documentation modifications

**Fallback Behavior:**
If `docs_changes.db` doesn't exist, returns empty array (not an error).

---

### 5.3 docs_index

**Description:**
Generate embeddings for pending documentation chunks. Enables semantic search functionality.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `validate` | boolean | No | Cross-check code references against codegraph (default: false) |
| `reindex` | boolean | No | Force reindexing even if already indexed (default: false) |

**Usage Examples:**

```javascript
// Index pending chunks
docs_index()

// Index with validation
docs_index({
  validate: true
})

// Force reindex all
docs_index({
  reindex: true
})
```

**Return Values:**

```json
{
  "indexed_chunks": 15,
  "message": "Indexed 15 documentation chunks",
  "validation_errors": [],
  "validated": true
}
```

**Common Use Cases:**
- After adding or editing documentation
- Before performing semantic search on docs
- When `docs_status` shows stale files

**Validation with `validate=true`:**
- Checks all code references against codegraph
- Returns list of invalid symbol references
- Use to ensure documentation code references are accurate

**Embedding Details:**
- Expensive LLM API calls (one per chunk)
- BGE-small-en-v1.5 embeddings (384 dimensions)
- Stored in `.codemcp/raggraph.db`
- Enables semantic search functionality

**Environment Variables Required:**
- `ANTHROPIC_AUTH_TOKEN` - API key for embedding generation

**Queuing Behavior:**
Documentation chunks are automatically queued by the docs watcher. This tool processes the queue.

---

### 5.4 docs_validate

**Description:**
Validate documentation links and code references. Enhanced with staging support to gate promotion.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `staged` | boolean | No | Validate staged docs instead of main index (default: false) |

**Usage Examples:**

```javascript
// Validate main index
docs_validate()

// Validate staged docs before promotion
docs_validate({
  staged: true
})
```

**Return Values:**

```json
{
  "valid": true,
  "staged": false,
  "invalid_links": [],
  "invalid_code_refs": [],
  "invalid_links_count": 0,
  "invalid_code_refs_count": 0
}
```

**Error Return Values:**

```json
{
  "valid": false,
  "staged": true,
  "invalid_links": [
    {
      "link_id": 42,
      "from_doc": "docs/tutorial.md",
      "to_doc": null,
      "to_heading": "Nonexistent Section",
      "link_text": "See Nonexistent Section"
    }
  ],
  "invalid_code_refs": [
    {
      "symbol_name": "NonexistentFunction",
      "from_chunk": "chunk_123",
      "from_doc": "docs/api.md"
    }
  ],
  "invalid_links_count": 1,
  "invalid_code_refs_count": 1
}
```

**Common Use Cases:**
- Check documentation quality before promotion
- Find broken internal links
- Detect invalid code references
- Validate staged docs before promoting to main index

**Validation Checks:**
1. **Internal Links**: All `[text](#anchor)` and `[text](file.md#anchor)` resolve
2. **Code References**: All `symbol_name` references exist in codegraph

**Governance Workflow:**
1. DocsDomainHandler stages changes automatically
2. Run `docs_validate(staged=true)` to check staged docs
3. Fix any errors found
4. Run `docs_promote(doc_path="...")` to promote

**Staging Support:**
- `staged=false`: Validates main index (default)
- `staged=true`: Validates pending changes separately
- Use before promotion to ensure quality

---

### 5.5 docs_promote

**Description:**
Promote staged documentation changes to main index with validation gating. Part of governance workflow.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `doc_path` | string | No | Path to document to promote (lists staged if omitted) |
| `validate` | boolean | No | Run validation before promotion (default: true) |
| `rollback` | boolean | No | Revert last promotion for given doc_path (default: false) |

**Usage Examples:**

```javascript
// List all staged docs
docs_promote()

// Promote specific doc with validation
docs_promote({
  doc_path: "docs/api.md",
  validate: true
})

// Promote without validation (unsafe)
docs_promote({
  doc_path: "docs/api.md",
  validate: false
})

// Rollback last promotion
docs_promote({
  doc_path: "docs/api.md",
  rollback: true
})
```

**Return Values:**

```json
// List staged docs
{
  "staged": [
    {
      "doc_path": "docs/api.md",
      "chunks": 15,
      "links": 8,
      "staged_at": 1704451200,
      "validated": true
    }
  ],
  "count": 1
}

// Successful promotion
{
  "message": "Promotion successful",
  "doc_path": "docs/api.md",
  "operation_id": "2025-01-05_123456abcdef"
}

// Rollback
{
  "message": "Rollback successful",
  "doc_path": "docs/api.md",
  "operation_id": "2025-01-05_654321fedcba"
}
```

**Common Use Cases:**
- Move validated documentation to production
- Promote individual documents after review
- Rollback problematic promotions

**Safety Features:**
- Validation gating by default
- Audit trail in operations.db
- Rollback support for last promotion
- Automatic removal from staging after promotion

**Workflow:**
1. Edit documentation files
2. DocsDomainHandler stages changes automatically
3. Run `docs_validate(staged=true)` to check
4. Run `docs_promote(doc_path="...")` to promote
5. Changes recorded to operations.db for audit

**Validation Behavior:**
- `validate=true` (default): Checks validation status, rejects if validation failed
- `validate=false` (unsafe): Skips validation, not recommended

---

### 5.6 docs_scan

**Description:**
Scan workspace for all documentation files and stage them in bulk. One-time bulk indexing for existing documentation.

**Parameters:**
None

**Usage Examples:**

```javascript
// Scan and stage all documentation
docs_scan()
```

**Return Values:**

```json
{
  "total_files": 48,
  "staged": 45,
  "failed": 3,
  "staged_files": [
    "README.md",
    "docs/api.md",
    "docs/tutorial.md"
  ],
  "failed_files": [
    "docs/broken.md: Failed to parse markdown",
    "docs/encoding.md: Invalid UTF-8 encoding",
    "docs/permission.md: Permission denied"
  ],
  "message": "Documentation Scan Complete\n- Total files found: 48\n- Staged: 45 files\n- Failed: 3 files"
}
```

**Common Use Cases:**
- First-time setup: Scan all existing docs before enabling watcher
- After bulk changes: When many docs are added/updated at once
- Manual refresh: Force re-scan of documentation

**Watch Integration:**
The docs watcher only handles incremental changes. This tool provides the initial scan functionality.

**File Types Scanned:**
- Markdown files (`.md`, `.markdown`)
- Text files (`.txt`)
- reStructuredText files (`.rst`)

**Directories Skipped:**
- `.codemcp/` - Internal CodeMCP directory
- `target/` - Rust build artifacts
- `node_modules/` - Node.js dependencies
- All hidden directories (starting with `.`)

**Staging:**
All found files are automatically staged. Use `docs_validate(staged=true)` and `docs_promote()` to promote to main index.

---

## 6. Database Management

Database management tools for exporting, importing, and backing up the graph database.

### 6.1 export_graph_snapshot

**Description:**
Export the entire Magellan graph database to a JSON file for debugging, testing, or backup purposes.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `output_path` | string | Yes | Path where JSON snapshot will be written (must end with .json) |

**Usage Examples:**

```javascript
// Export to snapshot file
export_graph_snapshot({
  output_path: "snapshot.json"
})

// Export with timestamp
export_graph_snapshot({
  output_path: "backups/graph_2025-01-05.json"
})
```

**Return Values:**

```json
{
  "db_path": "/home/user/project/.codemcp/codegraph.db",
  "output_path": "snapshot.json",
  "records_exported": 15234
}
```

**Format:**
JSONL (one JSON record per line):
- Entities: `{"type":"entity","id":1,"kind":"Function","name":"foo","file_path":"src/main.rs","data":{...}}`
- Edges: `{"type":"edge","id":1,"from_id":1,"to_id":2,"edge_type":"CALLS"}`

**Common Use Cases:**
- Creating database snapshots for testing
- Backing up graph data before experiments
- Debugging graph database issues
- Sharing graph state with others

**Prerequisites:**
- `magellan_init` must be run first
- `codegraph.db` must exist at `.codemcp/codegraph.db`

**Validation:**
- `output_path` must end with `.json` extension
- Parent directory must be writable

---

### 6.2 import_graph_snapshot

**Description:**
Import a previously exported snapshot into the database. **WARNING:** This will REPLACE all existing data in the database.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `input_path` | string | Yes | Path to JSON snapshot file (must end with .json) |

**Usage Examples:**

```javascript
// Import from snapshot file
import_graph_snapshot({
  input_path: "snapshot.json"
})

// Import from backup
import_graph_snapshot({
  input_path: "backups/graph_2025-01-05.json"
})
```

**Return Values:**

```json
{
  "db_path": "/home/user/project/.codemcp/codegraph.db",
  "input_path": "snapshot.json",
  "records_imported": 15234,
  "warning": "This operation REPLACED all existing data in the database"
}
```

**Common Use Cases:**
- Restoring database from snapshot for testing
- Loading shared graph data
- Reverting to previous database state
- Setting up reproducible test environments

**Prerequisites:**
- Input file must exist and be readable
- Input file must have `.json` extension
- File must be created by `export_graph_snapshot`

**Safety Warning:**
This operation **DELETES all existing data** before importing. Consider exporting current state first if you need it.

**Transaction Safety:**
Import is wrapped in a transaction. If it fails, no changes are made to the database.

**Format Requirements:**
- JSONL format (one JSON record per line)
- Valid entity and edge records
- Matching Magellan schema

---

## Workflows and Examples

### Workflow 1: Semantic Discovery for Code Understanding

**Scenario:** You're working with a large, unfamiliar codebase and need to understand authentication logic.

```javascript
// Step 1: Check semantic database status
semantic_stats()
// → {"symbol_count": 850, "file_count": 142, "status": "ready"}

// Step 2: Find all authentication functions
discover_by_purpose({
  purpose: "authentication",
  limit: 20
})
// → Returns 15 auth functions with summaries

// Step 3: Get detailed summary of key function
discover_summary({
  symbol: "authenticate_user",
  auto_index: true
})
// → "Validates JWT tokens and returns user session"

// Step 4: Get the actual code without file I/O
get_code_chunks({
  file_path: "src/auth.rs",
  symbol_name: "authenticate_user"
})

// Result: Understanding in ~500 tokens instead of ~50,000
```

### Workflow 2: Impact Analysis Before Refactoring

**Scenario:** You want to refactor a core utility function but need to know what will break.

```javascript
// Step 1: Get full impact analysis
get_impact_analysis({
  symbol_name: "process_request",
  depth: 3
})

// Returns:
// - 47 total impacted symbols
// - Hop 1: 12 direct references
// - Hop 2: 28 indirect references
// - Hop 3: 7 more dependencies

// Step 2: Tag high-risk symbols
add_symbol_label({
  symbol_name: "process_request",
  label: "unsafe"
})

// Step 3: Record complexity analysis
add_symbol_property({
  symbol_name: "process_request",
  key: "complexity",
  value: "high"
})

// Step 4: Now you know EXACTLY what will be affected
```

### Workflow 3: Documentation Maintenance

**Scenario:** You've renamed symbols and need to update documentation references.

```javascript
// Step 1: Check documentation health
docs_status()
// → Shows stale and missing docs

// Step 2: Scan for new/updated documentation
docs_scan()
// → Stages 48 documentation files

// Step 3: Validate staged docs
docs_validate({
  staged: true
})
// → Finds 3 broken links, 2 invalid code refs

// Step 4: Fix the issues...

// Step 5: Validate again
docs_validate({
  staged: true
})
// → All valid now

// Step 6: Promote to main index
docs_promote({
  doc_path: "docs/api.md"
})

// Result: Documentation synchronized with code
```

### Workflow 4: Memory-Aware Refactoring

**Scenario:** Check if similar refactoring has been done before.

```javascript
// Step 1: Search for similar operations
memory_similar({
  query: "rename function to more descriptive name",
  limit: 5
})

// → Found 3 similar operations (similarity > 0.7)

// Step 2: Get impact analysis
get_impact_analysis({
  symbol_name: "my_func",
  depth: 2
})

// Step 3: Perform refactoring
refactor_rename({
  symbol_name: "my_func",
  new_name: "descriptive_func_name",
  kind: "function",
  workspace_root: "."
})

// Step 4: Operation auto-recorded to memory
// Future searches can learn from this refactoring
```

---

## Performance Tips

### Semantic Discovery

1. **Use auto_index=true for first queries** - First query is slower but caches for future
2. **Filter by purpose when possible** - `discover_by_purpose` is faster than reading files
3. **Check semantic_stats first** - Verify database is ready before querying

### Symbol Metadata

1. **Labels for categories** - Use standard labels (deprecated, unsafe, async, public_api)
2. **Properties for metrics** - Store complexity, test coverage, deprecation reasons
3. **Query efficiently** - Use `get_symbols_by_label` for bulk queries

### Memory Tools

1. **Local embeddings only** - No external API latency for embeddings
2. **Search before refactoring** - Check if similar operations exist
3. **Automatic recording** - Most refactor tools auto-record to memory

### Impact Analysis

1. **Start with depth=2** - Direct + indirect references (recommended)
2. **Use depth=1 for quick checks** - Direct references only (fast)
3. **Avoid depth>5** - Usually too broad, consider refining query

### Documentation Tools

1. **Use docs_status before publishing** - Check documentation freshness
2. **Validate before promotion** - Run `docs_validate(staged=true)` first
3. **Scan after bulk changes** - Use `docs_scan` for initial indexing

---

## Troubleshooting

### Semantic Discovery Issues

**Problem:** `discover_summary` returns "Symbol not found"

**Solutions:**
1. Run `magellan_init` to build codegraph.db
2. Set `auto_index=true` to index on-demand
3. Check symbol name spelling and case

**Problem:** `discover_by_purpose` returns empty results

**Solutions:**
1. Semantic index may be empty (starts empty by design)
2. Run `index_by_purpose_demand` to index specific domains
3. Check purpose tag is valid

### Memory Tool Issues

**Problem:** `memory_similar` returns no results

**Solutions:**
1. Memory index starts empty - operations are recorded as you refactor
2. Check `memory_index_status` to verify initialization
3. Try different query terms

### Documentation Issues

**Problem:** `docs_validate` shows broken links

**Solutions:**
1. Check link syntax: `[text](file.md#heading)` or `[text](#heading)`
2. Verify target files exist
3. Ensure heading anchors match exactly

**Problem:** `docs_promote` fails with "validation failed"

**Solutions:**
1. Run `docs_validate(staged=true)` to see errors
2. Fix broken links and code references
3. Re-validate before promoting

### Database Issues

**Problem:** `export_graph_snapshot` fails

**Solutions:**
1. Ensure `output_path` ends with `.json`
2. Check write permissions for directory
3. Verify `codegraph.db` exists

**Problem:** `import_graph_snapshot` fails

**Solutions:**
1. Ensure `input_path` exists and ends with `.json`
2. Verify file was created by `export_graph_snapshot`
3. Check JSONL format is valid

---

## Best Practices

### 1. Lazy Indexing Strategy

- **Don't** run full semantic indexing upfront (takes days)
- **Do** use `discover_summary` with `auto_index=true` for on-demand indexing
- **Do** use `index_by_purpose_demand` to index specific domains

### 2. Metadata Management

- **Use standard labels:** `deprecated`, `unsafe`, `async`, `public_api`
- **Use consistent property keys:** `complexity`, `test_coverage`, `deprecation_reason`
- **Query before refactoring:** Check metadata to understand symbol context

### 3. Impact Analysis

- **Always** run `get_impact_analysis` before refactoring core symbols
- **Start with depth=2** for good balance of detail and performance
- **Tag high-risk symbols** with labels and properties for future reference

### 4. Documentation Governance

- **Always** validate before promotion
- **Use staging workflow** to review changes before publishing
- **Check docs_status** regularly to maintain documentation health

### 5. Memory-Aware Refactoring

- **Search first** with `memory_similar` to learn from past operations
- **Recording is automatic** for most refactor tools
- **Use metadata** parameter to store context for future reference

---

## Next Steps

This concludes **Part 3** of the CodeMCP manual. You should now be able to:

- Use semantic discovery tools for efficient code understanding
- Manage symbol metadata with labels and properties
- Perform impact analysis before refactoring
- Maintain documentation with validation and promotion workflow
- Use memory tools for semantic search of refactor history
- Export and import database snapshots

For more information, see:
- [Part 1](manual.md) - Overview, installation, and configuration
- [README.md](README.md) - Project overview and quick reference
- [CLAUDE.md](CLAUDE.md) - Usage guide for Claude Code
- [docs/](docs/) - Additional documentation
