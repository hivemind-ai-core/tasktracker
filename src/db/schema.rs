//! Database schema definitions for tt
//!
//! Includes all tables, constraints, indexes, and triggers per SPEC.md Section 4.2

/// SQL to create all database tables, indexes, and triggers
pub const CREATE_SCHEMA_SQL: &str = r#"
-- Enable foreign key enforcement
PRAGMA foreign_keys = ON;

-- Main tasks table
CREATE TABLE IF NOT EXISTS tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    description TEXT,
    dod TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    manual_order REAL NOT NULL DEFAULT 0.0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now')),
    started_at TEXT,
    completed_at TEXT,
    last_touched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now')),
    
    -- Status must be one of the valid values
    CONSTRAINT chk_status CHECK (status IN ('pending', 'in_progress', 'completed', 'blocked'))
);

-- Dependencies table (DAG edges)
CREATE TABLE IF NOT EXISTS dependencies (
    task_id INTEGER NOT NULL,
    depends_on INTEGER NOT NULL,
    
    -- Composite primary key
    PRIMARY KEY (task_id, depends_on),
    
    -- Foreign key constraints
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
    FOREIGN KEY (depends_on) REFERENCES tasks(id) ON DELETE CASCADE,
    
    -- Self-dependency is not allowed
    CONSTRAINT chk_no_self_dep CHECK (task_id != depends_on)
);

-- Artifacts table (linked files)
CREATE TABLE IF NOT EXISTS artifacts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    file_path TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now')),
    
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
);

-- Config table (key-value store)
CREATE TABLE IF NOT EXISTS config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_manual_order ON tasks(manual_order);
CREATE INDEX IF NOT EXISTS idx_dependencies_task_id ON dependencies(task_id);
CREATE INDEX IF NOT EXISTS idx_dependencies_depends_on ON dependencies(depends_on);
CREATE INDEX IF NOT EXISTS idx_artifacts_task_id ON artifacts(task_id);

-- Trigger to update last_touched_at on any task modification
CREATE TRIGGER IF NOT EXISTS trg_tasks_touch_update
AFTER UPDATE ON tasks
BEGIN
    UPDATE tasks SET last_touched_at = strftime('%Y-%m-%dT%H:%M:%S', 'now') WHERE id = NEW.id;
END;
"#;

/// SQL to enable WAL mode
pub const ENABLE_WAL_SQL: &str = "PRAGMA journal_mode = WAL;";

/// SQL to check if schema exists (check if tasks table exists)
pub const CHECK_SCHEMA_SQL: &str =
    "SELECT name FROM sqlite_master WHERE type='table' AND name='tasks'";

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn in_memory_db() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn test_schema_creation() {
        let conn = in_memory_db();
        conn.execute_batch(CREATE_SCHEMA_SQL).unwrap();

        // Verify tasks table exists
        let mut stmt = conn.prepare(CHECK_SCHEMA_SQL).unwrap();
        let exists = stmt.exists([]).unwrap();
        assert!(exists, "tasks table should exist after schema creation");
    }

    #[test]
    fn test_status_constraint() {
        let conn = in_memory_db();
        conn.execute_batch(CREATE_SCHEMA_SQL).unwrap();

        // Valid status should work
        conn.execute(
            "INSERT INTO tasks (title, status, manual_order) VALUES (?1, ?2, ?3)",
            ["Test", "pending", "10.0"],
        )
        .unwrap();

        // Invalid status should fail
        let result = conn.execute(
            "INSERT INTO tasks (title, status, manual_order) VALUES (?1, ?2, ?3)",
            ["Test2", "invalid_status", "20.0"],
        );
        assert!(result.is_err(), "Invalid status should be rejected");
    }

    #[test]
    fn test_self_dependency_constraint() {
        let conn = in_memory_db();
        conn.execute_batch(CREATE_SCHEMA_SQL).unwrap();

        // Create a task first
        conn.execute(
            "INSERT INTO tasks (title, status, manual_order) VALUES (?1, ?2, ?3)",
            ["Test", "pending", "10.0"],
        )
        .unwrap();

        // Self-dependency should fail
        let result = conn.execute(
            "INSERT INTO dependencies (task_id, depends_on) VALUES (1, 1)",
            [],
        );
        assert!(result.is_err(), "Self-dependency should be rejected");
    }

    #[test]
    fn test_foreign_key_constraint() {
        let conn = in_memory_db();
        conn.execute_batch(CREATE_SCHEMA_SQL).unwrap();

        // Dependency on non-existent task should fail
        let result = conn.execute(
            "INSERT INTO dependencies (task_id, depends_on) VALUES (999, 998)",
            [],
        );
        assert!(result.is_err(), "Foreign key constraint should be enforced");
    }

    #[test]
    fn test_last_touched_trigger() {
        let conn = in_memory_db();
        conn.execute_batch(CREATE_SCHEMA_SQL).unwrap();

        // Create a task
        conn.execute(
            "INSERT INTO tasks (title, status, manual_order) VALUES (?1, ?2, ?3)",
            ["Test", "pending", "10.0"],
        )
        .unwrap();

        // Get initial last_touched_at
        let initial: String = conn
            .query_row(
                "SELECT last_touched_at FROM tasks WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();

        // Delay to ensure timestamp changes (SQLite uses second precision)
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Update the task
        conn.execute("UPDATE tasks SET title = 'Updated' WHERE id = 1", [])
            .unwrap();

        // Get updated last_touched_at
        let updated: String = conn
            .query_row(
                "SELECT last_touched_at FROM tasks WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_ne!(
            initial, updated,
            "last_touched_at should be updated on modification"
        );
    }
}
