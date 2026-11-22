# Tool reference

All 77 tools with their arguments, generated from the tool definitions. To change a
description, edit `src/server.rs` or `src/args.rs` and run
`REAPER_MCP_UPDATE_DOCS=1 cargo test`.

Track indexes start at 0; `-1` is the master track.

## `add_fx`

Append an FX to a track by name. Returns the new FX index and parameter count.

| Argument | Type | Description |
|---|---|---|
| `name` (required) | string | FX name as REAPER shows it, for example `VST: ReaEQ (Cockos)` or just `ReaEQ`. |
| `track` (required) | integer | Zero-based track index; -1 is the master track. |

## `add_markers`

Add markers (a position) and regions (a position and an end) by name at project times in seconds, for navigating a long project. Existing markers are kept unless clear is true.

| Argument | Type | Description |
|---|---|---|
| `clear` | boolean | Delete all existing markers and regions first (default false). |
| `markers` (required) | array | Markers and regions to add. |

## `add_midi_notes`

Add notes to an existing MIDI item (times in project seconds).

| Argument | Type | Description |
|---|---|---|
| `item` (required) | integer | Zero-based index of the MIDI item on the track. |
| `notes` (required) | array |  |
| `track` (required) | integer |  |

## `analyze_audio`

Analyse a rendered WAV file: integrated loudness (LUFS), loudness range, sample peak, samples above full scale (overloads), crest factor, stereo width and the spectrum in third-octave and coarse mixing bands. Reads 32-bit float files without cutting off levels above 0 dBFS. Warns about overloads and missing headroom.

| Argument | Type | Description |
|---|---|---|
| `end_seconds` | number | Analyse only up to this time in seconds (default: end of the file). |
| `path` (required) | string | Absolute path of a WAV file (16, 24 or 32-bit integer, or 32-bit float). |
| `start_seconds` | number | Analyse only from this time in seconds (default: start of the file). |

## `apply_track_fx_template`

Apply static FX state from a named source track in an exported .RTrackTemplate. mode defaults to replace (existing behavior); append adds source FX after the destination chain and preserves all existing FX automation. Both modes preserve destination media, routing, pan and fader. Source timeline automation is never copied. Target -1 is the master track.

| Argument | Type | Description |
|---|---|---|
| `mode` | string | FX chain operation: replace (default) or append source FX after existing FX. |
| `source_track` (required) | string | Exact source track name inside the template. |
| `target_track` (required) | integer | Zero-based destination track index; -1 addresses the master track. |
| `template_path` (required) | string | Path to an exported .RTrackTemplate file. |

## `audio_device`

Read, close, or open REAPER's audio and MIDI devices. Reports observed running state before and after the action; open/close do not imply the device successfully changed state.

| Argument | Type | Description |
|---|---|---|
| `action` (required) | AudioDeviceAction | Read status or close/open REAPER's audio and MIDI devices. |

## `batch`

Run several tools in one call: calls is a list of {tool, args}. Stops at the first error unless continue_on_error is true. Each call has its own undo step.

| Argument | Type | Description |
|---|---|---|
| `calls` (required) | array |  |
| `continue_on_error` | boolean | Keep going after a failed call (default false: stop at the first error). |
| `timeout_seconds` | integer | Give up waiting after this many seconds (default 3600). |

## `close_project_tab`

Close the project tab with the given index (see list_projects). The index is required; the previously active tab stays active. Refuses if that tab has unsaved changes or if it is the last tab.

| Argument | Type | Description |
|---|---|---|
| `index` (required) | integer | Zero-based project tab index from `list_projects`. Required: only the tab you name is closed. |

## `close_reaper_fx_windows`

Post WM_CLOSE only to visible, owned REAPER #32770 dialogs whose title starts exactly with title_prefix (for example 'VST3: MyPlugin (Vendor) - Track '). Reports messages posted, not verified closure; never targets REAPER's main window or other dialogs. Windows only.

| Argument | Type | Description |
|---|---|---|
| `title_prefix` (required) | string | Exact start of the dialog title to close, for example "VST3: MyPlugin (Vendor) - Track ". Must not be empty. |

## `compare_audio`

Frequency and level comparison of two WAV files, for example your render against a professional reference. File B is turned to the loudness of A, so the per-band differences (third-octave and coarse mixing bands) describe tone, not volume; positive means B has more energy there. Also loudness, crest, width and overload differences. Use a_start_seconds and b_start_seconds to line up files that start at different times.

| Argument | Type | Description |
|---|---|---|
| `a` (required) | string | Reference file A (for example the professional mix or stem). |
| `a_start_seconds` | number | Start time in A in seconds (default 0). Use it with b_start_seconds to line up the two files. |
| `b` (required) | string | File B, compared against A (for example your render). |
| `b_start_seconds` | number | Start time in B in seconds (default 0). |
| `duration_seconds` | number | Length of the compared part in seconds (default: as much as both files have after their start). |

## `create_midi_item`

Create an empty MIDI item on a track between two project times.

| Argument | Type | Description |
|---|---|---|
| `end_seconds` (required) | number |  |
| `start_seconds` (required) | number |  |
| `track` (required) | integer |  |

## `create_multiout_tracks`

Route a multi-out instrument (Kontakt with Krimh, ML Drums ...) to separate tracks: gives the instrument track enough channels and creates one track per output pair right after it (optionally in a new folder), each receiving its pair through a send. Use probe_instrument_outputs first to see which pairs carry which drum.

| Argument | Type | Description |
|---|---|---|
| `folder` | string | Put the new tracks into a new folder track with this name. |
| `instrument_to_parent` | boolean | false turns off the instrument's own send to its parent, so only the new tracks are heard. |
| `outputs` (required) | array | One new track per output pair, created right after the instrument in this order. |
| `send_mode` | integer | Send mode: 0 post-fader (default), 1 pre-FX, 3 post-FX before the fader. |
| `track` (required) | integer | Track with the multi-out instrument (Kontakt, ML Drums ...). |

## `create_send`

Create a send between two tracks, optionally with level (dB) and pan.

| Argument | Type | Description |
|---|---|---|
| `dest_channel` | integer | First destination channel of the pair (0 = channels 1/2). |
| `destination` (required) | integer | Destination track index. |
| `mode` | integer | Where the send taps the track: 0 = post-fader, 1 = pre-FX, 3 = post-FX before the fader. |
| `mono` | boolean |  |
| `pan` | number |  |
| `phase_inverted` | boolean |  |
| `source` (required) | integer | Source track index. |
| `source_channel` | integer | First source channel of the pair: 0 = channels 1/2, 2 = 3/4, 4 = 5/6 ...; -1 = no audio. |
| `volume_db` | number |  |

## `create_track`

Create a named track at the end of the current REAPER project.

| Argument | Type | Description |
|---|---|---|
| `name` (required) | string | Name to assign to the new track. |

## `delete_item`

Delete one media item from a track (indexes from list_items).

| Argument | Type | Description |
|---|---|---|
| `item` (required) | integer | Zero-based item index from `list_items`. |
| `track` (required) | integer | Zero-based track index. |

## `delete_track`

Delete a track. With with_children also everything inside a folder track. The master track cannot be deleted.

| Argument | Type | Description |
|---|---|---|
| `track` (required) | integer | Zero-based track index from `list_tracks` (the master track cannot be deleted). |
| `with_children` | boolean | For a folder track: also delete everything inside it. Default false (the children stay). |

## `describe_track`

Everything that sets the level and sound of one track: fader, pan, whole-chain FX enable state, FX chain with pin mappings, automation envelopes (volume, pan, mute, plugin parameters), sends, receives, parent. Notes call out what acts after the plugins, such as an active volume envelope that shifts a limiter's ceiling.

| Argument | Type | Description |
|---|---|---|
| `track` (required) | integer | Zero-based track index from `list_tracks`; -1 is the master track. |

## `export_fx_chain`

Write the FX of a track (all, or the ones named in fx) to an .RfxChain file with the plugin states inside: the way to keep a plugin sound (an amp setting, a vocal chain) as a file. REAPER has no function that writes plugin preset files.

| Argument | Type | Description |
|---|---|---|
| `fx` | array | Only these plugins: FX indexes or parts of their names. Default: the whole chain. |
| `path` (required) | string | Path of the `.RfxChain` file to write. |
| `track` (required) | integer | Zero-based track index; -1 is the master track. |

## `export_mix_template`

Export item-free REAPER track templates for named mix groups (default: DRUMS, BASS, GUITARS, VOCALS, MIX BUS, RETURNS). Preserves track FX, envelopes, folder descendants and internal receives; receives from tracks outside a group are omitted and reported. Also exports the master chain to master.RfxChain by default; set include_master_chain=false to skip it.

| Argument | Type | Description |
|---|---|---|
| `directory` (required) | string | Destination directory for the .RTrackTemplate files. |
| `groups` | array | Optional track groups. Defaults to DRUMS, BASS, GUITARS, VOCALS, MIX BUS, RETURNS. |
| `include_master_chain` | boolean | Whether to also write the active project's master FX chain to master.RfxChain (defaults to true). |

## `format_fx_param`

Show the text a plugin displays for a normalized parameter value, without changing the parameter.

| Argument | Type | Description |
|---|---|---|
| `fx` (required) | integer |  |
| `param` (required) | ParamRef | Parameter index or name. |
| `track` (required) | integer |  |
| `value` (required) | number | Normalized value 0..1 to format. |

## `get_fx_pins`

Read an FX's pin mappings: which track channels (bitmask) feed each input pin and receive each output pin.

| Argument | Type | Description |
|---|---|---|
| `fx` (required) | integer | Zero-based FX index from `list_fx`. |
| `track` (required) | integer | Zero-based track index; -1 is the master track. |

## `get_grid`

Project tempo, time signature, grid size and the length of a bar in seconds: what one grid step or one bar means before moving anything.

No arguments.

## `get_project_info`

Read the current REAPER project name, tempo, time signature, and track count.

No arguments.

## `get_render_settings`

Read the project's current render settings (output folder, pattern, sample rate, bounds).

No arguments.

## `get_track`

Read one track in detail: fader, pan, mute/solo, whole-chain FX enabled state, folder depth, parent, channels, FX/send/receive/item counts. Track -1 is master.

| Argument | Type | Description |
|---|---|---|
| `track` (required) | integer | Zero-based track index from `list_tracks`; -1 is the master track. |

## `health`

Check whether the REAPER Lua bridge is running.

No arguments.

## `import_fx_chain`

Put the FX of an .RfxChain file on a track, after its existing FX (append) or instead of them (replace). Plugin states come with the file.

| Argument | Type | Description |
|---|---|---|
| `mode` | string | `append` (default, after the FX the track has) or `replace`. |
| `path` (required) | string | Path of an `.RfxChain` file. |
| `track` (required) | integer | Zero-based track index; -1 is the master track. |

## `insert_media`

Put an audio file on a track as a new item at a position in seconds.

| Argument | Type | Description |
|---|---|---|
| `path` (required) | string | Absolute path of the audio file. |
| `position` | number | Item position in seconds (default 0). |
| `track` (required) | integer | Zero-based track index; -1 is the master track. |

## `inspect_reaper_windows`

Inspect REAPER process windows and visible owned dialogs without using the Lua bridge. Reports window titles, class names, static child text, and whether the main window responds.

No arguments.

## `list_fx`

List the FX chain of a track with indexes, names, bypass state and parameter counts.

| Argument | Type | Description |
|---|---|---|
| `track` (required) | integer | Zero-based track index from `list_tracks`; -1 is the master track. |

## `list_fx_param_choices`

List the distinct texts a choice parameter can show (shapes, modes, styles) with the normalized value for each.

| Argument | Type | Description |
|---|---|---|
| `fx` (required) | integer |  |
| `param` (required) | ParamRef | Parameter index or name. |
| `steps` | integer | Sampling resolution across 0..1 (default 400). |
| `track` (required) | integer |  |

## `list_fx_params`

List all parameters of an FX with names, normalized values and formatted values (real units).

| Argument | Type | Description |
|---|---|---|
| `fx` (required) | integer | Zero-based FX index from `list_fx`. |
| `track` (required) | integer | Zero-based track index; -1 is the master track. |

## `list_fx_presets`

Read the current preset name and preset count of an FX.

| Argument | Type | Description |
|---|---|---|
| `fx` (required) | integer | Zero-based FX index from `list_fx`. |
| `track` (required) | integer | Zero-based track index; -1 is the master track. |

## `list_items`

List media items on a track with position, length, gain, mute, take pan, take channel mode, source channel count and source file. Take pan acts before the track FX.

| Argument | Type | Description |
|---|---|---|
| `track` (required) | integer | Zero-based track index from `list_tracks`; -1 is the master track. |

## `list_markers`

List project markers and regions with list index, marker number, name, position and region end (seconds) and color.

No arguments.

## `list_midi_notes`

List the notes of a MIDI item: start/end in project seconds, pitch, velocity, channel, plus the note count per pitch.

| Argument | Type | Description |
|---|---|---|
| `item` (required) | integer | Zero-based index of the MIDI item on the track (`list_items`). |
| `limit` | integer | Return at most this many notes (default 5000). |
| `offset` | integer | Skip this many notes first. |
| `pitch` | integer | Only notes of this MIDI pitch (the per-pitch counts always cover all notes). |
| `track` (required) | integer | Zero-based track index. |

## `list_plugins`

Plugins used in the project (instances, first tracks) with the VST3 version found on disk. REAPER keeps old DLLs loaded after an update, so restart it when the version on disk is newer than expected.

| Argument | Type | Description |
|---|---|---|
| `filter` | string | Only plugins whose name contains this text. |

## `list_projects`

List open project tabs with path, dirty state and track count. All other tools act on the active tab.

No arguments.

## `list_sends`

List sends and receives of a track.

| Argument | Type | Description |
|---|---|---|
| `track` (required) | integer | Zero-based track index from `list_tracks`; -1 is the master track. |

## `list_track_envelopes`

List automation envelopes of a track: volume, trim, pan, width and mute envelopes and plugin parameter envelopes, with active state, point count and value range (dB for volume).

| Argument | Type | Description |
|---|---|---|
| `include_all_parameters` | boolean | Also scan every plugin parameter for automation (slow on plugins with thousands of MIDI parameters). |
| `track` (required) | integer | Zero-based track index; -1 is the master track. |

## `list_tracks`

List current REAPER tracks with zero-based indexes, names, GUIDs, and volume.

No arguments.

## `move_fx`

Move an FX to another position in the same chain; to_index is its final index.

| Argument | Type | Description |
|---|---|---|
| `fx` (required) | integer | Current zero-based FX index. |
| `to_index` (required) | integer | Final zero-based FX index in the chain. |
| `track` (required) | integer |  |

## `move_items`

Move the items of tracks (or one item) in musical time: by bars, beats, the project grid or note values, optionally snapped to a grid, with the automation points. Negative values move earlier. dry_run only reports the from/to positions as seconds and bar.beat. Prefer this to moving by seconds: it stays on the grid.

| Argument | Type | Description |
|---|---|---|
| `dry_run` | boolean | Only report what would move. |
| `include_envelopes` | boolean | Move the automation points of the tracks by the same musical distance (default true). |
| `item` | integer | Move only this item (index from `list_items`); needs exactly one track. |
| `snap_to` | string | Round every new position to this grid (same units as `unit`). |
| `tracks` (required) | array | Zero-based indexes of the tracks whose items move. |
| `unit` | string | `bar`, `beat`, `grid` (the project grid), `1/1`, `1/2`, `1/4`, `1/8`, `1/16`, `1/32` or `1/64`. Default `bar`. |
| `value` (required) | number | How many units to move: positive later, negative earlier, fractions allowed (0.5 bar). |
| `with_children` | boolean | Also move everything inside the folder tracks listed. |

## `move_track`

Move one track before another position (0-based `before_index`; the track count moves it to the end). Folder state is left alone: reorder first, then call set_track_folder_depth.

| Argument | Type | Description |
|---|---|---|
| `before_index` (required) | integer | Zero-based index the track should end up before; the track count moves it to the end. |
| `index` (required) | integer | Zero-based index of the track to move (from `list_tracks`). |

## `new_project_tab`

Open a new empty project tab and make it active (the other tabs stay open).

No arguments.

## `normalize_items`

Normalize the audio items of the chosen tracks with REAPER's own normalization (the one behind Item processing > Normalize): the item gain is set so that the part of the source the item plays reaches target_db in peak, true_peak, lufs_i, lufs_m, lufs_s or rms. Replaces the existing item gain; take volume is kept. Use dry_run to see the gains first, exclude_tracks to skip drums or a reference.

| Argument | Type | Description |
|---|---|---|
| `dry_run` | boolean | Only report what would change. |
| `exclude_tracks` | array | Zero-based track indexes to leave alone. |
| `mode` | string | peak (default), true_peak, lufs_i, lufs_m, lufs_s or rms. |
| `target_db` (required) | number | Normalization target in dB (0 or lower): dBFS for peak and true_peak, LUFS for the lufs modes, dB for rms. |
| `tracks` | array | Zero-based track indexes to normalize (default: every track). |

## `open_project`

Open an .rpp project in REAPER. Refuses when the current project has unsaved changes unless discard_unsaved is true. Loading can take minutes for heavy projects.

| Argument | Type | Description |
|---|---|---|
| `discard_unsaved` | boolean | Open even if the current project has unsaved changes (they are lost). |
| `path` (required) | string | Absolute path of the .rpp file to open. |

## `probe_instrument_outputs`

Find out what a multi-out instrument plays on each output pair: renders every pair (outputs 1/2, 3/4 ...) of the track to its own file through temporary tracks, measures them and reports loudness, peak and the loudest frequency band per pair (silent pairs are marked). Temporary tracks are removed and the track's channels and mute are restored. Render a short busy range (start_seconds/end_seconds).

| Argument | Type | Description |
|---|---|---|
| `channels` | integer | How many instrument channels to probe; default: the largest output count of the track's FX. |
| `directory` (required) | string | Folder for the probe files (one WAV per output pair). |
| `end_seconds` | number |  |
| `start_seconds` | number |  |
| `track` (required) | integer | Track with the multi-out instrument; it must have MIDI to play in the range. |

## `project_snapshot`

The whole project in one call: tracks with folder structure, fader, pan, mute, FX, sends and item counts, plus the master FX. Compare two snapshots to see what changed.

No arguments.

## `reload_bridge`

Re-read the bridge's Lua handlers inside REAPER after the bridge files were updated, without restarting REAPER.

No arguments.

## `remove_fx`

Delete an FX from a track by index.

| Argument | Type | Description |
|---|---|---|
| `fx` (required) | integer | Zero-based FX index from `list_fx`. |
| `track` (required) | integer | Zero-based track index; -1 is the master track. |

## `remove_send`

Remove one send from a track. The later sends of that track move down by one index; the result lists the sends that remain.

| Argument | Type | Description |
|---|---|---|
| `send_index` (required) | integer | Send index from `list_sends`. |
| `source` (required) | integer | Source track index. |

## `rename_track`

Rename a REAPER track by its zero-based index from list_tracks.

| Argument | Type | Description |
|---|---|---|
| `index` (required) | integer | Zero-based track index from `list_tracks`. |
| `name` (required) | string | New track name. |

## `render_project`

Render the whole project master mix to directory/name.<ext> using the project's saved render format. Deletes an old file with that name first (overwrite=false keeps it), lists the written files and warns when one is missing. Blocks until the render finishes.

| Argument | Type | Description |
|---|---|---|
| `directory` (required) | string | Output directory. |
| `end_seconds` | number | Render only up to this time (seconds). |
| `format` | string | Output format: `pcm16`, `pcm24`, `float32` (keeps levels above 0 dBFS) or a raw base64 render config. Default: the project's saved format. |
| `fresh` | boolean | Save the project and open it again before rendering, so the render sees the saved state (default false). |
| `name` (required) | string | File name without extension; the extension comes from the project's render format. |
| `overwrite` | boolean | Delete the files this render is about to write (default true). REAPER itself never overwrites: it writes `NAME-001.wav` and leaves the old file, which a caller would then read by mistake. |
| `sample_rate` | integer |  |
| `start_seconds` | number | Render only from this time (seconds); give together with end_seconds. Default: the whole project. |
| `timeout_seconds` | integer | Give up waiting after this many seconds (default 1800). |

## `render_stems`

Render the given tracks as separate stem files (each track's own output including its children), using the project's saved render format. Deletes old stems with the same names first (overwrite=false keeps them), lists the written files, warns when one is missing and restores render settings and track selection afterwards. Blocks until done.

| Argument | Type | Description |
|---|---|---|
| `directory` (required) | string | Output directory. |
| `end_seconds` | number | Render only up to this time (seconds). |
| `format` | string | Output format: `pcm16`, `pcm24`, `float32` (keeps levels above 0 dBFS) or a raw base64 render config. Default: the project's saved format. |
| `fresh` | boolean | Save the project and open it again before rendering, so the render sees the saved state (default false). |
| `overwrite` | boolean | Delete the files this render is about to write (default true). REAPER itself never overwrites: it writes `NAME-001.wav` and leaves the old file, which a caller would then read by mistake. |
| `pattern` | string | File name pattern; defaults to `$track`. |
| `start_seconds` | number | Render only from this time (seconds); give together with end_seconds. Default: the whole project. |
| `timeout_seconds` | integer | Give up waiting after this many seconds (default 3600). |
| `tracks` (required) | array | Zero-based indexes of the tracks to render, each as its own file named after the track. |

## `render_variants`

Render the same tracks as stems several times with different tracks muted or unmuted per variant (for example live only, trigger only, both). Saves and reopens the project before each render and restores the original mutes at the end.

| Argument | Type | Description |
|---|---|---|
| `directory` (required) | string | Output directory. |
| `end_seconds` | number |  |
| `format` | string |  |
| `fresh` | boolean | Save and reopen the project before each render (default true). |
| `overwrite` | boolean | Delete the files a render is about to write (default true). |
| `pattern` | string | File name pattern, default `$track-{variant}`. |
| `start_seconds` | number |  |
| `timeout_seconds` | integer | Give up waiting after this many seconds (default 3600). |
| `tracks` (required) | array | Zero-based indexes of the tracks to render as stems in every variant. |
| `variants` (required) | array |  |

## `restart_bridge`

Launch a fresh REAPER bridge instance using the installed fixed bridge loader. This only reports that launch was requested; it does not verify bridge recovery or terminate any process.

No arguments.

## `run_action`

Run a REAPER action by numeric ID or named command. Escape hatch for features without a dedicated tool; prefer adding a real tool.

| Argument | Type | Description |
|---|---|---|
| `command_id` (required) | ActionId | Numeric action ID or a named command such as `_SWS_SAVEALLSELTRK`. |

## `save_project`

Save the current project in place, or save a copy to `path`.

| Argument | Type | Description |
|---|---|---|
| `path` | string | Save a copy to this path instead of saving in place. |

## `set_envelope_active`

Switch an automation envelope on or off without deleting its points, to check what it does to the level.

| Argument | Type | Description |
|---|---|---|
| `active` (required) | boolean | true switches the envelope on, false off (the points are kept). |
| `envelope` (required) | string | Track envelope name (`Volume`, `Pan`, `Mute`, `Trim Volume`, `Volume (Pre-FX)` ...), or the parameter name when `fx` is given. |
| `fx` | integer | Plugin index when the envelope belongs to a plugin parameter. |
| `track` (required) | integer | Zero-based track index; -1 is the master track. |

## `set_envelope_points`

Write automation points to a track envelope (Volume with value_db, Pan, or an FX parameter with fx). Replaces the existing points unless clear is false. Use set_envelope_active to switch the envelope on or off.

| Argument | Type | Description |
|---|---|---|
| `clear` | boolean | Delete the existing points first (default true). |
| `envelope` (required) | value | `Volume` or `Pan` (created and shown if it does not exist), or, with `fx`, the parameter name or index. |
| `fx` | integer | FX index when the envelope belongs to an FX parameter. |
| `points` (required) | array |  |
| `track` (required) | integer |  |

## `set_fx_enabled`

Enable or bypass an FX.

| Argument | Type | Description |
|---|---|---|
| `enabled` (required) | boolean |  |
| `fx` (required) | integer |  |
| `track` (required) | integer |  |

## `set_fx_envelope`

Replace the automation envelope of an FX parameter with the given points (normalized values).

| Argument | Type | Description |
|---|---|---|
| `fx` (required) | integer |  |
| `param` (required) | ParamRef | Parameter index or name. |
| `points` (required) | array | Replaces all existing points of the parameter's automation envelope. |
| `track` (required) | integer |  |

## `set_fx_param`

Set one FX parameter by index or name, with value (0..1), display_value (plugin units, e.g. -18 for -18 dB) or display_text (e.g. Clean). The result shows the formatted value.

| Argument | Type | Description |
|---|---|---|
| `display_text` | string | Text the plugin shows, for choice and switch parameters (for example "Clean" or "On"). |
| `display_value` | number | Number as the plugin shows it, in the plugin's own unit (for example -18 for "-18.0 dB", 1200 for "1.2 kHz"). Solved by bisection; the result shows the achieved formatted value. |
| `fx` (required) | integer |  |
| `param` (required) | ParamRef | Parameter index or name. |
| `track` (required) | integer |  |
| `value` | number | Normalized value from 0 to 1. |

## `set_fx_params`

Set many parameters of one FX at once. Each entry uses value (0..1), display_value (plugin units) or display_text. Per-entry failures are reported.

| Argument | Type | Description |
|---|---|---|
| `fx` (required) | integer |  |
| `params` (required) | array | Parameters to set; a failed entry is reported and the rest still apply. |
| `track` (required) | integer |  |

## `set_fx_pins`

Set FX pin mappings (track-channel bitmask per input or output pin), for example to run a mono plugin on one channel of a stereo track.

| Argument | Type | Description |
|---|---|---|
| `fx` (required) | integer |  |
| `pins` (required) | array |  |
| `track` (required) | integer |  |

## `set_fx_preset`

Load a named FX preset (as REAPER lists it for the plugin).

| Argument | Type | Description |
|---|---|---|
| `fx` (required) | integer |  |
| `preset` (required) | string | Preset name as REAPER lists it for the plugin. |
| `track` (required) | integer |  |

## `set_item`

Change one media item: gain (dB), mute, active take pan (-1..1) or take channel mode (normal, reverse_stereo, mono_mix, left, right). Only the given fields change.

| Argument | Type | Description |
|---|---|---|
| `channel_mode` | string | Active take channel mode: normal, reverse_stereo, mono_mix, left or right. |
| `item` (required) | integer | Zero-based item index on the track, from `list_items`. |
| `length` | number | Item length in seconds. |
| `mute` | boolean |  |
| `position` | number | Item position on the timeline in seconds (use `move_items` or `set_item_position` for bars and beats). |
| `set_length` | boolean | With `source`: set the item length to the new file's length (default true). |
| `source` | string | Path of an audio file to play instead of the current source. |
| `start_offset` | number | Start offset of the take in its file (a slip edit), seconds. The audio appears on the timeline at position - start_offset; 0 plays the file from its start at the item position. |
| `take_pan` | number | Active take pan from -1 (left) to 1 (right); acts before the track FX. |
| `track` (required) | integer | Zero-based track index from `list_tracks`. |
| `volume_db` | number | Item gain in dB (0 = unity). |

## `set_item_position`

Put one item at an absolute position given as bar and beat (or seconds), optionally snapped to a grid.

| Argument | Type | Description |
|---|---|---|
| `bar` | integer | Bar number, starting at 1. |
| `beat` | number | Beat within the bar, starting at 1 (fractions allowed). Default 1. |
| `item` (required) | integer | Zero-based item index from `list_items`. |
| `seconds` | number | Alternative to bar: a position in seconds. |
| `snap_to` | string | Round the position to this grid (`bar`, `beat`, `1/16` ...). |
| `track` (required) | integer |  |

## `set_items_time_base`

Set the time base of the items of the chosen tracks: time (stay at their seconds when the tempo map changes, the safe setting for recorded audio), beats, beats_position or default. Use before set_tempo_map on a project whose items must not move.

| Argument | Type | Description |
|---|---|---|
| `time_base` | string | `time` (default: items stay at their position in seconds when the tempo map changes), `beats`, `beats_position` (only the position follows the beats) or `default` (the project's setting). |
| `tracks` | array | Zero-based track indexes (default: every track). |

## `set_media_offline`

Set every item that plays a media file offline (REAPER closes the file, so it can be overwritten) or online again. Use before rewriting a WAV the project plays.

| Argument | Type | Description |
|---|---|---|
| `offline` (required) | boolean | true closes the file (so it can be overwritten), false opens it again. |
| `path` (required) | string | Path of the media file the items play. |

## `set_project_sample_rate`

Set the project sample rate in Hz (Project Settings, 'Project sample rate'), for example 48000 or 96000. Marks the project as using its own rate.

| Argument | Type | Description |
|---|---|---|
| `sample_rate` (required) | integer | Project sample rate in Hz, a whole number such as 44100, 48000 or 96000. |

## `set_send`

Change level, pan, mute, source/destination channel pair, mono or phase of an existing send. Channel pairs are given by their first channel: 0 = 1/2, 2 = 3/4, 4 = 5/6 ... (a multi-out plugin such as Kontakt sends its outputs on channel pairs).

| Argument | Type | Description |
|---|---|---|
| `dest_channel` | integer | First destination channel of the pair (0 = channels 1/2). |
| `mode` | integer | Where the send taps the track: 0 = post-fader, 1 = pre-FX, 3 = post-FX before the fader. |
| `mono` | boolean |  |
| `mute` | boolean |  |
| `pan` | number |  |
| `phase_inverted` | boolean |  |
| `send_index` (required) | integer | Send index from `list_sends`. |
| `source` (required) | integer | Source track index. |
| `source_channel` | integer | First source channel of the pair: 0 = channels 1/2, 2 = 3/4, 4 = 5/6 ...; -1 = no audio. |
| `volume_db` | number |  |

## `set_tempo_map`

Write the project tempo map from markers given in quarter notes (the unit of a MIDI file or a Cubase tempo track): the first marker at 0, each marker's time follows from the tempi before it. Replaces the existing tempo markers unless clear is false. Use it before importing MIDI so notes land on the same bars as the audio.

| Argument | Type | Description |
|---|---|---|
| `clear` | boolean | Remove the existing tempo markers first (default true). |
| `markers` (required) | array | Tempo markers in increasing quarter-note order, the first at 0. |

## `set_track`

Change track name, fader (dB), pan, mute, solo, phase, master send, channel count or the whole FX-chain enabled state. Only the given fields change. Track -1 is master.

| Argument | Type | Description |
|---|---|---|
| `channels` | integer | Number of track channels (even, 2-128), for example 34 so a multi-out instrument such as Kontakt can feed separate kit tracks through sends from channel pairs. |
| `fx_enabled` | boolean | Whether the track's entire FX chain is enabled. |
| `master_send` | boolean | Whether the track feeds its parent / master. |
| `mute` | boolean |  |
| `name` | string |  |
| `pan` | number | Pan from -1 (left) to 1 (right). |
| `phase_inverted` | boolean |  |
| `solo` | boolean |  |
| `track` (required) | integer | Zero-based track index from `list_tracks`; -1 is the master track. |
| `volume_db` | number | Track fader in dB (0 = unity). |

## `set_track_folder_depth`

Set a track's folder depth: 1 starts a folder (the following tracks are its children), 0 is a normal track, -1 or lower closes that many folder levels after this track. Routing is not changed: a child with its parent send on feeds the folder track.

| Argument | Type | Description |
|---|---|---|
| `depth` (required) | integer | 1 starts a folder, 0 is a normal track, -1 (or lower, down to -16) closes that many folder levels. |
| `index` (required) | integer | Zero-based track index. |

## `switch_project`

Make another open project tab the active one.

| Argument | Type | Description |
|---|---|---|
| `index` (required) | integer | Zero-based project tab index from `list_projects`. |
