//! Dump command - exports database to TOML file

use crate::db::{artifacts, config, dependencies, tasks};
use crate::error::Result;
use rusqlite::Connection;
use serde::Serialize;
use std::fs::File;
use std::io::Write;
use std::path::Path;

#[derive(Serialize)]
struct DumpData {
    tasks: Vec<TaskDump>,
    dependencies: Vec<DependencyDump>,
    artifacts: Vec<ArtifactDump>,
    config: Option<ConfigDump>,
}

#[derive(Serialize)]
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

#[derive(Serialize)]
struct DependencyDump {
    task_id: i64,
    depends_on: i64,
}

#[derive(Serialize)]
struct ArtifactDump {
    id: i64,
    task_id: i64,
    name: String,
    file_path: String,
    created_at: String,
}

#[derive(Serialize)]
struct ConfigDump {
    target_id: Option<i64>,
}

pub fn run(conn: &Connection, path: &Path) -> Result<()> {
    let all_tasks = tasks::get_all_tasks(conn)?;

    let task_dumps: Vec<TaskDump> = all_tasks
        .into_iter()
        .map(|t| TaskDump {
            id: t.id,
            title: t.title,
            description: t.description,
            dod: t.dod,
            status: t.status.to_db().to_string(),
            manual_order: t.manual_order,
            created_at: t.created_at,
            started_at: t.started_at,
            completed_at: t.completed_at,
            last_touched_at: t.last_touched_at,
        })
        .collect();

    let all_deps = dependencies::get_all_dependencies(conn)?;
    let dep_dumps: Vec<DependencyDump> = all_deps
        .into_iter()
        .map(|d| DependencyDump {
            task_id: d.task_id,
            depends_on: d.depends_on,
        })
        .collect();

    let mut artifact_dumps = Vec::new();
    for task in &task_dumps {
        let task_artifacts = artifacts::get_artifacts_for_task(conn, task.id)?;
        for a in task_artifacts {
            artifact_dumps.push(ArtifactDump {
                id: a.id,
                task_id: a.task_id,
                name: a.name,
                file_path: a.file_path,
                created_at: a.created_at,
            });
        }
    }

    let target_id = config::get_target(conn)?;
    let config_dump = ConfigDump { target_id };

    let dump_data = DumpData {
        tasks: task_dumps,
        dependencies: dep_dumps,
        artifacts: artifact_dumps,
        config: Some(config_dump),
    };

    let toml_string = toml::to_string_pretty(&dump_data).map_err(|e| {
        crate::error::Error::InvalidArgument(format!("TOML serialization error: {}", e))
    })?;

    let mut file = File::create(path)?;
    file.write_all(toml_string.as_bytes())?;

    println!(
        "Dumped {} tasks to {}",
        dump_data.tasks.len(),
        path.display()
    );

    Ok(())
}
