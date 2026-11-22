# What the tools do

A tour of the tools by task. Every tool with every argument is in the
[tool reference](reference.md). Track index `-1` is the master track everywhere; edits run inside
REAPER undo blocks.

## Bridge and REAPER

`health` reports whether the bridge heartbeat is fresh. `reload_bridge` re-reads the Lua handlers
after an update, without restarting REAPER. `audio_device` reads, closes or opens REAPER's audio
and MIDI devices.

On Windows, three tools work even when REAPER's Lua thread is stuck, because they do not go
through the bridge: `inspect_reaper_windows` reports REAPER processes, visible windows, owned
dialog text and whether the main window responds; `close_reaper_fx_windows` posts `WM_CLOSE` to
visible owned dialogs whose title starts with the given `title_prefix`; `restart_bridge` asks the
installed REAPER to load the bridge again. None of them terminates REAPER.

## Projects and tabs

`get_project_info` (name, tempo, time signature, track count, sample rate, length), `open_project`,
`save_project`, `set_project_sample_rate`. Project tabs: `list_projects`, `switch_project`,
`new_project_tab` and `close_project_tab`. Closing **requires `index`**, keeps the active tab
active and refuses a tab with unsaved changes or the last tab.

`project_snapshot` returns the whole project in one call: tracks, folders, faders, FX, sends, item
counts and master FX. Compare two snapshots to see what changed. `list_plugins` lists the plugins
in use with their instance count and the VST3 version on disk. REAPER keeps an old DLL loaded
after an update, so a newer version on disk needs a REAPER restart.

## Tracks and folders

`list_tracks`, `get_track`, `create_track`, `rename_track`, `delete_track`, and `set_track` for the
fader in dB, pan, mute, solo, phase, master send, channel count and the whole FX chain's enable
state. `describe_track` shows everything that sets the level and sound of one track (fader, pan, FX
chain with pin mappings, envelopes, sends, receives) and notes what acts after the plugins, for
example an active volume envelope that shifts a limiter's ceiling.

Folders: `move_track` reorders one track (`before_index`); `set_track_folder_depth` sets the
folder depth (1 starts a folder, 0 normal, -1..-16 closes levels). Reorder first, then set depths;
routing is not changed. `delete_track` with `with_children` also removes the inside of a folder.

Multi-output instruments such as Kontakt: `probe_instrument_outputs` renders a short range of
every output pair to its own file and reports what plays there (loudness, peak, loudest band;
silent pairs are marked), and `create_multiout_tracks` creates one track per output pair, each
fed by a send from the instrument.

## Templates and FX chains

`export_mix_template` writes item-free `.RTrackTemplate` files for the mix groups DRUMS, BASS,
GUITARS, VOCALS, MIX BUS and RETURNS, plus `master.RfxChain` unless `include_master_chain` is
false. It keeps FX, folder contents and the receives inside a group (remapped to template-local
indexes); receives from outside a group are dropped and counted in the result.

`apply_track_fx_template` copies only the static FX state of a named template track to a project
track. It keeps the destination's media, routing, pan, fader and non-FX envelopes. The default
`mode` is `replace`; `append` adds the FX after the existing chain and keeps its automation.

REAPER has no function that writes a plugin preset, so `export_fx_chain` writes the FX of a track
(all, or the ones named in `fx`) to an `.RfxChain` file with the plugin states inside, and
`import_fx_chain` puts such a file on a track (`append` or `replace`). Use a chain file for "this
guitar amp sound"; a whole group with its tracks, routing and FX is a track template.

## FX and parameters

`list_fx`, `add_fx`, `remove_fx`, `move_fx`, `set_fx_enabled`, presets with `list_fx_presets` and
`set_fx_preset`, and pin mappings with `get_fx_pins` and `set_fx_pins`.

Parameters: `list_fx_params`, `set_fx_param` and `set_fx_params` (several in one undo step). A
parameter is found by index, exact name or a case-insensitive part of the name, and set by
normalized value, by the number the plugin displays (`display_value`, solved by bisection) or by
its displayed text (`display_text`, for switches and choices). `list_fx_param_choices` lists the
texts a switch can show, and `format_fx_param` shows how the plugin would display a value.

## Sends and routing

`list_sends`, `create_send`, `set_send`, `remove_send`. A send takes `source_channel` and
`dest_channel` (the first channel of a pair: 0 = 1/2, 2 = 3/4, 4 = 5/6 …; this is how a multi-out
plugin feeds several tracks), `mono`, `phase_inverted` and `mode` (0 post-fader, 1 pre-FX,
3 post-FX). REAPER cannot change the destination of a send: create the new one and remove the old
one.

## Items and media

`list_items` reports position (also as bar.beat), length, gain, mute, take pan, channel mode,
source file, source channels and the take `start_offset`: a slip edit, so the file's audio appears
at `position - start_offset`. `set_item` changes gain, mute, position, length, `start_offset`,
take pan, channel mode and `source` (play another file). `insert_media` and `delete_item` add and
remove items; `set_items_time_base` sets how items follow tempo changes.

`normalize_items` uses REAPER's own normalization (the calculation behind Item processing >
Normalize): it sets the item gain so that the part of the source an item plays reaches
`target_db` in `peak`, `true_peak`, `lufs_i`, `lufs_m`, `lufs_s` or `rms`. `dry_run` only
reports.

`set_media_offline` closes (or reopens) the file of every item that plays it, so the file can be
rewritten; otherwise REAPER holds it open (`PermissionError` on Windows).

## Musical time, tempo and markers

Move by musical time instead of seconds, which leave the grid as soon as the tempo is not round.
`move_items` moves the items of tracks (or one item) by bars, beats, the project grid or note
values (`1/4`..`1/64`), optionally snapped to a grid, with the automation points
(`include_envelopes`) and folder contents (`with_children`); `dry_run` shows the positions first.
`set_item_position` puts one item at a bar and beat; `get_grid` shows tempo, time signature, grid
size and the length of a bar.

`set_tempo_map` writes tempo markers given in quarter notes (the unit of a MIDI file or a Cubase
tempo track), so MIDI placed by seconds lands on the bars of the audio. `add_markers` adds markers
and regions by project seconds with an optional `#rrggbb` color; `list_markers` lists them.

## MIDI

`create_midi_item`, `add_midi_notes` and `list_midi_notes` (start and end in project seconds,
pitch, velocity, note count per pitch, `pitch` filter). Editing CC and other events is on the
[roadmap](roadmap.md).

## Automation

`set_envelope_points` writes the points of a track envelope: `Volume` with `value_db`, `Pan`, or an
FX parameter with `fx`; a missing envelope is created and shown. `set_fx_envelope` does the same
for one FX parameter. `list_track_envelopes` lists volume, trim, pan, width, mute and plugin
envelopes with their state and range; `set_envelope_active` switches one on or off without
deleting it.

## Rendering

`get_render_settings`, `render_project`, `render_stems` and `render_variants`. Render tools delete
the files they are about to write (`overwrite: false` keeps them), because REAPER never overwrites
and writes `NAME-001.wav` next to the old file, which a caller then reads by mistake. The result
lists the written files with their sizes and warns when one is missing. `fresh: true` saves and
reopens the project before rendering. `render_variants` renders the same stems several times with
different tracks muted per variant and restores the mutes.

## Audio analysis

These work on WAV files and need no REAPER. `analyze_audio` reports integrated loudness, loudness
range, sample peak, samples above full scale, crest factor, stereo width and the spectrum in
third-octave and mixing bands, for a whole file or a time range. `compare_audio` compares two
files (for example a render against a reference) with the second turned to the loudness of the
first, so the band differences describe tone and not volume. Render analysis files as `float32`
(`render_project` and `render_stems` take a `format`) so that levels above 0 dBFS are not cut off.

## Batches and actions

`batch` runs a list of `{tool, args}` in one call and stops at the first error
(`continue_on_error` goes on). It runs the Lua handlers directly, so the arguments are the tools'
own. `run_action` runs a REAPER action by numeric ID or named command: a stop-gap until a feature
gets its own tool.
