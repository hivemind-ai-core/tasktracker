//! Add a new task

use crate::core::create_task;
use crate::error::Result;
use rusqlite::Connection;

/// Add a new task
pub fn run(
    conn: &Connection,
    title: &str,
    description: &str,
    dod: &str,
    after: Option<i64>,
    before: Option<i64>,
    depends_on: Vec<i64>,
) -> Result<()> {
    // Convert Vec<i64> to Option<Vec<i64>>
    let deps_opt = if depends_on.is_empty() {
        None
    } else {
        Some(depends_on)
    };

    let task = create_task(conn, title, description, dod, after, before, deps_opt)?;

    println!("Created task #{}: {}", task.id, task.title);

    Ok(())
}
