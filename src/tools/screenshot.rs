//! `Screenshot` tool: fast screenshot-first desktop capture.
//!
//! This is the vision-only, UI-tree-skipping counterpart to the (not yet
//! implemented) `Snapshot` tool: it captures the desktop (or a subset of
//! displays), scales it to fit within 1920x1080, optionally overlays a
//! reference grid, and returns a text summary plus a PNG image.

use std::env;
use std::time::Instant;

use image::ImageEncoder;
use image::imageops::FilterType;
use rmcp::schemars;
use serde::Deserialize;
use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

use crate::capture;
use crate::display;
use crate::params::{BoolOrString, ListOrString, opt_bool};

/// Screenshots are downscaled to fit within this size (before applying
/// `WINDOWS_MCP_SCREENSHOT_SCALE`).
pub const MAX_IMAGE_WIDTH: u32 = 1920;
pub const MAX_IMAGE_HEIGHT: u32 = 1080;

/// Parameters for the `Screenshot` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ScreenshotParams {
    /// Currently a no-op: this tool has no UI element/node information to
    /// annotate with, so the parameter is accepted (for interface
    /// compatibility) but ignored.
    #[schemars(description = "Reserved for future bounding-box annotation. Currently ignored.")]
    pub use_annotation: Option<BoolOrString>,
    #[schemars(
        description = "Number of vertical grid divisions to overlay; only takes effect when height_reference_line is also set."
    )]
    pub width_reference_line: Option<i64>,
    #[schemars(
        description = "Number of horizontal grid divisions to overlay; only takes effect when width_reference_line is also set."
    )]
    pub height_reference_line: Option<i64>,
    #[schemars(
        description = "Zero-based active display indices to restrict the capture to; omit for the full virtual desktop."
    )]
    pub display: Option<ListOrString<i32>>,
}

/// Text + PNG bytes making up a successful `Screenshot` response.
pub struct ScreenshotOutput {
    pub text: String,
    pub png_bytes: Vec<u8>,
}

/// Resolves `WINDOWS_MCP_SCREENSHOT_SCALE`, clamped to `0.1..=1.0`. Falls
/// back to `1.0` when unset or unparsable.
pub fn resolve_scale() -> f64 {
    resolve_scale_from(env::var("WINDOWS_MCP_SCREENSHOT_SCALE").ok().as_deref())
}

fn resolve_scale_from(raw: Option<&str>) -> f64 {
    let scale = raw
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or(1.0);
    scale.clamp(0.1, 1.0)
}

pub(crate) fn profiling_enabled() -> bool {
    matches!(
        env::var("WINDOWS_MCP_PROFILE_SNAPSHOT")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Combines the user-requested scale with the 1920x1080 cap into a single
/// downscale factor (never > 1.0, since neither input can exceed 1.0).
pub fn combined_scale(orig_width: u32, orig_height: u32, user_scale: f64) -> f64 {
    let scale_width = if orig_width > MAX_IMAGE_WIDTH {
        MAX_IMAGE_WIDTH as f64 / orig_width as f64
    } else {
        1.0
    };
    let scale_height = if orig_height > MAX_IMAGE_HEIGHT {
        MAX_IMAGE_HEIGHT as f64 / orig_height as f64
    } else {
        1.0
    };
    user_scale.min(scale_width).min(scale_height)
}

/// Applies `scale` to `(orig_width, orig_height)`, truncating towards zero
/// (matching Python's `int(width * scale)`).
pub fn scaled_size(orig_width: u32, orig_height: u32, scale: f64) -> (u32, u32) {
    (
        ((orig_width as f64) * scale) as u32,
        ((orig_height as f64) * scale) as u32,
    )
}

/// Builds the "Screenshot Coordinate Scale" explanatory line shown when a
/// screenshot has been downscaled.
pub fn coordinate_scale_text(coord_scale: f64) -> String {
    let sample_x = (200.0 * coord_scale).round() as i64;
    let sample_y = (150.0 * coord_scale).round() as i64;
    format!(
        "Screenshot Coordinate Scale: {coord_scale} \u{2014} image pixels are downscaled; \
         multiply every image pixel coordinate by {coord_scale} before passing to Click, Move, \
         Scroll, or any loc= argument (e.g. image pixel (200, 150) \u{2192} screen coordinate \
         ({sample_x}, {sample_y}))"
    )
}

/// Overlays a light grid (`w_count` vertical, `h_count` horizontal
/// divisions) onto `image` for spatial reference.
pub(crate) fn draw_grid_lines(image: &mut image::RgbaImage, w_count: i64, h_count: i64) {
    if w_count <= 0 || h_count <= 0 {
        return;
    }
    let width = image.width() as i64;
    let height = image.height() as i64;
    let color = image::Rgba([200u8, 200, 200, 128]);
    for i in 1..w_count {
        let x = width * i / w_count;
        if (0..width).contains(&x) {
            for y in 0..height {
                image.put_pixel(x as u32, y as u32, color);
            }
        }
    }
    for i in 1..h_count {
        let y = height * i / h_count;
        if (0..height).contains(&y) {
            for x in 0..width {
                image.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}

pub(crate) fn cursor_position() -> (i32, i32) {
    let mut point = POINT::default();
    let ok = unsafe { GetCursorPos(&mut point) };
    if ok.is_ok() {
        (point.x, point.y)
    } else {
        (0, 0)
    }
}

pub(crate) fn display_list_text(displays: &[display::Display]) -> String {
    displays
        .iter()
        .map(|d| {
            let primary = if d.primary { " primary" } else { "" };
            format!(
                "{}:{} ({},{},{},{}){}",
                d.index,
                d.device,
                d.bounds.left,
                d.bounds.top,
                d.bounds.right,
                d.bounds.bottom,
                primary
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Executes the `Screenshot` tool: captures the desktop (or the union of the
/// selected displays), scales it to fit the size cap, optionally overlays a
/// reference grid, and builds the text + PNG response.
///
/// Any failure (invalid `display` selection, capture failure, encode
/// failure) is returned as `Err` with a caller-facing message; the caller is
/// expected to wrap it as `"Error capturing screenshot: {e}. Please try
/// again."` per the tool's error contract.
pub fn screenshot(params: &ScreenshotParams) -> Result<ScreenshotOutput, String> {
    let _ = opt_bool(&params.use_annotation, false)?; // accepted, currently unused

    let profile = profiling_enabled();
    let started = Instant::now();

    let display_indices: Option<Vec<usize>> = match &params.display {
        None => None,
        Some(list) => {
            let raw = list.clone().into_list()?;
            Some(raw.into_iter().map(|v| v as usize).collect())
        }
    };

    let capture_start = Instant::now();
    let (capture_rect, region_text): (RECT, Option<(String, String)>) = match &display_indices {
        None => (capture::virtual_screen_rect(), None),
        Some(indices) => {
            let rect = display::get_display_union_rect(indices)?;
            let csv = indices
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let region = format!(
                "({},{},{},{})",
                rect.left, rect.top, rect.right, rect.bottom
            );
            (rect, Some((csv, region)))
        }
    };

    let backend = capture::resolve_backend();
    let captured = capture::capture_rect(capture_rect, backend)?;
    let capture_ms = capture_start.elapsed().as_secs_f64() * 1000.0;

    let orig_width = captured.width();
    let orig_height = captured.height();
    let user_scale = resolve_scale();
    let scale = combined_scale(orig_width, orig_height, user_scale);

    let resize_start = Instant::now();
    let mut image = if scale != 1.0 {
        let (w, h) = scaled_size(orig_width, orig_height, scale);
        image::imageops::resize(&captured, w.max(1), h.max(1), FilterType::Lanczos3)
    } else {
        captured
    };
    let resize_ms = resize_start.elapsed().as_secs_f64() * 1000.0;

    if let (Some(w), Some(h)) = (params.width_reference_line, params.height_reference_line) {
        draw_grid_lines(&mut image, w, h);
    }

    let encode_start = Instant::now();
    let mut png_bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png_bytes)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("PNG encoding failed: {e}"))?;
    let encode_ms = encode_start.elapsed().as_secs_f64() * 1000.0;

    if profile {
        eprintln!(
            "Screenshot tool profile: capture_ms={capture_ms:.1} resize_ms={resize_ms:.1} \
             encode_ms={encode_ms:.1} total_ms={:.1}",
            started.elapsed().as_secs_f64() * 1000.0
        );
    }

    let (cx, cy) = cursor_position();
    let mut text = format!("Cursor Position: ({cx}, {cy})\n");

    if scale < 1.0 {
        let coord_scale = (1.0 / scale * 1_000_000.0).round() / 1_000_000.0;
        text += &format!("Screenshot Original Size: ({orig_width},{orig_height})\n");
        text += &coordinate_scale_text(coord_scale);
        text += "\n";
    } else {
        text += &format!("Screenshot Size: ({},{})\n", image.width(), image.height());
    }

    let displays = display::get_displays();
    if !displays.is_empty() {
        text += &format!("Visible Displays: {}\n", display_list_text(&displays));
    }

    if let Some((csv, region)) = &region_text {
        text += &format!("Selected Displays: {csv}\n");
        text += &format!("Screenshot Region: {region}\n");
        text += "Coordinate Space: Virtual desktop coordinates\n";
    }

    text += &format!("Screenshot Backend: {}\n", backend.name());
    text += "UI Tree: Skipped for fast screenshot-only capture. Call Snapshot when you need interactive or scrollable elements.\n";
    text += "\nFocused Window:\nNo active window found\n\nOpened Windows:\nNo windows found\n";

    Ok(ScreenshotOutput { text, png_bytes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_clamps_below_minimum() {
        assert_eq!(resolve_scale_from(Some("0.01")), 0.1);
    }

    #[test]
    fn scale_clamps_above_maximum() {
        assert_eq!(resolve_scale_from(Some("2.5")), 1.0);
    }

    #[test]
    fn scale_falls_back_to_default_on_parse_failure() {
        assert_eq!(resolve_scale_from(Some("not-a-number")), 1.0);
    }

    #[test]
    fn scale_defaults_when_unset() {
        assert_eq!(resolve_scale_from(None), 1.0);
    }

    #[test]
    fn scale_within_range_is_unchanged() {
        assert_eq!(resolve_scale_from(Some("0.5")), 0.5);
    }

    #[test]
    fn cap_does_not_apply_below_the_limit() {
        let scale = combined_scale(800, 600, 1.0);
        assert_eq!(scale, 1.0);
        assert_eq!(scaled_size(800, 600, scale), (800, 600));
    }

    #[test]
    fn cap_shrinks_oversized_image_to_the_limit() {
        let scale = combined_scale(3840, 2160, 1.0);
        let (w, h) = scaled_size(3840, 2160, scale);
        assert_eq!((w, h), (1920, 1080));
    }

    #[test]
    fn cap_combines_with_user_scale() {
        let scale = combined_scale(1920, 1080, 0.5);
        let (w, h) = scaled_size(1920, 1080, scale);
        assert_eq!((w, h), (960, 540));
    }

    #[test]
    fn coordinate_scale_text_reports_multiplier_and_example() {
        let text = coordinate_scale_text(2.0);
        assert!(text.contains("Screenshot Coordinate Scale: 2"));
        assert!(text.contains("(400, 300)"));
    }
}
