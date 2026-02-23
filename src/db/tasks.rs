//! Task CRUD operations

use crate::core::models::{Task, TaskStatus};
use crate::error::{Error, Result};
use rusqlite::{params, Connection};

/// Create a new task
pub fn create_task(
    conn: &Connection,
    title: &str,
    description: &str,
    dod: &str,
    manual_order: f64,
) -> Result<Task> {
    conn.execute(
        "INSERT INTO tasks (title, description, dod, status, manual_order) 
         VALUES (?1, ?2, ?3, 'pending', ?4)",
        params![title, description, dod, manual_order],
    )?;

    let id = conn.last_insert_rowid();
    get_task(conn, id, false)?.ok_or(Error::TaskNotFound(id))
}

/// Get a task by ID
/// If include_archived is false (default), excludes soft-deleted tasks
/// If include_archived is true, includes archived tasks
pub fn get_task(conn: &Connection, id: i64, include_archived: bool) -> Result<Option<Task>> {
    let sql = if include_archived {
        "SELECT id, title, description, dod, status, manual_order, 
                created_at, started_at, completed_at, last_touched_at, deleted 
         FROM tasks WHERE id = ?1"
    } else {
        "SELECT id, title, description, dod, status, manual_order, 
                created_at, started_at, completed_at, last_touched_at, deleted 
         FROM tasks WHERE id = ?1 AND deleted = 0"
    };

    let mut stmt = conn.prepare(sql)?;

    let mut rows = stmt.query(params![id])?;

    if let Some(row) = rows.next()? {
        let task = row_to_task(row)?;
        Ok(Some(task))
    } else {
        Ok(None)
    }
}

/// Get all tasks (excludes soft-deleted tasks)
pub fn get_all_tasks(conn: &Connection) -> Result<Vec<Task>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, description, dod, status, manual_order, 
                created_at, started_at, completed_at, last_touched_at, deleted 
         FROM tasks WHERE deleted = 0 ORDER BY manual_order",
    )?;

    let rows = stmt.query_map([], row_to_task)?;

    let mut tasks = Vec::new();
    for task in rows {
        tasks.push(task?);
    }

    Ok(tasks)
}

/// Get tasks by status with optional limit and offset (excludes soft-deleted tasks)
pub fn get_tasks_by_status(
    conn: &Connection,
    status: TaskStatus,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<Task>> {
    let sql = format!(
        "SELECT id, title, description, dod, status, manual_order, 
                created_at, started_at, completed_at, last_touched_at, deleted 
         FROM tasks WHERE status = ?1 AND deleted = 0 ORDER BY manual_order{}{}",
        limit.map(|l| format!(" LIMIT {}", l)).unwrap_or_default(),
        offset.map(|o| format!(" OFFSET {}", o)).unwrap_or_default()
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![status.to_db()], row_to_task)?;

    let mut tasks = Vec::new();
    for task in rows {
        tasks.push(task?);
    }

    Ok(tasks)
}

/// Get active tasks (pending or in_progress) with optional limit and offset (excludes soft-deleted tasks)
pub fn get_active_tasks(
    conn: &Connection,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<Task>> {
    let sql = format!(
        "SELECT id, title, description, dod, status, manual_order, 
                created_at, started_at, completed_at, last_touched_at, deleted 
         FROM tasks WHERE status IN ('pending', 'in_progress') AND deleted = 0 ORDER BY manual_order{}{}",
        limit.map(|l| format!(" LIMIT {}", l)).unwrap_or_default(),
        offset.map(|o| format!(" OFFSET {}", o)).unwrap_or_default()
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_task)?;

    let mut tasks = Vec::new();
    for task in rows {
        tasks.push(task?);
    }

    Ok(tasks)
}

/// Get all tasks with optional limit and offset (excludes soft-deleted tasks)
pub fn get_all_tasks_paginated(
    conn: &Connection,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<Task>> {
    let sql = format!(
        "SELECT id, title, description, dod, status, manual_order, 
                created_at, started_at, completed_at, last_touched_at, deleted 
         FROM tasks WHERE deleted = 0 ORDER BY manual_order{}{}",
        limit.map(|l| format!(" LIMIT {}", l)).unwrap_or_default(),
        offset.map(|o| format!(" OFFSET {}", o)).unwrap_or_default()
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_task)?;

    let mut tasks = Vec::new();
    for task in rows {
        tasks.push(task?);
    }

    Ok(tasks)
}

/// Get the currently active task (in_progress) (excludes soft-deleted tasks)
pub fn get_active_task(conn: &Connection) -> Result<Option<Task>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, description, dod, status, manual_order, 
                created_at, started_at, completed_at, last_touched_at, deleted 
         FROM tasks WHERE status = 'in_progress' AND deleted = 0 LIMIT 1",
    )?;

    let mut rows = stmt.query([])?;

    if let Some(row) = rows.next()? {
        let task = row_to_task(row)?;
        Ok(Some(task))
    } else {
        Ok(None)
    }
}

/// Update task title
pub fn update_task_title(conn: &Connection, id: i64, title: &str) -> Result<()> {
    let affected = conn.execute(
        "UPDATE tasks SET title = ?1 WHERE id = ?2",
        params![title, id],
    )?;

    if affected == 0 {
        return Err(Error::TaskNotFound(id));
    }

    Ok(())
}

/// Update task description
pub fn update_task_description(
    conn: &Connection,
    id: i64,
    description: Option<&str>,
) -> Result<()> {
    let affected = conn.execute(
        "UPDATE tasks SET description = ?1 WHERE id = ?2",
        params![description, id],
    )?;

    if affected == 0 {
        return Err(Error::TaskNotFound(id));
    }

    Ok(())
}

/// Update task DoD
pub fn update_task_dod(conn: &Connection, id: i64, dod: Option<&str>) -> Result<()> {
    let affected = conn.execute("UPDATE tasks SET dod = ?1 WHERE id = ?2", params![dod, id])?;

    if affected == 0 {
        return Err(Error::TaskNotFound(id));
    }

    Ok(())
}

/// Update task status
pub fn update_task_status(conn: &Connection, id: i64, status: TaskStatus) -> Result<()> {
    let affected = conn.execute(
        "UPDATE tasks SET status = ?1 WHERE id = ?2",
        params![status.to_db(), id],
    )?;

    if affected == 0 {
        return Err(Error::TaskNotFound(id));
    }

    Ok(())
}

/// Update task manual_order
pub fn update_task_order(conn: &Connection, id: i64, manual_order: f64) -> Result<()> {
    let affected = conn.execute(
        "UPDATE tasks SET manual_order = ?1 WHERE id = ?2",
        params![manual_order, id],
    )?;

    if affected == 0 {
        return Err(Error::TaskNotFound(id));
    }

    Ok(())
}

/// Start a task: set status to in_progress and started_at
pub fn start_task(conn: &Connection, id: i64) -> Result<()> {
    let affected = conn.execute(
        "UPDATE tasks SET status = 'in_progress', started_at = strftime('%Y-%m-%dT%H:%M:%S', 'now') 
         WHERE id = ?1",
        params![id],
    )?;

    if affected == 0 {
        return Err(Error::TaskNotFound(id));
    }

    Ok(())
}

/// Stop a task: set status to pending (does not clear started_at per SPEC)
pub fn stop_task(conn: &Connection, id: i64) -> Result<()> {
    let affected = conn.execute(
        "UPDATE tasks SET status = 'pending' WHERE id = ?1",
        params![id],
    )?;

    if affected == 0 {
        return Err(Error::TaskNotFound(id));
    }

    Ok(())
}

/// Complete a task: set status to completed and completed_at
pub fn complete_task(conn: &Connection, id: i64) -> Result<()> {
    let affected = conn.execute(
        "UPDATE tasks SET status = 'completed', completed_at = strftime('%Y-%m-%dT%H:%M:%S', 'now') 
         WHERE id = ?1",
        params![id],
    )?;

    if affected == 0 {
        return Err(Error::TaskNotFound(id));
    }

    Ok(())
}

/// Cancel a task: set status to cancelled
pub fn cancel_task(conn: &Connection, id: i64) -> Result<()> {
    let affected = conn.execute(
        "UPDATE tasks SET status = 'cancelled' WHERE id = ?1",
        params![id],
    )?;

    if affected == 0 {
        return Err(Error::TaskNotFound(id));
    }

    Ok(())
}

/// Block a task: set status to blocked
pub fn block_task(conn: &Connection, id: i64) -> Result<()> {
    let affected = conn.execute(
        "UPDATE tasks SET status = 'blocked' WHERE id = ?1",
        params![id],
    )?;

    if affected == 0 {
        return Err(Error::TaskNotFound(id));
    }

    Ok(())
}

/// Unblock a task: set status to pending
pub fn unblock_task(conn: &Connection, id: i64) -> Result<()> {
    let affected = conn.execute(
        "UPDATE tasks SET status = 'pending' WHERE id = ?1",
        params![id],
    )?;

    if affected == 0 {
        return Err(Error::TaskNotFound(id));
    }

    Ok(())
}

/// Helper function to convert a database row to a Task
fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<Task> {
    let status_str: String = row.get(4)?;
    let status = TaskStatus::from_db(&status_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            4,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )),
        )
    })?;

    let deleted: i32 = row.get(10)?;

    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        dod: row.get(3)?,
        status,
        manual_order: row.get(5)?,
        created_at: row.get(6)?,
        started_at: row.get(7)?,
        completed_at: row.get(8)?,
        last_touched_at: row.get(9)?,
        deleted: deleted != 0,
    })
}

/// Soft delete a task by setting deleted = 1
pub fn soft_delete_task(conn: &Connection, id: i64) -> Result<()> {
    let affected = conn.execute("UPDATE tasks SET deleted = 1 WHERE id = ?1", params![id])?;

    if affected == 0 {
        return Err(Error::TaskNotFound(id));
    }

    Ok(())
}

/// Archive all completed and cancelled tasks (set deleted = 1)
pub fn archive_completed_tasks(conn: &Connection) -> Result<usize> {
    let affected = conn.execute(
        "UPDATE tasks SET deleted = 1 WHERE status IN ('completed', 'cancelled')",
        [],
    )?;
    Ok(affected)
}

/// Get archived tasks (deleted = 1) with optional pagination
pub fn get_archived_tasks(
    conn: &Connection,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<Task>> {
    let mut query = "SELECT id, title, description, dod, status, manual_order, 
                     created_at, started_at, completed_at, last_touched_at, deleted 
              FROM tasks WHERE deleted = 1 ORDER BY manual_order"
        .to_string();

    if let Some(lim) = limit {
        query.push_str(&format!(" LIMIT {}", lim));
    }
    if let Some(off) = offset {
        query.push_str(&format!(" OFFSET {}", off));
    }

    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query_map([], row_to_task)?;

    let mut tasks = Vec::new();
    for task in rows {
        tasks.push(task?);
    }

    Ok(tasks)
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
    fn test_create_task() {
        let conn = setup();
        let task = create_task(&conn, "Test Task", "", "", 10.0).unwrap();

        assert_eq!(task.title, "Test Task");
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.manual_order, 10.0);
        assert!(task.id > 0);
    }

    #[test]
    fn test_create_task_with_description() {
        let conn = setup();
        let task = create_task(&conn, "Test", "Description", "DoD", 10.0).unwrap();

        assert_eq!(task.description, Some("Description".to_string()));
        assert_eq!(task.dod, Some("DoD".to_string()));
    }

    #[test]
    fn test_get_task() {
        let conn = setup();
        let created = create_task(&conn, "Test", "", "", 10.0).unwrap();

        let fetched = get_task(&conn, created.id, false).unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().title, "Test");
    }

    #[test]
    fn test_get_task_not_found() {
        let conn = setup();
        let result = get_task(&conn, 999, false).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_all_tasks() {
        let conn = setup();
        create_task(&conn, "Task A", "", "", 20.0).unwrap();
        create_task(&conn, "Task B", "", "", 10.0).unwrap();

        let tasks = get_all_tasks(&conn).unwrap();
        assert_eq!(tasks.len(), 2);
        // Should be ordered by manual_order
        assert_eq!(tasks[0].title, "Task B");
        assert_eq!(tasks[1].title, "Task A");
    }

    #[test]
    fn test_get_tasks_by_status() {
        let conn = setup();
        let task = create_task(&conn, "Test", "", "", 10.0).unwrap();
        start_task(&conn, task.id).unwrap();

        let active = get_tasks_by_status(&conn, TaskStatus::InProgress, None, None).unwrap();
        assert_eq!(active.len(), 1);

        let pending = get_tasks_by_status(&conn, TaskStatus::Pending, None, None).unwrap();
        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn test_get_active_task() {
        let conn = setup();
        assert!(get_active_task(&conn).unwrap().is_none());

        let task = create_task(&conn, "Test", "", "", 10.0).unwrap();
        start_task(&conn, task.id).unwrap();

        let active = get_active_task(&conn).unwrap();
        assert!(active.is_some());
        assert_eq!(active.unwrap().id, task.id);
    }

    #[test]
    fn test_update_task_title() {
        let conn = setup();
        let task = create_task(&conn, "Old", "", "", 10.0).unwrap();

        update_task_title(&conn, task.id, "New").unwrap();

        let updated = get_task(&conn, task.id, false).unwrap().unwrap();
        assert_eq!(updated.title, "New");
    }

    #[test]
    fn test_update_task_not_found() {
        let conn = setup();
        let result = update_task_title(&conn, 999, "New");
        assert!(matches!(result.unwrap_err(), Error::TaskNotFound(999)));
    }

    #[test]
    fn test_start_stop_complete_task() {
        let conn = setup();
        let task = create_task(&conn, "Test", "", "", 10.0).unwrap();

        // Start
        start_task(&conn, task.id).unwrap();
        let started = get_task(&conn, task.id, false).unwrap().unwrap();
        assert_eq!(started.status, TaskStatus::InProgress);
        assert!(started.started_at.is_some());

        // Stop
        stop_task(&conn, task.id).unwrap();
        let stopped = get_task(&conn, task.id, false).unwrap().unwrap();
        assert_eq!(stopped.status, TaskStatus::Pending);
        // started_at is NOT cleared per SPEC
        assert!(stopped.started_at.is_some());

        // Restart and complete
        start_task(&conn, task.id).unwrap();
        complete_task(&conn, task.id).unwrap();
        let completed = get_task(&conn, task.id, false).unwrap().unwrap();
        assert_eq!(completed.status, TaskStatus::Completed);
        assert!(completed.completed_at.is_some());
    }

    #[test]
    fn test_block_unblock_task() {
        let conn = setup();
        let task = create_task(&conn, "Test", "", "", 10.0).unwrap();

        block_task(&conn, task.id).unwrap();
        let blocked = get_task(&conn, task.id, false).unwrap().unwrap();
        assert_eq!(blocked.status, TaskStatus::Blocked);

        unblock_task(&conn, task.id).unwrap();
        let unblocked = get_task(&conn, task.id, false).unwrap().unwrap();
        assert_eq!(unblocked.status, TaskStatus::Pending);
    }

    #[test]
    fn test_update_task_order() {
        let conn = setup();
        let task = create_task(&conn, "Test", "", "", 10.0).unwrap();

        update_task_order(&conn, task.id, 50.0).unwrap();

        let updated = get_task(&conn, task.id, false).unwrap().unwrap();
        assert_eq!(updated.manual_order, 50.0);
    }

    #[test]
    fn test_archive_completed_tasks() {
        let conn = setup();

        // Create tasks with different statuses
        let pending = create_task(&conn, "Pending", "", "", 10.0).unwrap();
        let completed = create_task(&conn, "Completed", "", "", 20.0).unwrap();
        let another_completed = create_task(&conn, "Another", "", "", 30.0).unwrap();

        // Manually set completed tasks (bypass validation for testing)
        conn.execute(
            "UPDATE tasks SET status = 'completed', completed_at = strftime('%Y-%m-%dT%H:%M:%S', 'now') WHERE id = ?1",
            params![completed.id],
        ).unwrap();
        conn.execute(
            "UPDATE tasks SET status = 'completed', completed_at = strftime('%Y-%m-%dT%H:%M:%S', 'now') WHERE id = ?1",
            params![another_completed.id],
        ).unwrap();

        // Verify initial state
        let all = get_all_tasks(&conn).unwrap();
        assert_eq!(all.len(), 3);

        // Archive completed tasks
        archive_completed_tasks(&conn).unwrap();

        // Verify completed tasks are now archived (deleted=1)
        let archived = get_archived_tasks(&conn, None, None).unwrap();
        assert_eq!(archived.len(), 2);

        // Verify archived tasks are no longer in get_all_tasks
        let remaining = get_all_tasks(&conn).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, pending.id);
    }

    #[test]
    fn test_get_archived_tasks() {
        let conn = setup();

        // Create and manually archive a task
        let task = create_task(&conn, "Test", "", "", 10.0).unwrap();
        soft_delete_task(&conn, task.id).unwrap();

        let archived = get_archived_tasks(&conn, None, None).unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].id, task.id);
    }

    #[test]
    fn test_get_task_by_id_includes_archived() {
        let conn = setup();

        // Create and archive a task
        let task = create_task(&conn, "Test", "", "", 10.0).unwrap();
        soft_delete_task(&conn, task.id).unwrap();

        // get_task with include_archived=true should find the task
        let found = get_task(&conn, task.id, true).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, task.id);

        // get_task with include_archived=false (default) should not find it
        let not_found = get_task(&conn, task.id, false).unwrap();
        assert!(not_found.is_none());
    }
}
