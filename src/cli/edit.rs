//! Edit a task

use crate::core::edit_task;
use crate::error::Result;
use rusqlite::Connection;

/// Edit a task
pub fn run(
    conn: &Connection,
    id: i64,
    title: Option<&str>,
    description: Option<&str>,
    dod: Option<&str>,
) -> Result<()> {
    let task = edit_task(conn, id, title, description, dod)?;

    println!("Updated task #{}: {}", task.id, task.title);

    Ok(())
}
