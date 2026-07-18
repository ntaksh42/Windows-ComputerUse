//! Tool implementations for the MCP server.
//!
//! Each tool lives in its own submodule (parameter types plus the tool's
//! logic). New tools are added here as they are implemented; the router in
//! `server.rs` wires them up as `#[tool]` methods.

pub mod click;
pub mod display_inventory;
pub mod move_mouse;
pub mod multi_edit;
pub mod multi_select;
pub mod screenshot;
pub mod scroll;
pub mod shortcut;
mod support;
pub mod typing;
pub mod wait;
