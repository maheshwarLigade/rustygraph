//! Model Context Protocol (MCP) server for RustyGraph.
//!
//! Enables standard integration with AI agents, Claude Desktop, Cursor, and LLM frameworks
//! via JSON-RPC 2.0 stdio transport.

pub mod protocol;
pub mod server;

pub use protocol::{
    JsonRpcError, JsonRpcRequest, JsonRpcResponse, ResourceContent, ResourceDefinition,
    ToolCallResult, ToolContent, ToolDefinition,
};
pub use server::{McpBackend, McpServer};
