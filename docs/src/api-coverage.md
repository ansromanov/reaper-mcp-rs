# REAPER API coverage

Measured against the [ReaScript API reference](https://www.reaper.fm/sdk/reascript/reascripthelp.html)
(734 functions, not counting the EEL2, Lua and Python built-ins) on 2026-11-22. The Lua bridge calls 129 of
them, about 18%. The function count understates what the tools can do: REAPER exposes most track, item, take,
send and project properties through a few generic `Get*Info_Value` / `GetSet*Info_String` functions, and
`run_action` reaches any action by its ID. Audio analysis reads rendered WAV files in Rust and needs no API
calls at all.

| Area | Functions used | Covered | Not covered yet |
|---|---|---|---|
| Track FX (`TrackFX_*`) | 21 / 51 | add, remove, move, enable, parameters by value, display value or text, presets, pin mappings, FX chains as `.RfxChain` | FX containers, parameter modulation and linking, offline state, record-input FX |
| Take FX (`TakeFX_*`) | 0 / 43 | — | everything |
| Tracks | 16 / 64 | list, create, rename, delete, reorder, folders, fader, pan, mute, solo, phase, master send, channels, templates | color, record arm, input and monitoring, automation mode, track groups, selection |
| Sends and receives | 5 / 15 | list, create, edit (level, pan, channels, mode, mute, phase), remove | hardware outputs |
| Media items and takes | 20 / 57 | list, insert media, move by musical time, position, length, offset, gain, take pan, channel mode, source, delete, normalize | split, fades, play rate and pitch, active take, take markers, stretch markers, automation items |
| MIDI | 8 / 75 | create MIDI items, list and add notes | edit or delete notes, CC, pitch bend, sysex and text events, MIDI editor |
| Envelopes | 15 / 40 | volume, pan and FX parameter envelopes on tracks: list, write points, activate | take envelopes, automation items, deleting an envelope |
| Markers, regions, tempo | 12 / 69 | markers and regions with colors, tempo map, grid, bar and beat conversion | edit or delete single markers, time signature changes, region render matrix, take markers |
| Project, transport, undo | 16 / 72 | open, save, project tabs, sample rate, length, undo blocks around edits | transport (play, stop, record), edit cursor, time selection and loop, undo and redo, project notes |
| Render and audio | 4 / 28 | render settings, project, stems and variants, loudness normalization; analysis of WAV files in Rust | region render matrix, peaks, audio accessors |
| Actions | 1 / 16 | `run_action` by command ID or named command | toggle state, action list |
| ExtState and config | 2 / 12 | resource path, version | ExtState, project ExtState, config variables |
| Control surfaces (`CSurf_*`) | 0 / 52 | — | not planned: an MCP client is not a control surface |
| UI, themes, docking, other | 9 / 140 | refresh after edits; window inspection on Windows (without the API) | mostly out of scope for an MCP server |

Extension APIs such as SWS, js_ReaScriptAPI and ReaImGui are not used. The order in which the gaps get
closed is in the [roadmap](roadmap.md).
