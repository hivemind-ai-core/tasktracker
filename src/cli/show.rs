//! Show task details

use crate::core::get_task_detail;
use crate::error::Result;
use rusqlite::Connection;

/// Show task details
pub fn run(conn: &Connection, id: i64) -> Result<()> {
    let detail = get_task_detail(conn, id)?;
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
