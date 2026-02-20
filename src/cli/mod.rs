//! CLI command handlers for tt
//!
//! Thin dispatchers that parse arguments and call core operations.

pub mod add;
pub mod artifacts;
pub mod dependencies;
pub mod edit;
pub mod init;
pub mod install;
pub mod list;
pub mod mcp;
pub mod ordering;
pub mod paths;
pub mod show;
pub mod target;
pub mod workflow;

pub use paths::ensure_db;

use crate::error::Result;
use clap::Subcommand;

/// CLI subcommands
#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new tt project
    Init,
    /// Add a new task
    Add {
        /// Task title
        title: String,
        /// Optional description
        #[arg(long)]
        desc: Option<String>,
        /// Optional definition of done
        #[arg(long)]
        dod: Option<String>,
        /// Insert after this task ID
        #[arg(long)]
        after: Option<i64>,
        /// Insert before this task ID
        #[arg(long)]
        before: Option<i64>,
    },
    /// Edit a task
    Edit {
        /// Task ID
        id: i64,
        /// New title
        #[arg(long)]
        title: Option<String>,
        /// New description
        #[arg(long)]
        desc: Option<String>,
        /// New definition of done
        #[arg(long)]
        dod: Option<String>,
    },
    /// Show task details
    Show {
        /// Task ID
        id: i64,
    },
    /// List tasks
    List {
        /// Show all tasks (not just target subgraph)
        #[arg(long)]
        all: bool,
    },
    /// Set target task
    Target {
        /// Task ID
        id: i64,
    },
    /// Show next task to work on
    Next,
    /// Start a task
    Start {
        /// Task ID (defaults to next task if not specified)
        id: Option<i64>,
    },
    /// Stop the active task
    Stop,
    /// Complete the active task
    Done,
    /// Block a task
    Block {
        /// Task ID
        id: i64,
    },
    /// Unblock a task
    Unblock {
        /// Task ID
        id: i64,
    },
    /// Show current active task
    Current,
    /// Add a dependency
    Depend {
        /// Task ID (the dependent)
        id: i64,
        /// Task ID to depend on (the prerequisite)
        on_id: i64,
    },
    /// Remove a dependency
    Undepend {
        /// Task ID (the dependent)
        id: i64,
        /// Task ID to stop depending on (the prerequisite)
        on_id: i64,
    },
    /// Log an artifact for the active task
    Log {
        /// Artifact name
        name: String,
        /// File path
        #[arg(long)]
        file: String,
    },
    /// List artifacts
    Artifacts {
        /// Task ID (defaults to active task)
        #[arg(long)]
        task: Option<i64>,
    },
    /// Reorder a task
    Reorder {
        /// Task ID
        id: i64,
        /// Insert after this task
        #[arg(long)]
        after: Option<i64>,
        /// Insert before this task
        #[arg(long)]
        before: Option<i64>,
    },
    /// Reindex all task orders
    Reindex,
    /// Start MCP server
    Mcp,
    /// Install tt as an MCP server in AI coding tools
    Install {
        /// Target AI coding tool (claude, kilo, or kimi)
        #[arg(long, value_enum)]
        tool: Option<InstallTool>,
        /// Install globally (user home directory)
        #[arg(long)]
        global: bool,
        /// Install locally (project directory)
        #[arg(long)]
        local: bool,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum InstallTool {
    Claude,
    Kilo,
    Kimi,
}

/// Dispatch a command to its handler
pub fn dispatch(cmd: Commands) -> Result<()> {
    match cmd {
        Commands::Init => init::run(),
        Commands::Add {
            title,
            desc,
            dod,
            after,
            before,
        } => {
            let conn = ensure_db()?;
            add::run(
                &conn,
                &title,
                desc.as_deref(),
                dod.as_deref(),
                after,
                before,
            )
        }
        Commands::Edit {
            id,
            title,
            desc,
            dod,
        } => {
            let conn = ensure_db()?;
            edit::run(&conn, id, title.as_deref(), desc.as_deref(), dod.as_deref())
        }
        Commands::Show { id } => {
            let conn = ensure_db()?;
            show::run(&conn, id)
        }
        Commands::List { all } => {
            let conn = ensure_db()?;
            list::run(&conn, all)
        }
        Commands::Target { id } => {
            let conn = ensure_db()?;
            target::run_set(&conn, id)
        }
        Commands::Next => {
            let conn = ensure_db()?;
            target::run_next(&conn)
        }
        Commands::Start { id } => {
            let conn = ensure_db()?;
            workflow::run_start(&conn, id)
        }
        Commands::Stop => {
            let conn = ensure_db()?;
            workflow::run_stop(&conn)
        }
        Commands::Done => {
            let conn = ensure_db()?;
            workflow::run_done(&conn)
        }
        Commands::Block { id } => {
            let conn = ensure_db()?;
            workflow::run_block(&conn, id)
        }
        Commands::Unblock { id } => {
            let conn = ensure_db()?;
            workflow::run_unblock(&conn, id)
        }
        Commands::Current => {
            let conn = ensure_db()?;
            workflow::run_current(&conn)
        }
        Commands::Depend { id, on_id } => {
            let conn = ensure_db()?;
            dependencies::run_depend(&conn, id, on_id)
        }
        Commands::Undepend { id, on_id } => {
            let conn = ensure_db()?;
            dependencies::run_undepend(&conn, id, on_id)
        }
        Commands::Log { name, file } => {
            let conn = ensure_db()?;
            artifacts::run_log(&conn, &name, &file)
        }
        Commands::Artifacts { task } => {
            let conn = ensure_db()?;
            artifacts::run_list(&conn, task)
        }
        Commands::Reorder { id, after, before } => {
            let conn = ensure_db()?;
            ordering::run_reorder(&conn, id, after, before)
        }
        Commands::Reindex => {
            let conn = ensure_db()?;
            ordering::run_reindex(&conn)
        }
        Commands::Mcp => mcp::run(),
        Commands::Install {
            tool,
            global,
            local,
        } => install::run(tool, global, local),
    }
}
