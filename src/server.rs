//! `ServerHandler` implementation and tool router for the MCP server.

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_handler, tool_router,
};

use crate::tools::click::{self, ClickParams};
use crate::tools::move_mouse::{self, MoveParams};
use crate::tools::multi_edit::{self, MultiEditParams};
use crate::tools::multi_select::{self, MultiSelectParams};
use crate::tools::scroll::{self, ScrollParams};
use crate::tools::shortcut::{self, ShortcutParams};
use crate::tools::typing::{self, TypeParams};
use crate::tools::wait::{self, WaitParams};

/// Wraps a tool's `Result<String, String>` into an MCP `CallToolResult`:
/// `Ok` becomes success content, `Err` becomes an `isError` result (an
/// expected, caller-facing failure — not a protocol-level error).
fn as_call_result(result: Result<String, String>) -> Result<CallToolResult, McpError> {
    match result {
        Ok(message) => Ok(CallToolResult::success(vec![ContentBlock::text(message)])),
        Err(message) => Ok(CallToolResult::error(vec![ContentBlock::text(message)])),
    }
}

/// MCP server exposing Windows desktop automation tools.
#[derive(Debug, Clone)]
pub struct WindowsComputerUseServer;

#[tool_router]
impl WindowsComputerUseServer {
    #[tool(description = "Wait for a number of seconds (1-60) before returning.")]
    async fn wait(
        &self,
        Parameters(WaitParams { duration }): Parameters<WaitParams>,
    ) -> Result<CallToolResult, McpError> {
        match wait::wait(duration).await {
            Ok(message) => Ok(CallToolResult::success(vec![ContentBlock::text(message)])),
            Err(message) => Ok(CallToolResult::error(vec![ContentBlock::text(message)])),
        }
    }

    #[tool(
        name = "Click",
        description = "Performs mouse clicks at specified coordinates [x, y] or passing a UI element's label/id. Supports button types: 'left' for selection/activation, 'right' for context menus, 'middle'. Supports clicks: 0=hover only (no click), 1=single click (select/focus), 2=double click (open/activate). Provide either loc or label."
    )]
    async fn click(
        &self,
        Parameters(params): Parameters<ClickParams>,
    ) -> Result<CallToolResult, McpError> {
        as_call_result(click::click(params))
    }

    #[tool(
        name = "Type",
        description = "Types text at specified coordinates [x, y] or passing a UI element's label/id. Set clear=True to clear existing text first, False to append. Set press_enter=True to submit after typing. Set caret_position to 'start' (beginning), 'end' (end), or 'idle' (default). Provide either loc or label."
    )]
    async fn type_tool(
        &self,
        Parameters(params): Parameters<TypeParams>,
    ) -> Result<CallToolResult, McpError> {
        as_call_result(typing::type_text(params))
    }

    #[tool(
        name = "Scroll",
        description = "Scrolls at coordinates [x, y], a UI element's label/id, or current mouse position if loc=None. Type: vertical (default) or horizontal. Direction: up/down for vertical, left/right for horizontal. wheel_times controls amount (1 wheel ≈ 3-5 lines). Use for navigating long content, lists, and web pages."
    )]
    async fn scroll(
        &self,
        Parameters(params): Parameters<ScrollParams>,
    ) -> Result<CallToolResult, McpError> {
        as_call_result(scroll::scroll(params))
    }

    #[tool(
        name = "Move",
        description = "Moves mouse cursor to coordinates [x, y] or passing a UI element's label/id. Set drag=True to perform a drag-and-drop operation from the current mouse position to the target coordinates, or provide from_loc=[x, y] to make the drag explicit-start and atomic in one tool call. Optional duration controls bounded intermediate movement. Default (drag=False) is a simple cursor move (hover). Provide either loc or label."
    )]
    async fn move_tool(
        &self,
        Parameters(params): Parameters<MoveParams>,
    ) -> Result<CallToolResult, McpError> {
        as_call_result(move_mouse::move_mouse(params))
    }

    #[tool(
        name = "Shortcut",
        description = "Executes keyboard shortcuts using key combinations separated by +. Examples: \"ctrl+c\" (copy), \"ctrl+v\" (paste), \"alt+tab\" (switch apps), \"win+r\" (Run dialog), \"win\" (Start menu), \"ctrl+shift+esc\" (Task Manager). Use for quick actions and system commands."
    )]
    async fn shortcut(
        &self,
        Parameters(params): Parameters<ShortcutParams>,
    ) -> Result<CallToolResult, McpError> {
        as_call_result(shortcut::shortcut(params))
    }

    #[tool(
        name = "MultiSelect",
        description = "Selects multiple items such as files, folders, or checkboxes if press_ctrl=True, or performs multiple clicks if False. Pass locs (list of coordinates) or labels (list of UI element labels/ids)."
    )]
    async fn multi_select(
        &self,
        Parameters(params): Parameters<MultiSelectParams>,
    ) -> Result<CallToolResult, McpError> {
        as_call_result(multi_select::multi_select(params))
    }

    #[tool(
        name = "MultiEdit",
        description = "Enters text into multiple input fields at specified coordinates locs=[[x,y,text], ...] or using labels=[[label,text], ...]. Provide either locs or labels."
    )]
    async fn multi_edit(
        &self,
        Parameters(params): Parameters<MultiEditParams>,
    ) -> Result<CallToolResult, McpError> {
        as_call_result(multi_edit::multi_edit(params))
    }
}

// `name` must be set explicitly: rmcp's default `Implementation::from_build_env()`
// bakes in `env!("CARGO_CRATE_NAME")` from rmcp's own build, not this crate's,
// so the server would otherwise report itself as "rmcp".
#[tool_handler(
    name = "windows-computeruse",
    instructions = "Windows desktop automation MCP server."
)]
impl ServerHandler for WindowsComputerUseServer {}
