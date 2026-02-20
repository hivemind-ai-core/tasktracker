//! Add a new task

use crate::core::create_task;
use crate::error::Result;
use rusqlite::Connection;

/// Add a new task
pub fn run(
    conn: &Connection,
    title: &str,
    description: Option<&str>,
    dod: Option<&str>,
    after: Option<i64>,
    before: Option<i64>,
) -> Result<()> {
    let task = create_task(conn, title, description, dod, after, before)?;

    println!("Created task #{}: {}", task.id, task.title);

    Ok(())
}
