//! Arguments of the media item and MIDI tools.

use super::*;

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
