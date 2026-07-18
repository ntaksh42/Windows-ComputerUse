//! Start Menu application discovery and launch logic for the App tool's
//! `launch` mode (docs/SPEC.md §1).

use std::collections::HashMap;
use std::path::Path;

use crate::{powershell, win};

/// Maps lowercase Start Menu app name -> AppID (for `shell:AppsFolder\<AppID>`)
/// or a filesystem path to a `.lnk`/executable.
pub fn get_apps_from_start_menu() -> HashMap<String, String> {
    let (output, status) =
        powershell::execute_command("Get-StartApps | ConvertTo-Csv -NoTypeInformation", 10, None);
    if status == 0 && !output.trim().is_empty() {
        let apps = parse_start_apps_csv(&output);
        if !apps.is_empty() {
            return apps;
        }
    }
    apps_from_shortcuts()
}

fn parse_start_apps_csv(csv_text: &str) -> HashMap<String, String> {
    let mut apps = HashMap::new();
    let mut reader = csv::Reader::from_reader(csv_text.as_bytes());
    let Ok(headers) = reader.headers() else {
        return apps;
    };
    let Some(name_idx) = headers.iter().position(|h| h == "Name") else {
        return apps;
    };
    let Some(appid_idx) = headers.iter().position(|h| h == "AppID") else {
        return apps;
    };
    for record in reader.records().flatten() {
        let name = record.get(name_idx).unwrap_or("").trim();
        let appid = record.get(appid_idx).unwrap_or("").trim();
        if !name.is_empty() && !appid.is_empty() {
            apps.entry(name.to_lowercase())
                .or_insert_with(|| appid.to_string());
        }
    }
    apps
}

/// Scans Start Menu shortcut folders for `.lnk` files as a fallback for `Get-StartApps`.
fn apps_from_shortcuts() -> HashMap<String, String> {
    let mut apps = HashMap::new();
    let program_data =
        std::env::var("PROGRAMDATA").unwrap_or_else(|_| r"C:\ProgramData".to_string());
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    let bases = [
        format!(r"{program_data}\Microsoft\Windows\Start Menu\Programs"),
        format!(r"{appdata}\Microsoft\Windows\Start Menu\Programs"),
    ];
    for base in bases {
        let base_path = Path::new(&base);
        if !base_path.is_dir() {
            continue;
        }
        for lnk in find_lnk_files(base_path) {
            if let Some(stem) = lnk.file_stem().and_then(|s| s.to_str()) {
                apps.entry(stem.to_lowercase())
                    .or_insert_with(|| lnk.to_string_lossy().into_owned());
            }
        }
    }
    apps
}

fn find_lnk_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(find_lnk_files(&path));
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("lnk"))
        {
            out.push(path);
        }
    }
    out
}

fn ps_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Checks whether an app with the given AppID exists in `shell:AppsFolder`.
fn check_app_exists(app_id: &str) -> bool {
    let command = format!(
        "$folder = (New-Object -ComObject Shell.Application).NameSpace('shell:AppsFolder'); \
         if ($folder) {{ [bool]$folder.ParseName({}) }} else {{ $false }}",
        ps_quote(app_id)
    );
    let (response, status) = powershell::execute_command(&command, 10, None);
    status == 0 && response.trim().eq_ignore_ascii_case("true")
}

/// Title-cases each whitespace-separated word, mirroring Python's `str.title()`
/// as used for the App tool's response messages.
pub fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Launches a Start Menu app matching `name` (fuzzy, score_cutoff 70).
/// Returns `(response, status_code, pid)`; `pid` is `None` when unknown
/// (e.g. `shell:AppsFolder` launches, which `Start-Process` does not report).
pub fn launch_app(name: &str) -> (String, i32, Option<u32>) {
    let apps = get_apps_from_start_menu();
    let keys: Vec<&str> = apps.keys().map(String::as_str).collect();
    let Some((matched_key, _)) = crate::fuzzy::extract_one(name, keys, 70.0) else {
        return (
            format!("{} not found in start menu.", title_case(name)),
            1,
            None,
        );
    };
    let matched_key = matched_key.to_string();
    let Some(appid) = apps.get(&matched_key) else {
        return (
            format!("{} not found in start menu.", title_case(name)),
            1,
            None,
        );
    };

    if Path::new(appid).exists() || appid.contains('\\') {
        let exe_path = win::resolve_known_folder_guid_path(appid);
        let command = format!(
            "Start-Process {} -PassThru | Select-Object -ExpandProperty Id",
            ps_quote(&exe_path)
        );
        let (response, status) = powershell::execute_command(&command, 10, None);
        let pid = if status == 0 {
            response.trim().parse::<u32>().ok()
        } else {
            None
        };
        (response, status, pid)
    } else {
        if !check_app_exists(appid) {
            return (format!("Invalid app identifier: {appid}"), 1, None);
        }
        let command = format!(
            "Start-Process {}",
            ps_quote(&format!("shell:AppsFolder\\{appid}"))
        );
        let (response, status) = powershell::execute_command(&command, 10, None);
        (response, status, None)
    }
}
