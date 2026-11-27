//! Arguments of the project, transport, marker and batch tools.

use super::*;

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
pub(crate) struct SetProjectSampleRateArgs {
    /// Project sample rate in Hz, a whole number such as 44100, 48000 or 96000.
    pub(crate) sample_rate: u32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct CloseProjectTabArgs {
    /// Zero-based project tab index from `list_projects`. Required: only the tab you name is closed.
    pub(crate) index: u32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct SwitchProjectArgs {
    /// Zero-based project tab index from `list_projects`.
    pub(crate) index: u32,
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

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct CloseFxWindowsArgs {
    /// Exact start of the dialog title to close, for example "VST3: MyPlugin (Vendor) - Track ". Must not be empty.
    pub(crate) title_prefix: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct ListPluginsArgs {
    /// Only plugins whose name contains this text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) filter: Option<String>,
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
pub(crate) struct InsertMediaArgs {
    /// Zero-based track index; -1 is the master track.
    pub(crate) track: i32,
    /// Absolute path of the audio file.
    pub(crate) path: String,
    /// Item position in seconds (default 0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) position: Option<f64>,
}

#[cfg(test)]
#[path = "project_test.rs"]
mod tests;
