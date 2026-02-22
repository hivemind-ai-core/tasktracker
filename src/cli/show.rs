//! Show task details

use crate::core::{get_task_detail_allow_archived, get_tasks_allow_archived};
use crate::error::Result;
use rusqlite::Connection;

/// Show task details for one or more tasks
pub fn run(conn: &Connection, ids: Vec<i64>) -> Result<()> {
    if ids.len() == 1 {
        // Single task - show full details (allows archived)
        show_single(conn, ids[0])
    } else {
        // Multiple tasks - show summary (allows archived)
        show_multiple(conn, ids)
    }
}

/// Show full details for a single task (includes archived)
fn show_single(conn: &Connection, id: i64) -> Result<()> {
    let detail = get_task_detail_allow_archived(conn, id)?;
    let task = &detail.task;

    println!("[#{}/{}] {}", task.id, task.manual_order, task.title);
    println!("Status:       {}", task.status);

    if let Some(ref desc) = task.description {
        println!("Description:  {desc}");
    }

    if let Some(ref dod) = task.dod {
        println!("DoD:          {dod}");
    }

    println!("Created:      {}", task.created_at);

    if !detail.dependencies.is_empty() {
        let deps_str: Vec<String> = detail
            .dependencies
            .iter()
            .map(|t| format!("#{} ({})", t.id, t.status.display_char()))
            .collect();
        println!("Dependencies: {}", deps_str.join(", "));
    }

    if !detail.dependents.is_empty() {
        let deps_str: Vec<String> = detail
            .dependents
            .iter()
            .map(|t| format!("#{}", t.id))
            .collect();
        println!("Dependents:   {}", deps_str.join(", "));
    }

    if !detail.artifacts.is_empty() {
        println!("Artifacts:");
        for artifact in &detail.artifacts {
            println!("  - {}: {}", artifact.name, artifact.file_path);
        }
    } else {
        println!("Artifacts:    (none)");
    }

    Ok(())
}

/// Show summary for multiple tasks (includes archived)
fn show_multiple(conn: &Connection, ids: Vec<i64>) -> Result<()> {
    let tasks = get_tasks_allow_archived(conn, ids)?;

    for task in tasks {
        let status_char = task.status.display_char();
        println!(
            "[#{}/{}] {} {}",
            task.id, task.manual_order, status_char, task.title
        );
    }

    Ok(())
}
