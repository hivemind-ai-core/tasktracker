//! Config key-value store operations

use crate::error::Result;
use rusqlite::{params, Connection};

pub const TARGET_KEY: &str = "target_id";

/// Get a config value by key
pub fn get_config(conn: &Connection, key: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM config WHERE key = ?1")?;
    let mut rows = stmt.query(params![key])?;

    if let Some(row) = rows.next()? {
        let value: String = row.get(0)?;
        Ok(Some(value))
    } else {
        Ok(None)
    }
}

/// Set a config value
pub fn set_config(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO config (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// Delete a config key
pub fn delete_config(conn: &Connection, key: &str) -> Result<()> {
    conn.execute("DELETE FROM config WHERE key = ?1", params![key])?;
    Ok(())
}

/// Get the current target ID
pub fn get_target(conn: &Connection) -> Result<Option<i64>> {
    match get_config(conn, TARGET_KEY)? {
        Some(value) => {
            let id: i64 = value.parse().map_err(|_| {
                crate::error::Error::InvalidStatus("Invalid target ID format".to_string())
            })?;
            Ok(Some(id))
        }
        None => Ok(None),
    }
}

/// Set the target ID
pub fn set_target(conn: &Connection, target_id: i64) -> Result<()> {
    set_config(conn, TARGET_KEY, &target_id.to_string())
}

/// Clear the target
pub fn clear_target(conn: &Connection) -> Result<()> {
    delete_config(conn, TARGET_KEY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_memory_db;
    use crate::db::schema::CREATE_SCHEMA_SQL;

    fn setup() -> Connection {
        let conn = open_memory_db().unwrap();
        conn.execute_batch(CREATE_SCHEMA_SQL).unwrap();
        conn
    }

    #[test]
    fn test_set_and_get_config() {
        let conn = setup();

        set_config(&conn, "test_key", "test_value").unwrap();
        let value = get_config(&conn, "test_key").unwrap();

        assert_eq!(value, Some("test_value".to_string()));
    }

    #[test]
    fn test_get_nonexistent_config() {
        let conn = setup();
        let value = get_config(&conn, "nonexistent").unwrap();
        assert_eq!(value, None);
    }

    #[test]
    fn test_update_config() {
        let conn = setup();

        set_config(&conn, "test_key", "value1").unwrap();
        set_config(&conn, "test_key", "value2").unwrap();

        let value = get_config(&conn, "test_key").unwrap();
        assert_eq!(value, Some("value2".to_string()));
    }

    #[test]
    fn test_delete_config() {
        let conn = setup();

        set_config(&conn, "test_key", "test_value").unwrap();
        delete_config(&conn, "test_key").unwrap();

        let value = get_config(&conn, "test_key").unwrap();
        assert_eq!(value, None);
    }

    #[test]
    fn test_set_and_get_target() {
        let conn = setup();

        set_target(&conn, 42).unwrap();
        let target = get_target(&conn).unwrap();

        assert_eq!(target, Some(42));
    }

    #[test]
    fn test_clear_target() {
        let conn = setup();

        set_target(&conn, 42).unwrap();
        clear_target(&conn).unwrap();

        let target = get_target(&conn).unwrap();
        assert_eq!(target, None);
    }
}
