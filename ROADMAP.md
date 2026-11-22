# Roadmap

> Last updated: 2026-11-22. A living document: it explains the order of work, and GitHub issues track the
> details. The gaps listed here come from [REAPER API coverage](README.md#reaper-api-coverage).

## Goal

reaper-mcp-rs lets an AI agent work on a real REAPER project the way an engineer does: arrange, edit, route,
mix, automate, render and check the result. Each version below closes the gaps that most often send an agent
to `run_action` or back to the human.

## Non-goals

- **Running Lua sent by the client.** The bridge dispatches a fixed set of handlers, and it stays that way.
- **Control surfaces** (`CSurf_*`). An MCP client is not a control surface.
- **REAPER's UI, themes and docking.** The server changes projects, not the screen layout.
- **Extensions as a requirement.** SWS, js_ReaScriptAPI or ReaImGui may be used when present, but every tool
  must work without them.

## 0.2: release infrastructure

Make the project easy to install and safe to change.

- CI on pull requests and `main`: `cargo fmt`, `clippy -D warnings` and tests on Linux, macOS and Windows.
- A release workflow: SemVer from Conventional Commits, `CHANGELOG.md` and release notes from git-cliff,
  archives for Windows, macOS and Linux holding the binary, the Lua bridge and the install scripts, plus
  `SHA256SUMS`.
- A documentation site (mdBook on GitHub Pages): installation, MCP client setup, a tool reference, security.
- Issue and pull request templates, `CONTRIBUTING.md`.
- macOS support in `install-bridge.sh`: it currently defaults to the Linux resource directory.

## 0.3: everyday editing

What an agent needs between "the project is open" and "the mix can start".

- Transport: play, stop, record, pause, and the play state and position.
- Edit cursor, time selection and loop range.
- Items: split, fades (length and shape), play rate and pitch, the active take.
- Tracks: color, record arm, record input and monitoring, automation mode, selection.
- Markers and regions: edit and delete a single one, not only add or clear all.
- Undo and redo.

## 0.4: MIDI

MIDI is the least covered area (8 of 75 functions).

- Edit and delete notes, not only add them.
- CC, pitch bend, program change and aftertouch events, including CC shapes.
- Text, lyric and sysex events.
- Quantize and humanize by the project grid.

## 0.5: take FX and envelopes

- Take FX through the same tools as track FX (one code path with a target of track or take).
- Take envelopes, and deleting an envelope.
- Automation items.

## Later

- FX containers, parameter modulation and parameter linking.
- Hardware outputs.
- The region render matrix and region-based rendering.
- Time signature changes in the tempo map.
- Take markers and stretch markers.
- Project notes and ExtState, so that an agent can leave notes in the project.
- The plugin version REAPER has actually loaded, not only the one on disk.

## Reliability

These run alongside the versions above.

- Several MCP clients per IPC directory: today two clients share one request file and disturb each other.
- Several REAPER instances.
- Tests without REAPER: a fake bridge that answers the IPC contract, so that the Rust side is covered in CI.
- A self-test that runs the project-changing tools in a throwaway project tab and closes it.
