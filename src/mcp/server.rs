//! MCP server implementation
//!
//! Main server loop that handles JSON-RPC requests over stdio.
//! Opens a new database connection for each command to ensure reads see latest data.

use crate::cli::paths;
use crate::constants;
use crate::mcp::tools::{get_all_tool_metadata, register_all_tools, HandlerRegistry};
use crate::mcp::transport::{
    JsonRpcRequest, JsonRpcResponse, McpError, McpResponse, StdioTransport,
};
use rusqlite::Connection;
use std::path::PathBuf;

/// MCP Server
pub struct McpServer {
    /// Transport
    pub(crate) transport: StdioTransport,
    db_path: PathBuf,
    handlers: HandlerRegistry,
    running: bool,
}

impl McpServer {
    /// Create a new MCP server - finds db path automatically
    pub fn new() -> Result<Self, McpError> {
        let handlers = register_all_tools();
        let db_path = paths::find_db_path().ok_or_else(|| {
            McpError::Database("No tt project found. Run `tt init` first.".to_string())
        })?;

        Ok(McpServer {
            transport: StdioTransport::new(),
            db_path,
            handlers,
            running: false,
        })
    }

    #[cfg(test)]
    pub fn with_path(db_path: PathBuf, transport: StdioTransport) -> Self {
        let handlers = register_all_tools();
        McpServer {
            transport,
            db_path,
            handlers,
            running: false,
        }
    }

    pub fn run(&mut self) -> Result<(), McpError> {
        self.running = true;
        while self.running {
            match self.transport.read_request()? {
                Some(request) => self.handle_request(request)?,
                None => self.running = false,
            }
        }
        Ok(())
    }

    pub(crate) fn handle_request(&mut self, request: JsonRpcRequest) -> Result<(), McpError> {
        let id = request.id.clone();
        match request.method.as_str() {
            "initialize" => self.handle_initialize(id),
            "notifications/initialized" => Ok(()),
            "tools/list" => self.handle_tools_list(id),
            "tools/call" => self.handle_tool_call(id, request.params),
            "shutdown" => {
                self.running = false;
                self.transport.send_mcp_response(
                    id,
                    McpResponse::ok(serde_json::json!({"message": "Shutting down"})),
                )
            }
            _ => self.transport.send_response(&JsonRpcResponse::error(
                id,
                -32601,
                format!("Method not found: {}", request.method),
            )),
        }
    }

    fn handle_initialize(&mut self, id: Option<serde_json::Value>) -> Result<(), McpError> {
        let capabilities = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": constants::APP_NAME, "version": constants::VERSION}
        });
        self.transport
            .send_response(&JsonRpcResponse::success(id, capabilities))
    }

    fn handle_tools_list(&mut self, id: Option<serde_json::Value>) -> Result<(), McpError> {
        let tools = get_all_tool_metadata(&self.handlers);
        let tools_json: Vec<serde_json::Value> = tools
            .into_iter()
            .map(|meta| {
                serde_json::json!({
                    "name": meta.name,
                    "description": meta.description,
                    "inputSchema": meta.input_schema
                })
            })
            .collect();
        self.transport.send_response(&JsonRpcResponse::success(
            id,
            serde_json::json!({ "tools": tools_json }),
        ))
    }

    fn handle_tool_call(
        &mut self,
        id: Option<serde_json::Value>,
        params: Option<serde_json::Value>,
    ) -> Result<(), McpError> {
        let params = params.unwrap_or(serde_json::Value::Null);
        let tool_name = params
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        let db = match crate::db::open_db(&self.db_path) {
            Ok(c) => c,
            Err(e) => {
                return self.transport.send_mcp_response(
                    id,
                    McpResponse::error("DatabaseError", &format!("Failed to open database: {e}")),
                )
            }
        };

        // Run the handler
        self.run_handler(id.clone(), &tool_name, arguments, &db)
    }

    fn run_handler(
        &mut self,
        id: Option<serde_json::Value>,
        tool_name: &str,
        arguments: serde_json::Value,
        db: &Connection,
    ) -> Result<(), McpError> {
        match self.handlers.get(tool_name) {
            Some(handler) => match handler.handle(db, arguments) {
                Ok(mcp_response) => {
                    let protocol_response = mcp_response_to_protocol(mcp_response);
                    self.transport
                        .send_response(&JsonRpcResponse::success(id, protocol_response))
                }
                Err(msg) => self
                    .transport
                    .send_mcp_response(id, McpResponse::error("InternalError", &msg)),
            },
            None => self.transport.send_mcp_response(
                id,
                McpResponse::error("ToolNotFound", &format!("Tool not found: {tool_name}")),
            ),
        }
    }

    pub fn shutdown(&mut self) {
        self.running = false;
    }
}

/// Convert McpResponse to MCP protocol format with content field
fn mcp_response_to_protocol(response: McpResponse) -> serde_json::Value {
    match response {
        McpResponse::Ok { data } => {
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string(&data).unwrap_or_default()
                }]
            })
        }
        McpResponse::Error {
            error_code,
            message,
        } => {
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string(&serde_json::json!({
                        "status": "error",
                        "error_code": error_code,
                        "message": message
                    })).unwrap_or_default()
                }],
                "isError": true
            })
        }
    }
}
