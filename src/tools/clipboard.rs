//! `Clipboard` tool: read/write the Windows text clipboard.

use rmcp::schemars;
use serde::Deserialize;

/// Parameters for the `Clipboard` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ClipboardParams {
    /// "get" to read the current clipboard text, "set" to write it.
    #[schemars(description = "Clipboard operation mode: \"get\" or \"set\".")]
    pub mode: ClipboardMode,
    /// Text to place on the clipboard. Required for `mode: "set"`.
    #[schemars(description = "Text to place on the clipboard (required for set mode).")]
    pub text: Option<String>,
}

/// `Clipboard` operation mode.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardMode {
    Get,
    Set,
}

/// Runs the `Clipboard` tool. Always returns a caller-facing text response.
pub fn clipboard(mode: ClipboardMode, text: Option<String>) -> String {
    match mode {
        ClipboardMode::Get => get(),
        ClipboardMode::Set => set(text),
    }
}

fn get() -> String {
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => return format!("Error: Failed to access clipboard: {e}"),
    };
    match clipboard.get_text() {
        Ok(data) => format!("Clipboard content:\n{data}"),
        Err(arboard::Error::ContentNotAvailable) => {
            "Clipboard is empty or contains non-text data.".to_string()
        }
        Err(e) => format!("Error: Failed to read clipboard: {e}"),
    }
}

fn set(text: Option<String>) -> String {
    let Some(text) = text else {
        return "Error: text parameter required for set mode.".to_string();
    };
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => return format!("Error: Failed to access clipboard: {e}"),
    };
    match clipboard.set_text(text.clone()) {
        Ok(()) => {
            let preview: String = text.chars().take(100).collect();
            let ellipsis = if text.chars().count() > 100 { "..." } else { "" };
            format!("Clipboard set to: {preview}{ellipsis}")
        }
        Err(e) => format!("Error: Failed to write clipboard: {e}"),
    }
}
