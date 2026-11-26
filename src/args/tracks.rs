//! Arguments of the track, send and template tools.

use super::*;

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
pub(crate) struct DeleteTrackArgs {
    /// Zero-based track index from `list_tracks` (the master track cannot be deleted).
    pub(crate) track: i32,
    /// For a folder track: also delete everything inside it. Default false (the children stay).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) with_children: Option<bool>,
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
