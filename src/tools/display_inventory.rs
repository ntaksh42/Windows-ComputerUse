//! `DisplayInventory` tool: read-only display layout and DPI metadata.

use rmcp::schemars;
use serde::{Deserialize, Serialize};

use crate::display;

/// Parameters for the `DisplayInventory` tool (none).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DisplayInventoryParams {}

#[derive(Debug, Serialize)]
struct RectJson {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    width: i32,
    height: i32,
}

impl From<display::DisplayRect> for RectJson {
    fn from(rect: display::DisplayRect) -> Self {
        Self {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
            width: rect.width(),
            height: rect.height(),
        }
    }
}

#[derive(Debug, Serialize)]
struct DisplayEntry {
    index: usize,
    device: String,
    primary: bool,
    bounds: RectJson,
    work_area: Option<RectJson>,
    resolution: String,
    orientation: String,
    effective_dpi: Option<u32>,
    scale: Option<f64>,
}

impl From<&display::Display> for DisplayEntry {
    fn from(d: &display::Display) -> Self {
        Self {
            index: d.index,
            device: d.device.clone(),
            primary: d.primary,
            bounds: d.bounds.into(),
            work_area: d.work_area.map(Into::into),
            resolution: d.resolution(),
            orientation: d.orientation.clone(),
            effective_dpi: d.effective_dpi,
            scale: d.scale,
        }
    }
}

/// Returns a pretty-printed JSON array describing every active display.
pub fn display_inventory() -> String {
    let displays = display::get_displays();
    let entries: Vec<DisplayEntry> = displays.iter().map(DisplayEntry::from).collect();
    serde_json::to_string_pretty(&entries).unwrap_or_else(|e| format!("Error serializing display inventory: {e}"))
}
