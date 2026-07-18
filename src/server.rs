//! `ServerHandler` implementation and tool router for the MCP server.

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_handler, tool_router,
};

use crate::tools::clipboard::{self, ClipboardParams};
use crate::tools::filesystem::{self, FileSystemParams};
use crate::tools::notification::{self, NotificationParams};
use crate::tools::registry::{self, RegistryParams};
use crate::tools::scrape::{self, ScrapeParams};
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
        description = "Manages file system operations with eight modes: 'read' (read text file contents with optional line offset/limit), 'write' (create or overwrite a file, set append=true to append), 'copy' (copy file or directory to destination), 'move' (move or rename file/directory), 'delete' (delete file or directory, set recursive=true for non-empty dirs), 'list' (list directory contents with optional pattern filter), 'search' (find files matching a glob pattern), 'info' (get file/directory metadata like size, dates, type). Relative paths are resolved from the user's Desktop folder. Use absolute paths to access other locations."
    )]
    async fn file_system(&self, Parameters(params): Parameters<FileSystemParams>) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(filesystem::file_system(params))]))
    }

    #[tool(
        description = "Read and write the Windows Registry. Keywords: regedit, registry key, HKEY, HKCU, HKLM, Windows settings, registry value. Use mode=\"get\" to read a value, mode=\"set\" to create/update a value, mode=\"delete\" to remove a value or key, mode=\"list\" to list values and sub-keys under a path. Paths use PowerShell format (e.g. \"HKCU:\\Software\\MyApp\", \"HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\")."
    )]
    async fn registry(&self, Parameters(params): Parameters<RegistryParams>) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(registry::registry(params))]))
    }

    #[tool(
        description = "Fetch/scrape web page content from a URL. Keywords: scrape, fetch, browse, web, URL, extract, download, read webpage. Performs a lightweight HTTP request to the URL and returns the page content converted to Markdown."
    )]
    async fn scrape(&self, Parameters(params): Parameters<ScrapeParams>) -> Result<CallToolResult, McpError> {
        match scrape::scrape(params).await {
            Ok(message) => Ok(CallToolResult::success(vec![ContentBlock::text(message)])),
            Err(message) => Ok(CallToolResult::error(vec![ContentBlock::text(message)])),
        }
    }

    #[tool(
        description = "Copy/paste clipboard operations. Keywords: copy, paste, cut, clipboard, text transfer. Use mode=\"get\" to read current clipboard content, mode=\"set\" to set clipboard text."
    )]
    async fn clipboard(&self, Parameters(ClipboardParams { mode, text }): Parameters<ClipboardParams>) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(clipboard::clipboard(mode, text))]))
    }

    #[tool(description = "Sends a Windows toast notification with a title and message.")]
    async fn notification(
        &self,
        Parameters(NotificationParams { title, message, app_id }): Parameters<NotificationParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(notification::send_notification(&title, &message, &app_id))]))
    }
}

// `name` must be set explicitly: rmcp's default `Implementation::from_build_env()`
// bakes in `env!("CARGO_CRATE_NAME")` from rmcp's own build, not this crate's,
// so the server would otherwise report itself as "rmcp".
#[tool_handler(name = "windows-computeruse", instructions = "Windows desktop automation MCP server.")]
impl ServerHandler for WindowsComputerUseServer {}
