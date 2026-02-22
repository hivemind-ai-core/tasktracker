//! Edit a task

use crate::core::edit_task;
use crate::core::models::{EditAction, TaskStatus};
use crate::error::{Error, Result};
use rusqlite::Connection;

/// Edit a task
pub fn run(
    conn: &Connection,
    id: i64,
    title: Option<&str>,
    description: Option<&str>,
    dod: Option<&str>,
    status: Option<String>,
    action: Option<String>,
) -> Result<()> {
    // Parse action if provided
    let action_opt = action.map(|a| EditAction::from_str(&a)).transpose()?;

    // Handle status change if requested
    let status_opt = status.map(|s| parse_status(&s)).transpose()?;

    // Pass everything to edit_task
    let task = edit_task(conn, id, title, description, dod, status_opt, action_opt)?;

    println!("Updated task #{}: {}", task.id, task.title);

    Ok(())
}

/// Parse a status string to TaskStatus
fn parse_status(s: &str) -> Result<TaskStatus> {
    match s.to_lowercase().as_str() {
        "pending" => Ok(TaskStatus::Pending),
        "in_progress" => Ok(TaskStatus::InProgress),
        "completed" => Ok(TaskStatus::Completed),
        "blocked" => Ok(TaskStatus::Blocked),
        "cancelled" => Ok(TaskStatus::Cancelled),
        _ => Err(Error::InvalidStatus(s.to_string())),
    }
}
