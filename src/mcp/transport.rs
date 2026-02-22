//! MCP transport layer - JSON-RPC over stdio
//!
//! Handles JSON-RPC message parsing, response formatting, and stdio transport.

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::io::{BufRead, Write};
use std::rc::Rc;

/// MCP-specific errors
#[derive(Debug)]
pub enum McpError {
    /// IO error from stdio operations
    Io(std::io::Error),
    /// JSON-RPC protocol error
    JsonRpc(String),
    /// JSON serialization/deserialization error
    Serialization(serde_json::Error),
    /// Graceful shutdown requested
    Shutdown,
    /// Database error
    Database(String),
}

impl std::fmt::Display for McpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpError::Io(e) => write!(f, "IO error: {e}"),
            McpError::JsonRpc(e) => write!(f, "JSON-RPC error: {e}"),
            McpError::Serialization(e) => write!(f, "Serialization error: {e}"),
            McpError::Shutdown => write!(f, "Shutdown requested"),
            McpError::Database(e) => write!(f, "Database error: {e}"),
        }
    }
}

impl std::error::Error for McpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            McpError::Io(e) => Some(e),
            McpError::Serialization(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for McpError {
    fn from(e: std::io::Error) -> Self {
        McpError::Io(e)
    }
}

impl From<serde_json::Error> for McpError {
    fn from(e: serde_json::Error) -> Self {
        McpError::Serialization(e)
    }
}

impl From<crate::error::Error> for McpError {
    fn from(e: crate::error::Error) -> Self {
        // Convert tt errors to MCP errors - the message will be preserved
        McpError::JsonRpc(e.to_string())
    }
}

impl From<McpError> for crate::error::Error {
    fn from(e: McpError) -> Self {
        crate::error::Error::Mcp(e.to_string())
    }
}

/// MCP response format per SPEC.md Section 13.3
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum McpResponse {
    /// Success response
    #[serde(rename = "ok")]
    Ok { data: serde_json::Value },
    /// Error response
    #[serde(rename = "error")]
    Error { error_code: String, message: String },
}

impl McpResponse {
    /// Create a success response
    pub fn ok<T: Serialize>(data: T) -> Self {
        let data = serde_json::to_value(data).unwrap_or(serde_json::Value::Null);
        McpResponse::Ok { data }
    }

    /// Create an error response from error code and message
    pub fn error(code: &str, message: &str) -> Self {
        McpResponse::Error {
            error_code: code.to_string(),
            message: message.to_string(),
        }
    }

    /// Convert a tt error to an MCP response
    pub fn from_tt_error(e: crate::error::Error) -> Self {
        let error_code = error_code_from_tt(&e);
        McpResponse::Error {
            error_code,
            message: e.to_string(),
        }
    }
}

/// Map tt error types to error codes for programmatic handling
fn error_code_from_tt(e: &crate::error::Error) -> String {
    match e {
        crate::error::Error::TaskNotFound(_) => "TaskNotFound".to_string(),
        crate::error::Error::TaskNotPending(_) => "TaskNotPending".to_string(),
        crate::error::Error::AnotherTaskActive(..) => "AnotherTaskActive".to_string(),
        crate::error::Error::NoActiveTask => "NoActiveTask".to_string(),
        crate::error::Error::UnmetDependencies(_, _) => "UnmetDependencies".to_string(),
        crate::error::Error::CycleDetected(_, _, _) => "CycleDetected".to_string(),
        crate::error::Error::NoTarget => "NoTarget".to_string(),
        crate::error::Error::TargetReached(_) => "TargetReached".to_string(),
        crate::error::Error::NoDod(_) => "NoDod".to_string(),
        crate::error::Error::AllBlocked(_) => "AllBlocked".to_string(),
        crate::error::Error::OrderConflict(_, _, _, _) => "OrderConflict".to_string(),
        crate::error::Error::InvalidStatus(_) => "InvalidStatus".to_string(),
        crate::error::Error::FloatPrecisionExhausted => "FloatPrecisionExhausted".to_string(),
        crate::error::Error::Db(_) => "DatabaseError".to_string(),
        crate::error::Error::Io(_) => "IoError".to_string(),
        crate::error::Error::JsonRpc(_) => "JsonRpcError".to_string(),
        crate::error::Error::NotSupported(_) => "NotSupported".to_string(),
        crate::error::Error::Mcp(_) => "McpError".to_string(),
        crate::error::Error::InvalidArgument(_) => "InvalidArgument".to_string(),
    }
}

/// JSON-RPC 2.0 request
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    /// Create a successful JSON-RPC response
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error JSON-RPC response
    pub fn error(id: Option<serde_json::Value>, code: i32, message: String) -> Self {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}

/// Stdio transport for MCP communication
pub struct StdioTransport {
    reader: Box<dyn BufRead>,
    writer: Box<dyn Write>,
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl StdioTransport {
    /// Create a new stdio transport using stdin/stdout
    pub fn new() -> Self {
        let stdin = std::io::stdin();
        let stdout = std::io::stdout();

        StdioTransport {
            reader: Box::new(std::io::BufReader::new(stdin)),
            writer: Box::new(stdout),
        }
    }

    /// Create a transport from arbitrary reader/writer (for testing)
    pub fn with_streams<R, W>(reader: R, writer: W) -> Self
    where
        R: BufRead + 'static,
        W: Write + 'static,
    {
        StdioTransport {
            reader: Box::new(reader),
            writer: Box::new(writer),
        }
    }

    /// Read a JSON-RPC request from stdin
    pub fn read_request(&mut self) -> Result<Option<JsonRpcRequest>, McpError> {
        let mut line = String::new();
        let bytes_read = self.reader.read_line(&mut line)?;

        // EOF reached
        if bytes_read == 0 {
            return Ok(None);
        }

        // Skip empty lines
        let line = line.trim();
        if line.is_empty() {
            return Ok(None);
        }

        // Parse JSON-RPC request
        let request: JsonRpcRequest =
            serde_json::from_str(line).map_err(McpError::Serialization)?;

        // Validate JSON-RPC version
        if request.jsonrpc != "2.0" {
            return Err(McpError::JsonRpc(
                "Invalid JSON-RPC version. Expected '2.0'".to_string(),
            ));
        }

        Ok(Some(request))
    }

    /// Send a JSON-RPC response to stdout
    pub fn send_response(&mut self, response: &JsonRpcResponse) -> Result<(), McpError> {
        let json = serde_json::to_string(response)?;
        writeln!(self.writer, "{json}")?;
        self.writer.flush()?;
        Ok(())
    }

    /// Send an MCP response as a JSON-RPC response
    ///
    /// Note: Per MCP spec, tool responses are always JSON-RPC success responses.
    /// Send an MCP response as a JSON-RPC response.
    /// Errors are returned as proper JSON-RPC errors, not embedded in result.
    pub fn send_mcp_response(
        &mut self,
        id: Option<serde_json::Value>,
        mcp_response: McpResponse,
    ) -> Result<(), McpError> {
        // If the MCP response is an error, return a proper JSON-RPC error
        let result = serde_json::to_value(&mcp_response)?;

        // Check if this is an error response by looking at the "status" field
        let is_error = result
            .get("status")
            .and_then(|v| v.as_str())
            .map(|s| s == "error")
            .unwrap_or(false);

        if is_error {
            let error_code = result
                .get("error_code")
                .and_then(|v| v.as_str())
                .unwrap_or("UnknownError");
            let message = result
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("An unknown error occurred");

            // Map our error codes to JSON-RPC error codes
            let jsonrpc_code = match error_code {
                "TaskNotFound" => -32001,
                "TaskNotPending" => -32002,
                "AnotherTaskActive" => -32003,
                "NoActiveTask" => -32004,
                "UnmetDependencies" => -32005,
                "NoDod" => -32006,
                "NoTarget" => -32007,
                "TargetReached" => -32008,
                "AllBlocked" => -32009,
                "CycleDetected" => -32010,
                "InvalidStatus" => -32011,
                _ => -32000,
            };

            let json_response = JsonRpcResponse::error(id, jsonrpc_code, message.to_string());
            return self.send_response(&json_response);
        }

        // Success case - return the result normally
        let json_response = JsonRpcResponse::success(id, result);
        self.send_response(&json_response)
    }
}

/// A wrapper around Rc<RefCell<Vec<u8>>> that implements Write for testing
#[derive(Clone)]
pub struct SharedBuffer {
    buffer: Rc<RefCell<Vec<u8>>>,
}

impl Default for SharedBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedBuffer {
    /// Create a new shared buffer
    pub fn new() -> Self {
        SharedBuffer {
            buffer: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Get the contents as a string
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        String::from_utf8(self.buffer.borrow().clone()).expect("Invalid UTF-8")
    }

    /// Get the contents as bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        self.buffer.borrow().clone()
    }
}

impl Write for SharedBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_response_ok() {
        let response = McpResponse::ok(serde_json::json!({"task_id": 42}));
        match response {
            McpResponse::Ok { data } => {
                assert_eq!(data["task_id"], 42);
            }
            _ => panic!("Expected Ok response"),
        }
    }

    #[test]
    fn test_mcp_response_error() {
        let response = McpResponse::error("TaskNotFound", "Task #42 not found");
        match response {
            McpResponse::Error {
                error_code,
                message,
            } => {
                assert_eq!(error_code, "TaskNotFound");
                assert_eq!(message, "Task #42 not found");
            }
            _ => panic!("Expected Error response"),
        }
    }

    #[test]
    fn test_error_code_mapping() {
        let e = crate::error::Error::TaskNotFound(42);
        let code = error_code_from_tt(&e);
        assert_eq!(code, "TaskNotFound");

        let e = crate::error::Error::NoActiveTask;
        let code = error_code_from_tt(&e);
        assert_eq!(code, "NoActiveTask");
    }

    #[test]
    fn test_json_rpc_response_serialization() {
        let response = JsonRpcResponse::success(
            Some(serde_json::json!(1)),
            serde_json::json!({"status": "ok"}),
        );
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"status\":\"ok\""));
    }

    #[test]
    fn test_stdio_transport_read_write() {
        let input = r#"{"jsonrpc":"2.0","id":1,"method":"test","params":{}}"#;
        let reader = std::io::BufReader::new(input.as_bytes());
        let writer = SharedBuffer::new();

        let mut transport = StdioTransport::with_streams(reader, writer.clone());
        let request = transport.read_request().unwrap().unwrap();

        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.method, "test");

        // Send a response
        let response =
            JsonRpcResponse::success(Some(serde_json::json!(1)), serde_json::json!({"ok": true}));
        transport.send_response(&response).unwrap();

        // Check output
        let output = writer.to_string();
        assert!(output.contains("\"ok\":true"));
    }

    #[test]
    fn test_stdio_transport_eof() {
        let reader = std::io::BufReader::new("".as_bytes());
        let writer = SharedBuffer::new();

        let mut transport = StdioTransport::with_streams(reader, writer);
        let result = transport.read_request().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_shared_buffer() {
        let buf = SharedBuffer::new();

        // Write some data
        {
            let mut writer: Box<dyn Write> = Box::new(buf.clone());
            write!(writer, "Hello, World!").unwrap();
            writer.flush().unwrap();
        }

        assert_eq!(buf.to_string(), "Hello, World!");
    }
}
