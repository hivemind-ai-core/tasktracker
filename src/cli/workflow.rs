//! Workflow commands (start, stop, done, block, unblock, current)

use crate::core::{
    block_task, complete_task, get_current_task, get_task_artifacts, start_task, stop_task,
    unblock_task,
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

/// Block a task
pub fn run_block(conn: &Connection, id: i64) -> Result<()> {
    let task = block_task(conn, id)?;

    println!("Blocked: [#{}] {}", task.id, task.title);

    Ok(())
}

/// Unblock a task
pub fn run_unblock(conn: &Connection, id: i64) -> Result<()> {
    let task = unblock_task(conn, id)?;

    println!("Unblocked: [#{}] {}", task.id, task.title);

    Ok(())
}

/// Show current active task
pub fn run_current(conn: &Connection) -> Result<()> {
    let task = get_current_task(conn)?;

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
