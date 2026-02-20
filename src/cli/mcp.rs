//! MCP server command handler
//!
//! Runs the MCP server over stdio for AI agent server opens a fresh communication.
//! The database connection for each command.

use crate::error::Result;
use crate::mcp::McpServer;

/// Run the MCP server
pub fn run() -> Result<()> {
    let mut server = McpServer::new().map_err(|e| crate::error::Error::Mcp(e.to_string()))?;

    // Run the server (this blocks until shutdown)
    if let Err(e) = server.run() {
        eprintln!("MCP server error: {e}");
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    // Most testing is done in the mcp module itself
    // These tests verify the CLI integration

    #[test]
    fn test_mcp_command_exports() {
        // Verify the module compiles and exports correctly
        use super::run;
        // Just check it exists - actual testing requires a database
        let _ = run;
    }
}
