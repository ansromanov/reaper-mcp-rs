//! Arguments of the rendering and audio analysis tools.

use super::*;

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
