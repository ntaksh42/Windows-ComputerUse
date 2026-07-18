//! `PowerShell` tool: shell/command execution (docs/SPEC.md §3).

use rmcp::schemars;
use serde::Deserialize;

fn default_timeout() -> i64 {
    30
}

/// Parameters for the `PowerShell` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PowerShellParams {
    /// PowerShell command to execute.
    pub command: String,
    /// Timeout in seconds.
    #[serde(default = "default_timeout")]
    #[schemars(description = "Timeout in seconds (default 30).")]
    pub timeout: i64,
}

/// Executes `command` via PowerShell and returns the formatted response.
pub fn powershell(command: &str, timeout: i64) -> String {
    let timeout_secs = timeout.max(0) as u64;
    let (response, status_code) = crate::powershell::execute_command(command, timeout_secs, None);
    format!("Response: {response}\nStatus Code: {status_code}")
}
