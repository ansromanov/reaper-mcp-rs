//! Windows helpers that must remain available when REAPER's Lua main thread is stuck.

use anyhow::{anyhow, Result};
use serde_json::Value;
#[cfg(windows)]
use {anyhow::Context, std::process::Command};

#[cfg(windows)]
fn powershell(script: &str) -> Result<Value> {
    powershell_with_env(script, &[])
}

#[cfg(windows)]
fn powershell_with_env(script: &str, env: &[(&str, &str)]) -> Result<Value> {
    let output = Command::new("powershell.exe")
        .envs(env.iter().copied())
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ])
        .output()
        .context("failed to start PowerShell")?;
    if !output.status.success() {
        return Err(anyhow!(
            "PowerShell failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout).context("PowerShell returned invalid JSON")
}

/// Enumerate REAPER's visible windows without asking the Lua bridge to run.
#[cfg(windows)]
pub fn inspect_reaper_windows() -> Result<Value> {
    const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class ReaperWindowProbe {
  public delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr hwnd, EnumWindowsProc cb, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr hwnd, uint cmd);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int max);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr hwnd, StringBuilder text, int max);
  [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr SendMessageTimeout(IntPtr hwnd, uint msg, IntPtr wParam, IntPtr lParam, uint flags, uint timeout, out IntPtr result);
  public static string Text(IntPtr hwnd) { var b = new StringBuilder(2048); GetWindowText(hwnd,b,b.Capacity); return b.ToString(); }
  public static string Class(IntPtr hwnd) { var b = new StringBuilder(256); GetClassName(hwnd,b,b.Capacity); return b.ToString(); }
  public static bool Responds(IntPtr hwnd) { IntPtr result; return SendMessageTimeout(hwnd,0,IntPtr.Zero,IntPtr.Zero,2,500,out result) != IntPtr.Zero; }
}
'@
$procs = @(Get-Process -Name reaper -ErrorAction SilentlyContinue | ForEach-Object {
  $p = $_
  $windows = [System.Collections.Generic.List[object]]::new()
  $enumTop = [ReaperWindowProbe+EnumWindowsProc]{ param($h,$l)
    [uint32]$windowProcessId = 0; [void][ReaperWindowProbe]::GetWindowThreadProcessId($h,[ref]$windowProcessId)
    if ($windowProcessId -eq [uint32]$p.Id -and [ReaperWindowProbe]::IsWindowVisible($h)) {
      $owner = [ReaperWindowProbe]::GetWindow($h,4)
      $children = [System.Collections.Generic.List[object]]::new()
      if ($owner -ne [IntPtr]::Zero) {
        $enumChild = [ReaperWindowProbe+EnumWindowsProc]{ param($ch,$cl)
          $class = [ReaperWindowProbe]::Class($ch)
          if ($class -eq 'Static') { $txt = [ReaperWindowProbe]::Text($ch); if ($txt) { $children.Add([pscustomobject]@{class=$class;text=$txt}) } }
          return $true
        }
        [void][ReaperWindowProbe]::EnumChildWindows($h,$enumChild,[IntPtr]::Zero)
      }
      $windows.Add([pscustomobject]@{handle=('0x{0:X}' -f $h.ToInt64());title=[ReaperWindowProbe]::Text($h);class=[ReaperWindowProbe]::Class($h);owned=($owner -ne [IntPtr]::Zero);responding=[ReaperWindowProbe]::Responds($h);static_children=@($children.ToArray())})
    }
    return $true
  }
  [void][ReaperWindowProbe]::EnumWindows($enumTop,[IntPtr]::Zero)
  $exe = $null; try { $exe = $p.MainModule.FileName } catch {}
  $priority = $null; try { $priority = $p.PriorityClass.ToString() } catch {}
  $mainResponding = $null; try { $mainResponding = [ReaperWindowProbe]::Responds($p.MainWindowHandle) } catch {}
  [pscustomobject]@{pid=$p.Id;executable=$exe;priority_class=$priority;main_window_title=$p.MainWindowTitle;responding=$mainResponding;windows=@($windows.ToArray())}
})
ConvertTo-Json -InputObject ([pscustomobject]@{processes=$procs}) -Depth 8 -Compress
"#;
    powershell(SCRIPT)
}

/// Post WM_CLOSE only to owned REAPER dialogs whose title starts exactly with
/// `title_prefix`. The result records posted messages, not closure.
#[cfg(windows)]
pub fn close_reaper_fx_windows(title_prefix: &str) -> Result<Value> {
    if title_prefix.trim().is_empty() {
        return Err(anyhow!("title_prefix must not be empty"));
    }
    const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class ReaperFxWindowCloser {
  public delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr hwnd, uint cmd);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int max);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr hwnd, StringBuilder text, int max);
  [DllImport("user32.dll", SetLastError=true)] public static extern bool PostMessage(IntPtr hwnd, uint message, IntPtr wParam, IntPtr lParam);
  public static string Text(IntPtr hwnd) { var b = new StringBuilder(2048); GetWindowText(hwnd,b,b.Capacity); return b.ToString(); }
  public static string Class(IntPtr hwnd) { var b = new StringBuilder(256); GetClassName(hwnd,b,b.Capacity); return b.ToString(); }
}
'@
$prefix = $env:REAPER_MCP_TITLE_PREFIX
$posted = [System.Collections.Generic.List[object]]::new()
$reaperProcesses = @(Get-Process -Name reaper -ErrorAction SilentlyContinue)
foreach ($process in $reaperProcesses) {
  $processId = [uint32]$process.Id
  $callback = [ReaperFxWindowCloser+EnumWindowsProc]{ param($hwnd,$unused)
    [uint32]$windowProcessId = 0; [void][ReaperFxWindowCloser]::GetWindowThreadProcessId($hwnd,[ref]$windowProcessId)
    $class = [ReaperFxWindowCloser]::Class($hwnd)
    $title = [ReaperFxWindowCloser]::Text($hwnd)
    $owner = [ReaperFxWindowCloser]::GetWindow($hwnd,4)
    if ([ReaperFxWindowCloser]::IsWindowVisible($hwnd) -and $windowProcessId -eq $processId -and $owner -ne [IntPtr]::Zero -and $class -eq '#32770' -and $title.StartsWith($prefix,[StringComparison]::Ordinal)) {
      $ok = [ReaperFxWindowCloser]::PostMessage($hwnd,0x0010,[IntPtr]::Zero,[IntPtr]::Zero)
      $posted.Add([pscustomobject]@{handle=('0x{0:X}' -f $hwnd.ToInt64());pid=$processId;title=$title;post_succeeded=$ok})
    }
    return $true
  }
  [void][ReaperFxWindowCloser]::EnumWindows($callback,[IntPtr]::Zero)
}
ConvertTo-Json -InputObject ([pscustomobject]@{target_title_prefix=$prefix;target_window_class='#32770';post_message='WM_CLOSE';posted_count=$posted.Count;posted_windows=@($posted.ToArray());closed_verified=$false}) -Depth 5 -Compress
"#;
    powershell_with_env(SCRIPT, &[("REAPER_MCP_TITLE_PREFIX", title_prefix)])
}

#[cfg(not(windows))]
pub fn inspect_reaper_windows() -> Result<Value> {
    Err(anyhow!(
        "REAPER window inspection is available only on Windows"
    ))
}

#[cfg(not(windows))]
pub fn close_reaper_fx_windows(_title_prefix: &str) -> Result<Value> {
    Err(anyhow!(
        "REAPER window control is available only on Windows"
    ))
}

/// Ask an already-installed REAPER executable to load the known bridge script.
/// This intentionally reports the launch request only; it never kills REAPER or
/// claims that a potentially blocked process recovered.
#[cfg(windows)]
pub fn restart_bridge() -> Result<Value> {
    const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$proc = Get-Process -Name reaper -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $proc) { throw 'No REAPER process with a main window was found.' }
$exe = $proc.MainModule.FileName
$root = Join-Path $env:APPDATA 'REAPER\Scripts\reaper-mcp'
$loader = Join-Path $root 'reaper_mcp_bridge.lua'
if (-not (Test-Path -LiteralPath $loader -PathType Leaf)) { throw "Installed bridge loader not found: $loader" }
$wrapper = Join-Path $root 'reaper_mcp_codex_startup.lua'
$expected = "local path = reaper.GetResourcePath() .. '/Scripts/reaper-mcp/reaper_mcp_bridge.lua'`nlocal ok, err = pcall(dofile, path)`nif not ok then reaper.ShowConsoleMsg('MCP bridge load failed: ' .. tostring(err) .. '\n') end`n"
if (Test-Path -LiteralPath $wrapper -PathType Leaf) {
  if ([IO.File]::ReadAllText($wrapper) -ne $expected) { throw "Refusing to overwrite unexpected wrapper: $wrapper" }
} else {
  [IO.File]::WriteAllText($wrapper,$expected,[Text.UTF8Encoding]::new($false))
}
$child = Start-Process -FilePath $exe -ArgumentList @('-nonewinst', ('"' + $wrapper + '"')) -WindowStyle Hidden -PassThru
ConvertTo-Json -InputObject ([pscustomobject]@{launch_requested=$true;verified_recovery=$false;existing_reaper_pid=$proc.Id;executable=$exe;loader=$loader;wrapper=$wrapper;helper_pid=$child.Id;note='Launch requested only; verify bridge heartbeat separately.'}) -Compress
"#;
    let mut result = powershell(SCRIPT)?;
    if let Some(object) = result.as_object_mut() {
        object.insert("verified_recovery".into(), Value::Bool(false));
    }
    Ok(result)
}

#[cfg(not(windows))]
pub fn restart_bridge() -> Result<Value> {
    Err(anyhow!(
        "Launching the REAPER bridge is available only on Windows"
    ))
}
