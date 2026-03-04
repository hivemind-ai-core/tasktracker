//! Generate Mermaid graph visualization of tasks

use crate::core::models::{Dependency, Task};
use crate::error::Result;
use std::io::Write;

pub fn run<W: Write>(tasks: &[Task], dependencies: &[Dependency], output: &mut W) -> Result<()> {
    writeln!(output, "flowchart TD")?;

    for task in tasks {
        let title = task.title.replace('"', "\\\"");
        writeln!(output, "    t{}[\"{}\"]", task.id, title)?;
    }

    for dep in dependencies {
        writeln!(output, "    t{} --> t{}", dep.depends_on, dep.task_id)?;
    }

    Ok(())
}
