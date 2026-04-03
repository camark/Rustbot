//! MCP (Model Context Protocol) Client Implementation
//!
//! This module provides MCP client functionality for connecting to external
//! MCP tool servers. It supports both stdio and SSE transports.
//!
//! MCP Specification: https://modelcontextprotocol.io/specification/2025-06-18

pub mod client;
pub mod protocol;
pub mod tools;
pub mod transport;

pub use client::*;
pub use protocol::*;
pub use tools::*;
pub use transport::*;
