//! Shared low-level Win32 helpers used across the shell-related tools
//! (PowerShell, App, Process).

use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows::Win32::UI::Shell::IsUserAnAdmin;
use windows::Win32::UI::Shell::{FOLDERID_Profile, KNOWN_FOLDER_FLAG, SHGetKnownFolderPath};
use windows::core::{GUID, PCWSTR};

/// Returns `true` when the current process is running elevated (Administrator).
pub fn is_elevated() -> bool {
    unsafe { IsUserAnAdmin().as_bool() }
}

/// Expands `%VAR%` references in `value` using the current process environment,
/// mirroring `winreg.ExpandEnvironmentStrings` used by the Python reference.
pub fn expand_env_string(value: &str) -> String {
    let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let needed = ExpandEnvironmentStringsW(PCWSTR(wide.as_ptr()), None);
        if needed == 0 {
            return value.to_string();
        }
        let mut buf = vec![0u16; needed as usize];
        let written = ExpandEnvironmentStringsW(PCWSTR(wide.as_ptr()), Some(&mut buf));
        if written == 0 || written > buf.len() as u32 {
            return value.to_string();
        }
        // `written` includes the trailing nul.
        let len = (written as usize).saturating_sub(1);
        String::from_utf16_lossy(&buf[..len])
    }
}

/// Resolves a Windows Known Folder GUID to its absolute filesystem path.
fn known_folder_path(guid: GUID) -> Option<String> {
    unsafe {
        let pwstr = SHGetKnownFolderPath(&guid, KNOWN_FOLDER_FLAG(0), None).ok()?;
        let result = pwstr.to_string().ok();
        CoTaskMemFree(Some(pwstr.0 as *const _));
        result
    }
}

/// Parses a hyphenated GUID string (with or without surrounding braces) into a [`GUID`].
fn parse_guid(text: &str) -> Option<GUID> {
    let hex: String = text
        .chars()
        .filter(|c| *c != '{' && *c != '}' && *c != '-')
        .collect();
    if hex.len() != 32 {
        return None;
    }
    let value = u128::from_str_radix(&hex, 16).ok()?;
    Some(GUID::from_u128(value))
}

/// Resolves a Windows Known Folder GUID path such as
/// `{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\msinfo32.exe` to an absolute
/// filesystem path. Returns `path_text` unchanged if it does not match the
/// `{GUID}\...` pattern or the GUID is unrecognised.
pub fn resolve_known_folder_guid_path(path_text: &str) -> String {
    let Some(rest_start) = path_text.strip_prefix('{') else {
        return path_text.to_string();
    };
    let Some(close) = rest_start.find('}') else {
        return path_text.to_string();
    };
    let guid_text = &rest_start[..close];
    let Some(guid) = parse_guid(guid_text) else {
        return path_text.to_string();
    };
    let Some(base) = known_folder_path(guid) else {
        return path_text.to_string();
    };
    let after = &rest_start[close + 1..];
    match after.strip_prefix('\\') {
        Some(tail) if !tail.is_empty() => format!("{base}\\{tail}"),
        _ => base,
    }
}

/// Best-effort resolution of the current user's home directory, mirroring
/// `os.path.expanduser("~")` on Windows: prefer `USERPROFILE`, then
/// `HOMEDRIVE`+`HOMEPATH`, then the Known Folder API as a last resort.
pub fn home_dir() -> String {
    if let Ok(profile) = std::env::var("USERPROFILE")
        && !profile.is_empty()
    {
        return profile;
    }
    if let (Ok(drive), Ok(path)) = (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH"))
        && !drive.is_empty()
        && !path.is_empty()
    {
        return format!("{drive}{path}");
    }
    known_folder_path(FOLDERID_Profile).unwrap_or_else(|| r"C:\Users\Default".to_string())
}
