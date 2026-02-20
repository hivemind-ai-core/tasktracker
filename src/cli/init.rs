//! Initialize a new tt project

use crate::cli::paths::{get_artifacts_dir, get_db_path, get_tt_dir};
use crate::db::{init_schema, open_db};
use crate::error::Result;
use std::fs;
use std::path::PathBuf;

/// Initialize a new tt project in the current directory
pub fn run() -> Result<()> {
    let db_path = get_db_path(); // Relative path for init (in CWD)
    let tt_dir = get_tt_dir();
    let artifacts_dir = get_artifacts_dir().unwrap_or_else(|| PathBuf::from(".tt/artifacts"));

    if db_path.exists() {
        eprintln!(
            "Error: tt project already initialized ({} exists)",
            db_path.display()
        );
        std::process::exit(1);
    }

    // Create tt directory
    fs::create_dir_all(&tt_dir)?;

    // Create database
    let conn = open_db(&db_path)?;
    init_schema(&conn)?;

    // Create artifacts directory
    fs::create_dir_all(&artifacts_dir)?;

    println!("Initialized tt project:");
    println!("  - Created {}", tt_dir.display());
    println!("  - Created {}", db_path.display());
    println!("  - Created {}", artifacts_dir.display());

    Ok(())
}
