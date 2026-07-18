//! `ServerHandler` implementation and tool router for the MCP server.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_handler, tool_router,
};

use crate::tools::display_inventory::{self, DisplayInventoryParams};
use crate::tools::screenshot::{self, ScreenshotParams};
use crate::tools::wait::{self, WaitParams};

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
        description = "Read active display layout and DPI metadata. Reports display index, device name, monitor/work-area bounds, resolution, orientation, primary flag, effective DPI, and scale."
    )]
    async fn display_inventory(
        &self,
        Parameters(DisplayInventoryParams {}): Parameters<DisplayInventoryParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(display_inventory::display_inventory())]))
    }

    #[tool(
        description = "Captures a fast screenshot-first desktop snapshot with cursor position, desktop/window summaries, and an image. This path skips UI tree extraction for speed. Use Snapshot when you need interactive element ids, scrollable regions, or browser DOM extraction. Note: the returned image may be downscaled for efficiency; when it is, multiply image coordinates by the ratio of original size to displayed size to get the actual screen coordinates for mouse actions (Click, Move, etc.)."
    )]
    async fn screenshot(
        &self,
        Parameters(params): Parameters<ScreenshotParams>,
    ) -> Result<CallToolResult, McpError> {
        match screenshot::screenshot(&params) {
            Ok(output) => Ok(CallToolResult::success(vec![
                ContentBlock::text(output.text),
                ContentBlock::image(BASE64.encode(output.png_bytes), "image/png"),
            ])),
            Err(e) => Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "Error capturing screenshot: {e}. Please try again."
            ))])),
        }
    }
}

// `name` must be set explicitly: rmcp's default `Implementation::from_build_env()`
// bakes in `env!("CARGO_CRATE_NAME")` from rmcp's own build, not this crate's,
// so the server would otherwise report itself as "rmcp".
#[tool_handler(name = "windows-computeruse", instructions = "Windows desktop automation MCP server.")]
impl ServerHandler for WindowsComputerUseServer {}
