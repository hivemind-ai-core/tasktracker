//! tt - DAG-Based Task Tracker (CLI + MCP Server)

use clap::Parser;

pub mod cli;
pub mod constants;
pub mod core;
pub mod db;
pub mod error;
pub mod mcp;

/// tt - DAG-Based Task Tracker
#[derive(Parser)]
#[command(name = "tt")]
#[command(about = "DAG-Based Task Tracker")]
#[command(version = constants::VERSION)]
struct Cli {
    #[command(subcommand)]
    command: cli::Commands,
}

fn main() {
    let cli = Cli::parse();

    if let Err(e) = cli::dispatch(cli.command) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_project_compiles() {
        // Basic sanity check that project compiles
    }
}
