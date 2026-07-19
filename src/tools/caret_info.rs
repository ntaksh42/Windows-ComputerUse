//! `CaretInfo` tool: reports the focused text caret or selection.

use rmcp::schemars;
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CaretInfoParams {}

pub fn caret_info() -> Result<String, String> {
    crate::uia::caret_info()
}
