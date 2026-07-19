[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$AuditLog,

    [string]$OutputPath
)

$ErrorActionPreference = 'Stop'
$auditPath = (Resolve-Path -LiteralPath $AuditLog).Path
if (-not $OutputPath) {
    $OutputPath = Join-Path (Split-Path -Parent $auditPath) 'trace-report.html'
}

$rows = foreach ($line in [System.IO.File]::ReadLines($auditPath)) {
    if ([string]::IsNullOrWhiteSpace($line)) { continue }
    $record = $line | ConvertFrom-Json
    $ts = [System.Net.WebUtility]::HtmlEncode([string]$record.ts)
    $tool = [System.Net.WebUtility]::HtmlEncode([string]$record.tool)
    $ok = if ($record.ok) { 'Success' } else { 'Failure' }
    "<tr><td>$ts</td><td>$tool</td><td>$ok</td></tr>"
}

$html = @"
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Windows MCP Audit Trace</title>
  <style>
    body { font-family: system-ui, sans-serif; margin: 2rem; }
    table { border-collapse: collapse; width: 100%; }
    th, td { border: 1px solid #ccc; padding: .5rem; text-align: left; }
    th { background: #f3f3f3; }
  </style>
</head>
<body>
  <h1>Windows MCP Audit Trace</h1>
  <table>
    <thead><tr><th>Time</th><th>Tool</th><th>Result</th></tr></thead>
    <tbody>
$($rows -join "`n")
    </tbody>
  </table>
</body>
</html>
"@

[System.IO.File]::WriteAllText(
    [System.IO.Path]::GetFullPath($OutputPath),
    $html,
    [System.Text.UTF8Encoding]::new($false)
)
Write-Output "Report written to $([System.IO.Path]::GetFullPath($OutputPath))"
