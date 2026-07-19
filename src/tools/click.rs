//! `Click` tool: mouse clicks at coordinates or a UI element label.

use std::time::Duration;

use rmcp::schemars;
use serde::Deserialize;

use crate::input_sim::{self, MouseButton};
use crate::params::ListOrString;
use crate::tools::support::resolve_point_required;

const AFTER_CLICK_WAIT: Duration = Duration::from_millis(500);

#[derive(Debug, Deserialize, schemars::JsonSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ClickButton {
    Left,
    Right,
    Middle,
}

impl ClickButton {
    fn as_mouse_button(self) -> MouseButton {
        match self {
            ClickButton::Left => MouseButton::Left,
            ClickButton::Right => MouseButton::Right,
            ClickButton::Middle => MouseButton::Middle,
        }
    }

    fn label(self) -> &'static str {
        match self {
            ClickButton::Left => "left",
            ClickButton::Right => "right",
            ClickButton::Middle => "middle",
        }
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ClickParams {
    /// Target coordinates `[x, y]`. Provide either `loc` or `label`.
    pub loc: Option<ListOrString<i32>>,
    /// UI element label/id from the most recent Snapshot. Provide either
    /// `loc` or `label`.
    pub label: Option<i64>,
    /// Mouse button to use. Defaults to `left`.
    pub button: Option<ClickButton>,
    /// Number of clicks: 0 = hover only, 1 = single click, 2 = double click.
    /// Defaults to 1.
    pub clicks: Option<i64>,
}

/// Performs `clicks` clicks with `button` at the resolved location.
pub fn click(params: ClickParams) -> Result<String, String> {
    let (x, y) = resolve_point_required(params.loc, params.label)?;
    let button = params.button.unwrap_or(ClickButton::Left);
    let clicks = params.clicks.unwrap_or(1);
    if !(0..=2).contains(&clicks) {
        return Err("clicks must be 0 (hover), 1 (single), or 2 (double).".to_string());
    }

    if clicks == 0 {
        input_sim::set_cursor_pos(x, y);
    } else if button == ClickButton::Left && clicks == 2 {
        let dbl_wait = Duration::from_millis((input_sim::get_double_click_time_ms() / 2) as u64);
        for i in 0..clicks {
            let wait_after = if i < clicks - 1 {
                dbl_wait
            } else {
                AFTER_CLICK_WAIT
            };
            input_sim::click_once(x, y, button.as_mouse_button(), wait_after);
        }
    } else {
        for _ in 0..clicks {
            input_sim::click_once(x, y, button.as_mouse_button(), AFTER_CLICK_WAIT);
        }
    }

    let clicks_word = match clicks {
        0 => "Hover",
        1 => "Single",
        2 => "Double",
        _ => unreachable!("click count validated above"),
    };
    Ok(format!(
        "{clicks_word} {} clicked at ({x},{y}).",
        button.label()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsupported_click_count_before_input() {
        let result = click(ClickParams {
            loc: Some(ListOrString::List(vec![10, 20])),
            label: None,
            button: None,
            clicks: Some(3),
        });
        assert!(result.unwrap_err().contains("clicks must be"));
    }
}
