# windows-computeruse

Windows desktop automation MCP server (Rust).

## Install (Claude Code)

```powershell
./install.ps1
```

Builds the release binary, copies it to
`%LOCALAPPDATA%\Programs\windows-computeruse`, installs the bundled skill
(`skills/windows-computeruse`) to `%USERPROFILE%\.claude\skills` so it
loads in every project, and registers the MCP server in Claude Code's
user-scope configuration (`claude mcp add --scope user`). Pass `-SkipBuild` to
reuse an existing `target\release` binary.

## Build

```
cargo build --release
```

## Run

```
cargo run --release
```

Use the release profile for desktop automation. Debug builds are substantially
slower when resizing and encoding 4K screenshots.

`Snapshot` scans only the foreground application by default, including its
same-process popup windows. Use `window` to target one titled application or
`scope="all"` for whole-desktop discovery. Returned UI
tree lines include generation-scoped element ids and supported semantic
actions; pass an id to `InvokeElement` to activate the control without screen
coordinates.

Prefer `WaitFor` after UI actions so execution resumes as soon as the expected
window or element appears. Fixed `Wait` durations always add their full delay.

Enable Snapshot timing diagnostics before starting the server:

```powershell
$env:WINDOWS_MCP_PROFILE_SNAPSHOT = '1'
cargo run --release
```
