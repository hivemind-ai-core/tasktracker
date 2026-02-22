//! Restore command - imports database from TOML file

use crate::db::config;
use crate::error::Result;
use rusqlite::Connection;
use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Deserialize)]
struct DumpData {
    tasks: Vec<TaskDump>,
    dependencies: Vec<DependencyDump>,
    artifacts: Vec<ArtifactDump>,
    config: Option<ConfigDump>,
}

#[derive(Deserialize)]
struct TaskDump {
    id: i64,
    title: String,
    description: Option<String>,
    dod: Option<String>,
    status: String,
    manual_order: f64,
    created_at: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    last_touched_at: String,
}

#[derive(Deserialize)]
struct DependencyDump {
    task_id: i64,
    depends_on: i64,
}

#[derive(Deserialize)]
struct ArtifactDump {
    id: i64,
    task_id: i64,
    name: String,
    file_path: String,
    created_at: String,
}

#[derive(Deserialize)]
struct ConfigDump {
    target_id: Option<i64>,
}

pub fn run(conn: &Connection, path: &Path) -> Result<()> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let dump_data: DumpData = toml::from_str(&contents)
        .map_err(|e| crate::error::Error::InvalidArgument(format!("Invalid TOML file: {}", e)))?;

    let mut task_count = 0;
    for task_dump in &dump_data.tasks {
        conn.execute(
            "INSERT OR REPLACE INTO tasks (id, title, description, dod, status, manual_order, created_at, started_at, completed_at, last_touched_at, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0)",
            rusqlite::params![
                task_dump.id,
                task_dump.title,
                task_dump.description,
                task_dump.dod,
                task_dump.status,
                task_dump.manual_order,
                task_dump.created_at,
                task_dump.started_at,
                task_dump.completed_at,
                task_dump.last_touched_at,
            ],
        )?;
        task_count += 1;
    }

    conn.execute("DELETE FROM dependencies", [])?;
    for dep_dump in &dump_data.dependencies {
        let _ = conn.execute(
            "INSERT OR IGNORE INTO dependencies (task_id, depends_on) VALUES (?1, ?2)",
            rusqlite::params![dep_dump.task_id, dep_dump.depends_on],
        );
    }

    conn.execute("DELETE FROM artifacts", [])?;
    for artifact_dump in &dump_data.artifacts {
        conn.execute(
            "INSERT INTO artifacts (id, task_id, name, file_path, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                artifact_dump.id,
                artifact_dump.task_id,
                artifact_dump.name,
                artifact_dump.file_path,
                artifact_dump.created_at,
            ],
        )?;
    }

    if let Some(config_dump) = &dump_data.config {
        if let Some(target_id) = config_dump.target_id {
            config::set_target(conn, target_id)?;
        } else {
            let _ = config::clear_target(conn);
        }
    }

    println!(
        "Restored {} tasks, {} dependencies, {} artifacts from {}",
        task_count,
        dump_data.dependencies.len(),
        dump_data.artifacts.len(),
        path.display()
    );

    Ok(())
}
