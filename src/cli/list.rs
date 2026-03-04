//! List tasks

use crate::cli::graph;
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
    active: bool,
    ids: Option<Vec<i64>>,
    limit: Option<usize>,
    offset: Option<usize>,
    graph: bool,
) -> Result<()> {
    // Validate mutual exclusivity of --status and --active
    if status.is_some() && active {
        return Err(Error::InvalidArgument(
            "Cannot use both --status and --active flags together. Use --active to show pending and in_progress tasks.".to_string()
        ));
    }

    // Parse status filter if provided
    let status_filter = match status {
        Some(s) => Some(parse_status(&s)?),
        None => None,
    };

    // Determine archived filter
    let archived_filter = if archived { Some(true) } else { None };

    let mut tasks = list_tasks(
        conn,
        all,
        status_filter,
        active,
        limit,
        offset,
        archived_filter,
    )?;

    // Filter by ids if provided
    if let Some(ref filter_ids) = ids {
        tasks.retain(|t| filter_ids.contains(&t.id));
    }

    // Handle graph output
    if graph {
        let deps = crate::db::dependencies::get_all_dependencies(conn)?;
        graph::run(&tasks, &deps, &mut std::io::stdout())?;
        return Ok(());
    }

    // Show header
    if archived {
        println!("Archived tasks:");
    } else if !all && status_filter.is_none() && !active && ids.is_none() {
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
    } else if active {
        println!("Active tasks (pending or in_progress):");
    } else if ids.is_some() {
        println!("Filtered by IDs");
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
            "Legend: {} completed  {} in_progress  {} pending  {} blocked  {} cancelled",
            TaskStatus::Completed.display_char(),
            TaskStatus::InProgress.display_char(),
            TaskStatus::Pending.display_char(),
            TaskStatus::Blocked.display_char(),
            TaskStatus::Cancelled.display_char(),
        );
    }

    Ok(())
}
