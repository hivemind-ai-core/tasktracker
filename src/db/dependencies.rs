//! Dependency CRUD operations

use crate::core::models::Dependency;
use crate::error::{Error, Result};
use rusqlite::{params, Connection};

/// Add a dependency edge (task_id depends on depends_on)
pub fn add_dependency(conn: &Connection, task_id: i64, depends_on: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO dependencies (task_id, depends_on) VALUES (?1, ?2)",
        params![task_id, depends_on],
    )?;
    Ok(())
}

/// Remove a dependency edge
pub fn remove_dependency(conn: &Connection, task_id: i64, depends_on: i64) -> Result<()> {
    let affected = conn.execute(
        "DELETE FROM dependencies WHERE task_id = ?1 AND depends_on = ?2",
        params![task_id, depends_on],
    )?;

    if affected == 0 {
        return Err(Error::NotSupported("Dependency does not exist".to_string()));
    }

    Ok(())
}

/// Get all dependencies for a task (what it depends on)
pub fn get_dependencies(conn: &Connection, task_id: i64) -> Result<Vec<Dependency>> {
    let mut stmt =
        conn.prepare("SELECT task_id, depends_on FROM dependencies WHERE task_id = ?1")?;

    let rows = stmt.query_map(params![task_id], |row| {
        Ok(Dependency {
            task_id: row.get(0)?,
            depends_on: row.get(1)?,
        })
    })?;

    let mut deps = Vec::new();
    for dep in rows {
        deps.push(dep?);
    }

    Ok(deps)
}

/// Get all dependents for a task (what depends on it)
pub fn get_dependents(conn: &Connection, depends_on: i64) -> Result<Vec<Dependency>> {
    let mut stmt =
        conn.prepare("SELECT task_id, depends_on FROM dependencies WHERE depends_on = ?1")?;

    let rows = stmt.query_map(params![depends_on], |row| {
        Ok(Dependency {
            task_id: row.get(0)?,
            depends_on: row.get(1)?,
        })
    })?;

    let mut deps = Vec::new();
    for dep in rows {
        deps.push(dep?);
    }

    Ok(deps)
}

/// Get all dependencies in the database
pub fn get_all_dependencies(conn: &Connection) -> Result<Vec<Dependency>> {
    let mut stmt = conn.prepare("SELECT task_id, depends_on FROM dependencies")?;

    let rows = stmt.query_map([], |row| {
        Ok(Dependency {
            task_id: row.get(0)?,
            depends_on: row.get(1)?,
        })
    })?;

    let mut deps = Vec::new();
    for dep in rows {
        deps.push(dep?);
    }

    Ok(deps)
}

/// Check if a task has any dependencies
pub fn has_dependencies(conn: &Connection, task_id: i64) -> Result<bool> {
    let mut stmt = conn.prepare("SELECT EXISTS(SELECT 1 FROM dependencies WHERE task_id = ?1)")?;
    let exists: bool = stmt.query_row(params![task_id], |row| row.get(0))?;
    Ok(exists)
}

/// Check if a dependency exists
pub fn dependency_exists(conn: &Connection, task_id: i64, depends_on: i64) -> Result<bool> {
    let mut stmt = conn.prepare(
        "SELECT EXISTS(SELECT 1 FROM dependencies WHERE task_id = ?1 AND depends_on = ?2)",
    )?;
    let exists: bool = stmt.query_row(params![task_id, depends_on], |row| row.get(0))?;
    Ok(exists)
}

/// Get incomplete dependencies for a task (dependencies that are not completed)
pub fn get_incomplete_dependencies(conn: &Connection, task_id: i64) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT d.depends_on 
         FROM dependencies d
         JOIN tasks t ON d.depends_on = t.id
         WHERE d.task_id = ?1 AND t.status != 'completed'",
    )?;

    let rows = stmt.query_map(params![task_id], |row| row.get::<_, i64>(0))?;

    let mut ids = Vec::new();
    for id in rows {
        ids.push(id?);
    }

    Ok(ids)
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
    fn test_add_dependency() {
        let conn = setup();
        let task1 = create_task(&conn, "Task 1", "", "", 10.0).unwrap();
        let task2 = create_task(&conn, "Task 2", "", "", 20.0).unwrap();

        add_dependency(&conn, task2.id, task1.id).unwrap();

        assert!(dependency_exists(&conn, task2.id, task1.id).unwrap());
    }

    #[test]
    fn test_add_duplicate_dependency_fails() {
        let conn = setup();
        let task1 = create_task(&conn, "Task 1", "", "", 10.0).unwrap();
        let task2 = create_task(&conn, "Task 2", "", "", 20.0).unwrap();

        add_dependency(&conn, task2.id, task1.id).unwrap();
        let result = add_dependency(&conn, task2.id, task1.id);
        assert!(result.is_err()); // Duplicate should fail
    }

    #[test]
    fn test_self_dependency_fails() {
        let conn = setup();
        let task1 = create_task(&conn, "Task 1", "", "", 10.0).unwrap();

        let result = add_dependency(&conn, task1.id, task1.id);
        assert!(result.is_err()); // Self-dependency should fail
    }

    #[test]
    fn test_remove_dependency() {
        let conn = setup();
        let task1 = create_task(&conn, "Task 1", "", "", 10.0).unwrap();
        let task2 = create_task(&conn, "Task 2", "", "", 20.0).unwrap();

        add_dependency(&conn, task2.id, task1.id).unwrap();
        remove_dependency(&conn, task2.id, task1.id).unwrap();

        assert!(!dependency_exists(&conn, task2.id, task1.id).unwrap());
    }

    #[test]
    fn test_remove_nonexistent_dependency() {
        let conn = setup();
        let result = remove_dependency(&conn, 1, 2);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_dependencies() {
        let conn = setup();
        let task1 = create_task(&conn, "Task 1", "", "", 10.0).unwrap();
        let task2 = create_task(&conn, "Task 2", "", "", 20.0).unwrap();
        let task3 = create_task(&conn, "Task 3", "", "", 30.0).unwrap();

        add_dependency(&conn, task3.id, task1.id).unwrap();
        add_dependency(&conn, task3.id, task2.id).unwrap();

        let deps = get_dependencies(&conn, task3.id).unwrap();
        assert_eq!(deps.len(), 2);
    }

    #[test]
    fn test_get_dependents() {
        let conn = setup();
        let task1 = create_task(&conn, "Task 1", "", "", 10.0).unwrap();
        let task2 = create_task(&conn, "Task 2", "", "", 20.0).unwrap();
        let task3 = create_task(&conn, "Task 3", "", "", 30.0).unwrap();

        // Both task2 and task3 depend on task1
        add_dependency(&conn, task2.id, task1.id).unwrap();
        add_dependency(&conn, task3.id, task1.id).unwrap();

        let dependents = get_dependents(&conn, task1.id).unwrap();
        assert_eq!(dependents.len(), 2);
    }

    #[test]
    fn test_get_incomplete_dependencies() {
        let conn = setup();
        let task1 = create_task(&conn, "Task 1", "", "", 10.0).unwrap();
        let task2 = create_task(&conn, "Task 2", "", "", 20.0).unwrap();
        let task3 = create_task(&conn, "Task 3", "", "", 30.0).unwrap();

        // task3 depends on task1 and task2
        add_dependency(&conn, task3.id, task1.id).unwrap();
        add_dependency(&conn, task3.id, task2.id).unwrap();

        // Both are incomplete (pending)
        let incomplete = get_incomplete_dependencies(&conn, task3.id).unwrap();
        assert_eq!(incomplete.len(), 2);
        assert!(incomplete.contains(&task1.id));
        assert!(incomplete.contains(&task2.id));
    }

    #[test]
    fn test_get_all_dependencies() {
        let conn = setup();
        let task1 = create_task(&conn, "Task 1", "", "", 10.0).unwrap();
        let task2 = create_task(&conn, "Task 2", "", "", 20.0).unwrap();

        add_dependency(&conn, task2.id, task1.id).unwrap();

        let all = get_all_dependencies(&conn).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].task_id, task2.id);
        assert_eq!(all[0].depends_on, task1.id);
    }

    #[test]
    fn test_has_dependencies() {
        let conn = setup();
        let task1 = create_task(&conn, "Task 1", "", "", 10.0).unwrap();
        let task2 = create_task(&conn, "Task 2", "", "", 20.0).unwrap();

        assert!(!has_dependencies(&conn, task2.id).unwrap());

        add_dependency(&conn, task2.id, task1.id).unwrap();

        assert!(has_dependencies(&conn, task2.id).unwrap());
    }
}
