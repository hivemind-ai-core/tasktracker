//! MCP (Model Context Protocol) server implementation
//!
//! This module provides an MCP server over stdio that exposes all tt operations
//! as MCP tools. The server uses JSON-RPC 2.0 for communication.

pub mod server;
pub mod tools;
pub mod transport;

pub use server::McpServer;
pub use tools::{register_all_tools, HandlerRegistry, ToolHandler, ToolMetadata};
pub use transport::{JsonRpcRequest, JsonRpcResponse, McpError, McpResponse, StdioTransport};

#[cfg(test)]
mod tests;
