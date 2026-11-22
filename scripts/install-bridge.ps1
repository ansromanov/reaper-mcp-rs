# Installs the Lua bridge into REAPER's Scripts folder on Windows and, with
# -Startup, makes REAPER start it automatically through Scripts\__startup.lua.
param(
    [string]$ResourceDir = (Join-Path $env:APPDATA 'REAPER'),
    [switch]$Startup
)
$ErrorActionPreference = 'Stop'
$source = Join-Path $PSScriptRoot '..\lua'
$target = Join-Path $ResourceDir 'Scripts\reaper-mcp'
New-Item -ItemType Directory -Force -Path (Join-Path $target 'vendor') | Out-Null
Copy-Item -Force (Join-Path $source '*.lua') $target
Copy-Item -Force (Join-Path $source 'vendor\*') (Join-Path $target 'vendor')
Write-Host "Installed bridge to $target"

if ($Startup) {
    $startupFile = Join-Path $ResourceDir 'Scripts\__startup.lua'
    $line = 'dofile(reaper.GetResourcePath() .. "/Scripts/reaper-mcp/reaper_mcp_bridge.lua")'
    $existing = if (Test-Path $startupFile) { Get-Content -Raw $startupFile } else { '' }
    if ($existing -notmatch [regex]::Escape('reaper-mcp/reaper_mcp_bridge.lua')) {
        Add-Content -Path $startupFile -Value "`n-- Starts the reaper-mcp-rs bridge.`n$line"
        Write-Host "Added bridge to $startupFile"
    }
}
else {
    Write-Host 'Run reaper_mcp_bridge.lua from the REAPER action list, or re-run with -Startup.'
}
