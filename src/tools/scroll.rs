//! `Scroll` tool: mouse wheel scrolling at coordinates, a UI element label,
//! or the current cursor position.

use std::time::Duration;

use rmcp::schemars;
use serde::Deserialize;
use windows::Win32::UI::Input::KeyboardAndMouse::VK_SHIFT;

use crate::input_sim;
use crate::params::ListOrString;
use crate::tools::support::resolve_point_optional;

const NOTCH_INTERVAL: Duration = Duration::from_millis(50);
const WHEEL_TRAILING_WAIT: Duration = Duration::from_millis(500);
const SHIFT_KEY_WAIT: Duration = Duration::from_millis(50);
const MOVE_WAIT: Duration = Duration::from_millis(500);

#[derive(Debug, Deserialize, schemars::JsonSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ScrollType {
    Horizontal,
    Vertical,
}

impl ScrollType {
    fn label(self) -> &'static str {
        match self {
            ScrollType::Horizontal => "horizontal",
            ScrollType::Vertical => "vertical",
        }
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

impl ScrollDirection {
    fn label(self) -> &'static str {
        match self {
            ScrollDirection::Up => "up",
            ScrollDirection::Down => "down",
            ScrollDirection::Left => "left",
            ScrollDirection::Right => "right",
        }
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ScrollParams {
    /// Target coordinates `[x, y]`. Optional — defaults to the current
    /// cursor position.
    pub loc: Option<ListOrString<i32>>,
    /// UI element label/id from the most recent Snapshot.
    pub label: Option<i64>,
    /// Scroll axis: `vertical` (default) or `horizontal`.
    #[serde(rename = "type")]
    pub scroll_type: Option<ScrollType>,
    /// Scroll direction: `up`/`down` for vertical, `left`/`right` for
    /// horizontal. Defaults to `down`.
    pub direction: Option<ScrollDirection>,
    /// Number of wheel notches. Defaults to 1.
    pub wheel_times: Option<i64>,
}

/// Scrolls at the resolved location (or the current cursor position) and
/// returns the confirmation message, or an error string for an invalid
/// type/direction combination.
pub fn scroll(params: ScrollParams) -> Result<String, String> {
    let point = resolve_point_optional(params.loc, params.label)?;
    let scroll_type = params.scroll_type.unwrap_or(ScrollType::Vertical);
    let direction = params.direction.unwrap_or(ScrollDirection::Down);
    let wheel_times = params.wheel_times.unwrap_or(1) as i32;

    if let Some((x, y)) = point {
        input_sim::move_smooth(x, y, 10.0, MOVE_WAIT);
    }

    match (scroll_type, direction) {
        (ScrollType::Vertical, ScrollDirection::Up) => {
            input_sim::wheel(wheel_times, NOTCH_INTERVAL, WHEEL_TRAILING_WAIT);
        }
        (ScrollType::Vertical, ScrollDirection::Down) => {
            input_sim::wheel(-wheel_times, NOTCH_INTERVAL, WHEEL_TRAILING_WAIT);
        }
        (ScrollType::Vertical, _) => {
            return Err(r#"Invalid direction. Use "up" or "down"."#.to_string());
        }
        (ScrollType::Horizontal, ScrollDirection::Left) => {
            input_sim::key_down(VK_SHIFT.0);
            std::thread::sleep(SHIFT_KEY_WAIT);
            input_sim::wheel(wheel_times, NOTCH_INTERVAL, WHEEL_TRAILING_WAIT);
            std::thread::sleep(NOTCH_INTERVAL);
            input_sim::key_up(VK_SHIFT.0);
            std::thread::sleep(SHIFT_KEY_WAIT);
        }
        (ScrollType::Horizontal, ScrollDirection::Right) => {
            input_sim::key_down(VK_SHIFT.0);
            std::thread::sleep(SHIFT_KEY_WAIT);
            input_sim::wheel(-wheel_times, NOTCH_INTERVAL, WHEEL_TRAILING_WAIT);
            std::thread::sleep(NOTCH_INTERVAL);
            input_sim::key_up(VK_SHIFT.0);
            std::thread::sleep(SHIFT_KEY_WAIT);
        }
        (ScrollType::Horizontal, _) => {
            return Err(r#"Invalid direction. Use "left" or "right"."#.to_string());
        }
    }

    let (x, y) = point.unwrap_or_else(input_sim::get_cursor_pos);
    Ok(format!(
        "Scrolled {} {} by {wheel_times} wheel times at ({x},{y}).",
        scroll_type.label(),
        direction.label()
    ))
}
