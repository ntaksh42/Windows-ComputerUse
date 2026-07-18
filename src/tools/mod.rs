//! Tool implementations for the MCP server.
//!
//! Each tool lives in its own submodule (parameter types plus the tool's
//! logic). New tools are added here as they are implemented; the router in
//! `server.rs` wires them up as `#[tool]` methods.

pub mod app;
pub mod process;
pub mod shell;
pub mod wait;
