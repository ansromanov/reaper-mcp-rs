# Troubleshooting

## "bridge heartbeat missing" or `connected: false`

The bridge is not running, or the server and the bridge use different IPC directories.

1. Start `reaper_mcp_bridge.lua` from REAPER's action list, or install it with the startup option
   ([Installation](installation.md#start-the-bridge-in-reaper)).
2. Compare `ipc_directory` in the `health` result with REAPER's resource directory
   (**Options > Show REAPER resource path**). If you set `REAPER_MCP_DIR`, set it for both.

## "heartbeat is stale"

REAPER is busy (loading a project, rendering, a modal dialog) or its Lua thread is stuck. Wait for
the operation to finish and close any open dialog. On Windows, `inspect_reaper_windows` shows which
dialogs are open and whether the main window responds, and `restart_bridge` loads the bridge again.

## "REAPER command … timed out"

The command took longer than its timeout ([Configuration](configuration.md#timeouts)). For
renders, pass a larger `timeout_seconds`. The command may still finish in REAPER; check with a
read tool before retrying.

## A render writes `NAME-001.wav`

REAPER never overwrites a file. The render tools delete the files they are about to write first;
with `overwrite: false`, or when an old file is locked, REAPER adds a number. Use
`set_media_offline` to make REAPER let go of a file it plays.

## A plugin update does not show

REAPER keeps the old plugin library loaded until it restarts. `list_plugins` shows the version on
disk.

## Two agents disturb each other

One server process per IPC directory is supported. Close the other client, or give each REAPER
instance its own `REAPER_MCP_DIR`.

## Reporting a bug

Open an [issue](https://github.com/ansromanov/reaper-mcp-rs/issues/new/choose) with the
`reaper-mcp-rs --version` output, your OS, REAPER version, the tool call and its result.
