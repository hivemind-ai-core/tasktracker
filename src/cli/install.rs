//! Install tt as an MCP server in AI coding tools

use crate::cli::InstallTool;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const TT_SERVER_NAME: &str = "tt";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpConfig {
    #[serde(rename = "mcpServers", default)]
    pub mcp_servers: HashMap<String, McpServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpServer {
    Stdio(StdioServer),
    Http(HttpServer),
}

impl Default for McpServer {
    fn default() -> Self {
        McpServer::Stdio(StdioServer::default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StdioServer {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HttpServer {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

pub fn run(tool: Option<InstallTool>, global: bool, local: bool) -> Result<()> {
    let has_scope = global || local;

    match (tool, has_scope) {
        (None, _) => {
            print_generic_info();
            Ok(())
        }
        (Some(t), false) => {
            print_tool_info(&t);
            Ok(())
        }
        (Some(t), true) => install_for_tool(&t, global, local),
    }
}

fn print_generic_info() {
    println!("tt can be installed as an MCP server in various AI coding tools.");
    println!();
    println!("Supported tools: claude, kilo, kimi");
    println!();
    println!("Usage:");
    println!("  tt install --tool claude           # Show how to install for Claude Code");
    println!("  tt install --tool claude --global  # Install globally for Claude Code");
    println!("  tt install --tool claude --local   # Install locally for Claude Code");
    println!();
    println!(
        "Run 'tt install --tool <tool>' to see installation instructions for a specific tool."
    );
}

fn print_tool_info(tool: &InstallTool) {
    let (name, config_path, cli_add) = match tool {
        InstallTool::Claude => (
            "Claude Code",
            "~/.claude.json or .mcp.json",
            "claude mcp add --transport stdio tt -- <path-to-tt> mcp",
        ),
        InstallTool::Kilo => (
            "Kilo CLI",
            "~/.kilocode/cli/global/settings/mcp_settings.json or .kilocode/mcp.json",
            "N/A (edit config file manually)",
        ),
        InstallTool::Kimi => (
            "Kimi CLI",
            "~/.kimi/mcp.json or .mcp.json",
            "kimi mcp add --transport stdio tt -- <path-to-tt> mcp",
        ),
    };

    println!("Installing tt as MCP server for {name}");
    println!();
    println!("Config file location: {config_path}");
    println!();
    println!("To add via CLI (if available):");
    println!("  {cli_add}");
    println!();
    println!("Or add manually to your config file:");
    print_config_example();
}

fn print_config_example() {
    let tt_path = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "/path/to/tt".to_string());

    let config = McpConfig {
        mcp_servers: HashMap::from([(
            TT_SERVER_NAME.to_string(),
            McpServer::Stdio(StdioServer {
                command: tt_path,
                args: vec!["mcp".to_string()],
                env: HashMap::new(),
            }),
        )]),
    };

    let json = serde_json::to_string_pretty(&config).unwrap();
    println!("{json}");
}

fn install_for_tool(tool: &InstallTool, global: bool, local: bool) -> Result<()> {
    if global && local {
        return Err(Error::InvalidArgument(
            "Cannot specify both --global and --local".to_string(),
        ));
    }

    match tool {
        InstallTool::Claude => install_claude(global),
        InstallTool::Kilo => install_kilo(global),
        InstallTool::Kimi => install_kimi(global),
    }
}

fn install_claude(global: bool) -> Result<()> {
    let config_path = if global {
        dirs::home_dir()
            .map(|p| p.join(".claude.json"))
            .ok_or_else(|| {
                Error::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Cannot find home directory",
                ))
            })?
    } else {
        PathBuf::from(".mcp.json")
    };

    install_to_config(&config_path, "Claude Code", global)
}

fn install_kilo(global: bool) -> Result<()> {
    let config_path = if global {
        dirs::home_dir()
            .map(|p| p.join(".kilocode/cli/global/settings/mcp_settings.json"))
            .ok_or_else(|| {
                Error::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Cannot find home directory",
                ))
            })?
    } else {
        PathBuf::from(".kilocode/mcp.json")
    };

    install_to_config(&config_path, "Kilo CLI", global)
}

fn install_kimi(global: bool) -> Result<()> {
    let config_path = if global {
        dirs::home_dir()
            .map(|p| p.join(".kimi/mcp.json"))
            .ok_or_else(|| {
                Error::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Cannot find home directory",
                ))
            })?
    } else {
        PathBuf::from(".mcp.json")
    };

    install_to_config(&config_path, "Kimi CLI", global)
}

fn install_to_config(config_path: &Path, tool_name: &str, global: bool) -> Result<()> {
    let config_dir = config_path.parent().ok_or_else(|| {
        Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid config path",
        ))
    })?;

    if let Err(e) = fs::create_dir_all(config_dir) {
        return Err(Error::Io(e));
    }

    let mut config = if config_path.exists() {
        let content = fs::read_to_string(config_path).map_err(Error::Io)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        McpConfig::default()
    };

    let tt_path = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "/path/to/tt".to_string());

    config.mcp_servers.insert(
        TT_SERVER_NAME.to_string(),
        McpServer::Stdio(StdioServer {
            command: tt_path,
            args: vec!["mcp".to_string()],
            env: HashMap::new(),
        }),
    );

    let json = serde_json::to_string_pretty(&config).map_err(|e| {
        Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })?;

    fs::write(config_path, json).map_err(Error::Io)?;

    println!("Installed tt as MCP server in {tool_name}");
    println!("  Config: {}", config_path.display());

    if !global {
        update_gitignore(config_path)?;
    }

    Ok(())
}

fn update_gitignore(config_path: &Path) -> Result<()> {
    let gitignore_path = PathBuf::from(".gitignore");
    let filename = config_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    if !gitignore_path.exists() {
        let content = format!("{filename}\n");
        fs::write(&gitignore_path, content).map_err(Error::Io)?;
        println!("  Added {filename} to .gitignore");
        return Ok(());
    }

    let content = fs::read_to_string(&gitignore_path).map_err(Error::Io)?;
    if content.lines().any(|line| line.trim() == filename) {
        return Ok(());
    }

    let new_content = format!("{}\n{}", content.trim_end(), filename);
    fs::write(&gitignore_path, new_content).map_err(Error::Io)?;
    println!("  Added {filename} to .gitignore");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_mcp_config() {
        let config = McpConfig::default();
        assert!(config.mcp_servers.is_empty());
    }

    #[test]
    fn test_mcp_server_serialization() {
        let server = McpServer::Stdio(StdioServer {
            command: "/usr/bin/tt".to_string(),
            args: vec!["mcp".to_string()],
            env: HashMap::new(),
        });

        let json = serde_json::to_string(&server).unwrap();
        assert!(json.contains("command"));
        assert!(json.contains("/usr/bin/tt"));
    }
}
