//! Database connection management
//!
//! Handles SQLite connection setup for tt.

use crate::error::Result;
use rusqlite::Connection;
use std::path::Path;

/// Open a database connection with proper settings
pub fn open_db<P: AsRef<Path>>(path: P) -> Result<Connection> {
    let conn = Connection::open(path)?;

    // Enable foreign key enforcement
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    Ok(conn)
}

/// Open an in-memory database (for testing)
pub fn open_memory_db() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;

    // Enable foreign key enforcement (WAL mode not available for in-memory)
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    Ok(conn)
}

/// Initialize the database schema
pub fn init_schema(conn: &Connection) -> Result<()> {
    use super::schema::CREATE_SCHEMA_SQL;
    conn.execute_batch(CREATE_SCHEMA_SQL)?;
    Ok(())
}

/// Check if database is initialized (has schema)
pub fn is_initialized(conn: &Connection) -> Result<bool> {
    use super::schema::CHECK_SCHEMA_SQL;
    let mut stmt = conn.prepare(CHECK_SCHEMA_SQL)?;
    let exists = stmt.exists([])?;
    Ok(exists)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_memory_db() {
        let conn = open_memory_db().unwrap();
        // Just verify it opens without error
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        // In-memory uses memory journal mode
        assert_eq!(journal_mode, "memory");
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let conn = open_memory_db().unwrap();
        let fk_enabled: i32 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fk_enabled, 1, "Foreign keys should be enabled");
    }

    #[test]
    fn test_init_schema() {
        let conn = open_memory_db().unwrap();

        // Before init, not initialized
        assert!(!is_initialized(&conn).unwrap());

        // Initialize
        init_schema(&conn).unwrap();

        // After init, initialized
        assert!(is_initialized(&conn).unwrap());
    }

    #[test]
    fn test_open_db_file() {
        // Create a temporary directory for the database file
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let conn = open_db(&db_path).unwrap();
        drop(conn);

        // File should exist
        assert!(db_path.exists());
    }
}

#[cfg(test)]
mod file_tests {
    //! Tests with file databases to verify persistence behavior
    
    use super::*;
    use std::path::PathBuf;
    use std::fs;
    
    fn setup_file_db() -> PathBuf {
        let test_dir = "/tmp/tt_file_conn_test";
        let db_path = PathBuf::from(format!("{}/test.db", test_dir));
        
        // Cleanup
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).unwrap();
        
        db_path
    }
    
    #[test]
    fn test_file_db_basic_persistence() {
        let db_path = setup_file_db();
        
        // Create table and insert
        {
            let conn = open_db(&db_path).unwrap();
            conn.execute(
                "CREATE TABLE test (id INTEGER PRIMARY KEY, value TEXT)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO test (value) VALUES ('initial')",
                [],
            ).unwrap();
        }
        
        // Update in new connection
        {
            let conn = open_db(&db_path).unwrap();
            conn.execute("UPDATE test SET value = 'updated' WHERE id = 1", []).unwrap();
        }
        
        // Verify with new connection
        {
            let conn = open_db(&db_path).unwrap();
            let value: String = conn.query_row(
                "SELECT value FROM test WHERE id = 1",
                [],
                |row| row.get(0),
            ).unwrap();
            assert_eq!(value, "updated");
        }
    }
    
    #[test]
    fn test_file_db_trigger_side_effect() {
        //! Test that trigger side effects persist correctly
        let db_path = setup_file_db();
        
        // Create table with trigger (similar to our schema)
        {
            let conn = open_db(&db_path).unwrap();
            conn.execute_batch(r#"
                CREATE TABLE tasks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    title TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'pending',
                    last_touched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now'))
                );
                
                CREATE TRIGGER trg_tasks_touch_update
                AFTER UPDATE ON tasks
                BEGIN
                    UPDATE tasks SET last_touched_at = strftime('%Y-%m-%dT%H:%M:%S', 'now') WHERE id = NEW.id;
                END;
            "#).unwrap();
            
            conn.execute(
                "INSERT INTO tasks (title, status) VALUES ('Test', 'pending')",
                [],
            ).unwrap();
        }
        
        // Get initial timestamp
        let initial_time: String = {
            let conn = open_db(&db_path).unwrap();
            conn.query_row("SELECT last_touched_at FROM tasks WHERE id = 1", [], |row| row.get(0)).unwrap()
        };
        
        // Wait and update
        std::thread::sleep(std::time::Duration::from_secs(1));
        
        {
            let conn = open_db(&db_path).unwrap();
            conn.execute("UPDATE tasks SET status = 'in_progress' WHERE id = 1", []).unwrap();
            
            // Check within same connection
            let time_in_conn: String = conn.query_row(
                "SELECT last_touched_at FROM tasks WHERE id = 1", [], |row| row.get(0)
            ).unwrap();
            println!("  Time within connection: {}", time_in_conn);
        }
        
        // Check with new connection
        let final_time: String = {
            let conn = open_db(&db_path).unwrap();
            conn.query_row("SELECT last_touched_at FROM tasks WHERE id = 1", [], |row| row.get(0)).unwrap()
        };
        
        println!("  Initial time: {}", initial_time);
        println!("  Final time: {}", final_time);
        
        assert_ne!(initial_time, final_time, "last_touched_at should be updated by trigger");
        
        // Also verify status
        {
            let conn = open_db(&db_path).unwrap();
            let status: String = conn.query_row(
                "SELECT status FROM tasks WHERE id = 1", [], |row| row.get(0)
            ).unwrap();
            assert_eq!(status, "in_progress");
        }
    }
}
