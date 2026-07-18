//! `Snapshot` tool: accessibility-tree-aware desktop capture.
//!
//! The performance-critical piece is `uia::walk_window`, which fetches an
//! entire window's interactive/scrollable subtree via a single
//! `FindAllBuildCache` call (see `src/uia.rs`) instead of walking the tree
//! element-by-element over COM. This module owns the response-text
//! formatting, the window bookkeeping (focused/opened windows, retry,
//! ordering), and the optional annotated-screenshot rendering; `state.rs`
//! is where the resulting `interactive_nodes`/`scrollable_nodes` end up so
//! Click/Type/Scroll/Move/MultiSelect/MultiEdit can resolve a `label`.

use std::time::{Duration, Instant};

use image::ImageEncoder;
use image::imageops::FilterType;
use rmcp::schemars;
use serde::Deserialize;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{
    IUIAutomation, IUIAutomationCacheRequest, IUIAutomationCondition,
};

use crate::params::{BoolOrString, ListOrString, opt_bool};
use crate::tools::screenshot;
use crate::{capture, display, state, uia, window};

/// Parameters for the `Snapshot` tool (docs/SPEC.md §6).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SnapshotParams {
    #[schemars(description = "Include a PNG screenshot in the response. Defaults to false.")]
    pub use_vision: Option<BoolOrString>,
    #[schemars(
        description = "Browser DOM extraction. Not implemented in this build; passing true returns an error."
    )]
    pub use_dom: Option<BoolOrString>,
    #[schemars(
        description = "Draw numbered bounding boxes for interactive elements on the screenshot (only applies when use_vision=true). Defaults to true."
    )]
    pub use_annotation: Option<BoolOrString>,
    #[schemars(
        description = "Walk the UIA accessibility tree and include interactive/scrollable elements plus the UI Tree text. Defaults to true."
    )]
    pub use_ui_tree: Option<BoolOrString>,
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

/// Text + optional PNG bytes making up a successful `Snapshot` response.
pub struct SnapshotOutput {
    pub text: String,
    pub png_bytes: Option<Vec<u8>>,
}

/// Internal capture result: the public [`SnapshotOutput`] plus the raw
/// element lists WaitFor polls and Snapshot writes into `state.rs`.
pub(crate) struct SnapshotResult {
    pub text: String,
    pub png_bytes: Option<Vec<u8>>,
    pub interactive_nodes: Vec<state::ElementNode>,
    pub scrollable_nodes: Vec<state::ElementNode>,
    pub focused_window_title: Option<String>,
    pub window_titles: Vec<String>,
}

impl SnapshotResult {
    pub(crate) fn to_desktop_state(&self) -> state::DesktopState {
        state::DesktopState {
            interactive_nodes: self.interactive_nodes.clone(),
            scrollable_nodes: self.scrollable_nodes.clone(),
        }
    }
}

/// Executes the `Snapshot` tool. `use_dom=true` is rejected up front (not
/// implemented, docs/SPEC.md §6 item 7); any other capture failure is
/// wrapped as `"Error capturing desktop state: {e}. Please try again."`.
/// On success, the accessibility-tree state is written to `state.rs` so
/// subsequent Click/Type/Scroll/Move calls can resolve `label`s.
pub fn snapshot(params: &SnapshotParams) -> Result<SnapshotOutput, String> {
    let use_dom = opt_bool(&params.use_dom, false)?;
    if use_dom {
        return Err("DOM mode not supported yet.".to_string());
    }
    let result = capture(params)
        .map_err(|e| format!("Error capturing desktop state: {e}. Please try again."))?;
    state::set_state(result.to_desktop_state());
    Ok(SnapshotOutput {
        text: result.text,
        png_bytes: result.png_bytes,
    })
}

/// One window's UI-tree children, pre-formatted for the `UI Tree:` section.
struct WindowTree {
    name: String,
    children: Vec<String>,
}

/// Runs `uia::walk_window` with up to 3 retries and exponential backoff
/// (0.5s, 1s, 2s — docs/SPEC.md "Snapshot 性能設計"). Returns `None` if the
/// window never succeeds, matching the Python reference's
/// `failed_handles`: that window silently contributes nothing.
fn walk_window_with_retry(
    automation: &IUIAutomation,
    cache_request: &IUIAutomationCacheRequest,
    condition: &IUIAutomationCondition,
    hwnd: HWND,
) -> Option<(uia::RawElement, Vec<uia::RawElement>)> {
    const MAX_RETRIES: u32 = 3;
    for attempt in 0..=MAX_RETRIES {
        match uia::walk_window(automation, cache_request, condition, hwnd) {
            Ok(result) => return Some(result),
            Err(_) if attempt < MAX_RETRIES => {
                std::thread::sleep(Duration::from_millis(500 * (1u64 << attempt)));
            }
            Err(_) => return None,
        }
    }
    None
}

fn format_tree_line(node: &state::ElementNode, action: &str) -> String {
    format!(
        "({},{}) {} \"{}\"  [action: {action}]",
        node.center.0, node.center.1, node.control_type, node.name
    )
}

/// Renders the `UI Tree:` box-drawing text: `desktop` root, one `window
/// "..."` child per window that contributed elements, each followed by its
/// interactive/scrollable element lines.
fn render_tree(window_trees: &[WindowTree]) -> String {
    if window_trees.is_empty() {
        return "No elements found.".to_string();
    }
    let mut lines = vec!["desktop".to_string()];
    let window_count = window_trees.len();
    for (i, tree) in window_trees.iter().enumerate() {
        let is_last_window = i == window_count - 1;
        let connector = if is_last_window {
            "\u{2514}\u{2500}\u{2500} "
        } else {
            "\u{251c}\u{2500}\u{2500} "
        };
        lines.push(format!("{connector}window \"{}\"", tree.name));
        let prefix = if is_last_window {
            "    "
        } else {
            "\u{2502}   "
        };
        let child_count = tree.children.len();
        for (j, child) in tree.children.iter().enumerate() {
            let child_connector = if j == child_count - 1 {
                "\u{2514}\u{2500}\u{2500} "
            } else {
                "\u{251c}\u{2500}\u{2500} "
            };
            lines.push(format!("{prefix}{child_connector}{child}"));
        }
    }
    lines.join("\n")
}

/// Renders a simple space-padded table (header, dashed rule, rows) —
/// structurally equivalent to the Python reference's `tabulate(tablefmt=
/// "simple")` output without pulling in a table-formatting dependency.
fn format_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }
    let mut lines = Vec::with_capacity(rows.len() + 2);
    lines.push(
        headers
            .iter()
            .enumerate()
            .map(|(i, h)| format!("{:<width$}", h, width = widths[i]))
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string(),
    );
    lines.push(
        widths
            .iter()
            .map(|w| "-".repeat(*w))
            .collect::<Vec<_>>()
            .join("  "),
    );
    for row in rows {
        lines.push(
            row.iter()
                .enumerate()
                .map(|(i, c)| format!("{:<width$}", c, width = widths[i]))
                .collect::<Vec<_>>()
                .join("  ")
                .trim_end()
                .to_string(),
        );
    }
    lines.join("\n")
}

fn window_status(handle: isize) -> String {
    if window::is_minimized(handle) {
        "Minimized".to_string()
    } else if window::is_maximized(handle) {
        "Maximized".to_string()
    } else {
        "Normal".to_string()
    }
}

const WINDOW_TABLE_HEADERS: &[&str] = &["Name", "Depth", "Status", "Width", "Height", "Handle"];

fn window_table_row(window: &window::WindowInfo, depth: usize) -> Vec<String> {
    let status = window_status(window.handle);
    let (_, _, width, height) = window::get_window_rect(window.handle).unwrap_or((0, 0, 0, 0));
    vec![
        window.title.clone(),
        depth.to_string(),
        status,
        width.to_string(),
        height.to_string(),
        window.handle.to_string(),
    ]
}

fn focused_window_text(foreground: &Option<window::WindowInfo>) -> String {
    match foreground {
        None => "No active window found".to_string(),
        Some(w) => format_table(WINDOW_TABLE_HEADERS, &[window_table_row(w, 0)]),
    }
}

/// "Active Desktop"/"All Desktops" virtual-desktop table. Hardcoded to a
/// single "Default Desktop" row — virtual desktop enumeration is out of
/// scope for this build (docs/SPEC.md §6 item 7); this matches the shape the
/// Python reference itself falls back to when the internal VDM COM
/// interface is unavailable.
fn desktop_table() -> String {
    format_table(&["Name"], &[vec!["Default Desktop".to_string()]])
}

// --- Annotated-screenshot drawing -----------------------------------------

fn hsv_to_rgb(hue: f64, saturation: f64, value: f64) -> image::Rgba<u8> {
    let c = value * saturation;
    let hh = (hue / 60.0) % 6.0;
    let x = c * (1.0 - (hh % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match hh as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = value - c;
    image::Rgba([
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
        255,
    ])
}

/// A distinct, deterministic color per element index (golden-angle hue
/// spacing keeps adjacent indices visually distinguishable).
fn index_color(index: usize) -> image::Rgba<u8> {
    let hue = (index as f64 * 137.508) % 360.0;
    hsv_to_rgb(hue, 0.65, 0.95)
}

fn put_pixel_checked(image: &mut image::RgbaImage, x: i32, y: i32, color: image::Rgba<u8>) {
    if x >= 0 && y >= 0 && (x as u32) < image.width() && (y as u32) < image.height() {
        image.put_pixel(x as u32, y as u32, color);
    }
}

fn draw_rect_outline(
    image: &mut image::RgbaImage,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    color: image::Rgba<u8>,
    thickness: i32,
) {
    for t in 0..thickness {
        for x in x1..x2 {
            put_pixel_checked(image, x, y1 + t, color);
            put_pixel_checked(image, x, y2 - 1 - t, color);
        }
        for y in y1..y2 {
            put_pixel_checked(image, x1 + t, y, color);
            put_pixel_checked(image, x2 - 1 - t, y, color);
        }
    }
}

/// 3x5 bitmap digits (no embedded font needed — SPEC allows a simple
/// bitmap/box readout for element numbers). Each row's low 3 bits are
/// pixels, MSB-first (left column = bit 2).
const DIGIT_FONT: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111], // 0
    [0b010, 0b110, 0b010, 0b010, 0b111], // 1
    [0b111, 0b001, 0b111, 0b100, 0b111], // 2
    [0b111, 0b001, 0b111, 0b001, 0b111], // 3
    [0b101, 0b101, 0b111, 0b001, 0b001], // 4
    [0b111, 0b100, 0b111, 0b001, 0b111], // 5
    [0b111, 0b100, 0b111, 0b101, 0b111], // 6
    [0b111, 0b001, 0b001, 0b001, 0b001], // 7
    [0b111, 0b101, 0b111, 0b101, 0b111], // 8
    [0b111, 0b101, 0b111, 0b001, 0b111], // 9
];

const DIGIT_SCALE: i32 = 2;

fn digit_width() -> i32 {
    3 * DIGIT_SCALE + 1
}

fn digit_height() -> i32 {
    5 * DIGIT_SCALE
}

fn draw_digit(image: &mut image::RgbaImage, x: i32, y: i32, digit: usize, color: image::Rgba<u8>) {
    for (row_idx, row) in DIGIT_FONT[digit].iter().enumerate() {
        for col in 0..3 {
            if row & (1 << (2 - col)) != 0 {
                for dy in 0..DIGIT_SCALE {
                    for dx in 0..DIGIT_SCALE {
                        put_pixel_checked(
                            image,
                            x + col * DIGIT_SCALE + dx,
                            y + row_idx as i32 * DIGIT_SCALE + dy,
                            color,
                        );
                    }
                }
            }
        }
    }
}

/// Draws a filled label rectangle with `index`'s digits in white at `(x, y)`
/// (top-left corner).
fn draw_label(image: &mut image::RgbaImage, x: i32, y: i32, index: usize, color: image::Rgba<u8>) {
    let digits: Vec<usize> = index
        .to_string()
        .chars()
        .filter_map(|c| c.to_digit(10))
        .map(|d| d as usize)
        .collect();
    let width = digit_width() * digits.len() as i32 + 2;
    let height = digit_height() + 2;
    for dy in 0..height {
        for dx in 0..width {
            put_pixel_checked(image, x + dx, y + dy, color);
        }
    }
    let white = image::Rgba([255, 255, 255, 255]);
    for (i, digit) in digits.iter().enumerate() {
        draw_digit(
            image,
            x + 1 + i as i32 * digit_width(),
            y + 1,
            *digit,
            white,
        );
    }
}

/// Draws a numbered bounding box for each `interactive_nodes` entry
/// (scrollable nodes are not drawn — they have no visual box in the Python
/// reference either, only `label` addressability).
fn draw_annotations(
    image: &mut image::RgbaImage,
    nodes: &[state::ElementNode],
    capture_left: i32,
    capture_top: i32,
    scale: f64,
) {
    for (index, node) in nodes.iter().enumerate() {
        let (left, top, right, bottom) = node.bounding_box;
        let x1 = (((left - capture_left) as f64) * scale).round() as i32;
        let y1 = (((top - capture_top) as f64) * scale).round() as i32;
        let x2 = (((right - capture_left) as f64) * scale).round() as i32;
        let y2 = (((bottom - capture_top) as f64) * scale).round() as i32;
        if x2 <= x1 || y2 <= y1 {
            continue;
        }
        let color = index_color(index);
        draw_rect_outline(image, x1, y1, x2, y2, color, 2);
        let label_y = if y1 - digit_height() - 3 >= 0 {
            y1 - digit_height() - 3
        } else {
            y2 + 2
        };
        draw_label(image, x1, label_y, index, color);
    }
}

// --- Capture ---------------------------------------------------------------

/// Runs the full capture: window enumeration, the UIA tree walk (if
/// `use_ui_tree`), the screenshot + annotation (if `use_vision`), and
/// assembles the response text. Returns a caller-facing error message (not
/// yet wrapped with "Error capturing desktop state: ...").
pub(crate) fn capture(params: &SnapshotParams) -> Result<SnapshotResult, String> {
    let use_vision = opt_bool(&params.use_vision, false)?;
    let use_annotation = opt_bool(&params.use_annotation, true)?;
    let use_ui_tree = opt_bool(&params.use_ui_tree, true)?;
    let display_indices: Option<Vec<usize>> = match &params.display {
        None => None,
        Some(list) => {
            let raw = list.clone().into_list()?;
            Some(raw.into_iter().map(|v| v as usize).collect())
        }
    };

    let profile = screenshot::profiling_enabled();
    let total_start = Instant::now();

    // --- Window enumeration ---
    let window_start = Instant::now();
    let table_windows = window::list_windows();
    let foreground = window::foreground_window();
    let walk_windows = if use_ui_tree {
        window::list_snapshot_windows()
    } else {
        Vec::new()
    };
    let window_ms = window_start.elapsed().as_secs_f64() * 1000.0;

    // --- UIA tree walk ---
    let uia_start = Instant::now();
    let mut interactive_nodes: Vec<state::ElementNode> = Vec::new();
    let mut scrollable_nodes: Vec<state::ElementNode> = Vec::new();
    let mut window_trees: Vec<WindowTree> = Vec::new();

    if use_ui_tree && !walk_windows.is_empty() {
        uia::ensure_com_initialized()?;
        let automation = uia::create_automation().map_err(|e| e.to_string())?;
        let cache_request = uia::build_cache_request(&automation).map_err(|e| e.to_string())?;
        let condition = uia::build_condition(&automation).map_err(|e| e.to_string())?;

        // Active/foreground window first so it gets the lowest label
        // indices, then the rest in enumeration order.
        let mut ordered: Vec<&window::SnapshotWindow> = Vec::new();
        if let Some(fg) = &foreground
            && let Some(w) = walk_windows.iter().find(|w| w.handle == fg.handle)
        {
            ordered.push(w);
        }
        for w in &walk_windows {
            if foreground.as_ref().map(|f| f.handle) != Some(w.handle) {
                ordered.push(w);
            }
        }

        for win in ordered {
            let hwnd = HWND(win.handle as *mut _);
            let Some((root_raw, elements)) =
                walk_window_with_retry(&automation, &cache_request, &condition, hwnd)
            else {
                continue; // window never succeeded; contributes nothing
            };

            let window_label = if !win.title.is_empty() {
                win.title.clone()
            } else if !root_raw.name.is_empty() {
                root_raw.name.clone()
            } else {
                win.class_name.clone()
            };

            let mut local_interactive: Vec<state::ElementNode> = Vec::new();
            let mut local_scrollable: Vec<state::ElementNode> = Vec::new();

            for el in elements {
                if el.control_type == uia::WINDOW_CONTROL_TYPE {
                    // Nested modal dialog: discard everything accumulated
                    // for this window so far (docs/SPEC.md §6 item 4).
                    if el.is_modal {
                        local_interactive.clear();
                    }
                    continue;
                }
                let (left, top, right, bottom) =
                    (el.rect.left, el.rect.top, el.rect.right, el.rect.bottom);
                if right <= left || bottom <= top {
                    continue;
                }
                let center = (left + (right - left) / 2, top + (bottom - top) / 2);
                let node = state::ElementNode {
                    name: el.name.clone(),
                    control_type: uia::control_type_name(el.control_type),
                    center,
                    bounding_box: (left, top, right, bottom),
                    has_focus: el.has_keyboard_focus,
                };
                let is_interactive_type = uia::INTERACTIVE_CONTROL_TYPES.contains(&el.control_type);
                if is_interactive_type && el.is_enabled && !el.is_offscreen {
                    local_interactive.push(node);
                } else if el.is_scrollable && !el.is_offscreen {
                    local_scrollable.push(node);
                }
            }

            if !local_interactive.is_empty() || !local_scrollable.is_empty() {
                let mut children =
                    Vec::with_capacity(local_interactive.len() + local_scrollable.len());
                children.extend(
                    local_interactive
                        .iter()
                        .map(|n| format_tree_line(n, "click")),
                );
                children.extend(
                    local_scrollable
                        .iter()
                        .map(|n| format_tree_line(n, "scroll")),
                );
                window_trees.push(WindowTree {
                    name: window_label,
                    children,
                });
            }

            interactive_nodes.extend(local_interactive);
            scrollable_nodes.extend(local_scrollable);
        }
    }
    let uia_ms = uia_start.elapsed().as_secs_f64() * 1000.0;

    // --- Screenshot + annotation (only when use_vision) ---
    let image_start = Instant::now();
    let mut png_bytes: Option<Vec<u8>> = None;
    let mut screenshot_size_line: Option<String> = None;
    let mut backend_name: Option<&'static str> = None;
    let mut region_text: Option<(String, String)> = None;

    if use_vision {
        let (capture_rect, region) = match &display_indices {
            None => (capture::virtual_screen_rect(), None),
            Some(indices) => {
                let rect = display::get_display_union_rect(indices)?;
                let csv = indices
                    .iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                let region_str = format!(
                    "({},{},{},{})",
                    rect.left, rect.top, rect.right, rect.bottom
                );
                (rect, Some((csv, region_str)))
            }
        };
        region_text = region;

        let backend = capture::resolve_backend();
        backend_name = Some(backend.name());
        let captured = capture::capture_rect(capture_rect, backend)?;

        let orig_width = captured.width();
        let orig_height = captured.height();
        let user_scale = screenshot::resolve_scale();
        let scale = screenshot::combined_scale(orig_width, orig_height, user_scale);

        let mut image = if scale != 1.0 {
            let (w, h) = screenshot::scaled_size(orig_width, orig_height, scale);
            image::imageops::resize(&captured, w.max(1), h.max(1), FilterType::Lanczos3)
        } else {
            captured
        };

        if use_annotation {
            draw_annotations(
                &mut image,
                &interactive_nodes,
                capture_rect.left,
                capture_rect.top,
                scale,
            );
        }

        if let (Some(w), Some(h)) = (params.width_reference_line, params.height_reference_line) {
            screenshot::draw_grid_lines(&mut image, w, h);
        }

        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| format!("PNG encoding failed: {e}"))?;
        png_bytes = Some(bytes);

        screenshot_size_line = Some(if scale < 1.0 {
            let coord_scale = (1.0 / scale * 1_000_000.0).round() / 1_000_000.0;
            format!(
                "Screenshot Original Size: ({orig_width},{orig_height})\n{}",
                screenshot::coordinate_scale_text(coord_scale)
            )
        } else {
            format!("Screenshot Size: ({},{})", image.width(), image.height())
        });
    }
    let image_ms = image_start.elapsed().as_secs_f64() * 1000.0;

    // --- Response text assembly ---
    let (cx, cy) = screenshot::cursor_position();
    let mut text = format!("Cursor Position: ({cx}, {cy})\n");

    if let Some(line) = &screenshot_size_line {
        text += line;
        text += "\n";
    }

    let displays = display::get_displays();
    if !displays.is_empty() {
        text += &format!(
            "Visible Displays: {}\n",
            screenshot::display_list_text(&displays)
        );
    }

    if let Some((csv, region)) = &region_text {
        text += &format!("Selected Displays: {csv}\n");
        text += &format!("Screenshot Region: {region}\n");
        text += "Coordinate Space: Virtual desktop coordinates\n";
    }
    if let Some(name) = backend_name {
        text += &format!("Screenshot Backend: {name}\n");
    }

    text += "\nActive Desktop:\n";
    text += &desktop_table();
    text += "\n\nAll Desktops:\n";
    text += &desktop_table();

    let focused_window_title = foreground.as_ref().map(|w| w.title.clone());
    text += "\n\nFocused Window:\n";
    text += &focused_window_text(&foreground);
    text += "\n\nOpened Windows:\n";

    let mut window_titles = Vec::new();
    let opened_rows: Vec<Vec<String>> = table_windows
        .iter()
        .filter(|w| foreground.as_ref().map(|f| f.handle) != Some(w.handle))
        .enumerate()
        .map(|(i, w)| {
            window_titles.push(w.title.clone());
            window_table_row(w, i)
        })
        .collect();
    if opened_rows.is_empty() {
        text += "No windows found";
    } else {
        text += &format_table(WINDOW_TABLE_HEADERS, &opened_rows);
    }
    if let Some(title) = &focused_window_title {
        window_titles.push(title.clone());
    }

    if use_ui_tree {
        text += "\n\nUI Tree:\n";
        text += &render_tree(&window_trees);
    } else {
        text += "\n\nUI Tree: Skipped for fast screenshot-only capture. Call Snapshot when you need interactive or scrollable elements.\n";
    }

    if profile {
        eprintln!(
            "Snapshot tool profile: window_enum_ms={window_ms:.1} uia_scan_ms={uia_ms:.1} image_ms={image_ms:.1} total_ms={:.1}",
            total_start.elapsed().as_secs_f64() * 1000.0
        );
    }

    Ok(SnapshotResult {
        text,
        png_bytes,
        interactive_nodes,
        scrollable_nodes,
        focused_window_title,
        window_titles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_renders_header_rule_and_rows() {
        let table = format_table(
            &["Name", "Depth"],
            &[vec!["Notepad".to_string(), "0".to_string()]],
        );
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("Name"));
        assert!(lines[1].starts_with("----"));
        assert!(lines[2].starts_with("Notepad"));
    }

    #[test]
    fn tree_renders_desktop_root_and_indentation() {
        let trees = vec![WindowTree {
            name: "Notepad".to_string(),
            children: vec!["(1,2) button \"OK\"  [action: click]".to_string()],
        }];
        let rendered = render_tree(&trees);
        assert!(rendered.starts_with("desktop\n"));
        assert!(rendered.contains("window \"Notepad\""));
        assert!(rendered.contains("(1,2) button \"OK\"  [action: click]"));
    }

    #[test]
    fn tree_falls_back_when_empty() {
        assert_eq!(render_tree(&[]), "No elements found.");
    }

    #[test]
    fn format_tree_line_uses_action_verb() {
        let node = state::ElementNode {
            name: "Submit".to_string(),
            control_type: "button".to_string(),
            center: (10, 20),
            bounding_box: (0, 0, 20, 40),
            has_focus: false,
        };
        assert_eq!(
            format_tree_line(&node, "click"),
            "(10,20) button \"Submit\"  [action: click]"
        );
    }
}
