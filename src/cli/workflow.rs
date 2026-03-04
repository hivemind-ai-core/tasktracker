//! Workflow commands (start, stop, done, advance, block, unblock, current)

use crate::core::{
    advance_task, block_task, block_tasks, complete_task, get_current_task, get_task_artifacts,
    start_task, stop_task, unblock_task, unblock_tasks,
};
use crate::error::Result;
use rusqlite::Connection;

/// Start a task
pub fn run_start(conn: &Connection, id: Option<i64>) -> Result<()> {
    let task = match id {
        Some(task_id) => start_task(conn, task_id)?,
        None => {
            // Start the next task
            match crate::core::get_next_task(conn)? {
                Some(next) => start_task(conn, next.id)?,
                None => {
                    println!("No task to start.");
                    return Ok(());
                }
            }
        }
    };

    println!("Started: [#{}] {}", task.id, task.title);

    Ok(())
}

/// Stop the active task
pub fn run_stop(conn: &Connection) -> Result<()> {
    let task = stop_task(conn)?;

    println!("Stopped: [#{}] {}", task.id, task.title);

    Ok(())
}

/// Complete the active task
pub fn run_done(conn: &Connection) -> Result<()> {
    let task = complete_task(conn)?;

    println!("Completed: [#{}] {}", task.id, task.title);

    Ok(())
}

/// Block one or more tasks
pub fn run_block(conn: &Connection, ids: Vec<i64>) -> Result<()> {
    if ids.len() == 1 {
        // Single task
        let task = block_task(conn, ids[0])?;
        println!("Blocked: [#{}] {}", task.id, task.title);
    } else {
        // Bulk block
        let results = block_tasks(conn, ids);
        for result in results {
            match result {
                Ok(task) => println!("Blocked: [#{}] {}", task.id, task.title),
                Err(e) => println!("Error: {}", e),
            }
        }
    }

    Ok(())
}

/// Unblock one or more tasks
pub fn run_unblock(conn: &Connection, ids: Vec<i64>) -> Result<()> {
    if ids.len() == 1 {
        // Single task
        let task = unblock_task(conn, ids[0])?;
        println!("Unblocked: [#{}] {}", task.id, task.title);
    } else {
        // Bulk unblock
        let results = unblock_tasks(conn, ids);
        for result in results {
            match result {
                Ok(task) => println!("Unblocked: [#{}] {}", task.id, task.title),
                Err(e) => println!("Error: {}", e),
            }
        }
    }

    Ok(())
}

/// Show current active task
pub fn run_current(conn: &Connection) -> Result<()> {
    let task = get_current_task(conn)?;

    // Check if focus is set and if active task is outside the focused subgraph
    if let Ok(Some(target_id)) = crate::db::config::get_target(conn) {
        let all_tasks = crate::db::tasks::get_all_tasks(conn)?;
        let all_deps = crate::db::dependencies::get_all_dependencies(conn)?;
        let focused_subgraph =
            crate::core::compute_target_subgraph(target_id, &all_tasks, &all_deps);

        let in_focus = focused_subgraph.iter().any(|t| t.id == task.id);
        if !in_focus {
            println!("WARNING: Active task is outside the focused subgraph.");
        }
    }

    println!("Active: [#{}] {}", task.id, task.title);
    println!("  Status:    {}", task.status);

    if let Some(ref started) = task.started_at {
        println!("  Started:   {started}");
    }

    if let Some(ref dod) = task.dod {
        println!("  DoD:       {dod}");
    }

    let artifacts = get_task_artifacts(conn, Some(task.id))?;
    if !artifacts.is_empty() {
        println!("  Artifacts:");
        for artifact in artifacts {
            println!("    - {}: {}", artifact.name, artifact.file_path);
        }
    }

    Ok(())
}

/// Advance workflow: complete current task and start the next one
pub fn run_advance(conn: &Connection, dry_run: bool) -> Result<()> {
    let result = advance_task(conn, dry_run)?;

    if dry_run {
        println!("Dry run:");
        match result.completed {
            Some(task) => println!("  Would complete: [#{}] {}", task.id, task.title),
            None => println!("  No current task to complete"),
        }
        match result.started {
            Some(task) => println!("  Would start: [#{}] {}", task.id, task.title),
            None => println!("  No next task to start"),
        }
    } else {
        match result.completed {
            Some(task) => println!("Completed: [#{}] {}", task.id, task.title),
            None => println!("No current task to complete"),
        }
        match result.started {
            Some(task) => {
                println!("Started: [#{}] {}", task.id, task.title);
            }
            None => {
                // Check if it's because there are no tasks or all blocked
                match crate::core::find_next_runnable(conn)? {
                    Some(_) => println!("No next task available"),
                    None => {
                        // Check if all remaining are blocked
                        let all_tasks = crate::db::tasks::get_all_tasks(conn)?;
                        let incomplete: Vec<_> = all_tasks
                            .iter()
                            .filter(|t| t.status != crate::core::TaskStatus::Completed)
                            .collect();
                        if incomplete.is_empty() {
                            println!("All tasks completed!");
                        } else {
                            let blocked_count = incomplete
                                .iter()
                                .filter(|t| t.status == crate::core::TaskStatus::Blocked)
                                .count();
                            if blocked_count == incomplete.len() {
                                println!("All remaining tasks are blocked.");
                            } else {
                                println!("No runnable tasks available.");
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
