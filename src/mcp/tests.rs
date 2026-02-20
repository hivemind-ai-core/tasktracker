//! Integration tests for MCP module

#[cfg(test)]
mod tests {
    use crate::mcp::server::McpServer;
    use crate::mcp::transport::{SharedBuffer, StdioTransport};
    use std::io::Cursor;

    #[test]
    fn test_server_with_path() {
        let input = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let reader = Cursor::new(input.as_bytes());
        let writer = SharedBuffer::new();
        let transport = StdioTransport::with_streams(reader, writer);

        // Just verify we can create the server with a path
        let _server = McpServer::with_path(std::path::PathBuf::from("/tmp/test.db"), transport);
    }
}
