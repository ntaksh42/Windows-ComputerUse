//! `CursorPosition` tool: reports the current cursor coordinates.

use rmcp::schemars;
use serde::Deserialize;

use crate::input_sim;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CursorPositionParams {}

pub fn cursor_position() -> String {
    let (x, y) = input_sim::get_cursor_pos();
    format!("Cursor position: ({x}, {y})")
}
