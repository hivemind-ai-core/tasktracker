//! List tasks

use crate::core::{get_target, list_tasks, TaskStatus};
use crate::error::{Error, Result};
use rusqlite::Connection;

/// Parse status string to TaskStatus
fn parse_status(status: &str) -> Result<TaskStatus> {
    match status.to_lowercase().as_str() {
        "pending" => Ok(TaskStatus::Pending),
        "in_progress" => Ok(TaskStatus::InProgress),
        "completed" => Ok(TaskStatus::Completed),
        "blocked" => Ok(TaskStatus::Blocked),
        _ => Err(Error::InvalidStatus(status.to_string())),
    }
}

/// List tasks
pub fn run(
    conn: &Connection,
    all: bool,
    archived: bool,
    status: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<()> {
    // Parse status filter if provided
    let status_filter = match status {
        Some(s) => Some(parse_status(&s)?),
        None => None,
    };

    // Determine archived filter
    let archived_filter = if archived { Some(true) } else { None };

    let tasks = list_tasks(conn, all, status_filter, limit, offset, archived_filter)?;

    // Show header
    if archived {
        println!("Archived tasks:");
    } else if !all && status_filter.is_none() {
        // Show focus header if set
        match get_target(conn)? {
            Some(target) => {
                println!("Focus: #{} ({})", target.id, target.title);
            }
            None => {
                println!("No focus set (showing all tasks)");
            }
        }
    } else if let Some(s) = status_filter {
        println!("Tasks with status: {}", s);
    }

    for task in &tasks {
        let status_char = task.status.display_char();
        let deps = crate::db::dependencies::get_dependencies(conn, task.id)?;

        if deps.is_empty() {
            println!("  [#{:>3}] {} {}", task.id, status_char, task.title);
        } else {
            let dep_strs: Vec<String> = deps
                .iter()
                .map(|d| {
                    let dep_task = crate::db::tasks::get_task(conn, d.depends_on, false);
                    match dep_task {
                        Ok(Some(t)) => format!("#{} {}", t.id, t.status.display_char()),
                        _ => format!("#{}", d.depends_on),
                    }
                })
                .collect();
            println!(
                "  [#{:>3}] {} {:<25} (deps: {})",
                task.id,
                status_char,
                task.title,
                dep_strs.join(", ")
            );
        }
    }

    if !tasks.is_empty() || all {
        println!();
        println!(
            "Legend: {} completed  {} in_progress  {} pending  {} blocked",
            TaskStatus::Completed.display_char(),
            TaskStatus::InProgress.display_char(),
            TaskStatus::Pending.display_char(),
            TaskStatus::Blocked.display_char(),
        );
    }

    Ok(())
}
