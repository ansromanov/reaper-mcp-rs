# Installs reaper-mcp-rs from the latest GitHub release on Windows:
#
#   irm https://raw.githubusercontent.com/ansromanov/reaper-mcp-rs/main/install.ps1 | iex
#
# Settings (environment variables):
#   REAPER_MCP_VERSION   release tag such as v0.2.0 (default: the latest release)
#   REAPER_MCP_BIN_DIR   where the server binary goes (default: %LOCALAPPDATA%\Programs\reaper-mcp-rs)
#   REAPER_RESOURCE_PATH REAPER resource directory (default: %APPDATA%\REAPER)
#   REAPER_MCP_STARTUP   1 to start the bridge with REAPER through Scripts\__startup.lua
$ErrorActionPreference = 'Stop'

$repo = 'ansromanov/reaper-mcp-rs'
$version = if ($env:REAPER_MCP_VERSION) { $env:REAPER_MCP_VERSION } else { 'latest' }
$binDir = if ($env:REAPER_MCP_BIN_DIR) { $env:REAPER_MCP_BIN_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\reaper-mcp-rs' }
$base = if ($env:REAPER_MCP_BASE_URL) { $env:REAPER_MCP_BASE_URL }
        elseif ($version -eq 'latest') { "https://github.com/$repo/releases/latest/download" }
        else { "https://github.com/$repo/releases/download/$version" }
$archive = 'reaper-mcp-rs-x86_64-pc-windows-msvc.zip'

$work = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid())
New-Item -ItemType Directory -Path $work | Out-Null
try {
    Write-Host "Downloading $archive ($version)"
    Invoke-WebRequest -UseBasicParsing -Uri "$base/$archive" -OutFile (Join-Path $work $archive)
    Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS" -OutFile (Join-Path $work 'SHA256SUMS')

    $line = Get-Content (Join-Path $work 'SHA256SUMS') | Where-Object { $_ -match " $([regex]::Escape($archive))$" }
    $expected = if ($line) { ($line -split ' ')[0] } else { '' }
    $actual = (Get-FileHash -Algorithm SHA256 (Join-Path $work $archive)).Hash.ToLower()
    if (-not $expected -or $expected -ne $actual) { throw "Checksum mismatch for $archive" }

    Expand-Archive -Path (Join-Path $work $archive) -DestinationPath $work
    New-Item -ItemType Directory -Force -Path $binDir | Out-Null
    $exe = Join-Path $binDir 'reaper-mcp-rs.exe'
    Copy-Item -Force (Join-Path $work 'reaper-mcp-rs\reaper-mcp-rs.exe') $exe
    Write-Host "Installed server to $exe"

    $bridgeArgs = @{}
    if ($env:REAPER_RESOURCE_PATH) { $bridgeArgs.ResourceDir = $env:REAPER_RESOURCE_PATH }
    if ($env:REAPER_MCP_STARTUP -eq '1') { $bridgeArgs.Startup = $true }
    & (Join-Path $work 'reaper-mcp-rs\scripts\install-bridge.ps1') @bridgeArgs

    $json = $exe.Replace('\', '\\')
    Write-Host ''
    Write-Host 'Next: register the server with your MCP client.'
    Write-Host "  Claude Code:    claude mcp add reaper -- `"$exe`""
    Write-Host '  Claude Desktop: add to claude_desktop_config.json'
    Write-Host "    { `"mcpServers`": { `"reaper`": { `"command`": `"$json`" } } }"
}
finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
