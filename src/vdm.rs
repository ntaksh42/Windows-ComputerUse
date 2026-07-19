//! Windows virtual-desktop discovery and current-desktop filtering.

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::Shell::{IVirtualDesktopManager, VirtualDesktopManager};
use windows::core::GUID;
use windows_registry::{Type, Value};

const VIRTUAL_DESKTOPS_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\VirtualDesktops";

#[derive(Debug, Clone)]
pub struct VirtualDesktop {
    pub name: String,
    pub active: bool,
}

fn value_bytes(value: Value) -> Option<Vec<u8>> {
    (value.ty() == Type::Bytes).then(|| value.iter().copied().collect())
}

fn guid_from_registry_bytes(bytes: &[u8]) -> Option<GUID> {
    if bytes.len() != 16 {
        return None;
    }
    Some(GUID::from_values(
        u32::from_le_bytes(bytes[0..4].try_into().ok()?),
        u16::from_le_bytes(bytes[4..6].try_into().ok()?),
        u16::from_le_bytes(bytes[6..8].try_into().ok()?),
        bytes[8..16].try_into().ok()?,
    ))
}

fn desktop_key_name(id: &GUID) -> String {
    format!("{{{id:?}}}")
}

/// Reads desktop order/names from Explorer's supported per-user state.
/// Falls back to one default desktop when the registry data is unavailable.
pub fn desktops() -> Vec<VirtualDesktop> {
    let Ok(key) = windows_registry::CURRENT_USER.open(VIRTUAL_DESKTOPS_KEY) else {
        return default_desktop();
    };
    let current = key
        .get_value("CurrentVirtualDesktop")
        .ok()
        .and_then(value_bytes)
        .and_then(|bytes| guid_from_registry_bytes(&bytes));
    let Some(ids) = key
        .get_value("VirtualDesktopIDs")
        .ok()
        .and_then(value_bytes)
    else {
        return default_desktop();
    };

    let desktops_key = key.open("Desktops").ok();
    let mut result = Vec::new();
    for (index, bytes) in ids.chunks_exact(16).enumerate() {
        let Some(id) = guid_from_registry_bytes(bytes) else {
            continue;
        };
        let name = desktops_key
            .as_ref()
            .and_then(|root| root.open(desktop_key_name(&id)).ok())
            .and_then(|desktop| desktop.get_string("Name").ok())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| format!("Desktop {}", index + 1));
        result.push(VirtualDesktop {
            name,
            active: current == Some(id),
        });
    }
    if result.is_empty() {
        default_desktop()
    } else {
        if !result.iter().any(|desktop| desktop.active)
            && let Some(first) = result.first_mut()
        {
            first.active = true;
        }
        result
    }
}

fn default_desktop() -> Vec<VirtualDesktop> {
    vec![VirtualDesktop {
        name: "Default Desktop".to_string(),
        active: true,
    }]
}

/// Returns whether a top-level window belongs to the current virtual desktop.
/// If the COM service is unavailable, keeps the window rather than hiding it.
pub fn is_window_on_current_desktop(handle: isize) -> bool {
    if crate::uia::ensure_com_initialized().is_err() {
        return true;
    }
    unsafe {
        let manager: Result<IVirtualDesktopManager, _> =
            CoCreateInstance(&VirtualDesktopManager, None, CLSCTX_INPROC_SERVER);
        manager
            .and_then(|manager| manager.IsWindowOnCurrentVirtualDesktop(HWND(handle as *mut _)))
            .map(|current| current.as_bool())
            .unwrap_or(true)
    }
}

pub fn api_available() -> Result<(), String> {
    crate::uia::ensure_com_initialized()?;
    unsafe {
        let _: IVirtualDesktopManager =
            CoCreateInstance(&VirtualDesktopManager, None, CLSCTX_INPROC_SERVER)
                .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_registry_guid_byte_order() {
        let bytes = [
            0x43, 0x68, 0x78, 0xca, 0x42, 0x81, 0xbd, 0x43, 0xad, 0x26, 0x5f, 0x2e, 0xf7, 0x91,
            0xa9, 0x17,
        ];
        assert_eq!(
            desktop_key_name(&guid_from_registry_bytes(&bytes).unwrap()),
            "{CA786843-8142-43BD-AD26-5F2EF791A917}"
        );
    }
}
