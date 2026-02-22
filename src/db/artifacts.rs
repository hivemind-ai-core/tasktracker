//! Artifact CRUD operations

use crate::core::models::Artifact;
use crate::error::Result;
use rusqlite::{params, Connection};

/// Create a new artifact for a task
pub fn create_artifact(
    conn: &Connection,
    task_id: i64,
    name: &str,
    file_path: &str,
) -> Result<Artifact> {
    conn.execute(
        "INSERT INTO artifacts (task_id, name, file_path) VALUES (?1, ?2, ?3)",
        params![task_id, name, file_path],
    )?;

    let id = conn.last_insert_rowid();
    get_artifact(conn, id)?.ok_or_else(|| {
        crate::error::Error::NotSupported("Failed to retrieve created artifact".to_string())
    })
}

/// Get an artifact by ID
pub fn get_artifact(conn: &Connection, id: i64) -> Result<Option<Artifact>> {
    let mut stmt = conn
        .prepare("SELECT id, task_id, name, file_path, created_at FROM artifacts WHERE id = ?1")?;

    let mut rows = stmt.query(params![id])?;

    if let Some(row) = rows.next()? {
        let artifact = row_to_artifact(row)?;
        Ok(Some(artifact))
    } else {
        Ok(None)
    }
}

/// Get all artifacts for a task
pub fn get_artifacts_for_task(conn: &Connection, task_id: i64) -> Result<Vec<Artifact>> {
    let mut stmt = conn.prepare(
        "SELECT id, task_id, name, file_path, created_at 
         FROM artifacts WHERE task_id = ?1 ORDER BY created_at",
    )?;

    let rows = stmt.query_map(params![task_id], row_to_artifact)?;

    let mut artifacts = Vec::new();
    for artifact in rows {
        artifacts.push(artifact?);
    }

    Ok(artifacts)
}

/// Delete an artifact by ID
pub fn delete_artifact(conn: &Connection, id: i64) -> Result<()> {
    let affected = conn.execute("DELETE FROM artifacts WHERE id = ?1", params![id])?;

    if affected == 0 {
        return Err(crate::error::Error::NotSupported(
            "Artifact not found".to_string(),
        ));
    }

    Ok(())
}

/// Helper function to convert a database row to an Artifact
fn row_to_artifact(row: &rusqlite::Row) -> rusqlite::Result<Artifact> {
    Ok(Artifact {
        id: row.get(0)?,
        task_id: row.get(1)?,
        name: row.get(2)?,
        file_path: row.get(3)?,
        created_at: row.get(4)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_memory_db;
    use crate::db::schema::CREATE_SCHEMA_SQL;
    use crate::db::tasks::create_task;

    fn setup() -> Connection {
        let conn = open_memory_db().unwrap();
        conn.execute_batch(CREATE_SCHEMA_SQL).unwrap();
        conn
    }

    #[test]
    fn test_create_artifact() {
        let conn = setup();
        let task = create_task(&conn, "Test Task", "", "", 10.0).unwrap();

        let artifact =
            create_artifact(&conn, task.id, "research", ".tt/artifacts/1-research.md").unwrap();

        assert_eq!(artifact.task_id, task.id);
        assert_eq!(artifact.name, "research");
        assert_eq!(artifact.file_path, ".tt/artifacts/1-research.md");
        assert!(artifact.id > 0);
    }

    #[test]
    fn test_get_artifact() {
        let conn = setup();
        let task = create_task(&conn, "Test Task", "", "", 10.0).unwrap();
        let created =
            create_artifact(&conn, task.id, "research", ".tt/artifacts/1-research.md").unwrap();

        let fetched = get_artifact(&conn, created.id).unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "research");
    }

    #[test]
    fn test_get_artifacts_for_task() {
        let conn = setup();
        let task1 = create_task(&conn, "Task 1", "", "", 10.0).unwrap();
        let task2 = create_task(&conn, "Task 2", "", "", 20.0).unwrap();

        create_artifact(&conn, task1.id, "research", ".tt/artifacts/1-research.md").unwrap();
        create_artifact(&conn, task1.id, "plan", ".tt/artifacts/1-plan.md").unwrap();
        create_artifact(&conn, task2.id, "research", ".tt/artifacts/2-research.md").unwrap();

        let artifacts = get_artifacts_for_task(&conn, task1.id).unwrap();
        assert_eq!(artifacts.len(), 2);
    }

    #[test]
    fn test_delete_artifact() {
        let conn = setup();
        let task = create_task(&conn, "Test Task", "", "", 10.0).unwrap();
        let artifact =
            create_artifact(&conn, task.id, "research", ".tt/artifacts/1-research.md").unwrap();

        delete_artifact(&conn, artifact.id).unwrap();

        let fetched = get_artifact(&conn, artifact.id).unwrap();
        assert!(fetched.is_none());
    }

    #[test]
    fn test_delete_nonexistent_artifact() {
        let conn = setup();
        let result = delete_artifact(&conn, 999);
        assert!(result.is_err());
    }
}
