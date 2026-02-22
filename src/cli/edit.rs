//! Edit a task

use crate::core::edit_task;
use crate::core::models::{EditAction, TaskStatus};
use crate::core::{add_dependencies, remove_dependency, reorder_task};
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
    depends_on: Option<Vec<i64>>,
    remove_depends_on: Option<Vec<i64>>,
    after: Option<i64>,
    before: Option<i64>,
) -> Result<()> {
    // Handle dependencies
    if let Some(deps) = depends_on {
        if !deps.is_empty() {
            add_dependencies(conn, id, deps)?;
            println!("Added dependencies to #{}", id);
        }
    }

    if let Some(deps) = remove_depends_on {
        for dep in deps {
            let _ = remove_dependency(conn, id, dep);
        }
        println!("Removed dependencies from #{}", id);
    }

    // Handle reordering
    if after.is_some() || before.is_some() {
        reorder_task(conn, id, after, before)?;
        println!("Reordered #{}", id);
    }

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
