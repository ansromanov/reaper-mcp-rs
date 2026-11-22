# Configuration and IPC

## Environment variables

| Variable | Read by | Meaning |
|---|---|---|
| `REAPER_MCP_DIR` | server and bridge | IPC directory. Default: `<REAPER resource directory>/reaper-mcp-bridge`. Set the same value for both. |

The default REAPER resource directory is `~/.config/REAPER` on Linux, `%APPDATA%\REAPER` on
Windows and `~/Library/Application Support/REAPER` on macOS. The installers also read the
variables listed in [Installation](installation.md#one-line-installer).

## IPC contract

The server and the bridge exchange files in the IPC directory:

| File | Written by | Content |
|---|---|---|
| `request.json` | server | `{"id": "<uuid>", "command": "<name>", "args": {…}}` |
| `response.json` | bridge | `{"id": "<same uuid>", "ok": true, "result": …}` or `{"id": …, "ok": false, "error": "…"}` |
| `heartbeat` | bridge | Unix time in seconds, rewritten every second |
| `generation` | bridge | ID of the running bridge copy; a newer copy makes the older one stop |

Both sides publish files with an atomic rename, so the other side never reads half a file. The
server serializes requests and removes a request that timed out.

## Timeouts

| Situation | Timeout |
|---|---|
| Most commands | 5 seconds |
| Slow commands: opening, saving and switching projects, adding FX, `run_action` and others | 5 minutes |
| Rendering | 30 to 60 minutes depending on the tool; `timeout_seconds` changes it |
| Heartbeat | A heartbeat older than 5 seconds counts as stale. Before giving up the server waits another 5 seconds, because a long REAPER operation stalls the heartbeat. |

## Limits

- One MCP server process per IPC directory: two clients at the same time share one request file
  and disturb each other.
- One REAPER instance per IPC directory.

Both are on the [roadmap](roadmap.md).
