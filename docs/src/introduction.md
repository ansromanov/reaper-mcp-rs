# reaper-mcp-rs

reaper-mcp-rs is a [Model Context Protocol](https://modelcontextprotocol.io) server for
[REAPER](https://www.reaper.fm). It lets an AI agent, such as Claude Code or Claude Desktop, work
on a real REAPER project the way an engineer does: create and route tracks, place and move media
by bars and beats, set FX parameters, write automation, render stems and check the result with
loudness and spectrum analysis.

Unofficial: not affiliated with or endorsed by Cockos. REAPER is a trademark of Cockos
Incorporated.

## How it works

```text
MCP client ──stdio──▶ reaper-mcp-rs ──files──▶ Lua bridge inside REAPER ──▶ ReaScript API
(Claude Code,          (Rust binary)   request.json      (reaper_mcp_bridge.lua,
 Claude Desktop, …)                    response.json      reaper_mcp_handlers.lua)
                                       heartbeat
```

- The **server** is a single Rust binary. The MCP client starts it and talks to it over stdio.
- The **bridge** is a Lua ReaScript that runs inside REAPER. REAPER stays the only process that
  calls its own API, so a crash in the server cannot corrupt a project.
- They exchange versioned JSON files in an IPC directory. The bridge writes a heartbeat every
  second, so the server can tell when REAPER is busy or the bridge has stopped.
- The bridge runs a fixed set of handlers. It never executes code sent by the client.

## Status

Early (0.x): the tools grew out of real mixing work and are used on real projects, but argument
names can still change between minor versions. See the [changelog](changelog.md).

| Platform | State |
|---|---|
| Windows | Main platform; extra tools inspect REAPER's windows and restart the bridge when REAPER hangs. |
| Linux | Supported; set up with the same installer. |
| macOS | Builds and passes CI; not yet tried with REAPER. Reports are welcome. |

What the tools cover, and what not yet, is in [REAPER API coverage](api-coverage.md); the order of
the next steps is in the [roadmap](roadmap.md).
