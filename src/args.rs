//! Arguments of the MCP tools, with the JSON schemas clients see.

use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct CloseFxWindowsArgs {
    /// Exact start of the dialog title to close, for example "VST3: MyPlugin (Vendor) - Track ". Must not be empty.
    pub(crate) title_prefix: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct TrackNameArgs {
    /// Name to assign to the new track.
    pub(crate) name: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct MoveTrackArgs {
    /// Zero-based index of the track to move (from `list_tracks`).
    pub(crate) index: u32,
    /// Zero-based index the track should end up before; the track count moves it to the end.
    pub(crate) before_index: u32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SetTrackFolderDepthArgs {
    /// Zero-based track index.
    pub(crate) index: u32,
    /// 1 starts a folder, 0 is a normal track, -1 (or lower, down to -16) closes that many folder levels.
    pub(crate) depth: i32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct RenameTrackArgs {
    /// Zero-based track index from `list_tracks`.
    pub(crate) index: u32,
    /// New track name.
    pub(crate) name: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub(crate) enum ParamRef {
    /// Zero-based parameter index from `list_fx_params`.
    Index(u32),
    /// Parameter name; exact match first, then case-insensitive substring.
    Name(String),
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct OpenProjectArgs {
    /// Absolute path of the .rpp file to open.
    pub(crate) path: String,
    /// Open even if the current project has unsaved changes (they are lost).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) discard_unsaved: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SaveProjectArgs {
    /// Save a copy to this path instead of saving in place.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AudioDeviceAction {
    Status,
    Close,
    Open,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct AudioDeviceArgs {
    /// Read status or close/open REAPER's audio and MIDI devices.
    pub(crate) action: AudioDeviceAction,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct ExportMixTemplateArgs {
    /// Destination directory for the .RTrackTemplate files.
    pub(crate) directory: String,
    /// Optional track groups. Defaults to DRUMS, BASS, GUITARS, VOCALS, MIX BUS, RETURNS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) groups: Option<Vec<String>>,
    /// Whether to also write the active project's master FX chain to master.RfxChain (defaults to true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) include_master_chain: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct ApplyTrackFxTemplateArgs {
    /// Path to an exported .RTrackTemplate file.
    pub(crate) template_path: String,
    /// Exact source track name inside the template.
    pub(crate) source_track: String,
    /// Zero-based destination track index; -1 addresses the master track.
    pub(crate) target_track: i32,
    /// FX chain operation: replace (default) or append source FX after existing FX.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mode: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SetProjectSampleRateArgs {
    /// Project sample rate in Hz, a whole number such as 44100, 48000 or 96000.
    pub(crate) sample_rate: u32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct TrackRef {
    /// Zero-based track index from `list_tracks`; -1 is the master track.
    pub(crate) track: i32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SetTrackArgs {
    /// Zero-based track index from `list_tracks`; -1 is the master track.
    pub(crate) track: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    /// Track fader in dB (0 = unity).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) volume_db: Option<f64>,
    /// Pan from -1 (left) to 1 (right).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pan: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mute: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) solo: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) phase_inverted: Option<bool>,
    /// Whether the track feeds its parent / master.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) master_send: Option<bool>,
    /// Whether the track's entire FX chain is enabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fx_enabled: Option<bool>,
    /// Number of track channels (even, 2-128), for example 34 so a multi-out instrument such as Kontakt
    /// can feed separate kit tracks through sends from channel pairs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) channels: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct MultioutOutput {
    /// Name of the new track, for example `KICK IN`.
    pub(crate) name: String,
    /// First instrument output channel of the pair: 0 = outputs 1/2, 2 = 3/4, 32 = 33/34.
    pub(crate) source_channel: u32,
    /// Take only that one channel (a mono output).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mono: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) volume_db: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pan: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct CreateMultioutTracksArgs {
    /// Track with the multi-out instrument (Kontakt, ML Drums ...).
    pub(crate) track: i32,
    /// One new track per output pair, created right after the instrument in this order.
    pub(crate) outputs: Vec<MultioutOutput>,
    /// Put the new tracks into a new folder track with this name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) folder: Option<String>,
    /// Send mode: 0 post-fader (default), 1 pre-FX, 3 post-FX before the fader.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) send_mode: Option<i32>,
    /// false turns off the instrument's own send to its parent, so only the new tracks are heard.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) instrument_to_parent: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct ProbeInstrumentOutputsArgs {
    /// Track with the multi-out instrument; it must have MIDI to play in the range.
    pub(crate) track: i32,
    /// Folder for the probe files (one WAV per output pair).
    pub(crate) directory: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) start_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) end_seconds: Option<f64>,
    /// How many instrument channels to probe; default: the largest output count of the track's FX.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) channels: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SetItemsTimeBaseArgs {
    /// `time` (default: items stay at their position in seconds when the tempo map changes), `beats`,
    /// `beats_position` (only the position follows the beats) or `default` (the project's setting).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) time_base: Option<String>,
    /// Zero-based track indexes (default: every track).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tracks: Option<Vec<i32>>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct NormalizeItemsArgs {
    /// Normalization target in dB (0 or lower): dBFS for peak and true_peak, LUFS for the lufs modes, dB for rms.
    pub(crate) target_db: f64,
    /// peak (default), true_peak, lufs_i, lufs_m, lufs_s or rms.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mode: Option<String>,
    /// Zero-based track indexes to normalize (default: every track).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tracks: Option<Vec<i32>>,
    /// Zero-based track indexes to leave alone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) exclude_tracks: Option<Vec<i32>>,
    /// Only report what would change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) dry_run: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SetItemArgs {
    /// Zero-based track index from `list_tracks`.
    pub(crate) track: i32,
    /// Zero-based item index on the track, from `list_items`.
    pub(crate) item: i32,
    /// Item gain in dB (0 = unity).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) volume_db: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mute: Option<bool>,
    /// Active take pan from -1 (left) to 1 (right); acts before the track FX.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) take_pan: Option<f64>,
    /// Active take channel mode: normal, reverse_stereo, mono_mix, left or right.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) channel_mode: Option<String>,
    /// Item position on the timeline in seconds (use `move_items` or `set_item_position` for bars and beats).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) position: Option<f64>,
    /// Item length in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) length: Option<f64>,
    /// Start offset of the take in its file (a slip edit), seconds. The audio appears on the timeline at
    /// position - start_offset; 0 plays the file from its start at the item position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) start_offset: Option<f64>,
    /// Path of an audio file to play instead of the current source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) source: Option<String>,
    /// With `source`: set the item length to the new file's length (default true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) set_length: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct FxRef {
    /// Zero-based track index; -1 is the master track.
    pub(crate) track: i32,
    /// Zero-based FX index from `list_fx`.
    pub(crate) fx: u32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct AddFxArgs {
    /// Zero-based track index; -1 is the master track.
    pub(crate) track: i32,
    /// FX name as REAPER shows it, for example `VST: ReaEQ (Cockos)` or just `ReaEQ`.
    pub(crate) name: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SetFxEnabledArgs {
    pub(crate) track: i32,
    pub(crate) fx: u32,
    pub(crate) enabled: bool,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SetFxParamArgs {
    pub(crate) track: i32,
    pub(crate) fx: u32,
    /// Parameter index or name.
    pub(crate) param: ParamRef,
    #[serde(flatten)]
    pub(crate) target: ParamTarget,
}

/// Exactly one of the three fields must be set.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct ParamTarget {
    /// Normalized value from 0 to 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) value: Option<f64>,
    /// Number as the plugin shows it, in the plugin's own unit (for example -18 for "-18.0 dB",
    /// 1200 for "1.2 kHz"). Solved by bisection; the result shows the achieved formatted value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) display_value: Option<f64>,
    /// Text the plugin shows, for choice and switch parameters (for example "Clean" or "On").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) display_text: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct ParamEntry {
    /// Parameter index or name.
    pub(crate) param: ParamRef,
    #[serde(flatten)]
    pub(crate) target: ParamTarget,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SetFxParamsArgs {
    pub(crate) track: i32,
    pub(crate) fx: u32,
    /// Parameters to set; a failed entry is reported and the rest still apply.
    pub(crate) params: Vec<ParamEntry>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SetFxPresetArgs {
    pub(crate) track: i32,
    pub(crate) fx: u32,
    /// Preset name as REAPER lists it for the plugin.
    pub(crate) preset: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct MoveFxArgs {
    pub(crate) track: i32,
    /// Current zero-based FX index.
    pub(crate) fx: u32,
    /// Final zero-based FX index in the chain.
    pub(crate) to_index: u32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct ParamChoicesArgs {
    pub(crate) track: i32,
    pub(crate) fx: u32,
    /// Parameter index or name.
    pub(crate) param: ParamRef,
    /// Sampling resolution across 0..1 (default 400).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) steps: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct FormatFxParamArgs {
    pub(crate) track: i32,
    pub(crate) fx: u32,
    /// Parameter index or name.
    pub(crate) param: ParamRef,
    /// Normalized value 0..1 to format.
    pub(crate) value: f64,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct EnvelopePoint {
    /// Time in seconds.
    pub(crate) time: f64,
    /// Normalized parameter value 0..1.
    pub(crate) value: f64,
    /// REAPER shape: 0 linear, 1 square, 2 slow start/end, 3 fast start, 4 fast end, 5 bezier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) shape: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tension: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SetFxEnvelopeArgs {
    pub(crate) track: i32,
    pub(crate) fx: u32,
    /// Parameter index or name.
    pub(crate) param: ParamRef,
    /// Replaces all existing points of the parameter's automation envelope.
    pub(crate) points: Vec<EnvelopePoint>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct PinMapping {
    /// Zero-based FX pin.
    pub(crate) pin: u32,
    /// Bitmask of track channels routed to this pin (1 = channel 1, 2 = channel 2, 3 = both).
    pub(crate) mask: u32,
    /// true for an output pin, false (default) for an input pin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) output: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SetFxPinsArgs {
    pub(crate) track: i32,
    pub(crate) fx: u32,
    pub(crate) pins: Vec<PinMapping>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct CreateSendArgs {
    /// Source track index.
    pub(crate) source: i32,
    /// Destination track index.
    pub(crate) destination: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) volume_db: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pan: Option<f64>,
    /// First source channel of the pair: 0 = channels 1/2, 2 = 3/4, 4 = 5/6 ...; -1 = no audio.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) source_channel: Option<i32>,
    /// First destination channel of the pair (0 = channels 1/2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) dest_channel: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mono: Option<bool>,
    /// Where the send taps the track: 0 = post-fader, 1 = pre-FX, 3 = post-FX before the fader.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mode: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) phase_inverted: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SetSendArgs {
    /// Source track index.
    pub(crate) source: i32,
    /// Send index from `list_sends`.
    pub(crate) send_index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) volume_db: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pan: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mute: Option<bool>,
    /// First source channel of the pair: 0 = channels 1/2, 2 = 3/4, 4 = 5/6 ...; -1 = no audio.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) source_channel: Option<i32>,
    /// First destination channel of the pair (0 = channels 1/2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) dest_channel: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mono: Option<bool>,
    /// Where the send taps the track: 0 = post-fader, 1 = pre-FX, 3 = post-FX before the fader.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mode: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) phase_inverted: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct RemoveSendArgs {
    /// Source track index.
    pub(crate) source: i32,
    /// Send index from `list_sends`.
    pub(crate) send_index: u32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct RenderProjectArgs {
    /// Output directory.
    pub(crate) directory: String,
    /// File name without extension; the extension comes from the project's render format.
    pub(crate) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sample_rate: Option<u32>,
    /// Render only from this time (seconds); give together with end_seconds. Default: the whole project.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) start_seconds: Option<f64>,
    /// Render only up to this time (seconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) end_seconds: Option<f64>,
    /// Output format: `pcm16`, `pcm24`, `float32` (keeps levels above 0 dBFS) or a raw base64 render config.
    /// Default: the project's saved format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) format: Option<String>,
    /// Delete the files this render is about to write (default true). REAPER itself never overwrites:
    /// it writes `NAME-001.wav` and leaves the old file, which a caller would then read by mistake.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) overwrite: Option<bool>,
    /// Save the project and open it again before rendering, so the render sees the saved state (default false).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fresh: Option<bool>,
    /// Give up waiting after this many seconds (default 1800).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) timeout_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct RenderStemsArgs {
    /// Output directory.
    pub(crate) directory: String,
    /// Zero-based indexes of the tracks to render, each as its own file named after the track.
    pub(crate) tracks: Vec<u32>,
    /// File name pattern; defaults to `$track`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pattern: Option<String>,
    /// Render only from this time (seconds); give together with end_seconds. Default: the whole project.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) start_seconds: Option<f64>,
    /// Render only up to this time (seconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) end_seconds: Option<f64>,
    /// Output format: `pcm16`, `pcm24`, `float32` (keeps levels above 0 dBFS) or a raw base64 render config.
    /// Default: the project's saved format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) format: Option<String>,
    /// Delete the files this render is about to write (default true). REAPER itself never overwrites:
    /// it writes `NAME-001.wav` and leaves the old file, which a caller would then read by mistake.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) overwrite: Option<bool>,
    /// Save the project and open it again before rendering, so the render sees the saved state (default false).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fresh: Option<bool>,
    /// Give up waiting after this many seconds (default 3600).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) timeout_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct CloseProjectTabArgs {
    /// Zero-based project tab index from `list_projects`. Required: only the tab you name is closed.
    pub(crate) index: u32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct DeleteTrackArgs {
    /// Zero-based track index from `list_tracks` (the master track cannot be deleted).
    pub(crate) track: i32,
    /// For a folder track: also delete everything inside it. Default false (the children stay).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) with_children: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct DeleteItemArgs {
    /// Zero-based track index.
    pub(crate) track: i32,
    /// Zero-based item index from `list_items`.
    pub(crate) item: i32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SetMediaOfflineArgs {
    /// Path of the media file the items play.
    pub(crate) path: String,
    /// true closes the file (so it can be overwritten), false opens it again.
    pub(crate) offline: bool,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct ListMidiNotesArgs {
    /// Zero-based track index.
    pub(crate) track: i32,
    /// Zero-based index of the MIDI item on the track (`list_items`).
    pub(crate) item: i32,
    /// Return at most this many notes (default 5000).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) limit: Option<u32>,
    /// Skip this many notes first.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) offset: Option<u32>,
    /// Only notes of this MIDI pitch (the per-pitch counts always cover all notes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pitch: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct TempoMarker {
    /// Position in quarter notes from the start of the project (the first marker must be 0).
    pub(crate) quarter_notes: f64,
    /// Tempo in BPM from this position on, 10-960.
    pub(crate) bpm: f64,
    /// Time signature numerator from this position on (omit to keep the current one).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) numerator: Option<i32>,
    /// Time signature denominator from this position on (omit to keep the current one).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) denominator: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SetTempoMapArgs {
    /// Tempo markers in increasing quarter-note order, the first at 0.
    pub(crate) markers: Vec<TempoMarker>,
    /// Remove the existing tempo markers first (default true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) clear: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct NewMarker {
    /// Name shown in the ruler and the marker list.
    pub(crate) name: String,
    /// Position in project seconds.
    pub(crate) position_seconds: f64,
    /// End in project seconds. Given, the entry is a region from the position to the end; omitted, a marker.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) end_seconds: Option<f64>,
    /// Color as a hex string such as "#ff8800" (default: REAPER's color).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) color: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct AddMarkersArgs {
    /// Markers and regions to add.
    pub(crate) markers: Vec<NewMarker>,
    /// Delete all existing markers and regions first (default false).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) clear: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct MidiNote {
    /// Start in project seconds.
    pub(crate) start_seconds: f64,
    /// End in project seconds.
    pub(crate) end_seconds: f64,
    pub(crate) pitch: i32,
    /// 1-127, default 100.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) velocity: Option<i32>,
    /// 0-15, default 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) channel: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct AddMidiNotesArgs {
    pub(crate) track: i32,
    /// Zero-based index of the MIDI item on the track.
    pub(crate) item: i32,
    pub(crate) notes: Vec<MidiNote>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct CreateMidiItemArgs {
    pub(crate) track: i32,
    pub(crate) start_seconds: f64,
    pub(crate) end_seconds: f64,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct AutomationPoint {
    /// Project time in seconds.
    pub(crate) time: f64,
    /// Value in the envelope's own range (Pan -1..1, FX parameters 0..1 and so on).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) value: Option<f64>,
    /// For the Volume envelope: the gain in dB (0 = unity).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) value_db: Option<f64>,
    /// 0 linear, 1 square, 2 slow start/end, 3 fast start, 4 fast end, 5 bezier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) shape: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SetEnvelopePointsArgs {
    pub(crate) track: i32,
    /// `Volume` or `Pan` (created and shown if it does not exist), or, with `fx`, the parameter name or index.
    pub(crate) envelope: serde_json::Value,
    /// FX index when the envelope belongs to an FX parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fx: Option<i32>,
    pub(crate) points: Vec<AutomationPoint>,
    /// Delete the existing points first (default true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) clear: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct ListPluginsArgs {
    /// Only plugins whose name contains this text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) filter: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct RenderVariant {
    /// Name of the variant, used in the file names (`{variant}` in the pattern).
    pub(crate) name: String,
    /// Tracks to mute for this variant: names or zero-based indexes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mute: Option<Vec<serde_json::Value>>,
    /// Tracks to unmute for this variant: names or indexes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) unmute: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct RenderVariantsArgs {
    /// Output directory.
    pub(crate) directory: String,
    /// Zero-based indexes of the tracks to render as stems in every variant.
    pub(crate) tracks: Vec<u32>,
    pub(crate) variants: Vec<RenderVariant>,
    /// File name pattern, default `$track-{variant}`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) start_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) end_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) format: Option<String>,
    /// Save and reopen the project before each render (default true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fresh: Option<bool>,
    /// Delete the files a render is about to write (default true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) overwrite: Option<bool>,
    /// Give up waiting after this many seconds (default 3600).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) timeout_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct BatchCall {
    /// Name of any tool except `batch` itself.
    pub(crate) tool: String,
    /// The tool's arguments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) args: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct BatchArgs {
    pub(crate) calls: Vec<BatchCall>,
    /// Keep going after a failed call (default false: stop at the first error).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) continue_on_error: Option<bool>,
    /// Give up waiting after this many seconds (default 3600).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) timeout_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct MoveItemsArgs {
    /// Zero-based indexes of the tracks whose items move.
    pub(crate) tracks: Vec<i32>,
    /// Move only this item (index from `list_items`); needs exactly one track.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) item: Option<i32>,
    /// How many units to move: positive later, negative earlier, fractions allowed (0.5 bar).
    pub(crate) value: f64,
    /// `bar`, `beat`, `grid` (the project grid), `1/1`, `1/2`, `1/4`, `1/8`, `1/16`, `1/32` or `1/64`. Default `bar`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) unit: Option<String>,
    /// Round every new position to this grid (same units as `unit`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) snap_to: Option<String>,
    /// Also move everything inside the folder tracks listed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) with_children: Option<bool>,
    /// Move the automation points of the tracks by the same musical distance (default true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) include_envelopes: Option<bool>,
    /// Only report what would move.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) dry_run: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SetItemPositionArgs {
    pub(crate) track: i32,
    /// Zero-based item index from `list_items`.
    pub(crate) item: i32,
    /// Bar number, starting at 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bar: Option<i32>,
    /// Beat within the bar, starting at 1 (fractions allowed). Default 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) beat: Option<f64>,
    /// Alternative to bar: a position in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) seconds: Option<f64>,
    /// Round the position to this grid (`bar`, `beat`, `1/16` ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) snap_to: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct ExportFxChainArgs {
    /// Zero-based track index; -1 is the master track.
    pub(crate) track: i32,
    /// Path of the `.RfxChain` file to write.
    pub(crate) path: String,
    /// Only these plugins: FX indexes or parts of their names. Default: the whole chain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fx: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct ImportFxChainArgs {
    /// Zero-based track index; -1 is the master track.
    pub(crate) track: i32,
    /// Path of an `.RfxChain` file.
    pub(crate) path: String,
    /// `append` (default, after the FX the track has) or `replace`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mode: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SwitchProjectArgs {
    /// Zero-based project tab index from `list_projects`.
    pub(crate) index: u32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct InsertMediaArgs {
    /// Zero-based track index; -1 is the master track.
    pub(crate) track: i32,
    /// Absolute path of the audio file.
    pub(crate) path: String,
    /// Item position in seconds (default 0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) position: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct AnalyzeAudioArgs {
    /// Absolute path of a WAV file (16, 24 or 32-bit integer, or 32-bit float).
    pub(crate) path: String,
    /// Analyse only from this time in seconds (default: start of the file).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) start_seconds: Option<f64>,
    /// Analyse only up to this time in seconds (default: end of the file).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) end_seconds: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct CompareAudioArgs {
    /// Reference file A (for example the professional mix or stem).
    pub(crate) a: String,
    /// File B, compared against A (for example your render).
    pub(crate) b: String,
    /// Start time in A in seconds (default 0). Use it with b_start_seconds to line up the two files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) a_start_seconds: Option<f64>,
    /// Start time in B in seconds (default 0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) b_start_seconds: Option<f64>,
    /// Length of the compared part in seconds (default: as much as both files have after their start).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) duration_seconds: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct ListTrackEnvelopesArgs {
    /// Zero-based track index; -1 is the master track.
    pub(crate) track: i32,
    /// Also scan every plugin parameter for automation (slow on plugins with thousands of MIDI parameters).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) include_all_parameters: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SetEnvelopeActiveArgs {
    /// Zero-based track index; -1 is the master track.
    pub(crate) track: i32,
    /// Track envelope name (`Volume`, `Pan`, `Mute`, `Trim Volume`, `Volume (Pre-FX)` ...), or the
    /// parameter name when `fx` is given.
    pub(crate) envelope: String,
    /// Plugin index when the envelope belongs to a plugin parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fx: Option<u32>,
    /// true switches the envelope on, false off (the points are kept).
    pub(crate) active: bool,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct RunActionArgs {
    /// Numeric action ID or a named command such as `_SWS_SAVEALLSELTRK`.
    pub(crate) command_id: ActionId,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub(crate) enum ActionId {
    Numeric(u32),
    Named(String),
}

pub(crate) fn to_args<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or_else(|_| json!({}))
}

#[cfg(test)]
mod tests {
    use super::AudioDeviceArgs;

    #[test]
    fn audio_device_action_schema_accepts_only_supported_values() {
        for action in ["status", "close", "open"] {
            let args =
                serde_json::from_str::<AudioDeviceArgs>(&format!("{{\"action\":\"{action}\"}}"))
                    .unwrap();
            assert_eq!(serde_json::to_value(args).unwrap()["action"], action);
        }
        assert!(serde_json::from_str::<AudioDeviceArgs>(r#"{"action":"restart"}"#).is_err());
    }
}
