//! Archive completed tasks

use crate::core::archive_completed;
use crate::error::Result;
use rusqlite::Connection;

/// Archive all completed tasks
pub fn run_archive_all(conn: &Connection) -> Result<()> {
    let count = archive_completed(conn)?;
    println!("Archived {} completed task(s)", count);
    Ok(())
}
