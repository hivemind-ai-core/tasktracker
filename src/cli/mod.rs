//! CLI command handlers for tt
//!
//! Thin dispatchers that parse arguments and call core operations.

pub mod add;
pub mod archive;
pub mod artifacts;
pub mod dependencies;
pub mod dump;
pub mod edit;
pub mod focus;
pub mod init;
pub mod install;
pub mod list;
pub mod mcp;
pub mod ordering;
pub mod paths;
pub mod restore;
pub mod show;
pub mod split;
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
        /// Task description (can be empty string "")
        description: String,
        /// Definition of done (can be empty string "")
        dod: String,
        /// Insert after this task ID
        #[arg(long)]
        after: Option<i64>,
        /// Insert before this task ID
        #[arg(long)]
        before: Option<i64>,
        /// Task ID(s) this task depends on
        #[arg(long)]
        depends_on: Vec<i64>,
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
        /// Set task status directly (pending, in_progress, completed, blocked, cancelled)
        #[arg(long)]
        status: Option<String>,
        /// Action to perform on task (complete, stop, cancel, block, unblock)
        #[arg(long)]
        action: Option<String>,
    },
    /// Show task details (one or more)
    Show {
        /// Task ID(s)
        ids: Vec<i64>,
    },
    /// List tasks with optional filtering
    #[command(alias = "ls")]
    List {
        /// Show all tasks (not just focused subgraph)
        #[arg(long)]
        all: bool,
        /// Show only archived tasks
        #[arg(long)]
        archived: bool,
        /// Filter by status (pending, in_progress, completed, blocked)
        #[arg(long)]
        status: Option<String>,
        /// Limit number of results
        #[arg(long)]
        limit: Option<usize>,
        /// Offset for pagination
        #[arg(long)]
        offset: Option<usize>,
    },
    /// Focus task operations (set, show, clear, next, last)
    Focus {
        #[command(subcommand)]
        action: FocusCommand,
    },
    /// Complete current task and start the next one
    Advance {
        /// Preview without executing
        #[arg(long)]
        dry_run: bool,
    },
    /// Show current active task
    Current,
    /// Manage dependencies (add or remove)
    Depend {
        /// Task ID (the dependent)
        id: i64,
        /// Task ID(s) to depend on (the prerequisites)
        #[arg(required = true)]
        on_ids: Vec<i64>,
        /// Remove dependencies instead of adding
        #[arg(long, short)]
        remove: bool,
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
    #[command(alias = "mv")]
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
    /// Split a task into subtasks
    Split {
        /// Task ID to split
        id: i64,
        /// Subtask definitions in format: "title" "description" "dod"
        /// Can specify multiple: "title1" "desc1" "dod1" "title2" "desc2" "dod2"
        #[arg(required = true)]
        parts: Vec<String>,
    },
    /// Archive completed tasks
    Archive {
        /// Archive all completed tasks
        #[command(subcommand)]
        action: ArchiveCommand,
    },
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
    /// Dump database to a TOML file
    Dump {
        /// Output file path
        file: String,
    },
    /// Restore database from a TOML file
    Restore {
        /// Input file path
        file: String,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum InstallTool {
    Claude,
    Kilo,
    Kimi,
}

#[derive(Subcommand, Clone, Debug)]
pub enum ArchiveCommand {
    /// Archive all completed tasks
    All,
}

/// Focus subcommands
#[derive(Subcommand)]
pub enum FocusCommand {
    /// Show current focus
    Show,
    /// Set focus to a specific task
    Set { id: i64 },
    /// Clear the focus
    Clear,
    /// Set focus to next incomplete task (lowest manual_order pending)
    Next,
    /// Set focus to most recent task (highest id)
    Last,
}

/// Dispatch a command to its handler
pub fn dispatch(cmd: Commands) -> Result<()> {
    match cmd {
        Commands::Init => init::run(),
        Commands::Add {
            title,
            description,
            dod,
            after,
            before,
            depends_on,
        } => {
            let conn = ensure_db()?;
            add::run(&conn, &title, &description, &dod, after, before, depends_on)
        }
        Commands::Edit {
            id,
            title,
            desc,
            dod,
            status,
            action,
        } => {
            let conn = ensure_db()?;
            edit::run(
                &conn,
                id,
                title.as_deref(),
                desc.as_deref(),
                dod.as_deref(),
                status,
                action,
            )
        }
        Commands::Show { ids } => {
            let conn = ensure_db()?;
            show::run(&conn, ids)
        }
        Commands::List {
            all,
            archived,
            status,
            limit,
            offset,
        } => {
            let conn = ensure_db()?;
            list::run(&conn, all, archived, status, limit, offset)
        }
        Commands::Focus { action } => {
            let conn = ensure_db()?;
            match action {
                FocusCommand::Show => focus::run_show(&conn),
                FocusCommand::Set { id } => focus::run_set(&conn, id),
                FocusCommand::Clear => focus::run_clear(&conn),
                FocusCommand::Next => focus::run_target_next(&conn),
                FocusCommand::Last => focus::run_target_last(&conn),
            }
        }

        Commands::Advance { dry_run } => {
            let conn = ensure_db()?;
            workflow::run_advance(&conn, dry_run)
        }
        Commands::Current => {
            let conn = ensure_db()?;
            workflow::run_current(&conn)
        }
        Commands::Depend { id, on_ids, remove } => {
            let conn = ensure_db()?;
            dependencies::run_depend(&conn, id, on_ids, remove)
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
        Commands::Split { id, parts } => {
            let conn = ensure_db()?;
            split::run(&conn, id, parts)
        }
        Commands::Archive { action } => {
            let conn = ensure_db()?;
            match action {
                ArchiveCommand::All => archive::run_archive_all(&conn),
            }
        }
        Commands::Mcp => mcp::run(),
        Commands::Install {
            tool,
            global,
            local,
        } => install::run(tool, global, local),
        Commands::Dump { file } => {
            let conn = ensure_db()?;
            let path = std::path::Path::new(&file);
            dump::run(&conn, path)
        }
        Commands::Restore { file } => {
            let conn = ensure_db()?;
            let path = std::path::Path::new(&file);
            restore::run(&conn, path)
        }
    }
}
