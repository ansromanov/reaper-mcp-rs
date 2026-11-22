# reaper-mcp-rs

Rust MCP server for REAPER, built around real project workflows.

Unofficial: not affiliated with or endorsed by Cockos. REAPER is a trademark of Cockos Incorporated.

> **Security.** This server lets an AI agent change and render your REAPER projects, open and save
> files, and run arbitrary REAPER actions (`run_action`). Back up your projects, and connect it only
> to clients and agents you trust.

Licensed under MIT, see `LICENSE`.

The first vertical slice uses an MCP stdio server in Rust and a Lua ReaScript
inside REAPER. They exchange versioned JSON request and response files in a
local IPC directory. REAPER remains the only process that calls its own API.

## Current tools

Project: `health`, `inspect_reaper_windows`, `close_reaper_fx_windows`, `restart_bridge`, `audio_device`, `get_project_info`, `open_project`, `save_project`,
`get_render_settings`, `render_project`, `run_action`, `reload_bridge`, `set_project_sample_rate` (project sample
rate in Hz; `get_project_info` reports it as `sample_rate`), `set_tempo_map` (tempo markers given in quarter notes,
the first at 0, each marker's time follows from the tempi before it; replaces the existing markers unless
`clear` is false; the unit of a MIDI file or a Cubase tempo track, so MIDI placed by seconds lands on the bars of the
audio).
Markers: `add_markers` (markers by position and regions by position and end, in project seconds, with an
optional `#rrggbb` color; existing ones are kept unless `clear` is true) and `list_markers`.
`export_mix_template` writes item-free `.RTrackTemplate` files for the default
mix groups DRUMS, BASS, GUITARS, VOCALS, MIX BUS, and RETURNS, plus
`master.RfxChain` unless `include_master_chain=false`. It retains FX, folder
descendants, and internal receives (remapped to template-local indexes).
Receives from tracks outside a group are omitted and counted in the result.
`apply_track_fx_template` copies only static FX state from a named template
track to a project track. It removes old FX parameter/program envelopes but
keeps the destination's media, routing, pan, fader, and non-FX envelopes.
Pass `mode: "append"` to add those FX after the existing chain while retaining
the destination FX and their automation; the default mode remains `replace`.

Folders: `move_track` reorders one track (`before_index`); `set_track_folder_depth` sets I_FOLDERDEPTH (1 starts a folder, 0 normal, -1..-16 closes levels). Reorder first, then set depths; routing is not changed.
`delete_track` removes a track (`with_children` also removes the inside of a folder; the master track is refused).

Project tabs: `list_projects`, `switch_project`, `new_project_tab`, `open_project`, and
`close_project_tab`, which now **requires `index`** and keeps the previously active tab active (it used to close
the active tab, which closed the wrong project once). It refuses a tab with unsaved changes and the last tab.

Tracks: `list_tracks`, `get_track`, `create_track`, `rename_track`, `set_track`
(fader in dB, pan, mute, solo, phase, master send, whole FX-chain enable), `list_items`. Track index
`-1` is the master track.

FX: `list_fx`, `add_fx`, `remove_fx`, `set_fx_enabled`, `list_fx_params`,
`set_fx_param`, `set_fx_params`, `set_fx_preset`, `list_fx_presets`. A
parameter can be set by normalized value, by the number the plugin displays
(`display_value`, solved by bisection) or by its displayed text
(`display_text`, for switches and choices).

Sends: `list_sends`, `create_send`, `set_send`, `remove_send`. A send reports and takes
`source_channel` / `dest_channel` (first channel of the pair: 0 = 1/2, 2 = 3/4, 4 = 5/6 ...; this is how a multi-out plugin
such as Kontakt feeds several tracks), `mono`, `phase_inverted` and `mode` (0 post-fader, 1 pre-FX, 3 post-FX).
REAPER cannot change the destination of a send: create the new one and remove the old one.

Items and media: `list_items` (also bar.beat position, take pan, channel mode, source channels and the take
`start_offset`: a slip edit, the file's audio then appears at `position - start_offset`), `set_item` (gain, mute,
position, length, `start_offset`, take pan, channel mode, `source` to play another file), `insert_media`, `delete_item`,
`normalize_items` (REAPER's own normalization, the calculation behind Item processing > Normalize: sets the item gain so
that the part of the source an item plays reaches `target_db` in `peak`, `true_peak`, `lufs_i`, `lufs_m`, `lufs_s` or
`rms`; `tracks` / `exclude_tracks` pick the tracks, `dry_run` only reports; the existing item gain is replaced, take
volume is kept).

Musical time (use these instead of moving by seconds, which leaves the grid as soon as the tempo is not round):
`move_items` moves the items of tracks (or one item) by bars, beats, the project grid or note values (`1/4`..`1/64`),
negative values earlier, optionally snapped to a grid (`snap_to`), with the automation points (`include_envelopes`),
with the folder contents (`with_children`) and with `dry_run` to see the from/to positions (seconds and bar.beat)
first; `set_item_position` puts one item at a bar and beat; `get_grid` shows tempo, time signature, grid size and the
length of a bar. `set_media_offline` closes (or reopens) the
file of every item that plays it, so the WAV can be rewritten; REAPER otherwise holds it open
(`PermissionError` on Windows).

MIDI: `list_midi_notes` (start/end in project seconds, pitch, velocity, note count per pitch, `pitch` filter),
`add_midi_notes`, `create_midi_item`.

Automation: `set_envelope_points` writes the points of a track envelope (`Volume` with `value_db`, `Pan`, or an FX
parameter with `fx`; the envelope is created and shown when missing), next to `list_track_envelopes` and
`set_envelope_active`.

Inspection: `project_snapshot` returns the whole project in one call (tracks, folders, faders, FX, sends, item
counts, master FX); compare two snapshots to see what changed. `list_plugins` lists the plugins
used with their instance count and the VST3 version on disk. REAPER keeps an old DLL loaded after an update, so a
newer version on disk needs a REAPER restart. The project length is `length_seconds` in `get_project_info`.

Rendering: `render_project` and `render_stems` delete the files they are about to write (`overwrite: false` keeps
them), because REAPER never overwrites and writes `NAME-001.wav` next to the old file, which a caller then reads by
mistake. The result lists the written `files` with their sizes and gives `warnings` when a file is missing, for
example when an old file could not be deleted. `fresh: true` saves the project and reopens it before the render (a
project edited in place has rendered silent or wrong). `render_variants` renders the same stems several times with
different tracks muted or unmuted per variant (for example live only, trigger only, both) and restores the mutes.

Batch: `batch` runs a list of `{tool, args}` in one call and stops at the first error (`continue_on_error` to go on).
It runs the Lua handlers directly, so the arguments are the tool's own arguments.

Plugin states as files: REAPER has no function that writes a plugin preset, so `export_fx_chain` writes the FX of a track
(all of them, or the ones named in `fx` by index or name) to an `.RfxChain` file with the plugin states inside, and
`import_fx_chain` puts such a file on a track (`append` after the existing FX, or `replace`). Use a chain file for "this guitar
amp sound"; a whole group with its tracks, routing and FX is a `.RTrackTemplate` (`export_mix_template`).

Level path: `describe_track` shows everything that sets the level and sound of one track (fader,
pan, whole-chain FX enable state, FX chain with pin mappings, automation envelopes, sends, receives) and notes what acts after
the plugins, for example an active volume envelope that shifts a limiter's ceiling.
`list_track_envelopes` lists volume, trim, pan, width, mute and plugin parameter envelopes with
their state and value range; `set_envelope_active` switches one on or off without deleting it.

Audio analysis (no REAPER needed): `analyze_audio` reports integrated loudness, loudness range,
sample peak, samples above full scale (overloads), crest factor, stereo width and the spectrum
in third-octave and coarse mixing bands for a WAV file or a time range of it;
`compare_audio` compares two files (for example a render against a professional reference) with
the second file turned to the loudness of the first, so the band differences describe tone and
not volume. Render analysis files as `float32` (`render_project` and `render_stems` take a
`format`) so that levels above 0 dBFS are not cut off.

Track edits run inside REAPER undo blocks. The bridge uses static command
dispatch; it does not execute Lua supplied by the MCP client. `run_action`
runs a REAPER action by ID or named command and is meant as a stop-gap until
a feature gets its own tool.

## Setup on Windows

Run `scripts/install-bridge.ps1` (add `-Startup` to start the bridge from
`Scripts\__startup.lua`). Start the bridge in a running REAPER by loading
`Scripts/reaper-mcp/reaper_mcp_bridge.lua`, or with
`reaper.exe -nonewinst <that file>`. Starting it again supersedes the running
copy, and `reload_bridge` re-reads the handlers after an update.

`inspect_reaper_windows` reports REAPER processes, visible windows, owned dialog text,
priority class, and main-window responsiveness without calling the Lua bridge.
`close_reaper_fx_windows` posts WM_CLOSE only to visible, owned dialogs whose title starts with the required `title_prefix` argument (for example
`VST3: MyPlugin (Vendor) - Track `); its result reports posted messages, not closure.
`restart_bridge` launches the installed bridge loader through a fixed wrapper and
reports that launch was requested; it does not terminate REAPER or verify recovery.

## Setup on Linux

1. Install the Lua bridge into REAPER's resource directory:

   ```sh
   ./scripts/install-bridge.sh
   ```

   For a portable REAPER install, pass its resource directory:

   ```sh
   ./scripts/install-bridge.sh /path/to/REAPER
   ```

2. In REAPER, choose **Actions > Show action list > New action > Load ReaScript**.
   Select `reaper_mcp_bridge.lua` and run it. Keep REAPER open.

3. Build the server:

   ```sh
   cargo build --release
   ```

4. Register the binary as a stdio MCP server in your MCP client:

   ```json
   {
     "mcpServers": {
       "reaper": {
         "command": "/absolute/path/to/reaper-mcp-rs/target/release/reaper-mcp-rs"
       }
     }
   }
   ```

The default IPC directory is the platform REAPER resource directory plus
`reaper-mcp-bridge`. Set `REAPER_MCP_DIR` in both REAPER and the MCP server process to
override it.

On Linux, the default REAPER resource directory is `~/.config/REAPER`. On
Windows it is `%APPDATA%/REAPER`. On macOS it is
`~/Library/Application Support/REAPER`.

## IPC contract

Each request has `id`, `command`, and `args` fields. The response echoes the
request ID and contains either `result` or `error`. The server serializes
requests, publishes files with atomic rename, and times out after five
seconds. The bridge heartbeat lets the server report a stopped or stale Lua
script.

## Scope

It supports one MCP server process per REAPER IPC directory (two clients at the same time share one
request file and disturb each other) and one REAPER instance. Not covered yet: writing MIDI CC or other events
(notes only), creating a take FX or item envelope, removing an envelope, and reading the plugin version REAPER has
actually loaded (only the one on disk).

The Lua JSON module is vendored from [rxi/json.lua](https://github.com/rxi/json.lua)
under MIT; see `lua/vendor/json.LICENSE`.
