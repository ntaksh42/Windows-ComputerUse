//! `Wait` tool: pauses execution for a bounded number of seconds.

use rmcp::schemars;
use serde::Deserialize;

/// Minimum allowed `duration` value, in seconds.
pub const MIN_DURATION: i64 = 1;
/// Maximum allowed `duration` value, in seconds.
pub const MAX_DURATION: i64 = 60;

/// Parameters for the `Wait` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WaitParams {
    /// Number of seconds to wait.
    #[schemars(description = "Number of seconds to wait (1-60).")]
    pub duration: i64,
}

/// Waits for `duration` seconds and returns a status message.
///
/// Returns `Err` with a caller-facing message when `duration` falls outside
/// the allowed `1..=60` range.
pub async fn wait(duration: i64) -> Result<String, String> {
    if !(MIN_DURATION..=MAX_DURATION).contains(&duration) {
        return Err(format!(
            "duration must be between {MIN_DURATION} and {MAX_DURATION} seconds, got {duration}"
        ));
    }

    tokio::time::sleep(std::time::Duration::from_secs(duration as u64)).await;
    Ok(format!("Waited for {duration} seconds."))
}
