//! List tasks

use crate::core::{get_target, list_tasks, TaskStatus};
use crate::error::{Error, Result};
use rusqlite::Connection;

/// List tasks
pub fn run(conn: &Connection, all: bool) -> Result<()> {
    let tasks = list_tasks(conn, all)?;

    if !all {
        // Show target header
        match get_target(conn)? {
            Some(target) => {
                println!("Target: #{} ({})", target.id, target.title);
            }
            None => {
                return Err(Error::NoTarget);
            }
        }
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
                    let dep_task = crate::db::tasks::get_task(conn, d.depends_on);
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
