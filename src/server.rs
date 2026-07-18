//! `ServerHandler` implementation and tool router for the MCP server.

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_handler, tool_router,
};

use crate::tools::app::{self, AppParams};
use crate::tools::process::{self, ProcessParams};
use crate::tools::shell::{self, PowerShellParams};
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
    async fn app(&self, Parameters(params): Parameters<AppParams>) -> Result<CallToolResult, McpError> {
        let result = tokio::task::spawn_blocking(move || app::app(params))
            .await
            .unwrap_or_else(|e| Err(format!("App tool panicked: {e}")));
        match result {
            Ok(message) => Ok(CallToolResult::success(vec![ContentBlock::text(message)])),
            Err(message) => Ok(CallToolResult::error(vec![ContentBlock::text(message)])),
        }
    }

    #[tool(
        name = "Process",
        description = "List and kill running processes. Use mode=\"list\" to list running processes with filtering and sorting options. Use mode=\"kill\" to terminate processes by PID or name."
    )]
    async fn process(&self, Parameters(params): Parameters<ProcessParams>) -> Result<CallToolResult, McpError> {
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
#[tool_handler(name = "windows-computeruse", instructions = "Windows desktop automation MCP server.")]
impl ServerHandler for WindowsComputerUseServer {}
