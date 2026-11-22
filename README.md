# reaper-mcp-rs

[![CI](https://github.com/ansromanov/reaper-mcp-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/ansromanov/reaper-mcp-rs/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/ansromanov/reaper-mcp-rs?include_prereleases&sort=semver)](https://github.com/ansromanov/reaper-mcp-rs/releases)
[![Docs](https://img.shields.io/badge/docs-mdBook-blue)](https://ansromanov.github.io/reaper-mcp-rs/)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)

An [MCP](https://modelcontextprotocol.io) server for [REAPER](https://www.reaper.fm), written in
Rust. It lets an AI agent such as Claude Code or Claude Desktop work on a real REAPER project the
way an engineer does: build and route tracks, move media by bars and beats, set FX parameters by
the values the plugin displays, write automation, render stems, and check the result with loudness
and spectrum analysis.

**[Documentation](https://ansromanov.github.io/reaper-mcp-rs/)** ·
[Tool reference](https://ansromanov.github.io/reaper-mcp-rs/reference.html) ·
[Roadmap](ROADMAP.md) · [Changelog](CHANGELOG.md)

## Status

Early, version 0.x. The 77 tools grew out of real mixing work and are used on real projects, but
argument names can still change between minor versions.

| Platform | State |
|---|---|
| Windows | Main platform, with extra tools for when REAPER hangs |
| Linux | Supported |
| macOS | Builds and passes CI; not yet tried with REAPER |

## Install

**1. Install the server and the REAPER bridge.**

```sh
# Linux and macOS
curl -fsSL https://raw.githubusercontent.com/ansromanov/reaper-mcp-rs/main/install.sh | sh
```

```powershell
# Windows (PowerShell)
irm https://raw.githubusercontent.com/ansromanov/reaper-mcp-rs/main/install.ps1 | iex
```

Set `REAPER_MCP_STARTUP=1` to start the bridge with REAPER. Other options, release archives and
building from source: [Installation](https://ansromanov.github.io/reaper-mcp-rs/installation.html).

**2. Start the bridge in REAPER** (skip if it starts with REAPER): Actions > Show action list >
New action > Load ReaScript > `Scripts/reaper-mcp/reaper_mcp_bridge.lua`.

**3. Register the server with your MCP client**, using the path the installer printed:

```sh
claude mcp add reaper -- ~/.local/bin/reaper-mcp-rs
```

For Claude Desktop and other clients see
[Connecting an MCP client](https://ansromanov.github.io/reaper-mcp-rs/clients.html). Then ask the
agent to call `health`, or to describe the open project.

## What it can do

| Area | Tools |
|---|---|
| Project | open, save, project tabs, sample rate, tempo map, markers and regions, whole-project snapshot |
| Tracks | create, rename, delete, reorder, folders, fader, pan, mute, solo, phase, multi-output instruments |
| FX | add, remove, move, enable, parameters by name and displayed value, presets, pins, FX chains and track templates |
| Routing | sends with channels, mode, level and phase |
| Items | insert media, move by bars and beats or grid, slip, gain, take pan, channel mode, normalization |
| MIDI | create items, add and list notes |
| Automation | track and FX parameter envelopes |
| Render | project, stems, variants with different mutes |
| Analysis | loudness (LUFS), peaks, crest factor, stereo width and spectrum of WAV files; compare a mix to a reference |

The [REAPER API coverage](https://ansromanov.github.io/reaper-mcp-rs/api-coverage.html) page lists
what is covered area by area and what is not yet (take FX, MIDI CC, transport, item splits and
fades among others); the [roadmap](ROADMAP.md) gives the order.

## How it works

```text
MCP client ──stdio──▶ reaper-mcp-rs ──JSON files──▶ Lua bridge in REAPER ──▶ ReaScript API
```

The server is one Rust binary. The bridge is a Lua ReaScript inside REAPER, so REAPER stays the
only process that calls its own API. They exchange JSON files in a local directory, and the bridge
runs a fixed set of handlers: it never executes code sent by the client.

## Security

The server lets an agent change and render your projects, open and save files, and run any REAPER
action (`run_action`). Back up your projects, and connect it only to clients and agents you trust.
See [Security](https://ansromanov.github.io/reaper-mcp-rs/security.html) and
[SECURITY.md](SECURITY.md).

## Contributing

Issues and pull requests are welcome; see [CONTRIBUTING.md](CONTRIBUTING.md) and
[Development](https://ansromanov.github.io/reaper-mcp-rs/development.html).

## License

[MIT](LICENSE). The Lua JSON module is vendored from [rxi/json.lua](https://github.com/rxi/json.lua)
under MIT (`lua/vendor/json.LICENSE`).

Unofficial: not affiliated with or endorsed by Cockos. REAPER is a trademark of Cockos
Incorporated.
