//! `ServerHandler` implementation and tool router for the MCP server.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_handler, tool_router,
};

use crate::tools::app::{self, AppParams};
use crate::tools::click::{self, ClickParams};
use crate::tools::clipboard::{self, ClipboardParams};
use crate::tools::display_inventory::{self, DisplayInventoryParams};
use crate::tools::filesystem::{self, FileSystemParams};
use crate::tools::move_mouse::{self, MoveParams};
use crate::tools::multi_edit::{self, MultiEditParams};
use crate::tools::multi_select::{self, MultiSelectParams};
use crate::tools::notification::{self, NotificationParams};
use crate::tools::process::{self, ProcessParams};
use crate::tools::registry::{self, RegistryParams};
use crate::tools::scrape::{self, ScrapeParams};
use crate::tools::screenshot::{self, ScreenshotParams};
use crate::tools::scroll::{self, ScrollParams};
use crate::tools::shell::{self, PowerShellParams};
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
    #[tool(
        name = "Wait",
        description = "Wait for a number of seconds (1-60) before returning."
    )]
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
<<<<<<< HEAD
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

    #[tool(
        name = "DisplayInventory",
        description = "Read active display layout and DPI metadata. Reports display index, device name, monitor/work-area bounds, resolution, orientation, primary flag, effective DPI, and scale."
    )]
    async fn display_inventory(
        &self,
        Parameters(DisplayInventoryParams {}): Parameters<DisplayInventoryParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            display_inventory::display_inventory(),
        )]))
    }

    #[tool(
        name = "Screenshot",
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

    #[tool(
        name = "FileSystem",
        description = "Manages file system operations with eight modes: 'read' (read text file contents with optional line offset/limit), 'write' (create or overwrite a file, set append=true to append), 'copy' (copy file or directory to destination), 'move' (move or rename file/directory), 'delete' (delete file or directory, set recursive=true for non-empty dirs), 'list' (list directory contents with optional pattern filter), 'search' (find files matching a glob pattern), 'info' (get file/directory metadata like size, dates, type). Relative paths are resolved from the user's Desktop folder. Use absolute paths to access other locations."
    )]
    async fn file_system(
        &self,
        Parameters(params): Parameters<FileSystemParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            filesystem::file_system(params),
        )]))
    }

    #[tool(
        name = "Registry",
        description = "Read and write the Windows Registry. Keywords: regedit, registry key, HKEY, HKCU, HKLM, Windows settings, registry value. Use mode=\"get\" to read a value, mode=\"set\" to create/update a value, mode=\"delete\" to remove a value or key, mode=\"list\" to list values and sub-keys under a path. Paths use PowerShell format (e.g. \"HKCU:\\Software\\MyApp\", \"HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\")."
    )]
    async fn registry(
        &self,
        Parameters(params): Parameters<RegistryParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            registry::registry(params),
        )]))
    }

    #[tool(
        name = "Scrape",
        description = "Fetch/scrape web page content from a URL. Keywords: scrape, fetch, browse, web, URL, extract, download, read webpage. Performs a lightweight HTTP request to the URL and returns the page content converted to Markdown."
    )]
    async fn scrape(
        &self,
        Parameters(params): Parameters<ScrapeParams>,
    ) -> Result<CallToolResult, McpError> {
        match scrape::scrape(params).await {
            Ok(message) => Ok(CallToolResult::success(vec![ContentBlock::text(message)])),
            Err(message) => Ok(CallToolResult::error(vec![ContentBlock::text(message)])),
        }
    }

    #[tool(
        name = "PowerShell",
        description = "Shell/command execution. A comprehensive system tool for executing any PowerShell commands: navigate the file system, manage files and processes, and execute system-level operations."
    )]
    async fn power_shell(
        &self,
        Parameters(PowerShellParams { command, timeout }): Parameters<PowerShellParams>,
    ) -> Result<CallToolResult, McpError> {
        let message = tokio::task::spawn_blocking(move || shell::powershell(&command, timeout))
            .await
            .unwrap_or_else(|e| format!("Response: Command execution failed: {e}\nStatus Code: 1"));
        Ok(CallToolResult::success(vec![ContentBlock::text(message)]))
    }

    #[tool(
        name = "App",
        description = "Open/start/launch applications and manage windows. Four modes: 'launch' (opens an application by Start Menu name), 'launch_executable' (launches one executable path with separated argv and optional cwd), 'resize' (adjusts a named or active window), and 'switch' (brings a specific window into focus)."
    )]
    async fn app(
        &self,
        Parameters(params): Parameters<AppParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = tokio::task::spawn_blocking(move || app::app(params))
            .await
            .unwrap_or_else(|e| Err(format!("App tool panicked: {e}")));
        match result {
            Ok(message) => Ok(CallToolResult::success(vec![ContentBlock::text(message)])),
            Err(message) => Ok(CallToolResult::error(vec![ContentBlock::text(message)])),
        }
    }

    #[tool(
        name = "Clipboard",
        description = "Copy/paste clipboard operations. Keywords: copy, paste, cut, clipboard, text transfer. Use mode=\"get\" to read current clipboard content, mode=\"set\" to set clipboard text."
    )]
    async fn clipboard(
        &self,
        Parameters(ClipboardParams { mode, text }): Parameters<ClipboardParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            clipboard::clipboard(mode, text),
        )]))
    }

    #[tool(
        name = "Notification",
        description = "Sends a Windows toast notification with a title and message."
    )]
    async fn notification(
        &self,
        Parameters(NotificationParams {
            title,
            message,
            app_id,
        }): Parameters<NotificationParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            notification::send_notification(&title, &message, &app_id),
        )]))
    }

    #[tool(
        name = "Process",
        description = "List and kill running processes. Use mode=\"list\" to list running processes with filtering and sorting options. Use mode=\"kill\" to terminate processes by PID or name."
    )]
    async fn process(
        &self,
        Parameters(params): Parameters<ProcessParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = tokio::task::spawn_blocking(move || process::process(params))
            .await
            .unwrap_or_else(|e| Err(format!("Process tool panicked: {e}")));
        match result {
            Ok(message) => Ok(CallToolResult::success(vec![ContentBlock::text(message)])),
            Err(message) => Ok(CallToolResult::error(vec![ContentBlock::text(message)])),
        }
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
