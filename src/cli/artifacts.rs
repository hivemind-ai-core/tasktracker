//! Artifact commands

use crate::core::{get_task_artifacts, log_artifact};
use crate::error::Result;
use rusqlite::Connection;

/// Log an artifact
pub fn run_log(conn: &Connection, name: &str, file: &str) -> Result<()> {
    let artifact = log_artifact(conn, name, file)?;

    println!(
        "Logged artifact '{}' for task #{}: {}",
        artifact.name, artifact.task_id, artifact.file_path
    );

    Ok(())
}

/// List artifacts
pub fn run_list(conn: &Connection, task_id: Option<i64>) -> Result<()> {
    let artifacts = get_task_artifacts(conn, task_id)?;

    if artifacts.is_empty() {
        println!("No artifacts found.");
    } else {
        for artifact in artifacts {
            println!(
                "  - {}: {} (task #{})",
                artifact.name, artifact.file_path, artifact.task_id
            );
        }
    }

    Ok(())
}
