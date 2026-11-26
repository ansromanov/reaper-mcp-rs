//! The MCP server: one tool per REAPER operation.

use std::time::Duration;

use anyhow::{anyhow, Result};
use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};
use serde_json::json;

mod handlers;
mod support;

pub(crate) use support::{round_json, run_blocking, tool_result};

use crate::{
    args::*,
    bridge::{bridge_tool, bridge_tool_timeout, BridgeClient, LONG_TIMEOUT},
    reaper_host,
};

/// The MCP server. Tools that change a project go through the Lua bridge.
#[derive(Clone)]
pub struct ReaperMcp {
    bridge: BridgeClient,
}

impl ReaperMcp {
    pub fn new(bridge: BridgeClient) -> Self {
        Self { bridge }
    }
}

#[tool_router(server_handler)]
impl ReaperMcp {
    #[tool(
        description = "Inspect REAPER process windows and visible owned dialogs without using the Lua bridge. Reports window titles, class names, static child text, and whether the main window responds."
    )]
    async fn inspect_reaper_windows(&self) -> CallToolResult {
        run_blocking(reaper_host::inspect_reaper_windows).await
    }

    #[tool(
        description = "Post WM_CLOSE only to visible, owned REAPER #32770 dialogs whose title starts exactly with title_prefix (for example 'VST3: MyPlugin (Vendor) - Track '). Reports messages posted, not verified closure; never targets REAPER's main window or other dialogs. Windows only."
    )]
    async fn close_reaper_fx_windows(
        &self,
        Parameters(CloseFxWindowsArgs { title_prefix }): Parameters<CloseFxWindowsArgs>,
    ) -> CallToolResult {
        run_blocking(move || reaper_host::close_reaper_fx_windows(&title_prefix)).await
    }

    #[tool(
        description = "Launch a fresh REAPER bridge instance using the installed fixed bridge loader. This only reports that launch was requested; it does not verify bridge recovery or terminate any process."
    )]
    async fn restart_bridge(&self) -> CallToolResult {
        run_blocking(reaper_host::restart_bridge).await
    }

    #[tool(
        description = "Re-read the bridge's Lua handlers inside REAPER after the bridge files were updated, without restarting REAPER."
    )]
    async fn reload_bridge(&self) -> CallToolResult {
        bridge_tool(&self.bridge, "reload", json!({})).await
    }

    #[tool(description = "Check whether the REAPER Lua bridge is running.")]
    async fn health(&self) -> CallToolResult {
        tool_result(Ok(self.bridge.health()))
    }

    #[tool(
        description = "Read, close, or open REAPER's audio and MIDI devices. Reports observed running state before and after the action; open/close do not imply the device successfully changed state."
    )]
    async fn audio_device(&self, Parameters(args): Parameters<AudioDeviceArgs>) -> CallToolResult {
        bridge_tool_timeout(&self.bridge, "audio_device", to_args(&args), LONG_TIMEOUT).await
    }

    #[tool(
        description = "Read the current REAPER project name, tempo, time signature, and track count."
    )]
    async fn get_project_info(&self) -> CallToolResult {
        bridge_tool(&self.bridge, "get_project_info", json!({})).await
    }

    #[tool(
        description = "Set the project sample rate in Hz (Project Settings, 'Project sample rate'), for example 48000 or 96000. Marks the project as using its own rate."
    )]
    async fn set_project_sample_rate(
        &self,
        Parameters(args): Parameters<SetProjectSampleRateArgs>,
    ) -> CallToolResult {
        bridge_tool(&self.bridge, "set_project_sample_rate", to_args(&args)).await
    }

    #[tool(
        description = "List current REAPER tracks with zero-based indexes, names, GUIDs, and volume."
    )]
    async fn list_tracks(&self) -> CallToolResult {
        bridge_tool(&self.bridge, "list_tracks", json!({})).await
    }

    #[tool(description = "Create a named track at the end of the current REAPER project.")]
    async fn create_track(
        &self,
        Parameters(TrackNameArgs { name }): Parameters<TrackNameArgs>,
    ) -> CallToolResult {
        if name.trim().is_empty() || name.len() > 255 {
            return tool_result(Err(anyhow!("name must contain 1 to 255 bytes")));
        }
        bridge_tool(&self.bridge, "create_track", json!({ "name": name })).await
    }

    #[tool(description = "Rename a REAPER track by its zero-based index from list_tracks.")]
    async fn rename_track(
        &self,
        Parameters(RenameTrackArgs { index, name }): Parameters<RenameTrackArgs>,
    ) -> CallToolResult {
        if name.trim().is_empty() || name.len() > 255 {
            return tool_result(Err(anyhow!("name must contain 1 to 255 bytes")));
        }
        bridge_tool(
            &self.bridge,
            "rename_track",
            json!({ "index": index, "name": name }),
        )
        .await
    }

    #[tool(
        description = "Move one track before another position (0-based `before_index`; the track count moves it to the end). Folder state is left alone: reorder first, then call set_track_folder_depth."
    )]
    async fn move_track(&self, Parameters(args): Parameters<MoveTrackArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "move_track", to_args(&args)).await
    }

    #[tool(
        description = "Set a track's folder depth: 1 starts a folder (the following tracks are its children), 0 is a normal track, -1 or lower closes that many folder levels after this track. Routing is not changed: a child with its parent send on feeds the folder track."
    )]
    async fn set_track_folder_depth(
        &self,
        Parameters(args): Parameters<SetTrackFolderDepthArgs>,
    ) -> CallToolResult {
        bridge_tool(&self.bridge, "set_track_folder_depth", to_args(&args)).await
    }

    #[tool(
        description = "Open an .rpp project in REAPER. Refuses when the current project has unsaved changes unless discard_unsaved is true. Loading can take minutes for heavy projects."
    )]
    async fn open_project(&self, Parameters(args): Parameters<OpenProjectArgs>) -> CallToolResult {
        bridge_tool_timeout(&self.bridge, "open_project", to_args(&args), LONG_TIMEOUT).await
    }

    #[tool(description = "Save the current project in place, or save a copy to `path`.")]
    async fn save_project(&self, Parameters(args): Parameters<SaveProjectArgs>) -> CallToolResult {
        bridge_tool_timeout(&self.bridge, "save_project", to_args(&args), LONG_TIMEOUT).await
    }

    #[tool(
        description = "Read one track in detail: fader, pan, mute/solo, whole-chain FX enabled state, folder depth, parent, channels, FX/send/receive/item counts. Track -1 is master."
    )]
    async fn get_track(&self, Parameters(args): Parameters<TrackRef>) -> CallToolResult {
        bridge_tool(&self.bridge, "get_track", to_args(&args)).await
    }

    #[tool(
        description = "Export item-free REAPER track templates for named mix groups (default: DRUMS, BASS, GUITARS, VOCALS, MIX BUS, RETURNS). Preserves track FX, envelopes, folder descendants and internal receives; receives from tracks outside a group are omitted and reported. Also exports the master chain to master.RfxChain by default; set include_master_chain=false to skip it."
    )]
    async fn export_mix_template(
        &self,
        Parameters(args): Parameters<ExportMixTemplateArgs>,
    ) -> CallToolResult {
        bridge_tool_timeout(
            &self.bridge,
            "export_mix_template",
            to_args(&args),
            LONG_TIMEOUT,
        )
        .await
    }

    #[tool(
        description = "Apply static FX state from a named source track in an exported .RTrackTemplate. mode defaults to replace (existing behavior); append adds source FX after the destination chain and preserves all existing FX automation. Both modes preserve destination media, routing, pan and fader. Source timeline automation is never copied. Target -1 is the master track."
    )]
    async fn apply_track_fx_template(
        &self,
        Parameters(args): Parameters<ApplyTrackFxTemplateArgs>,
    ) -> CallToolResult {
        self.apply_fx_template(args).await
    }

    #[tool(
        description = "Change track name, fader (dB), pan, mute, solo, phase, master send, channel count or the whole FX-chain enabled state. Only the given fields change. Track -1 is master."
    )]
    async fn set_track(&self, Parameters(args): Parameters<SetTrackArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "set_track", to_args(&args)).await
    }

    #[tool(
        description = "List media items on a track with position, length, gain, mute, take pan, take channel mode, source channel count and source file. Take pan acts before the track FX."
    )]
    async fn list_items(&self, Parameters(args): Parameters<TrackRef>) -> CallToolResult {
        bridge_tool(&self.bridge, "list_items", to_args(&args)).await
    }

    #[tool(
        description = "Route a multi-out instrument (Kontakt with Krimh, ML Drums ...) to separate tracks: gives the instrument track enough channels and creates one track per output pair right after it (optionally in a new folder), each receiving its pair through a send. Use probe_instrument_outputs first to see which pairs carry which drum."
    )]
    async fn create_multiout_tracks(
        &self,
        Parameters(args): Parameters<CreateMultioutTracksArgs>,
    ) -> CallToolResult {
        bridge_tool(&self.bridge, "create_multiout_tracks", to_args(&args)).await
    }

    #[tool(
        description = "Find out what a multi-out instrument plays on each output pair: renders every pair (outputs 1/2, 3/4 ...) of the track to its own file through temporary tracks, measures them and reports loudness, peak and the loudest frequency band per pair (silent pairs are marked). Temporary tracks are removed and the track's channels and mute are restored. Render a short busy range (start_seconds/end_seconds)."
    )]
    async fn probe_instrument_outputs(
        &self,
        Parameters(args): Parameters<ProbeInstrumentOutputsArgs>,
    ) -> CallToolResult {
        self.probe_outputs(args).await
    }

    #[tool(
        description = "Set the time base of the items of the chosen tracks: time (stay at their seconds when the tempo map changes, the safe setting for recorded audio), beats, beats_position or default. Use before set_tempo_map on a project whose items must not move."
    )]
    async fn set_items_time_base(
        &self,
        Parameters(args): Parameters<SetItemsTimeBaseArgs>,
    ) -> CallToolResult {
        bridge_tool(&self.bridge, "set_items_time_base", to_args(&args)).await
    }

    #[tool(
        description = "Normalize the audio items of the chosen tracks with REAPER's own normalization (the one behind Item processing > Normalize): the item gain is set so that the part of the source the item plays reaches target_db in peak, true_peak, lufs_i, lufs_m, lufs_s or rms. Replaces the existing item gain; take volume is kept. Use dry_run to see the gains first, exclude_tracks to skip drums or a reference."
    )]
    async fn normalize_items(
        &self,
        Parameters(args): Parameters<NormalizeItemsArgs>,
    ) -> CallToolResult {
        bridge_tool_timeout(
            &self.bridge,
            "normalize_items",
            to_args(&args),
            LONG_TIMEOUT,
        )
        .await
    }

    #[tool(
        description = "Change one media item: gain (dB), mute, active take pan (-1..1) or take channel mode (normal, reverse_stereo, mono_mix, left, right). Only the given fields change."
    )]
    async fn set_item(&self, Parameters(args): Parameters<SetItemArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "set_item", to_args(&args)).await
    }

    #[tool(
        description = "List the FX chain of a track with indexes, names, bypass state and parameter counts."
    )]
    async fn list_fx(&self, Parameters(args): Parameters<TrackRef>) -> CallToolResult {
        bridge_tool(&self.bridge, "list_fx", to_args(&args)).await
    }

    #[tool(
        description = "Append an FX to a track by name. Returns the new FX index and parameter count."
    )]
    async fn add_fx(&self, Parameters(args): Parameters<AddFxArgs>) -> CallToolResult {
        bridge_tool_timeout(&self.bridge, "add_fx", to_args(&args), LONG_TIMEOUT).await
    }

    #[tool(description = "Delete an FX from a track by index.")]
    async fn remove_fx(&self, Parameters(args): Parameters<FxRef>) -> CallToolResult {
        bridge_tool(&self.bridge, "remove_fx", to_args(&args)).await
    }

    #[tool(description = "Enable or bypass an FX.")]
    async fn set_fx_enabled(
        &self,
        Parameters(args): Parameters<SetFxEnabledArgs>,
    ) -> CallToolResult {
        bridge_tool(&self.bridge, "set_fx_enabled", to_args(&args)).await
    }

    #[tool(
        description = "List all parameters of an FX with names, normalized values and formatted values (real units)."
    )]
    async fn list_fx_params(&self, Parameters(args): Parameters<FxRef>) -> CallToolResult {
        bridge_tool(&self.bridge, "list_fx_params", to_args(&args)).await
    }

    #[tool(
        description = "Set one FX parameter by index or name, with value (0..1), display_value (plugin units, e.g. -18 for -18 dB) or display_text (e.g. Clean). The result shows the formatted value."
    )]
    async fn set_fx_param(&self, Parameters(args): Parameters<SetFxParamArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "set_fx_param", to_args(&args)).await
    }

    #[tool(
        description = "Set many parameters of one FX at once. Each entry uses value (0..1), display_value (plugin units) or display_text. Per-entry failures are reported."
    )]
    async fn set_fx_params(&self, Parameters(args): Parameters<SetFxParamsArgs>) -> CallToolResult {
        bridge_tool_timeout(&self.bridge, "set_fx_params", to_args(&args), LONG_TIMEOUT).await
    }

    #[tool(description = "Load a named FX preset (as REAPER lists it for the plugin).")]
    async fn set_fx_preset(&self, Parameters(args): Parameters<SetFxPresetArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "set_fx_preset", to_args(&args)).await
    }

    #[tool(description = "Read the current preset name and preset count of an FX.")]
    async fn list_fx_presets(&self, Parameters(args): Parameters<FxRef>) -> CallToolResult {
        bridge_tool(&self.bridge, "list_fx_presets", to_args(&args)).await
    }

    #[tool(
        description = "Move an FX to another position in the same chain; to_index is its final index."
    )]
    async fn move_fx(&self, Parameters(args): Parameters<MoveFxArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "move_fx", to_args(&args)).await
    }

    #[tool(
        description = "List the distinct texts a choice parameter can show (shapes, modes, styles) with the normalized value for each."
    )]
    async fn list_fx_param_choices(
        &self,
        Parameters(args): Parameters<ParamChoicesArgs>,
    ) -> CallToolResult {
        bridge_tool_timeout(
            &self.bridge,
            "list_fx_param_choices",
            to_args(&args),
            LONG_TIMEOUT,
        )
        .await
    }

    #[tool(
        description = "Show the text a plugin displays for a normalized parameter value, without changing the parameter."
    )]
    async fn format_fx_param(
        &self,
        Parameters(args): Parameters<FormatFxParamArgs>,
    ) -> CallToolResult {
        bridge_tool(&self.bridge, "format_fx_param", to_args(&args)).await
    }

    #[tool(
        description = "Replace the automation envelope of an FX parameter with the given points (normalized values)."
    )]
    async fn set_fx_envelope(
        &self,
        Parameters(args): Parameters<SetFxEnvelopeArgs>,
    ) -> CallToolResult {
        bridge_tool_timeout(
            &self.bridge,
            "set_fx_envelope",
            to_args(&args),
            LONG_TIMEOUT,
        )
        .await
    }

    #[tool(
        description = "Read an FX's pin mappings: which track channels (bitmask) feed each input pin and receive each output pin."
    )]
    async fn get_fx_pins(&self, Parameters(args): Parameters<FxRef>) -> CallToolResult {
        bridge_tool(&self.bridge, "get_fx_pins", to_args(&args)).await
    }

    #[tool(
        description = "Set FX pin mappings (track-channel bitmask per input or output pin), for example to run a mono plugin on one channel of a stereo track."
    )]
    async fn set_fx_pins(&self, Parameters(args): Parameters<SetFxPinsArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "set_fx_pins", to_args(&args)).await
    }

    #[tool(description = "List sends and receives of a track.")]
    async fn list_sends(&self, Parameters(args): Parameters<TrackRef>) -> CallToolResult {
        bridge_tool(&self.bridge, "list_sends", to_args(&args)).await
    }

    #[tool(description = "Create a send between two tracks, optionally with level (dB) and pan.")]
    async fn create_send(&self, Parameters(args): Parameters<CreateSendArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "create_send", to_args(&args)).await
    }

    #[tool(
        description = "Change level, pan, mute, source/destination channel pair, mono or phase of an existing send. Channel pairs are given by their first channel: 0 = 1/2, 2 = 3/4, 4 = 5/6 ... (a multi-out plugin such as Kontakt sends its outputs on channel pairs)."
    )]
    async fn set_send(&self, Parameters(args): Parameters<SetSendArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "set_send", to_args(&args)).await
    }

    #[tool(
        description = "Remove one send from a track. The later sends of that track move down by one index; the result lists the sends that remain."
    )]
    async fn remove_send(&self, Parameters(args): Parameters<RemoveSendArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "remove_send", to_args(&args)).await
    }

    #[tool(
        description = "Read the project's current render settings (output folder, pattern, sample rate, bounds)."
    )]
    async fn get_render_settings(&self) -> CallToolResult {
        bridge_tool(&self.bridge, "get_render_settings", json!({})).await
    }

    #[tool(
        description = "Render the whole project master mix to directory/name.<ext> using the project's saved render format. Deletes an old file with that name first (overwrite=false keeps it), lists the written files and warns when one is missing. Blocks until the render finishes."
    )]
    async fn render_project(
        &self,
        Parameters(args): Parameters<RenderProjectArgs>,
    ) -> CallToolResult {
        let timeout = Duration::from_secs(args.timeout_seconds.unwrap_or(1800));
        bridge_tool_timeout(&self.bridge, "render_project", to_args(&args), timeout).await
    }

    #[tool(
        description = "Render the given tracks as separate stem files (each track's own output including its children), using the project's saved render format. Deletes old stems with the same names first (overwrite=false keeps them), lists the written files, warns when one is missing and restores render settings and track selection afterwards. Blocks until done."
    )]
    async fn render_stems(&self, Parameters(args): Parameters<RenderStemsArgs>) -> CallToolResult {
        let timeout = Duration::from_secs(args.timeout_seconds.unwrap_or(3600));
        bridge_tool_timeout(&self.bridge, "render_stems", to_args(&args), timeout).await
    }

    #[tool(
        description = "List open project tabs with path, dirty state and track count. All other tools act on the active tab."
    )]
    async fn list_projects(&self) -> CallToolResult {
        bridge_tool(&self.bridge, "list_projects", json!({})).await
    }

    #[tool(description = "Make another open project tab the active one.")]
    async fn switch_project(
        &self,
        Parameters(args): Parameters<SwitchProjectArgs>,
    ) -> CallToolResult {
        // opening or closing a heavy project (samplers, many plugins) takes far longer than 5 s
        bridge_tool_timeout(&self.bridge, "switch_project", to_args(&args), LONG_TIMEOUT).await
    }

    #[tool(
        description = "Open a new empty project tab and make it active (the other tabs stay open)."
    )]
    async fn new_project_tab(&self) -> CallToolResult {
        // opening or closing a heavy project (samplers, many plugins) takes far longer than 5 s
        bridge_tool_timeout(&self.bridge, "new_project_tab", json!({}), LONG_TIMEOUT).await
    }

    #[tool(
        description = "Close the project tab with the given index (see list_projects). The index is required; the previously active tab stays active. Refuses if that tab has unsaved changes or if it is the last tab."
    )]
    async fn close_project_tab(
        &self,
        Parameters(args): Parameters<CloseProjectTabArgs>,
    ) -> CallToolResult {
        // opening or closing a heavy project (samplers, many plugins) takes far longer than 5 s
        bridge_tool_timeout(
            &self.bridge,
            "close_project_tab",
            to_args(&args),
            LONG_TIMEOUT,
        )
        .await
    }

    #[tool(
        description = "Delete a track. With with_children also everything inside a folder track. The master track cannot be deleted."
    )]
    async fn delete_track(&self, Parameters(args): Parameters<DeleteTrackArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "delete_track", to_args(&args)).await
    }

    #[tool(
        description = "Move the items of tracks (or one item) in musical time: by bars, beats, the project grid or note values, optionally snapped to a grid, with the automation points. Negative values move earlier. dry_run only reports the from/to positions as seconds and bar.beat. Prefer this to moving by seconds: it stays on the grid."
    )]
    async fn move_items(&self, Parameters(args): Parameters<MoveItemsArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "move_items", to_args(&args)).await
    }

    #[tool(
        description = "Put one item at an absolute position given as bar and beat (or seconds), optionally snapped to a grid."
    )]
    async fn set_item_position(
        &self,
        Parameters(args): Parameters<SetItemPositionArgs>,
    ) -> CallToolResult {
        bridge_tool(&self.bridge, "set_item_position", to_args(&args)).await
    }

    #[tool(
        description = "Project tempo, time signature, grid size and the length of a bar in seconds: what one grid step or one bar means before moving anything."
    )]
    async fn get_grid(&self) -> CallToolResult {
        bridge_tool(&self.bridge, "get_grid", json!({})).await
    }

    #[tool(
        description = "Write the FX of a track (all, or the ones named in fx) to an .RfxChain file with the plugin states inside: the way to keep a plugin sound (an amp setting, a vocal chain) as a file. REAPER has no function that writes plugin preset files."
    )]
    async fn export_fx_chain(
        &self,
        Parameters(args): Parameters<ExportFxChainArgs>,
    ) -> CallToolResult {
        bridge_tool(&self.bridge, "export_fx_chain", to_args(&args)).await
    }

    #[tool(
        description = "Put the FX of an .RfxChain file on a track, after its existing FX (append) or instead of them (replace). Plugin states come with the file."
    )]
    async fn import_fx_chain(
        &self,
        Parameters(args): Parameters<ImportFxChainArgs>,
    ) -> CallToolResult {
        bridge_tool(&self.bridge, "import_fx_chain", to_args(&args)).await
    }

    #[tool(description = "Delete one media item from a track (indexes from list_items).")]
    async fn delete_item(&self, Parameters(args): Parameters<DeleteItemArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "delete_item", to_args(&args)).await
    }

    #[tool(
        description = "Set every item that plays a media file offline (REAPER closes the file, so it can be overwritten) or online again. Use before rewriting a WAV the project plays."
    )]
    async fn set_media_offline(
        &self,
        Parameters(args): Parameters<SetMediaOfflineArgs>,
    ) -> CallToolResult {
        bridge_tool(&self.bridge, "set_media_offline", to_args(&args)).await
    }

    #[tool(
        description = "List the notes of a MIDI item: start/end in project seconds, pitch, velocity, channel, plus the note count per pitch."
    )]
    async fn list_midi_notes(
        &self,
        Parameters(args): Parameters<ListMidiNotesArgs>,
    ) -> CallToolResult {
        bridge_tool(&self.bridge, "list_midi_notes", to_args(&args)).await
    }

    #[tool(description = "Add notes to an existing MIDI item (times in project seconds).")]
    async fn add_midi_notes(
        &self,
        Parameters(args): Parameters<AddMidiNotesArgs>,
    ) -> CallToolResult {
        bridge_tool(&self.bridge, "add_midi_notes", to_args(&args)).await
    }

    #[tool(
        description = "Write the project tempo map from markers given in quarter notes (the unit of a MIDI file or a Cubase tempo track): the first marker at 0, each marker's time follows from the tempi before it. Replaces the existing tempo markers unless clear is false. Use it before importing MIDI so notes land on the same bars as the audio."
    )]
    async fn set_tempo_map(&self, Parameters(args): Parameters<SetTempoMapArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "set_tempo_map", to_args(&args)).await
    }

    #[tool(
        description = "Add markers (a position) and regions (a position and an end) by name at project times in seconds, for navigating a long project. Existing markers are kept unless clear is true."
    )]
    async fn add_markers(&self, Parameters(args): Parameters<AddMarkersArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "add_markers", to_args(&args)).await
    }

    #[tool(
        description = "List project markers and regions with list index, marker number, name, position and region end (seconds) and color."
    )]
    async fn list_markers(&self) -> CallToolResult {
        bridge_tool(&self.bridge, "list_markers", json!({})).await
    }

    #[tool(description = "Create an empty MIDI item on a track between two project times.")]
    async fn create_midi_item(
        &self,
        Parameters(args): Parameters<CreateMidiItemArgs>,
    ) -> CallToolResult {
        bridge_tool(&self.bridge, "create_midi_item", to_args(&args)).await
    }

    #[tool(
        description = "Write automation points to a track envelope (Volume with value_db, Pan, or an FX parameter with fx). Replaces the existing points unless clear is false. Use set_envelope_active to switch the envelope on or off."
    )]
    async fn set_envelope_points(
        &self,
        Parameters(args): Parameters<SetEnvelopePointsArgs>,
    ) -> CallToolResult {
        bridge_tool(&self.bridge, "set_envelope_points", to_args(&args)).await
    }

    #[tool(
        description = "The whole project in one call: tracks with folder structure, fader, pan, mute, FX, sends and item counts, plus the master FX. Compare two snapshots to see what changed."
    )]
    async fn project_snapshot(&self) -> CallToolResult {
        bridge_tool(&self.bridge, "project_snapshot", json!({})).await
    }

    #[tool(
        description = "Plugins used in the project (instances, first tracks) with the VST3 version found on disk. REAPER keeps old DLLs loaded after an update, so restart it when the version on disk is newer than expected."
    )]
    async fn list_plugins(&self, Parameters(args): Parameters<ListPluginsArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "list_plugins", to_args(&args)).await
    }

    #[tool(
        description = "Render the same tracks as stems several times with different tracks muted or unmuted per variant (for example live only, trigger only, both). Saves and reopens the project before each render and restores the original mutes at the end."
    )]
    async fn render_variants(
        &self,
        Parameters(args): Parameters<RenderVariantsArgs>,
    ) -> CallToolResult {
        let timeout = Duration::from_secs(args.timeout_seconds.unwrap_or(3600));
        bridge_tool_timeout(&self.bridge, "render_variants", to_args(&args), timeout).await
    }

    #[tool(
        description = "Run several tools in one call: calls is a list of {tool, args}. Stops at the first error unless continue_on_error is true. Each call has its own undo step."
    )]
    async fn batch(&self, Parameters(args): Parameters<BatchArgs>) -> CallToolResult {
        let timeout = Duration::from_secs(args.timeout_seconds.unwrap_or(3600));
        bridge_tool_timeout(&self.bridge, "batch", to_args(&args), timeout).await
    }

    #[tool(description = "Put an audio file on a track as a new item at a position in seconds.")]
    async fn insert_media(&self, Parameters(args): Parameters<InsertMediaArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "insert_media", to_args(&args)).await
    }

    #[tool(
        description = "Analyse a rendered WAV file: integrated loudness (LUFS), loudness range, sample peak, samples above full scale (overloads), crest factor, stereo width and the spectrum in third-octave and coarse mixing bands. Reads 32-bit float files without cutting off levels above 0 dBFS. Warns about overloads and missing headroom."
    )]
    async fn analyze_audio(
        &self,
        Parameters(args): Parameters<AnalyzeAudioArgs>,
    ) -> CallToolResult {
        self.analyze(args).await
    }

    #[tool(
        description = "Frequency and level comparison of two WAV files, for example your render against a professional reference. File B is turned to the loudness of A, so the per-band differences (third-octave and coarse mixing bands) describe tone, not volume; positive means B has more energy there. Also loudness, crest, width and overload differences. Use a_start_seconds and b_start_seconds to line up files that start at different times."
    )]
    async fn compare_audio(
        &self,
        Parameters(args): Parameters<CompareAudioArgs>,
    ) -> CallToolResult {
        self.compare(args).await
    }

    #[tool(
        description = "Everything that sets the level and sound of one track: fader, pan, whole-chain FX enable state, FX chain with pin mappings, automation envelopes (volume, pan, mute, plugin parameters), sends, receives, parent. Notes call out what acts after the plugins, such as an active volume envelope that shifts a limiter's ceiling."
    )]
    async fn describe_track(&self, Parameters(args): Parameters<TrackRef>) -> CallToolResult {
        bridge_tool_timeout(&self.bridge, "describe_track", to_args(&args), LONG_TIMEOUT).await
    }

    #[tool(
        description = "List automation envelopes of a track: volume, trim, pan, width and mute envelopes and plugin parameter envelopes, with active state, point count and value range (dB for volume)."
    )]
    async fn list_track_envelopes(
        &self,
        Parameters(args): Parameters<ListTrackEnvelopesArgs>,
    ) -> CallToolResult {
        bridge_tool_timeout(
            &self.bridge,
            "list_track_envelopes",
            to_args(&args),
            LONG_TIMEOUT,
        )
        .await
    }

    #[tool(
        description = "Switch an automation envelope on or off without deleting its points, to check what it does to the level."
    )]
    async fn set_envelope_active(
        &self,
        Parameters(args): Parameters<SetEnvelopeActiveArgs>,
    ) -> CallToolResult {
        bridge_tool(&self.bridge, "set_envelope_active", to_args(&args)).await
    }

    #[tool(
        description = "Run a REAPER action by numeric ID or named command. Escape hatch for features without a dedicated tool; prefer adding a real tool."
    )]
    async fn run_action(&self, Parameters(args): Parameters<RunActionArgs>) -> CallToolResult {
        bridge_tool_timeout(&self.bridge, "run_action", to_args(&args), LONG_TIMEOUT).await
    }
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;
