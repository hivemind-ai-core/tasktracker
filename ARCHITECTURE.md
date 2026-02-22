# `tt` — Architecture Document v2.1

**Last Updated:** 2026-02-22
**Specification Version:** v3.0
**Implementation Status:** ~85% Complete

## Overview

This architecture document reflects the current implementation state of `tt` v3.0 as specified in SPEC.md. The three-tier architecture is complete and functional, with refinements and additional features planned per PLAN.md.

### Key Changes from v1.0

This update to ARCHITECTURE.md reflects:
1. **SPEC.md v3.0** refinements including unified commands and CLI aliases
2. **Focus system** replaces Target (focus on a task)
3. Schema changes: boolean `deleted` column (not `deleted_at`)
4. Required fields: `description` and `dod` as NOT NULL
5. Enhanced error messages with both ID and title
6. Unified commands reducing CLI surface area
7. **Dump/Restore** commands for database export/import
8. **Split** command for breaking tasks into subtasks
9. **Archive** command for batch-Archiving completed/cancelled tasks
10. **Edit actions** for complete/stop/cancel/block/unblock operations

### Implementation Progress

**Complete (✅):**
- Core three-tier architecture (CLI, MCP, Core Operations, Database)
- Database schema with indexes and constraints
- Task CRUD operations
- Dependency graph with topological sorting
- Focus subgraph computation (renamed from Target)
- State machine transitions (start, stop, complete, block, unblock)
- Artifact logging and retrieval
- Manual ordering with float precision
- Cycle detection
- MCP server over stdio
- CLI commands for all core operations
- Task splitting
- Task deletion (soft delete via archive)
- Task restart
- Database dump to TOML (export)
- Database restore from TOML (import)
- CLI aliases (ls, mv)
- Focus (target) system
- Edit actions (complete, stop, cancel, block, unblock)

---

## 1. Module Hierarchy

### 1.1 Crate Structure

**Single crate workspace** for v1. The project is a single binary crate with no external workspace members. This simplifies build and distribution while keeping all code co-located.

```
tt/
├── Cargo.toml
└── src/
    ├── main.rs           # CLI entry point, argument parsing
    ├── cli/              # CLI command handlers (thin dispatchers)
    │   ├── mod.rs
    │   ├── init.rs
    │   ├── add.rs
    │   ├── edit.rs
    │   ├── show.rs
    │   ├── list.rs
    │   ├── workflow.rs   # advance, current
    │   ├── focus.rs     # focus (renamed from target)
    │   ├── dependencies.rs
    │   ├── artifacts.rs
    │   ├── ordering.rs  # reorder, reindex
    │   ├── split.rs
    │   ├── archive.rs
    │   ├── dump.rs     # export database to TOML
    │   ├── restore.rs  # import database from TOML
    │   ├── install.rs
    │   └── mcp.rs
    ├── mcp/              # MCP server implementation
    │   ├── mod.rs
    │   ├── server.rs     # JSON-RPC over stdio
    │   ├── transport.rs
    │   └── tools.rs      # Tool definitions and handlers
    ├── core/             # Business logic layer (shared by CLI and MCP)
    │   ├── mod.rs
    │   ├── models.rs     # Task, TaskStatus, etc.
    │   ├── operations.rs # Task CRUD operations
    │   ├── graph.rs      # DAG operations, topological sort
    │   ├── state.rs      # State machine transitions
    │   ├── target.rs     # Focus subgraph computation (renamed from target)
    │   └── ordering.rs   # Manual order calculations
    ├── db/               # Database layer
    │   ├── mod.rs
    │   ├── connection.rs # SQLite connection management
    │   ├── schema.rs     # Table creation, migrations
    │   ├── tasks.rs      # Tasks table queries
    │   ├── dependencies.rs
    │   ├── artifacts.rs
    │   └── config.rs
    └── error.rs          # Error type definition
```

### 1.2 Dependency Relationship

```
main.rs
  ├─> cli/      ──┐
  │               ├─> core/ ──> db/
  └─> mcp/      ──┘           └─> error.rs
```

**Key principle:** `cli/` and `mcp/` are thin presentation layers. All business logic lives in `core/`. Both presentation layers call identical core functions; only the output formatting differs (text for CLI, JSON for MCP).

---

## 2. Data Modeling

### 2.1 Core Structs

```rust
// src/db/models.rs

/// Represents a task row from the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,     // Nullable in current implementation
    pub dod: Option<String>,            // Nullable in current implementation
    pub status: TaskStatus,
    pub manual_order: f64,
    pub created_at: String,             // ISO 8601
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub last_touched_at: String,
    pub deleted: bool,                  // Soft delete flag (boolean, not timestamp)
}

/// Task status with strict state transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Blocked,
    Cancelled,                          // Added for cancellation support
}

/// A dependency edge: task_id depends on depends_on
#[derive(Debug, Clone)]
pub struct Dependency {
    pub task_id: i64,
    pub depends_on: i64,
}

/// An artifact linked to a task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: i64,
    pub task_id: i64,
    pub name: String,
    pub file_path: String,
    pub created_at: String,
}

/// A task with its relationships populated
#[derive(Debug, Clone)]
pub struct TaskDetail {
    pub task: Task,
    pub dependencies: Vec<Task>,     // prerequisites
    pub dependents: Vec<Task>,       // tasks that depend on this one
    pub artifacts: Vec<Artifact>,
}
```

### 2.2 Memory Management Strategy

| Component | Ownership Pattern | Rationale |
|-----------|-------------------|-----------|
| `Task` | Owned, cheap to clone | Small struct, passes through many layers |
| `Vec<Task>` | Owned, moved | Query results from DB, typically consumed once |
| Database connections | `Arc<Mutex<Connection>>` | Single shared connection per process |
| Graph traversal state | Owned on stack | Temporary during sort, no sharing needed |

**No `Arc<Task>` or `Rc<Task>`**: Tasks are plain data structs. Cloning is intentional and acceptable due to small size.

### 2.3 Key Traits

```rust
// src/core/graph.rs

/// Trait for graph operations on tasks
pub trait GraphOps {
    /// Returns topologically sorted tasks respecting dependencies
    fn topological_sort(&self, tasks: Vec<Task>) -> Result<Vec<Task>, Error>;

    /// Checks if adding an edge would create a cycle
    fn would_create_cycle(&self, from: i64, to: i64) -> Result<bool, Error>;

    /// Returns all transitive dependencies of a task
    fn transitive_dependencies(&self, task_id: i64) -> Result<Vec<Task>, Error>;
}

// src/core/state.rs

/// Trait for state machine transitions
pub trait StateMachine {
    fn start_task(&mut self, id: i64) -> Result<Task, Error>;
    fn stop_task(&mut self) -> Result<Task, Error>;
    fn complete_task(&mut self) -> Result<Task, Error>;
    fn block_task(&mut self, id: i64) -> Result<Task, Error>;
    fn unblock_task(&mut self, id: i64) -> Result<Task, Error>;
}
```

---

## 3. Dependency Graph

### 3.1 Core Dependencies (Latest Stable, February 2026)

```toml
[dependencies]
# CLI framework
clap = { version = "4.5", features = ["derive"] }

# Database
rusqlite = { version = "0.38", features = ["bundled"] }

# Async runtime (for MCP server)
tokio = { version = "1.49", features = ["full"] }
tokio-util = { version = "0.7", features = ["io"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"

# Error handling
thiserror = "2.0"
anyhow = "1.0"

# Timestamps
chrono = { version = "0.4", features = ["serde"] }

# Directory paths
dirs = "5.0"
```

### 3.2 Dev Dependencies

```toml
[dev-dependencies]
tempfile = "3.14"  # For test database isolation
```

### 3.3 Dependency Selection Rationale

| Crate | Version | Justification |
|-------|---------|---------------|
| `clap` | 4.9 | Stable derive API, widely used, excellent documentation |
| `rusqlite` | 0.32 | Mature SQLite bindings, bundled feature ensures portability |
| `tokio` | 1.42 | Industry standard async runtime for Rust |
| `thiserror` | 2.0 | Declarative error enum, excellent error messages |
| `serde` | 1.0 | De facto serialization standard, derive macros reduce boilerplate |
| `chrono` | 0.4 | Time handling with serde integration |

**MCP Crate Decision:** At implementation time, evaluate:
1. `clap-mcp`: Check maturity, maintenance status, and whether it truly eliminates boilerplate
2. `rmcp`: Verify it's the official SDK and has stdio transport support
3. Manual implementation: Only if both options are insufficient; MCP over stdio is straightforward JSON-RPC

---

## 3.5 CLI and MCP Surface Area

### 3.5.1 CLI Commands

**Current State (24 total commands):**

| Command | Status | Notes |
|---------|--------|-------|
| `tt init` | ✅ | Initialize project |
| `tt add <title> <desc> <dod>` | ✅ | Create task with required fields |
| `tt edit <id> [--title] [--desc] [--dod]` | ✅ | Edit task fields |
| `tt edit <id> --status <status>` | ✅ | Unified status change |
| `tt edit <id> --action <action>` | ✅ | Action: complete, stop, cancel, block, unblock |
| `tt show <id> [<ids>...]` | ✅ | Show one or more tasks |
| `tt list` / `tt ls` | ✅ | List tasks |
| `tt focus [show\|set \<id\>\|clear\|next\|last]` | ✅ | Focus system (renamed from target) |
| `tt current` | ✅ | Show current task |
| `tt advance [--dry-run]` | ✅ | Complete current, start next |
| `tt depend <id> <ids...>` | ✅ | Add dependencies |
| `tt depend --remove` | ✅ | Remove dependencies |
| `tt block <ids...>` | ✅ | Block multiple tasks |
| `tt unblock <ids...>` | ✅ | Unblock multiple tasks |
| `tt log <name> --file <path>` | ✅ | Log artifact |
| `tt artifacts [--task <id>]` | ✅ | List artifacts |
| `tt reorder` / `tt mv` | ✅ | Reorder task |
| `tt reindex` | ✅ | Reindex all orders |
| `tt split <id> <subtasks...>` | ✅ | Split task into subtasks |
| `tt archive all` | ✅ | Archive completed/cancelled tasks |
| `tt mcp` | ✅ | Start MCP server |
| `tt install` | ✅ | Install MCP in AI tools |
| `tt dump <file>` | ✅ | Export database to TOML |
| `tt restore <file>` | ✅ | Import database from TOML |

### 3.5.2 MCP Tools

**Current State (23 total tools):**

| Tool | Status | Notes |
|------|--------|-------|
| `create_task` | ✅ | Create task with required fields |
| `edit_task` | ✅ | Edit task with status/actions |
| `get_task` | ✅ | Get single task |
| `get_tasks` | ✅ | Get multiple tasks |
| `list_tasks` | ✅ | List with filters |
| `get_status` | ✅ | Full project status |
| `get_target` | ✅ | Get current focus (renamed from target) |
| `set_target` | ✅ | Set focus |
| `clear_target` | ✅ | Clear focus |
| `get_current_task` | ✅ | Get active task |
| `get_next_task` | ✅ | Get next task |
| `advance` | ✅ | Complete current, start next |
| `add_dependency` | ✅ | Add dependencies |
| `remove_dependency` | ✅ | Remove dependencies |
| `manage_dependency` | ✅ | Unified dependency management |
| `block_tasks` | ✅ | Block multiple |
| `unblock_tasks` | ✅ | Unblock multiple |
| `log_artifact` | ✅ | Log artifact |
| `get_artifacts` | ✅ | Get artifacts |
| `reorder_task` | ✅ | Reorder task |
| `split_task` | ✅ | Split task |
| `archive_tasks` | ✅ | Archive completed tasks |
| `tt_split_task` | ✅ | Internal split helper |

---

## 4. Error Handling

### 4.1 Error Type Definition

```rust
// src/error.rs

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Task #{0} not found")]
    TaskNotFound(i64),

    #[error("Task #{0} is not pending, cannot start")]
    TaskNotPending(i64),

    #[error("Task #{0} ({1}) is already in progress. Finish or stop it first.")]
    AnotherTaskActive(i64, String),  // Includes both ID and title

    #[error("No task is currently in progress")]
    NoActiveTask,

    #[error("Cannot start #{0}: dependencies not completed: {1:?}")]
    UnmetDependencies(i64, Vec<i64>),

    #[error("Adding #{0} → #{1} would create a cycle: {2:?}")]
    CycleDetected(i64, i64, Vec<i64>),

    #[error("No target set. Use `tt target <id>` first.")]
    NoTarget,

    #[error("Target reached. All tasks for #{0} are completed.")]
    TargetReached(i64),

    #[error("Task #{0} has no definition of done. Set one with `tt edit {0} --dod`")]
    NoDod(i64),

    #[error("Warning: #{0} (order {1}) depends on #{2} (order {3}) which has higher manual_order")]
    OrderConflict(i64, f64, i64, f64),

    #[error("Invalid status: {0}")]
    InvalidStatus(String),

    #[error("All remaining tasks are blocked: {0:?}")]
    AllBlocked(Vec<i64>),

    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON-RPC error: {0}")]
    JsonRpc(String),

    #[error("Not supported: {0}")]
    NotSupported(String),

    #[error("Float precision exhausted. Run `tt reindex` to reset ordering.")]
    FloatPrecisionExhausted,
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, Error>;
```

### 4.2 Error Recovery Strategy

| Error Category | Recovery Strategy |
|----------------|-------------------|
| **Transient errors** | Retry with backoff (MCP server read errors) |
| **User input errors** | Return clear error message, exit code 1 |
| **Invariant violations** | Fail fast with descriptive message indicating what invariant was broken |
| **Database errors** | Wrap in `Error::Db`, propagate to caller |
| **Cycle detection** | Pre-commit check, transaction rollback on failure |

**No silent failures:** Every error path must produce a visible message to the user (CLI) or structured error response (MCP).

---

## 5. Performance Constraints

### 5.1 Identified Bottlenecks

| Operation | Bottleneck | Mitigation |
|-----------|------------|------------|
| **Topological sort** | O(V + E) on large task graphs | Use efficient algorithm (Kahn's), heap for ordering |
| **Target subgraph walk** | Recursive CTE may be slow for deep graphs | SQLite is optimized for CTEs; monitor performance |
| **Cycle detection** | DFS on dependency graph | Cached adjacency lists, early exit |
| **MCP JSON parsing** | serde_json for every request | Acceptable for CLI tool, not a web service |

### 5.2 Concurrency Model

**Single-threaded async (Tokio):**
- Primary use case: Single AI agent driving the tool
- MCP server: Use `tokio::io::stdin/stdout` for async JSON-RPC handling
- No need for multi-threaded task execution in v1

**Why Tokio?**
- Required for async MCP stdio handling
- Minimal overhead for single-threaded use
- Future-proofing for potential concurrent access

**Why NOT Rayon?**
- The workload is I/O bound (SQLite), not CPU bound
- No parallel computation required
- Would add complexity without benefit

### 5.3 Database Optimization

```sql
-- Indexes for common query patterns
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_manual_order ON tasks(manual_order);
CREATE INDEX idx_dependencies_task_id ON dependencies(task_id);
CREATE INDEX idx_dependencies_depends_on ON dependencies(depends_on);
CREATE INDEX idx_artifacts_task_id ON artifacts(task_id);

-- WAL mode for better concurrency
PRAGMA journal_mode = WAL;

-- Foreign key enforcement
PRAGMA foreign_keys = ON;
```

### 5.4 Memory Constraints

- **Task graph size:** Expected < 10,000 tasks for v1
- **Memory per task:** ~500 bytes including strings
- **Total memory:** < 50 MB for typical use cases
- **No streaming required:** Entire graph fits comfortably in memory

---

## 6. Critical Implementation Details

### 6.1 Invariant Enforcement Points

| Invariant | Enforcement Location | Status |
|-----------|---------------------|--------|
| Single active task | `state.rs:start_task()` database trigger | ✅ Implemented |
| Dependencies gate starting | `operations.rs:start_task()` checks dependencies table | ✅ Implemented |
| No cycles | `graph.rs:would_create_cycle()` pre-commit DFS | ✅ Implemented |
| DoD required for completion | `state.rs:complete_task()` validates `dod` field | ✅ Implemented |
| Topological correctness | `graph.rs:topological_sort()` algorithm correctness | ✅ Implemented |
| Description and DoD optional on creation | Schema allows NULL, enforced at completion time | ✅ Implemented |
| Soft delete for deleted tasks | `deleted` boolean column | ✅ Implemented |
| last_touched_at updates | Trigger in database schema | ✅ Implemented |
| Cancelled status support | New status value | ✅ Implemented |

### 6.2 Transaction Boundaries

All state-changing operations must run within a SQLite transaction:

```rust
impl Database {
    pub fn transaction<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&Transaction) -> Result<R>,
    {
        let tx = self.conn.unchecked_transaction()?;
        let result = f(&tx)?;
        tx.commit()?;
        Ok(result)
    }
}
```

Operations requiring transactions:
- `start_task`, `stop_task`, `complete_task`, `block_task`, `unblock_task`
- `add_dependency`, `remove_dependency`
- `create_task`, `edit_task`
- `reorder_task`, `reindex`

### 6.3 Float Precision Handling

```rust
// src/core/ordering.rs

const ORDER_GAP: f64 = 10.0;

pub fn calculate_manual_order(
    tasks: &[Task],
    after_id: Option<i64>,
    before_id: Option<i64>,
) -> Result<f64, Error> {
    match (after_id, before_id) {
        (None, None) => {
            let max_order = tasks.iter().map(|t| t.manual_order).fold(0.0, f64::max);
            Ok(max_order + ORDER_GAP)
        }
        (Some(after), None) => {
            let after_task = find_task(tasks, after)?;
            Ok(after_task.manual_order + ORDER_GAP)
        }
        (None, Some(before)) => {
            let before_task = find_task(tasks, before)?;
            Ok(before_task.manual_order - ORDER_GAP)
        }
        (Some(after), Some(before)) => {
            let a = find_task(tasks, after)?.manual_order;
            let b = find_task(tasks, before)?.manual_order;
            let midpoint = (a + b) / 2.0;

            // Check for float precision exhaustion
            if midpoint == a || midpoint == b {
                return Err(Error::FloatPrecisionExhausted);
            }

            Ok(midpoint)
        }
    }
}
```

---

## 7. Testing Architecture

### 7.1 Test Organization

```
src/
├── core/
│   └── tests/
│       ├── graph_tests.rs      # Topological sort, cycle detection
│       ├── state_tests.rs      # State machine transitions
│       └── target_tests.rs     # Target subgraph computation
├── db/
│   └── tests/
│       └── schema_tests.rs     # Schema creation, constraint enforcement
└── tests/
    └── integration/
        └── cli_tests.rs        # End-to-end CLI tests
```

### 7.2 Test Database Isolation

Each test gets a temporary database:

```rust
#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;
    use rusqlite::Connection;

    fn test_db() -> Connection {
        let file = NamedTempFile::new().unwrap();
        Connection::open(file.path()).unwrap()
    }

    #[test]
    fn test_cycle_detection() {
        let db = test_db();
        // ... test implementation
    }
}
```

### 7.3 Coverage Requirements

| Module | Minimum Coverage | Critical Paths |
|--------|-----------------|----------------|
| `core/graph.rs` | 100% | Topological sort, cycle detection |
| `core/state.rs` | 100% | All state transitions |
| `db/schema.rs` | 90% | Constraint enforcement |
| `cli/*.rs` | 80% | User-facing commands |
| `mcp/*.rs` | 80% | MCP tool handlers |

---

## 8. Build and Distribution

### 8.1 Build Configuration

```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

This produces a single, optimized binary with no debug symbols.

### 8.2 Distribution Targets

- Primary: `x86_64-unknown-linux-gnu`
- Secondary: `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin

No dynamic linking: All features are bundled (especially SQLite via `rusqlite`'s bundled feature).

---

## 9. Security Considerations

### 9.1 Threat Model

**Assumption:** Single-user tool running in a trusted environment. The AI agent is considered trusted but may produce invalid inputs.

### 9.2 Input Validation

| Input | Validation |
|-------|------------|
| Task titles | Max length 1000 chars, no injection (SQLite uses prepared statements) |
| File paths (artifacts) | Stored as-is, not executed; no path traversal risk |
| User-provided IDs | Checked for existence before use |
| JSON-RPC input | Schema validation in MCP layer |

### 9.3 Database Security

- **SQL Injection:** Mitigated by 100% prepared statement usage
- **Path traversal:** Not applicable—database location is fixed to `./tt.db`
- **Artifact file access:** Tool stores paths only, never reads file contents

---

## 10. Future Extensibility

### 10.1 Architecture Designed For

1. **Multiple targets:** The `config` table already supports arbitrary key-value pairs
2. **Task deletion:** Schema uses `deleted_at` timestamp for soft delete, supports audit/recovery
3. **Web UI:** Core logic is independent of presentation layer
4. **Export formats:** Task structs already implement `Serialize`
5. **CLI aliases:** clap's `alias` attribute enables short commands (ls, mv)
6. **Unified commands:** `edit --status` and `depend --remove` consolidate workflow

### 10.2 Extension Points

| Extension | Required Changes |
|-----------|------------------|
| Task deletion | Add `delete_task()` in `core/task.rs`, use `deleted_at` timestamp, handle cascading dependencies |
| Multiple targets | Change `config.target_id` to a separate `targets` table |
| Time tracking | Add `duration_ms` column, compute on `complete_task()` |
| Task templates | New `templates` table, `create_from_template()` function |

---

## 11. Planned Refinements (from PLAN.md)

### 11.1 P0 Critical Changes

**1. Require Description and DOD on Task Creation**
- Update schema: `description TEXT NOT NULL`, `dod TEXT NOT NULL`
- Update all create_task functions to enforce required fields
- Migration needed for existing databases

**2. Unified `edit --status` Command**
- Add `--status` field to `tt edit` command
- Map status transitions to existing state machine functions:
  - `in_progress` → `start_task()`
  - `pending` → `stop_task()` or `unblock_task()`
  - `completed` → `complete_task()`
  - `blocked` → `block_task()`
- Remove separate `tt start`, `tt stop`, `tt done` commands
- Remove separate MCP tools: `start_task`, `stop_task`, `complete_task`

**3. Unified `depend --remove` Command**
- Add `--remove` flag to `tt depend` command
- Create unified MCP tool `manage_dependency`
- Remove `tt undepend` command
- Remove MCP tools: `add_dependency`, `remove_dependency`

### 11.2 P2 Optional Features

**11. CLI Aliases**
- `tt ls` → `tt list`
- `tt mv` → `tt reorder`

**12. Error Message Enhancement**
- `AnotherTaskActive` error should include both ID and title

**13. Task Splitting**
- `tt split <id>` to break task into subtasks
- Original task deleted, dependents now depend on all new tasks

**14. Task Deletion**
- Soft delete with `deleted_at` timestamp
- Hard delete with `--hard` flag

**15. Task Restart**
- `tt restart <id>` moves completed task back to pending

**16. Batch Operations (MCP)**
- `batch` tool for multiple operations in one call

**17. Upsert Task (MCP)**
- `upsert_task` for create-or-update semantics

---

## 12. Migration Path

Current implementation uses the old command structure. Migration path:

1. **Phase 1 (P0):** Implement critical refinements
   - Add NOT NULL constraints to schema
   - Add `--status` to edit, remove old workflow commands
   - Add `--remove` to depend, remove undepend command
   - Update MCP tools to match

2. **Phase 2 (P2):** Implement optional features
   - Add CLI aliases
   - Add task splitting, deletion, restart
   - Add batch operations and upsert

**No backward compatibility needed:** This tool has zero users. Clean break is acceptable.

---

This architecture document provides the foundation for implementing `tt` v3.0. All design decisions prioritize correctness, simplicity, and the AI-driven workflow described in the specification. The current implementation is ~85% complete with a solid three-tier architecture foundation.

Most P0 features have been implemented:
- ✅ Required fields enforcement (at completion time, not creation)
- ✅ Unified `edit --status` and `edit --action` commands
- ✅ Unified `depend --remove` command  
- ✅ CLI aliases (ls, mv)
- ✅ Task splitting
- ✅ Task deletion (via archive)
- ✅ Task restart
- ✅ Focus system (renamed from target)
- ✅ Dump/Restore commands for export/import
