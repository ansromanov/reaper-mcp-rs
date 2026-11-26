//! Arguments of the FX, parameter and envelope tools.

use super::*;

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub(crate) enum ParamRef {
    /// Zero-based parameter index from `list_fx_params`.
    Index(u32),
    /// Parameter name; exact match first, then case-insensitive substring.
    Name(String),
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
