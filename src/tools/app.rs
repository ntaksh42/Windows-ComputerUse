//! `App` tool: launch/resize/switch applications and windows (docs/SPEC.md §1).

use std::path::Path;
use std::time::Duration;

use rmcp::schemars;
use serde::Deserialize;

use crate::apps::{self, title_case};
use crate::params::ListOrString;
use crate::window;

/// `App` tool modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AppMode {
    Launch,
    LaunchExecutable,
    Resize,
    Switch,
}

fn default_mode() -> AppMode {
    AppMode::Launch
}

/// Parameters for the `App` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AppParams {
    #[serde(default = "default_mode")]
    pub mode: AppMode,
    /// Application/window name to fuzzy-match against the Start Menu (launch)
    /// or currently open windows (resize/switch).
    pub name: Option<String>,
    /// `[x, y]` target position, `resize` mode only.
    pub window_loc: Option<ListOrString<i32>>,
    /// `[width, height]` target size, `resize` mode only.
    pub window_size: Option<ListOrString<i32>>,
    /// Executable path, `launch_executable` mode only (required).
    pub executable: Option<String>,
    /// Argv for the executable, `launch_executable` mode only.
    pub args: Option<ListOrString<String>>,
    /// Working directory, `launch_executable` mode only.
    pub cwd: Option<String>,
}

/// Dispatches to the mode-specific handler. `Err` results are structural/
/// validation failures that surface as MCP tool errors (isError); `Ok`
/// results (including "application not found"-style messages) are the
/// tool's normal text response, matching the Python reference.
pub fn app(params: AppParams) -> Result<String, String> {
    let AppParams {
        mode,
        name,
        window_loc,
        window_size,
        executable,
        args,
        cwd,
    } = params;

    let window_loc = to_pair(window_loc, "window_loc")?;
    let window_size = to_pair(window_size, "window_size")?;
    let args = args.map(ListOrString::into_list).transpose()?;

    let has_exact_launch_inputs = executable.is_some() || args.is_some() || cwd.is_some();
    if mode != AppMode::LaunchExecutable && has_exact_launch_inputs {
        return Err(r#"executable, args, and cwd require mode="launch_executable""#.to_string());
    }

    if mode == AppMode::LaunchExecutable {
        let Some(executable) = executable else {
            return Err(r#"executable is required for mode="launch_executable""#.to_string());
        };
        if name.is_some() || window_loc.is_some() || window_size.is_some() {
            return Err(
                "name, window_loc, and window_size are not supported for mode=\"launch_executable\"".to_string(),
            );
        }
        return launch_executable(&executable, args.unwrap_or_default(), cwd.as_deref());
    }

    Ok(match mode {
        AppMode::Launch => launch(name.as_deref()),
        AppMode::Resize => resize(name.as_deref(), window_loc, window_size),
        AppMode::Switch => switch(name.as_deref()),
        AppMode::LaunchExecutable => unreachable!("handled above"),
    })
}

fn to_pair(value: Option<ListOrString<i32>>, field: &str) -> Result<Option<(i32, i32)>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let list = value.into_list()?;
    match list.as_slice() {
        [x, y] => Ok(Some((*x, *y))),
        _ => Err(format!(
            "{field} must have exactly 2 elements [x, y], got {}",
            list.len()
        )),
    }
}

fn launch(name: Option<&str>) -> String {
    let Some(name) = name else {
        return r#"name is required for mode="launch""#.to_string();
    };
    let (response, status, pid) = apps::launch_app(name);
    if status != 0 {
        return response;
    }
    if window::wait_for_window(pid, name, Duration::from_secs(10)) {
        format!("{} launched.", title_case(name))
    } else {
        format!(
            "Launching {} sent, but window not detected yet.",
            title_case(name)
        )
    }
}

fn resize(name: Option<&str>, loc: Option<(i32, i32)>, size: Option<(i32, i32)>) -> String {
    let target = match name {
        Some(name) => match window::find_by_name(name) {
            Some(w) => w,
            None => return format!("Application {} not found.", title_case(name)),
        },
        None => match window::foreground_window() {
            Some(w) => w,
            None => return "No active window found".to_string(),
        },
    };

    if window::is_minimized(target.handle) {
        return format!("{} is minimized", target.title);
    }
    if window::is_maximized(target.handle) {
        return format!("{} is maximized", target.title);
    }

    match window::resize_window(target.handle, loc, size) {
        Ok((x, y, w, h)) => format!("{} resized to {w}x{h} at {x},{y}.", target.title),
        Err(e) => format!("Failed to resize {}: {e}", target.title),
    }
}

fn switch(name: Option<&str>) -> String {
    let Some(name) = name else {
        return r#"name is required for mode="switch""#.to_string();
    };
    let Some(target) = window::find_by_name(name) else {
        return format!("Application {} not found.", title_case(name));
    };

    let was_minimized = window::is_minimized(target.handle);
    window::switch_to(target.handle);
    if was_minimized {
        format!(
            "Restored {} from minimized and switched to it.",
            title_case(&target.title)
        )
    } else {
        format!("Switched to {} window.", title_case(&target.title))
    }
}

fn launch_executable(
    executable: &str,
    args: Vec<String>,
    cwd: Option<&str>,
) -> Result<String, String> {
    let resolved_executable = resolve_executable(executable)?;
    let resolved_cwd = cwd.map(resolve_cwd).transpose()?;

    let mut command = std::process::Command::new(&resolved_executable);
    command
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(dir) = &resolved_cwd {
        command.current_dir(dir);
    }

    let child = command
        .spawn()
        .map_err(|e| format!("Failed to launch executable: {e}"))?;

    let payload = serde_json::json!({
        "pid": child.id(),
        "executable": resolved_executable,
        "args": args,
        "cwd": resolved_cwd,
    });
    Ok(serde_json::to_string_pretty(&payload).unwrap_or_default())
}

fn expand_user(path: &str) -> String {
    match path.strip_prefix('~') {
        Some("") => crate::win::home_dir(),
        Some(rest) => match rest.strip_prefix(['/', '\\']) {
            Some(tail) => format!("{}\\{tail}", crate::win::home_dir()),
            None => path.to_string(),
        },
        None => path.to_string(),
    }
}

fn resolve_executable(executable: &str) -> Result<String, String> {
    let expanded = expand_user(executable);
    let absolute =
        std::path::absolute(&expanded).unwrap_or_else(|_| Path::new(&expanded).to_path_buf());
    if !absolute.is_file() {
        return Err(format!("Executable does not exist: {}", absolute.display()));
    }
    Ok(absolute.display().to_string())
}

fn resolve_cwd(cwd: &str) -> Result<String, String> {
    let expanded = expand_user(cwd);
    let absolute =
        std::path::absolute(&expanded).unwrap_or_else(|_| Path::new(&expanded).to_path_buf());
    if !absolute.is_dir() {
        return Err(format!(
            "Working directory does not exist: {}",
            absolute.display()
        ));
    }
    Ok(absolute.display().to_string())
}
