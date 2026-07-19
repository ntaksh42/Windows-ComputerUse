//! Display enumeration: monitor geometry, DPI, and orientation metadata.
//!
//! `index` is assigned from `EnumDisplayDevicesW`, counting only devices with
//! the `ATTACHED_TO_DESKTOP` flag, 0-based, in enumeration order. This must
//! match the index space used by the Snapshot tool's `display` parameter.

use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    DEVMODEW, DISPLAY_DEVICE_ATTACHED_TO_DESKTOP, DISPLAY_DEVICEW, ENUM_CURRENT_SETTINGS,
    EnumDisplayDevicesW, EnumDisplayMonitors, EnumDisplaySettingsW, GetMonitorInfoW, HDC, HMONITOR,
    MONITORINFOEXW,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForSystem, MDT_EFFECTIVE_DPI};
use windows::core::BOOL;

/// Monitor bounds, expressed both as edges and as width/height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl DisplayRect {
    fn from_rect(rect: RECT) -> Self {
        Self {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        }
    }

    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

/// A single display's identity, geometry, and DPI metadata.
#[derive(Debug, Clone)]
pub struct Display {
    pub index: usize,
    pub device: String,
    pub primary: bool,
    pub bounds: DisplayRect,
    pub work_area: Option<DisplayRect>,
    pub effective_dpi: Option<u32>,
    pub scale: Option<f64>,
    pub orientation: String,
}

impl Display {
    pub fn resolution(&self) -> String {
        format!("{}x{}", self.bounds.width(), self.bounds.height())
    }
}

/// Reads the device names (uppercased) attached to the desktop, via
/// `EnumDisplayDevicesW`. The position in the returned list is the display
/// index used throughout the tool surface.
fn attached_device_names() -> Vec<String> {
    let mut names = Vec::new();
    let mut device_index = 0u32;
    loop {
        let mut device = DISPLAY_DEVICEW {
            cb: size_of::<DISPLAY_DEVICEW>() as u32,
            ..Default::default()
        };
        let ok = unsafe { EnumDisplayDevicesW(None, device_index, &mut device, 0) };
        if !ok.as_bool() {
            break;
        }
        if device
            .StateFlags
            .contains(DISPLAY_DEVICE_ATTACHED_TO_DESKTOP)
        {
            names.push(wchar_to_string(&device.DeviceName).to_uppercase());
        }
        device_index += 1;
    }
    names
}

fn wchar_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

unsafe extern "system" fn monitor_enum_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let monitors = unsafe { &mut *(lparam.0 as *mut Vec<HMONITOR>) };
    monitors.push(hmonitor);
    BOOL(1)
}

fn monitor_effective_dpi(hmonitor: HMONITOR) -> (Option<u32>, Option<f64>) {
    let mut dpi_x = 0u32;
    let mut dpi_y = 0u32;
    let dpi = unsafe { GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }
        .ok()
        .map(|_| dpi_x)
        .filter(|&dpi| dpi > 0)
        .or_else(|| {
            let dpi = unsafe { GetDpiForSystem() };
            if dpi > 0 { Some(dpi) } else { None }
        });
    match dpi {
        Some(dpi) => (
            Some(dpi),
            Some((dpi as f64 / 96.0 * 1_000_000.0).round() / 1_000_000.0),
        ),
        None => (None, None),
    }
}

fn display_orientation(device_name: &str, bounds: &DisplayRect) -> String {
    let mut device_name_wide: Vec<u16> = device_name.encode_utf16().collect();
    device_name_wide.push(0);
    let mut devmode = DEVMODEW {
        dmSize: size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };
    let ok = unsafe {
        EnumDisplaySettingsW(
            windows::core::PCWSTR(device_name_wide.as_ptr()),
            ENUM_CURRENT_SETTINGS,
            &mut devmode,
        )
    };
    if ok.as_bool() {
        let width = devmode.dmPelsWidth;
        let height = devmode.dmPelsHeight;
        if width > 0 && height > 0 {
            return if width >= height {
                "landscape".to_string()
            } else {
                "portrait".to_string()
            };
        }
    }
    if bounds.width() >= bounds.height() {
        "landscape".to_string()
    } else {
        "portrait".to_string()
    }
}

/// Enumerates active displays, ordered by `index`.
pub fn get_displays() -> Vec<Display> {
    let attached = attached_device_names();

    let mut monitors: Vec<HMONITOR> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(monitor_enum_proc),
            LPARAM(&mut monitors as *mut Vec<HMONITOR> as isize),
        );
    }

    let mut displays: Vec<Display> = Vec::new();
    let mut used_indices: Vec<usize> = Vec::new();

    for hmonitor in monitors {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        let ok = unsafe { GetMonitorInfoW(hmonitor, &mut info.monitorInfo) };
        if !ok.as_bool() {
            // Monitor vanished mid-enumeration or the API failed; skip it
            // rather than fabricating geometry for a display we can't describe.
            continue;
        }

        let device_name = wchar_to_string(&info.szDevice);
        let index = attached
            .iter()
            .position(|name| *name == device_name.to_uppercase())
            .unwrap_or_else(|| {
                let mut candidate = 0usize;
                while used_indices.contains(&candidate) {
                    candidate += 1;
                }
                candidate
            });
        used_indices.push(index);

        let bounds = DisplayRect::from_rect(info.monitorInfo.rcMonitor);
        let work_area = Some(DisplayRect::from_rect(info.monitorInfo.rcWork));
        // MONITORINFOF_PRIMARY = 1
        let primary = info.monitorInfo.dwFlags & 1 != 0;
        let (effective_dpi, scale) = monitor_effective_dpi(hmonitor);
        let orientation = display_orientation(&device_name, &bounds);

        displays.push(Display {
            index,
            device: device_name,
            primary,
            bounds,
            work_area,
            effective_dpi,
            scale,
            orientation,
        });
    }

    displays.sort_by_key(|display| display.index);
    displays
}

/// Returns the union of the bounding rectangles for `indices`.
///
/// Errors when any index is not among the currently active displays, in the
/// form `"Invalid display index {i}. Available displays: {csv}"`.
pub fn get_display_union_rect(indices: &[usize]) -> Result<RECT, String> {
    let displays = get_displays();
    if displays.is_empty() {
        return Err("No displays detected".to_string());
    }
    if indices.is_empty() {
        return Err("display must contain at least one display index".to_string());
    }

    if let Some(&invalid) = indices
        .iter()
        .find(|i| !displays.iter().any(|d| d.index == **i))
    {
        let available = displays
            .iter()
            .map(|d| d.index.to_string())
            .collect::<Vec<_>>()
            .join(",");
        return Err(format!(
            "Invalid display index {invalid}. Available displays: {available}"
        ));
    }

    let selected: Vec<&Display> = indices
        .iter()
        .filter_map(|i| displays.iter().find(|d| d.index == *i))
        .collect();

    let left = selected.iter().map(|d| d.bounds.left).min().unwrap();
    let top = selected.iter().map(|d| d.bounds.top).min().unwrap();
    let right = selected.iter().map(|d| d.bounds.right).max().unwrap();
    let bottom = selected.iter().map(|d| d.bounds.bottom).max().unwrap();

    Ok(RECT {
        left,
        top,
        right,
        bottom,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_rect_reports_invalid_index() {
        // With no displays selected the function still needs a live display
        // set from the host; this test only exercises the error-formatting
        // branch by requesting an index far outside any plausible display
        // count so the "invalid" path is what triggers (unless the CI host
        // genuinely has 100+ monitors).
        match get_display_union_rect(&[9999]) {
            Err(message) => {
                assert!(message.starts_with("Invalid display index 9999. Available displays: "));
            }
            Ok(_) => panic!("expected index 9999 to be invalid"),
        }
    }
}
