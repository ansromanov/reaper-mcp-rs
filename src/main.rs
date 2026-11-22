mod audio;
mod reaper_host;
mod track_templates;

use std::{
    env,
    fs::{self, File},
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Context, Result};
use directories::BaseDirs;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, tool, tool_router,
    transport::stdio,
    ServiceExt,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const HEARTBEAT_GRACE: Duration = Duration::from_secs(5);
const LONG_TIMEOUT: Duration = Duration::from_secs(300);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
struct BridgeClient {
    dir: PathBuf,
    serial: Arc<Mutex<()>>,
}

#[derive(Serialize)]
struct Request<'a> {
    id: &'a str,
    command: &'a str,
    args: Value,
}

#[derive(Deserialize)]
struct Response {
    id: String,
    ok: bool,
    #[serde(default)]
    result: Value,
    #[serde(default)]
    error: Option<String>,
}

impl BridgeClient {
    fn new() -> Result<Self> {
        let dir = match env::var_os("REAPER_MCP_DIR") {
            Some(path) => PathBuf::from(path),
            None => {
                let base = BaseDirs::new().context("cannot locate user configuration directory")?;
                base.config_dir().join("REAPER").join("reaper-mcp-bridge")
            }
        };
        fs::create_dir_all(&dir)
            .with_context(|| format!("cannot create IPC directory {}", dir.display()))?;
        Ok(Self {
            dir,
            serial: Arc::new(Mutex::new(())),
        })
    }

    fn heartbeat_age(&self) -> Result<Duration> {
        let path = self.dir.join("heartbeat");
        let value = fs::read_to_string(&path)
            .with_context(|| format!("bridge heartbeat missing at {}", path.display()))?;
        let timestamp: u64 = value
            .trim()
            .parse()
            .context("bridge heartbeat has invalid timestamp")?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        Ok(Duration::from_secs(now.saturating_sub(timestamp)))
    }

    fn call(&self, command: &str, args: Value, timeout: Duration) -> Result<Value> {
        let _guard = self
            .serial
            .lock()
            .map_err(|_| anyhow!("IPC request lock was poisoned"))?;
        // A long REAPER operation (project load, render) stalls the heartbeat;
        // give the bridge a moment to resume before declaring it stopped.
        let grace_deadline = std::time::Instant::now() + HEARTBEAT_GRACE;
        loop {
            // The bridge replaces the heartbeat file (remove, then rename), so a
            // read can briefly miss it; treat any failure as stale until the grace ends.
            let state = self.heartbeat_age();
            if matches!(&state, Ok(age) if *age <= HEARTBEAT_TIMEOUT) {
                break;
            }
            if std::time::Instant::now() >= grace_deadline {
                return match state {
                    Ok(age) => Err(anyhow!("REAPER bridge heartbeat is stale ({age:?})")),
                    Err(error) => Err(error),
                };
            }
            thread::sleep(Duration::from_millis(100));
        }

        let id = uuid::Uuid::new_v4().to_string();
        let request = Request {
            id: &id,
            command,
            args,
        };
        let request_path = self.dir.join("request.json");
        let request_tmp = self.dir.join(format!("request.{id}.tmp"));
        let response_path = self.dir.join("response.json");

        let _ = fs::remove_file(&response_path);
        let bytes = serde_json::to_vec(&request)?;
        let mut file = File::create(&request_tmp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&request_tmp, &request_path).context("cannot publish IPC request atomically")?;

        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            match fs::read(&response_path) {
                Ok(bytes) => {
                    let response: Response = serde_json::from_slice(&bytes)
                        .context("REAPER bridge returned invalid JSON")?;
                    if response.id == id {
                        let _ = fs::remove_file(&response_path);
                        if response.ok {
                            return Ok(response.result);
                        }
                        return Err(anyhow!(
                            "REAPER command {command} failed: {}",
                            response
                                .error
                                .unwrap_or_else(|| "unknown bridge error".into())
                        ));
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("cannot read bridge response"),
            }
            thread::sleep(Duration::from_millis(20));
        }

        let _ = fs::remove_file(&request_path);
        Err(anyhow!(
            "REAPER command {command} timed out after {timeout:?}"
        ))
    }

    fn health(&self) -> Value {
        match self.heartbeat_age() {
            Ok(age) => json!({
                "connected": age <= HEARTBEAT_TIMEOUT,
                "heartbeat_age_seconds": age.as_secs(),
                "ipc_directory": self.dir,
            }),
            Err(error) => json!({
                "connected": false,
                "error": error.to_string(),
                "ipc_directory": self.dir,
            }),
        }
    }
}

async fn bridge_tool(bridge: &BridgeClient, command: &'static str, args: Value) -> CallToolResult {
    bridge_tool_timeout(bridge, command, args, REQUEST_TIMEOUT).await
}

async fn bridge_tool_timeout(
    bridge: &BridgeClient,
    command: &'static str,
    args: Value,
    timeout: Duration,
) -> CallToolResult {
    let bridge = bridge.clone();
    let command = command.to_owned();
    let result = tokio::task::spawn_blocking(move || bridge.call(&command, args, timeout))
        .await
        .context("REAPER IPC worker stopped unexpectedly")
        .and_then(|result| result);
    tool_result(result)
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CloseFxWindowsArgs {
    /// Exact start of the dialog title to close, for example "VST3: MyPlugin (Vendor) - Track ". Must not be empty.
    title_prefix: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct TrackNameArgs {
    /// Name to assign to the new track.
    name: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct MoveTrackArgs {
    /// Zero-based index of the track to move (from `list_tracks`).
    index: u32,
    /// Zero-based index the track should end up before; the track count moves it to the end.
    before_index: u32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SetTrackFolderDepthArgs {
    /// Zero-based track index.
    index: u32,
    /// 1 starts a folder, 0 is a normal track, -1 (or lower, down to -16) closes that many folder levels.
    depth: i32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RenameTrackArgs {
    /// Zero-based track index from `list_tracks`.
    index: u32,
    /// New track name.
    name: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
enum ParamRef {
    /// Zero-based parameter index from `list_fx_params`.
    Index(u32),
    /// Parameter name; exact match first, then case-insensitive substring.
    Name(String),
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct OpenProjectArgs {
    /// Absolute path of the .rpp file to open.
    path: String,
    /// Open even if the current project has unsaved changes (they are lost).
    #[serde(skip_serializing_if = "Option::is_none")]
    discard_unsaved: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SaveProjectArgs {
    /// Save a copy to this path instead of saving in place.
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum AudioDeviceAction {
    Status,
    Close,
    Open,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct AudioDeviceArgs {
    /// Read status or close/open REAPER's audio and MIDI devices.
    action: AudioDeviceAction,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct ExportMixTemplateArgs {
    /// Destination directory for the .RTrackTemplate files.
    directory: String,
    /// Optional track groups. Defaults to DRUMS, BASS, GUITARS, VOCALS, MIX BUS, RETURNS.
    #[serde(skip_serializing_if = "Option::is_none")]
    groups: Option<Vec<String>>,
    /// Whether to also write the active project's master FX chain to master.RfxChain (defaults to true).
    #[serde(skip_serializing_if = "Option::is_none")]
    include_master_chain: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct ApplyTrackFxTemplateArgs {
    /// Path to an exported .RTrackTemplate file.
    template_path: String,
    /// Exact source track name inside the template.
    source_track: String,
    /// Zero-based destination track index; -1 addresses the master track.
    target_track: i32,
    /// FX chain operation: replace (default) or append source FX after existing FX.
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SetProjectSampleRateArgs {
    /// Project sample rate in Hz, a whole number such as 44100, 48000 or 96000.
    sample_rate: u32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct TrackRef {
    /// Zero-based track index from `list_tracks`; -1 is the master track.
    track: i32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SetTrackArgs {
    /// Zero-based track index from `list_tracks`; -1 is the master track.
    track: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    /// Track fader in dB (0 = unity).
    #[serde(skip_serializing_if = "Option::is_none")]
    volume_db: Option<f64>,
    /// Pan from -1 (left) to 1 (right).
    #[serde(skip_serializing_if = "Option::is_none")]
    pan: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mute: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    solo: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phase_inverted: Option<bool>,
    /// Whether the track feeds its parent / master.
    #[serde(skip_serializing_if = "Option::is_none")]
    master_send: Option<bool>,
    /// Whether the track's entire FX chain is enabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    fx_enabled: Option<bool>,
    /// Number of track channels (even, 2-128), for example 34 so a multi-out instrument such as Kontakt
    /// can feed separate kit tracks through sends from channel pairs.
    #[serde(skip_serializing_if = "Option::is_none")]
    channels: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct MultioutOutput {
    /// Name of the new track, for example `KICK IN`.
    name: String,
    /// First instrument output channel of the pair: 0 = outputs 1/2, 2 = 3/4, 32 = 33/34.
    source_channel: u32,
    /// Take only that one channel (a mono output).
    #[serde(skip_serializing_if = "Option::is_none")]
    mono: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    volume_db: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pan: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct CreateMultioutTracksArgs {
    /// Track with the multi-out instrument (Kontakt, ML Drums ...).
    track: i32,
    /// One new track per output pair, created right after the instrument in this order.
    outputs: Vec<MultioutOutput>,
    /// Put the new tracks into a new folder track with this name.
    #[serde(skip_serializing_if = "Option::is_none")]
    folder: Option<String>,
    /// Send mode: 0 post-fader (default), 1 pre-FX, 3 post-FX before the fader.
    #[serde(skip_serializing_if = "Option::is_none")]
    send_mode: Option<i32>,
    /// false turns off the instrument's own send to its parent, so only the new tracks are heard.
    #[serde(skip_serializing_if = "Option::is_none")]
    instrument_to_parent: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct ProbeInstrumentOutputsArgs {
    /// Track with the multi-out instrument; it must have MIDI to play in the range.
    track: i32,
    /// Folder for the probe files (one WAV per output pair).
    directory: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_seconds: Option<f64>,
    /// How many instrument channels to probe; default: the largest output count of the track's FX.
    #[serde(skip_serializing_if = "Option::is_none")]
    channels: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SetItemsTimeBaseArgs {
    /// `time` (default: items stay at their position in seconds when the tempo map changes), `beats`,
    /// `beats_position` (only the position follows the beats) or `default` (the project's setting).
    #[serde(skip_serializing_if = "Option::is_none")]
    time_base: Option<String>,
    /// Zero-based track indexes (default: every track).
    #[serde(skip_serializing_if = "Option::is_none")]
    tracks: Option<Vec<i32>>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct NormalizeItemsArgs {
    /// Normalization target in dB (0 or lower): dBFS for peak and true_peak, LUFS for the lufs modes, dB for rms.
    target_db: f64,
    /// peak (default), true_peak, lufs_i, lufs_m, lufs_s or rms.
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
    /// Zero-based track indexes to normalize (default: every track).
    #[serde(skip_serializing_if = "Option::is_none")]
    tracks: Option<Vec<i32>>,
    /// Zero-based track indexes to leave alone.
    #[serde(skip_serializing_if = "Option::is_none")]
    exclude_tracks: Option<Vec<i32>>,
    /// Only report what would change.
    #[serde(skip_serializing_if = "Option::is_none")]
    dry_run: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SetItemArgs {
    /// Zero-based track index from `list_tracks`.
    track: i32,
    /// Zero-based item index on the track, from `list_items`.
    item: i32,
    /// Item gain in dB (0 = unity).
    #[serde(skip_serializing_if = "Option::is_none")]
    volume_db: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mute: Option<bool>,
    /// Active take pan from -1 (left) to 1 (right); acts before the track FX.
    #[serde(skip_serializing_if = "Option::is_none")]
    take_pan: Option<f64>,
    /// Active take channel mode: normal, reverse_stereo, mono_mix, left or right.
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_mode: Option<String>,
    /// Item position on the timeline in seconds (use `move_items` or `set_item_position` for bars and beats).
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<f64>,
    /// Item length in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    length: Option<f64>,
    /// Start offset of the take in its file (a slip edit), seconds. The audio appears on the timeline at
    /// position - start_offset; 0 plays the file from its start at the item position.
    #[serde(skip_serializing_if = "Option::is_none")]
    start_offset: Option<f64>,
    /// Path of an audio file to play instead of the current source.
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    /// With `source`: set the item length to the new file's length (default true).
    #[serde(skip_serializing_if = "Option::is_none")]
    set_length: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct FxRef {
    /// Zero-based track index; -1 is the master track.
    track: i32,
    /// Zero-based FX index from `list_fx`.
    fx: u32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct AddFxArgs {
    /// Zero-based track index; -1 is the master track.
    track: i32,
    /// FX name as REAPER shows it, for example `VST: ReaEQ (Cockos)` or just `ReaEQ`.
    name: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SetFxEnabledArgs {
    track: i32,
    fx: u32,
    enabled: bool,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SetFxParamArgs {
    track: i32,
    fx: u32,
    /// Parameter index or name.
    param: ParamRef,
    #[serde(flatten)]
    target: ParamTarget,
}

/// Exactly one of the three fields must be set.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct ParamTarget {
    /// Normalized value from 0 to 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<f64>,
    /// Number as the plugin shows it, in the plugin's own unit (for example -18 for "-18.0 dB",
    /// 1200 for "1.2 kHz"). Solved by bisection; the result shows the achieved formatted value.
    #[serde(skip_serializing_if = "Option::is_none")]
    display_value: Option<f64>,
    /// Text the plugin shows, for choice and switch parameters (for example "Clean" or "On").
    #[serde(skip_serializing_if = "Option::is_none")]
    display_text: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct ParamEntry {
    /// Parameter index or name.
    param: ParamRef,
    #[serde(flatten)]
    target: ParamTarget,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SetFxParamsArgs {
    track: i32,
    fx: u32,
    /// Parameters to set; a failed entry is reported and the rest still apply.
    params: Vec<ParamEntry>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SetFxPresetArgs {
    track: i32,
    fx: u32,
    /// Preset name as REAPER lists it for the plugin.
    preset: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct MoveFxArgs {
    track: i32,
    /// Current zero-based FX index.
    fx: u32,
    /// Final zero-based FX index in the chain.
    to_index: u32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct ParamChoicesArgs {
    track: i32,
    fx: u32,
    /// Parameter index or name.
    param: ParamRef,
    /// Sampling resolution across 0..1 (default 400).
    #[serde(skip_serializing_if = "Option::is_none")]
    steps: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct FormatFxParamArgs {
    track: i32,
    fx: u32,
    /// Parameter index or name.
    param: ParamRef,
    /// Normalized value 0..1 to format.
    value: f64,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct EnvelopePoint {
    /// Time in seconds.
    time: f64,
    /// Normalized parameter value 0..1.
    value: f64,
    /// REAPER shape: 0 linear, 1 square, 2 slow start/end, 3 fast start, 4 fast end, 5 bezier.
    #[serde(skip_serializing_if = "Option::is_none")]
    shape: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tension: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SetFxEnvelopeArgs {
    track: i32,
    fx: u32,
    /// Parameter index or name.
    param: ParamRef,
    /// Replaces all existing points of the parameter's automation envelope.
    points: Vec<EnvelopePoint>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct PinMapping {
    /// Zero-based FX pin.
    pin: u32,
    /// Bitmask of track channels routed to this pin (1 = channel 1, 2 = channel 2, 3 = both).
    mask: u32,
    /// true for an output pin, false (default) for an input pin.
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SetFxPinsArgs {
    track: i32,
    fx: u32,
    pins: Vec<PinMapping>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct CreateSendArgs {
    /// Source track index.
    source: i32,
    /// Destination track index.
    destination: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    volume_db: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pan: Option<f64>,
    /// First source channel of the pair: 0 = channels 1/2, 2 = 3/4, 4 = 5/6 ...; -1 = no audio.
    #[serde(skip_serializing_if = "Option::is_none")]
    source_channel: Option<i32>,
    /// First destination channel of the pair (0 = channels 1/2).
    #[serde(skip_serializing_if = "Option::is_none")]
    dest_channel: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mono: Option<bool>,
    /// Where the send taps the track: 0 = post-fader, 1 = pre-FX, 3 = post-FX before the fader.
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phase_inverted: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SetSendArgs {
    /// Source track index.
    source: i32,
    /// Send index from `list_sends`.
    send_index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    volume_db: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pan: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mute: Option<bool>,
    /// First source channel of the pair: 0 = channels 1/2, 2 = 3/4, 4 = 5/6 ...; -1 = no audio.
    #[serde(skip_serializing_if = "Option::is_none")]
    source_channel: Option<i32>,
    /// First destination channel of the pair (0 = channels 1/2).
    #[serde(skip_serializing_if = "Option::is_none")]
    dest_channel: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mono: Option<bool>,
    /// Where the send taps the track: 0 = post-fader, 1 = pre-FX, 3 = post-FX before the fader.
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phase_inverted: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct RemoveSendArgs {
    /// Source track index.
    source: i32,
    /// Send index from `list_sends`.
    send_index: u32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct RenderProjectArgs {
    /// Output directory.
    directory: String,
    /// File name without extension; the extension comes from the project's render format.
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sample_rate: Option<u32>,
    /// Render only from this time (seconds); give together with end_seconds. Default: the whole project.
    #[serde(skip_serializing_if = "Option::is_none")]
    start_seconds: Option<f64>,
    /// Render only up to this time (seconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    end_seconds: Option<f64>,
    /// Output format: `pcm16`, `pcm24`, `float32` (keeps levels above 0 dBFS) or a raw base64 render config.
    /// Default: the project's saved format.
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
    /// Delete the files this render is about to write (default true). REAPER itself never overwrites:
    /// it writes `NAME-001.wav` and leaves the old file, which a caller would then read by mistake.
    #[serde(skip_serializing_if = "Option::is_none")]
    overwrite: Option<bool>,
    /// Save the project and open it again before rendering, so the render sees the saved state (default false).
    #[serde(skip_serializing_if = "Option::is_none")]
    fresh: Option<bool>,
    /// Give up waiting after this many seconds (default 1800).
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct RenderStemsArgs {
    /// Output directory.
    directory: String,
    /// Zero-based indexes of the tracks to render, each as its own file named after the track.
    tracks: Vec<u32>,
    /// File name pattern; defaults to `$track`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pattern: Option<String>,
    /// Render only from this time (seconds); give together with end_seconds. Default: the whole project.
    #[serde(skip_serializing_if = "Option::is_none")]
    start_seconds: Option<f64>,
    /// Render only up to this time (seconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    end_seconds: Option<f64>,
    /// Output format: `pcm16`, `pcm24`, `float32` (keeps levels above 0 dBFS) or a raw base64 render config.
    /// Default: the project's saved format.
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
    /// Delete the files this render is about to write (default true). REAPER itself never overwrites:
    /// it writes `NAME-001.wav` and leaves the old file, which a caller would then read by mistake.
    #[serde(skip_serializing_if = "Option::is_none")]
    overwrite: Option<bool>,
    /// Save the project and open it again before rendering, so the render sees the saved state (default false).
    #[serde(skip_serializing_if = "Option::is_none")]
    fresh: Option<bool>,
    /// Give up waiting after this many seconds (default 3600).
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct CloseProjectTabArgs {
    /// Zero-based project tab index from `list_projects`. Required: only the tab you name is closed.
    index: u32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct DeleteTrackArgs {
    /// Zero-based track index from `list_tracks` (the master track cannot be deleted).
    track: i32,
    /// For a folder track: also delete everything inside it. Default false (the children stay).
    #[serde(skip_serializing_if = "Option::is_none")]
    with_children: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct DeleteItemArgs {
    /// Zero-based track index.
    track: i32,
    /// Zero-based item index from `list_items`.
    item: i32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SetMediaOfflineArgs {
    /// Path of the media file the items play.
    path: String,
    /// true closes the file (so it can be overwritten), false opens it again.
    offline: bool,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct ListMidiNotesArgs {
    /// Zero-based track index.
    track: i32,
    /// Zero-based index of the MIDI item on the track (`list_items`).
    item: i32,
    /// Return at most this many notes (default 5000).
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u32>,
    /// Skip this many notes first.
    #[serde(skip_serializing_if = "Option::is_none")]
    offset: Option<u32>,
    /// Only notes of this MIDI pitch (the per-pitch counts always cover all notes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pitch: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct TempoMarker {
    /// Position in quarter notes from the start of the project (the first marker must be 0).
    quarter_notes: f64,
    /// Tempo in BPM from this position on, 10-960.
    bpm: f64,
    /// Time signature numerator from this position on (omit to keep the current one).
    #[serde(skip_serializing_if = "Option::is_none")]
    numerator: Option<i32>,
    /// Time signature denominator from this position on (omit to keep the current one).
    #[serde(skip_serializing_if = "Option::is_none")]
    denominator: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SetTempoMapArgs {
    /// Tempo markers in increasing quarter-note order, the first at 0.
    markers: Vec<TempoMarker>,
    /// Remove the existing tempo markers first (default true).
    #[serde(skip_serializing_if = "Option::is_none")]
    clear: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct NewMarker {
    /// Name shown in the ruler and the marker list.
    name: String,
    /// Position in project seconds.
    position_seconds: f64,
    /// End in project seconds. Given, the entry is a region from the position to the end; omitted, a marker.
    #[serde(skip_serializing_if = "Option::is_none")]
    end_seconds: Option<f64>,
    /// Color as a hex string such as "#ff8800" (default: REAPER's color).
    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct AddMarkersArgs {
    /// Markers and regions to add.
    markers: Vec<NewMarker>,
    /// Delete all existing markers and regions first (default false).
    #[serde(skip_serializing_if = "Option::is_none")]
    clear: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct MidiNote {
    /// Start in project seconds.
    start_seconds: f64,
    /// End in project seconds.
    end_seconds: f64,
    pitch: i32,
    /// 1-127, default 100.
    #[serde(skip_serializing_if = "Option::is_none")]
    velocity: Option<i32>,
    /// 0-15, default 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    channel: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct AddMidiNotesArgs {
    track: i32,
    /// Zero-based index of the MIDI item on the track.
    item: i32,
    notes: Vec<MidiNote>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct CreateMidiItemArgs {
    track: i32,
    start_seconds: f64,
    end_seconds: f64,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct AutomationPoint {
    /// Project time in seconds.
    time: f64,
    /// Value in the envelope's own range (Pan -1..1, FX parameters 0..1 and so on).
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<f64>,
    /// For the Volume envelope: the gain in dB (0 = unity).
    #[serde(skip_serializing_if = "Option::is_none")]
    value_db: Option<f64>,
    /// 0 linear, 1 square, 2 slow start/end, 3 fast start, 4 fast end, 5 bezier.
    #[serde(skip_serializing_if = "Option::is_none")]
    shape: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SetEnvelopePointsArgs {
    track: i32,
    /// `Volume` or `Pan` (created and shown if it does not exist), or, with `fx`, the parameter name or index.
    envelope: serde_json::Value,
    /// FX index when the envelope belongs to an FX parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    fx: Option<i32>,
    points: Vec<AutomationPoint>,
    /// Delete the existing points first (default true).
    #[serde(skip_serializing_if = "Option::is_none")]
    clear: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct ListPluginsArgs {
    /// Only plugins whose name contains this text.
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct RenderVariant {
    /// Name of the variant, used in the file names (`{variant}` in the pattern).
    name: String,
    /// Tracks to mute for this variant: names or zero-based indexes.
    #[serde(skip_serializing_if = "Option::is_none")]
    mute: Option<Vec<serde_json::Value>>,
    /// Tracks to unmute for this variant: names or indexes.
    #[serde(skip_serializing_if = "Option::is_none")]
    unmute: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct RenderVariantsArgs {
    /// Output directory.
    directory: String,
    /// Zero-based indexes of the tracks to render as stems in every variant.
    tracks: Vec<u32>,
    variants: Vec<RenderVariant>,
    /// File name pattern, default `$track-{variant}`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
    /// Save and reopen the project before each render (default true).
    #[serde(skip_serializing_if = "Option::is_none")]
    fresh: Option<bool>,
    /// Delete the files a render is about to write (default true).
    #[serde(skip_serializing_if = "Option::is_none")]
    overwrite: Option<bool>,
    /// Give up waiting after this many seconds (default 3600).
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct BatchCall {
    /// Name of any tool except `batch` itself.
    tool: String,
    /// The tool's arguments.
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct BatchArgs {
    calls: Vec<BatchCall>,
    /// Keep going after a failed call (default false: stop at the first error).
    #[serde(skip_serializing_if = "Option::is_none")]
    continue_on_error: Option<bool>,
    /// Give up waiting after this many seconds (default 3600).
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct MoveItemsArgs {
    /// Zero-based indexes of the tracks whose items move.
    tracks: Vec<i32>,
    /// Move only this item (index from `list_items`); needs exactly one track.
    #[serde(skip_serializing_if = "Option::is_none")]
    item: Option<i32>,
    /// How many units to move: positive later, negative earlier, fractions allowed (0.5 bar).
    value: f64,
    /// `bar`, `beat`, `grid` (the project grid), `1/1`, `1/2`, `1/4`, `1/8`, `1/16`, `1/32` or `1/64`. Default `bar`.
    #[serde(skip_serializing_if = "Option::is_none")]
    unit: Option<String>,
    /// Round every new position to this grid (same units as `unit`).
    #[serde(skip_serializing_if = "Option::is_none")]
    snap_to: Option<String>,
    /// Also move everything inside the folder tracks listed.
    #[serde(skip_serializing_if = "Option::is_none")]
    with_children: Option<bool>,
    /// Move the automation points of the tracks by the same musical distance (default true).
    #[serde(skip_serializing_if = "Option::is_none")]
    include_envelopes: Option<bool>,
    /// Only report what would move.
    #[serde(skip_serializing_if = "Option::is_none")]
    dry_run: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SetItemPositionArgs {
    track: i32,
    /// Zero-based item index from `list_items`.
    item: i32,
    /// Bar number, starting at 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    bar: Option<i32>,
    /// Beat within the bar, starting at 1 (fractions allowed). Default 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    beat: Option<f64>,
    /// Alternative to bar: a position in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    seconds: Option<f64>,
    /// Round the position to this grid (`bar`, `beat`, `1/16` ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    snap_to: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct ExportFxChainArgs {
    /// Zero-based track index; -1 is the master track.
    track: i32,
    /// Path of the `.RfxChain` file to write.
    path: String,
    /// Only these plugins: FX indexes or parts of their names. Default: the whole chain.
    #[serde(skip_serializing_if = "Option::is_none")]
    fx: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct ImportFxChainArgs {
    /// Zero-based track index; -1 is the master track.
    track: i32,
    /// Path of an `.RfxChain` file.
    path: String,
    /// `append` (default, after the FX the track has) or `replace`.
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SwitchProjectArgs {
    /// Zero-based project tab index from `list_projects`.
    index: u32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct InsertMediaArgs {
    /// Zero-based track index; -1 is the master track.
    track: i32,
    /// Absolute path of the audio file.
    path: String,
    /// Item position in seconds (default 0).
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct AnalyzeAudioArgs {
    /// Absolute path of a WAV file (16, 24 or 32-bit integer, or 32-bit float).
    path: String,
    /// Analyse only from this time in seconds (default: start of the file).
    #[serde(skip_serializing_if = "Option::is_none")]
    start_seconds: Option<f64>,
    /// Analyse only up to this time in seconds (default: end of the file).
    #[serde(skip_serializing_if = "Option::is_none")]
    end_seconds: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct CompareAudioArgs {
    /// Reference file A (for example the professional mix or stem).
    a: String,
    /// File B, compared against A (for example your render).
    b: String,
    /// Start time in A in seconds (default 0). Use it with b_start_seconds to line up the two files.
    #[serde(skip_serializing_if = "Option::is_none")]
    a_start_seconds: Option<f64>,
    /// Start time in B in seconds (default 0).
    #[serde(skip_serializing_if = "Option::is_none")]
    b_start_seconds: Option<f64>,
    /// Length of the compared part in seconds (default: as much as both files have after their start).
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_seconds: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct ListTrackEnvelopesArgs {
    /// Zero-based track index; -1 is the master track.
    track: i32,
    /// Also scan every plugin parameter for automation (slow on plugins with thousands of MIDI parameters).
    #[serde(skip_serializing_if = "Option::is_none")]
    include_all_parameters: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct SetEnvelopeActiveArgs {
    /// Zero-based track index; -1 is the master track.
    track: i32,
    /// Track envelope name (`Volume`, `Pan`, `Mute`, `Trim Volume`, `Volume (Pre-FX)` ...), or the
    /// parameter name when `fx` is given.
    envelope: String,
    /// Plugin index when the envelope belongs to a plugin parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    fx: Option<u32>,
    /// true switches the envelope on, false off (the points are kept).
    active: bool,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct RunActionArgs {
    /// Numeric action ID or a named command such as `_SWS_SAVEALLSELTRK`.
    command_id: ActionId,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
enum ActionId {
    Numeric(u32),
    Named(String),
}

fn to_args<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or_else(|_| json!({}))
}

#[derive(Clone)]
struct ReaperMcp {
    bridge: BridgeClient,
}

fn tool_result(result: Result<Value>) -> CallToolResult {
    match result {
        Ok(value) => CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()),
        )]),
        Err(error) => CallToolResult::error(vec![ContentBlock::text(error.to_string())]),
    }
}

fn round_json(value: Value, places: i32) -> Value {
    match value {
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.is_finite() && !n.is_i64() && !n.is_u64() => {
                let scale = 10f64.powi(places);
                json!((f * scale).round() / scale)
            }
            Some(f) if !f.is_finite() => Value::Null,
            _ => Value::Number(n),
        },
        Value::Array(items) => Value::Array(items.into_iter().map(|v| round_json(v, places)).collect()),
        Value::Object(map) => Value::Object(map.into_iter().map(|(k, v)| (k, round_json(v, places))).collect()),
        other => other,
    }
}

async fn run_blocking<T: Serialize + Send + 'static>(job: impl FnOnce() -> Result<T> + Send + 'static) -> CallToolResult {
    let result = tokio::task::spawn_blocking(job)
        .await
        .context("analysis worker stopped unexpectedly")
        .and_then(|result| result)
        .and_then(|value| Ok(round_json(serde_json::to_value(value)?, 1)));
    tool_result(result)
}

#[tool_router(server_handler)]
impl ReaperMcp {
    #[tool(description = "Inspect REAPER process windows and visible owned dialogs without using the Lua bridge. Reports window titles, class names, static child text, and whether the main window responds.")]
    async fn inspect_reaper_windows(&self) -> CallToolResult {
        run_blocking(reaper_host::inspect_reaper_windows).await
    }

    #[tool(description = "Post WM_CLOSE only to visible, owned REAPER #32770 dialogs whose title starts exactly with title_prefix (for example 'VST3: MyPlugin (Vendor) - Track '). Reports messages posted, not verified closure; never targets REAPER's main window or other dialogs. Windows only.")]
    async fn close_reaper_fx_windows(
        &self,
        Parameters(CloseFxWindowsArgs { title_prefix }): Parameters<CloseFxWindowsArgs>,
    ) -> CallToolResult {
        run_blocking(move || reaper_host::close_reaper_fx_windows(&title_prefix)).await
    }

    #[tool(description = "Launch a fresh REAPER bridge instance using the installed fixed bridge loader. This only reports that launch was requested; it does not verify bridge recovery or terminate any process.")]
    async fn restart_bridge(&self) -> CallToolResult {
        run_blocking(reaper_host::restart_bridge).await
    }

    #[tool(description = "Re-read the bridge's Lua handlers inside REAPER after the bridge files were updated, without restarting REAPER.")]
    async fn reload_bridge(&self) -> CallToolResult {
        bridge_tool(&self.bridge, "reload", json!({})).await
    }

    #[tool(description = "Check whether the REAPER Lua bridge is running.")]
    async fn health(&self) -> CallToolResult {
        tool_result(Ok(self.bridge.health()))
    }

    #[tool(description = "Read, close, or open REAPER's audio and MIDI devices. Reports observed running state before and after the action; open/close do not imply the device successfully changed state.")]
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

    #[tool(description = "Move one track before another position (0-based `before_index`; the track count moves it to the end). Folder state is left alone: reorder first, then call set_track_folder_depth.")]
    async fn move_track(&self, Parameters(args): Parameters<MoveTrackArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "move_track", to_args(&args)).await
    }

    #[tool(description = "Set a track's folder depth: 1 starts a folder (the following tracks are its children), 0 is a normal track, -1 or lower closes that many folder levels after this track. Routing is not changed: a child with its parent send on feeds the folder track.")]
    async fn set_track_folder_depth(&self, Parameters(args): Parameters<SetTrackFolderDepthArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "set_track_folder_depth", to_args(&args)).await
    }

    #[tool(description = "Open an .rpp project in REAPER. Refuses when the current project has unsaved changes unless discard_unsaved is true. Loading can take minutes for heavy projects.")]
    async fn open_project(&self, Parameters(args): Parameters<OpenProjectArgs>) -> CallToolResult {
        bridge_tool_timeout(&self.bridge, "open_project", to_args(&args), LONG_TIMEOUT).await
    }

    #[tool(description = "Save the current project in place, or save a copy to `path`.")]
    async fn save_project(&self, Parameters(args): Parameters<SaveProjectArgs>) -> CallToolResult {
        bridge_tool_timeout(&self.bridge, "save_project", to_args(&args), LONG_TIMEOUT).await
    }

    #[tool(description = "Read one track in detail: fader, pan, mute/solo, whole-chain FX enabled state, folder depth, parent, channels, FX/send/receive/item counts. Track -1 is master.")]
    async fn get_track(&self, Parameters(args): Parameters<TrackRef>) -> CallToolResult {
        bridge_tool(&self.bridge, "get_track", to_args(&args)).await
    }

    #[tool(description = "Export item-free REAPER track templates for named mix groups (default: DRUMS, BASS, GUITARS, VOCALS, MIX BUS, RETURNS). Preserves track FX, envelopes, folder descendants and internal receives; receives from tracks outside a group are omitted and reported. Also exports the master chain to master.RfxChain by default; set include_master_chain=false to skip it.")]
    async fn export_mix_template(
        &self,
        Parameters(args): Parameters<ExportMixTemplateArgs>,
    ) -> CallToolResult {
        bridge_tool_timeout(&self.bridge, "export_mix_template", to_args(&args), LONG_TIMEOUT).await
    }

    #[tool(description = "Apply static FX state from a named source track in an exported .RTrackTemplate. mode defaults to replace (existing behavior); append adds source FX after the destination chain and preserves all existing FX automation. Both modes preserve destination media, routing, pan and fader. Source timeline automation is never copied. Target -1 is the master track.")]
    async fn apply_track_fx_template(
        &self,
        Parameters(args): Parameters<ApplyTrackFxTemplateArgs>,
    ) -> CallToolResult {
        if args.template_path.trim().is_empty() {
            return tool_result(Err(anyhow!("template_path must be a non-empty path")));
        }
        if args.source_track.trim().is_empty() {
            return tool_result(Err(anyhow!("source_track must be a non-empty name")));
        }
        let mode = args.mode.unwrap_or_else(|| "replace".to_string());
        if !matches!(mode.as_str(), "replace" | "append") {
            return tool_result(Err(anyhow!("mode must be 'replace' or 'append'")));
        }
        let bridge = self.bridge.clone();
        let result = tokio::task::spawn_blocking(move || {
            let source_chain = track_templates::read_template_fx_chain(
                std::path::Path::new(&args.template_path),
                &args.source_track,
            )
            .map_err(|error| anyhow!(error))?;
            let target = args.target_track;
            let current = bridge.call(
                "get_track_state_chunk",
                json!({ "track": target }),
                LONG_TIMEOUT,
            )?;
            let current_chunk = current
                .get("chunk")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("REAPER did not return the destination track state chunk"))?;
            let replacement = match mode.as_str() {
                "append" => track_templates::append_track_fx_chain(
                    current_chunk,
                    source_chain.as_deref(),
                ),
                _ => track_templates::replace_track_fx_chain(
                    current_chunk,
                    source_chain.as_deref(),
                ),
            }
            .map_err(|error| anyhow!(error))?;
            bridge.call(
                "set_track_state_chunk",
                json!({
                    "track": target,
                    "chunk": replacement,
                    "close_fx_windows": true,
                    "preserve_existing_fx_windows": mode == "append",
                }),
                LONG_TIMEOUT,
            )
        })
        .await
        .context("FX template worker stopped unexpectedly")
        .and_then(|result| result);
        tool_result(result)
    }

    #[tool(description = "Change track name, fader (dB), pan, mute, solo, phase, master send, channel count or the whole FX-chain enabled state. Only the given fields change. Track -1 is master.")]
    async fn set_track(&self, Parameters(args): Parameters<SetTrackArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "set_track", to_args(&args)).await
    }

    #[tool(description = "List media items on a track with position, length, gain, mute, take pan, take channel mode, source channel count and source file. Take pan acts before the track FX.")]
    async fn list_items(&self, Parameters(args): Parameters<TrackRef>) -> CallToolResult {
        bridge_tool(&self.bridge, "list_items", to_args(&args)).await
    }

    #[tool(description = "Route a multi-out instrument (Kontakt with Krimh, ML Drums ...) to separate tracks: gives the instrument track enough channels and creates one track per output pair right after it (optionally in a new folder), each receiving its pair through a send. Use probe_instrument_outputs first to see which pairs carry which drum.")]
    async fn create_multiout_tracks(&self, Parameters(args): Parameters<CreateMultioutTracksArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "create_multiout_tracks", to_args(&args)).await
    }

    #[tool(description = "Find out what a multi-out instrument plays on each output pair: renders every pair (outputs 1/2, 3/4 ...) of the track to its own file through temporary tracks, measures them and reports loudness, peak and the loudest frequency band per pair (silent pairs are marked). Temporary tracks are removed and the track's channels and mute are restored. Render a short busy range (start_seconds/end_seconds).")]
    async fn probe_instrument_outputs(&self, Parameters(args): Parameters<ProbeInstrumentOutputsArgs>) -> CallToolResult {
        let bridge = self.bridge.clone();
        let result = tokio::task::spawn_blocking(move || -> Result<Value> {
            let mut probe = bridge.call("probe_instrument_outputs", to_args(&args), LONG_TIMEOUT)?;
            if let Some(pairs) = probe.get_mut("pairs").and_then(Value::as_array_mut) {
                for pair in pairs.iter_mut() {
                    let Some(path) = pair.get("file").and_then(Value::as_str).map(str::to_owned) else { continue };
                    match audio::read(std::path::Path::new(&path), None, None) {
                        Ok(audio) => {
                            let analysis = audio::analyze(&audio);
                            let silent = analysis.peak_dbfs < -90.0;
                            let loudest = analysis
                                .bands
                                .iter()
                                .max_by(|a, b| a.db.partial_cmp(&b.db).unwrap_or(std::cmp::Ordering::Equal))
                                .map(|band| band.name.clone());
                            pair["silent"] = json!(silent);
                            if !silent {
                                pair["integrated_lufs"] = json!(analysis.integrated_lufs);
                                pair["peak_dbfs"] = json!(analysis.peak_dbfs);
                                pair["crest_db"] = json!(analysis.crest_db);
                                pair["loudest_band"] = json!(loudest);
                                pair["side_to_mid_db"] = json!(analysis.side_to_mid_db);
                            }
                        }
                        Err(error) => pair["error"] = json!(error.to_string()),
                    }
                }
            }
            Ok(round_json(probe, 1))
        })
        .await
        .context("REAPER IPC worker stopped unexpectedly")
        .and_then(|result| result);
        tool_result(result)
    }

    #[tool(description = "Set the time base of the items of the chosen tracks: time (stay at their seconds when the tempo map changes, the safe setting for recorded audio), beats, beats_position or default. Use before set_tempo_map on a project whose items must not move.")]
    async fn set_items_time_base(&self, Parameters(args): Parameters<SetItemsTimeBaseArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "set_items_time_base", to_args(&args)).await
    }

    #[tool(description = "Normalize the audio items of the chosen tracks with REAPER's own normalization (the one behind Item processing > Normalize): the item gain is set so that the part of the source the item plays reaches target_db in peak, true_peak, lufs_i, lufs_m, lufs_s or rms. Replaces the existing item gain; take volume is kept. Use dry_run to see the gains first, exclude_tracks to skip drums or a reference.")]
    async fn normalize_items(&self, Parameters(args): Parameters<NormalizeItemsArgs>) -> CallToolResult {
        bridge_tool_timeout(&self.bridge, "normalize_items", to_args(&args), LONG_TIMEOUT).await
    }

    #[tool(description = "Change one media item: gain (dB), mute, active take pan (-1..1) or take channel mode (normal, reverse_stereo, mono_mix, left, right). Only the given fields change.")]
    async fn set_item(&self, Parameters(args): Parameters<SetItemArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "set_item", to_args(&args)).await
    }

    #[tool(description = "List the FX chain of a track with indexes, names, bypass state and parameter counts.")]
    async fn list_fx(&self, Parameters(args): Parameters<TrackRef>) -> CallToolResult {
        bridge_tool(&self.bridge, "list_fx", to_args(&args)).await
    }

    #[tool(description = "Append an FX to a track by name. Returns the new FX index and parameter count.")]
    async fn add_fx(&self, Parameters(args): Parameters<AddFxArgs>) -> CallToolResult {
        bridge_tool_timeout(&self.bridge, "add_fx", to_args(&args), LONG_TIMEOUT).await
    }

    #[tool(description = "Delete an FX from a track by index.")]
    async fn remove_fx(&self, Parameters(args): Parameters<FxRef>) -> CallToolResult {
        bridge_tool(&self.bridge, "remove_fx", to_args(&args)).await
    }

    #[tool(description = "Enable or bypass an FX.")]
    async fn set_fx_enabled(&self, Parameters(args): Parameters<SetFxEnabledArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "set_fx_enabled", to_args(&args)).await
    }

    #[tool(description = "List all parameters of an FX with names, normalized values and formatted values (real units).")]
    async fn list_fx_params(&self, Parameters(args): Parameters<FxRef>) -> CallToolResult {
        bridge_tool(&self.bridge, "list_fx_params", to_args(&args)).await
    }

    #[tool(description = "Set one FX parameter by index or name, with value (0..1), display_value (plugin units, e.g. -18 for -18 dB) or display_text (e.g. Clean). The result shows the formatted value.")]
    async fn set_fx_param(&self, Parameters(args): Parameters<SetFxParamArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "set_fx_param", to_args(&args)).await
    }

    #[tool(description = "Set many parameters of one FX at once. Each entry uses value (0..1), display_value (plugin units) or display_text. Per-entry failures are reported.")]
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

    #[tool(description = "Move an FX to another position in the same chain; to_index is its final index.")]
    async fn move_fx(&self, Parameters(args): Parameters<MoveFxArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "move_fx", to_args(&args)).await
    }

    #[tool(description = "List the distinct texts a choice parameter can show (shapes, modes, styles) with the normalized value for each.")]
    async fn list_fx_param_choices(&self, Parameters(args): Parameters<ParamChoicesArgs>) -> CallToolResult {
        bridge_tool_timeout(&self.bridge, "list_fx_param_choices", to_args(&args), LONG_TIMEOUT).await
    }

    #[tool(description = "Show the text a plugin displays for a normalized parameter value, without changing the parameter.")]
    async fn format_fx_param(&self, Parameters(args): Parameters<FormatFxParamArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "format_fx_param", to_args(&args)).await
    }

    #[tool(description = "Replace the automation envelope of an FX parameter with the given points (normalized values).")]
    async fn set_fx_envelope(&self, Parameters(args): Parameters<SetFxEnvelopeArgs>) -> CallToolResult {
        bridge_tool_timeout(&self.bridge, "set_fx_envelope", to_args(&args), LONG_TIMEOUT).await
    }

    #[tool(description = "Read an FX's pin mappings: which track channels (bitmask) feed each input pin and receive each output pin.")]
    async fn get_fx_pins(&self, Parameters(args): Parameters<FxRef>) -> CallToolResult {
        bridge_tool(&self.bridge, "get_fx_pins", to_args(&args)).await
    }

    #[tool(description = "Set FX pin mappings (track-channel bitmask per input or output pin), for example to run a mono plugin on one channel of a stereo track.")]
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

    #[tool(description = "Change level, pan, mute, source/destination channel pair, mono or phase of an existing send. Channel pairs are given by their first channel: 0 = 1/2, 2 = 3/4, 4 = 5/6 ... (a multi-out plugin such as Kontakt sends its outputs on channel pairs).")]
    async fn set_send(&self, Parameters(args): Parameters<SetSendArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "set_send", to_args(&args)).await
    }

    #[tool(description = "Remove one send from a track. The later sends of that track move down by one index; the result lists the sends that remain.")]
    async fn remove_send(&self, Parameters(args): Parameters<RemoveSendArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "remove_send", to_args(&args)).await
    }

    #[tool(description = "Read the project's current render settings (output folder, pattern, sample rate, bounds).")]
    async fn get_render_settings(&self) -> CallToolResult {
        bridge_tool(&self.bridge, "get_render_settings", json!({})).await
    }

    #[tool(description = "Render the whole project master mix to directory/name.<ext> using the project's saved render format. Deletes an old file with that name first (overwrite=false keeps it), lists the written files and warns when one is missing. Blocks until the render finishes.")]
    async fn render_project(&self, Parameters(args): Parameters<RenderProjectArgs>) -> CallToolResult {
        let timeout = Duration::from_secs(args.timeout_seconds.unwrap_or(1800));
        bridge_tool_timeout(&self.bridge, "render_project", to_args(&args), timeout).await
    }

    #[tool(description = "Render the given tracks as separate stem files (each track's own output including its children), using the project's saved render format. Deletes old stems with the same names first (overwrite=false keeps them), lists the written files, warns when one is missing and restores render settings and track selection afterwards. Blocks until done.")]
    async fn render_stems(&self, Parameters(args): Parameters<RenderStemsArgs>) -> CallToolResult {
        let timeout = Duration::from_secs(args.timeout_seconds.unwrap_or(3600));
        bridge_tool_timeout(&self.bridge, "render_stems", to_args(&args), timeout).await
    }

    #[tool(description = "List open project tabs with path, dirty state and track count. All other tools act on the active tab.")]
    async fn list_projects(&self) -> CallToolResult {
        bridge_tool(&self.bridge, "list_projects", json!({})).await
    }

    #[tool(description = "Make another open project tab the active one.")]
    async fn switch_project(&self, Parameters(args): Parameters<SwitchProjectArgs>) -> CallToolResult {
        // opening or closing a heavy project (samplers, many plugins) takes far longer than 5 s
        bridge_tool_timeout(&self.bridge, "switch_project", to_args(&args), LONG_TIMEOUT).await
    }

    #[tool(description = "Open a new empty project tab and make it active (the other tabs stay open).")]
    async fn new_project_tab(&self) -> CallToolResult {
        // opening or closing a heavy project (samplers, many plugins) takes far longer than 5 s
        bridge_tool_timeout(&self.bridge, "new_project_tab", json!({}), LONG_TIMEOUT).await
    }

    #[tool(description = "Close the project tab with the given index (see list_projects). The index is required; the previously active tab stays active. Refuses if that tab has unsaved changes or if it is the last tab.")]
    async fn close_project_tab(&self, Parameters(args): Parameters<CloseProjectTabArgs>) -> CallToolResult {
        // opening or closing a heavy project (samplers, many plugins) takes far longer than 5 s
        bridge_tool_timeout(&self.bridge, "close_project_tab", to_args(&args), LONG_TIMEOUT).await
    }

    #[tool(description = "Delete a track. With with_children also everything inside a folder track. The master track cannot be deleted.")]
    async fn delete_track(&self, Parameters(args): Parameters<DeleteTrackArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "delete_track", to_args(&args)).await
    }

    #[tool(description = "Move the items of tracks (or one item) in musical time: by bars, beats, the project grid or note values, optionally snapped to a grid, with the automation points. Negative values move earlier. dry_run only reports the from/to positions as seconds and bar.beat. Prefer this to moving by seconds: it stays on the grid.")]
    async fn move_items(&self, Parameters(args): Parameters<MoveItemsArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "move_items", to_args(&args)).await
    }

    #[tool(description = "Put one item at an absolute position given as bar and beat (or seconds), optionally snapped to a grid.")]
    async fn set_item_position(&self, Parameters(args): Parameters<SetItemPositionArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "set_item_position", to_args(&args)).await
    }

    #[tool(description = "Project tempo, time signature, grid size and the length of a bar in seconds: what one grid step or one bar means before moving anything.")]
    async fn get_grid(&self) -> CallToolResult {
        bridge_tool(&self.bridge, "get_grid", json!({})).await
    }

    #[tool(description = "Write the FX of a track (all, or the ones named in fx) to an .RfxChain file with the plugin states inside: the way to keep a plugin sound (an amp setting, a vocal chain) as a file. REAPER has no function that writes plugin preset files.")]
    async fn export_fx_chain(&self, Parameters(args): Parameters<ExportFxChainArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "export_fx_chain", to_args(&args)).await
    }

    #[tool(description = "Put the FX of an .RfxChain file on a track, after its existing FX (append) or instead of them (replace). Plugin states come with the file.")]
    async fn import_fx_chain(&self, Parameters(args): Parameters<ImportFxChainArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "import_fx_chain", to_args(&args)).await
    }

    #[tool(description = "Delete one media item from a track (indexes from list_items).")]
    async fn delete_item(&self, Parameters(args): Parameters<DeleteItemArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "delete_item", to_args(&args)).await
    }

    #[tool(description = "Set every item that plays a media file offline (REAPER closes the file, so it can be overwritten) or online again. Use before rewriting a WAV the project plays.")]
    async fn set_media_offline(&self, Parameters(args): Parameters<SetMediaOfflineArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "set_media_offline", to_args(&args)).await
    }

    #[tool(description = "List the notes of a MIDI item: start/end in project seconds, pitch, velocity, channel, plus the note count per pitch.")]
    async fn list_midi_notes(&self, Parameters(args): Parameters<ListMidiNotesArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "list_midi_notes", to_args(&args)).await
    }

    #[tool(description = "Add notes to an existing MIDI item (times in project seconds).")]
    async fn add_midi_notes(&self, Parameters(args): Parameters<AddMidiNotesArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "add_midi_notes", to_args(&args)).await
    }

    #[tool(description = "Write the project tempo map from markers given in quarter notes (the unit of a MIDI file or a Cubase tempo track): the first marker at 0, each marker's time follows from the tempi before it. Replaces the existing tempo markers unless clear is false. Use it before importing MIDI so notes land on the same bars as the audio.")]
    async fn set_tempo_map(&self, Parameters(args): Parameters<SetTempoMapArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "set_tempo_map", to_args(&args)).await
    }

    #[tool(description = "Add markers (a position) and regions (a position and an end) by name at project times in seconds, for navigating a long project. Existing markers are kept unless clear is true.")]
    async fn add_markers(&self, Parameters(args): Parameters<AddMarkersArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "add_markers", to_args(&args)).await
    }

    #[tool(description = "List project markers and regions with list index, marker number, name, position and region end (seconds) and color.")]
    async fn list_markers(&self) -> CallToolResult {
        bridge_tool(&self.bridge, "list_markers", json!({})).await
    }

    #[tool(description = "Create an empty MIDI item on a track between two project times.")]
    async fn create_midi_item(&self, Parameters(args): Parameters<CreateMidiItemArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "create_midi_item", to_args(&args)).await
    }

    #[tool(description = "Write automation points to a track envelope (Volume with value_db, Pan, or an FX parameter with fx). Replaces the existing points unless clear is false. Use set_envelope_active to switch the envelope on or off.")]
    async fn set_envelope_points(&self, Parameters(args): Parameters<SetEnvelopePointsArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "set_envelope_points", to_args(&args)).await
    }

    #[tool(description = "The whole project in one call: tracks with folder structure, fader, pan, mute, FX, sends and item counts, plus the master FX. Compare two snapshots to see what changed.")]
    async fn project_snapshot(&self) -> CallToolResult {
        bridge_tool(&self.bridge, "project_snapshot", json!({})).await
    }

    #[tool(description = "Plugins used in the project (instances, first tracks) with the VST3 version found on disk. REAPER keeps old DLLs loaded after an update, so restart it when the version on disk is newer than expected.")]
    async fn list_plugins(&self, Parameters(args): Parameters<ListPluginsArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "list_plugins", to_args(&args)).await
    }

    #[tool(description = "Render the same tracks as stems several times with different tracks muted or unmuted per variant (for example live only, trigger only, both). Saves and reopens the project before each render and restores the original mutes at the end.")]
    async fn render_variants(&self, Parameters(args): Parameters<RenderVariantsArgs>) -> CallToolResult {
        let timeout = Duration::from_secs(args.timeout_seconds.unwrap_or(3600));
        bridge_tool_timeout(&self.bridge, "render_variants", to_args(&args), timeout).await
    }

    #[tool(description = "Run several tools in one call: calls is a list of {tool, args}. Stops at the first error unless continue_on_error is true. Each call has its own undo step.")]
    async fn batch(&self, Parameters(args): Parameters<BatchArgs>) -> CallToolResult {
        let timeout = Duration::from_secs(args.timeout_seconds.unwrap_or(3600));
        bridge_tool_timeout(&self.bridge, "batch", to_args(&args), timeout).await
    }

    #[tool(description = "Put an audio file on a track as a new item at a position in seconds.")]
    async fn insert_media(&self, Parameters(args): Parameters<InsertMediaArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "insert_media", to_args(&args)).await
    }

    #[tool(description = "Analyse a rendered WAV file: integrated loudness (LUFS), loudness range, sample peak, samples above full scale (overloads), crest factor, stereo width and the spectrum in third-octave and coarse mixing bands. Reads 32-bit float files without cutting off levels above 0 dBFS. Warns about overloads and missing headroom.")]
    async fn analyze_audio(&self, Parameters(args): Parameters<AnalyzeAudioArgs>) -> CallToolResult {
        run_blocking(move || {
            let audio = audio::read(std::path::Path::new(&args.path), args.start_seconds, args.end_seconds)?;
            Ok(audio::analyze(&audio))
        })
        .await
    }

    #[tool(description = "Frequency and level comparison of two WAV files, for example your render against a professional reference. File B is turned to the loudness of A, so the per-band differences (third-octave and coarse mixing bands) describe tone, not volume; positive means B has more energy there. Also loudness, crest, width and overload differences. Use a_start_seconds and b_start_seconds to line up files that start at different times.")]
    async fn compare_audio(&self, Parameters(args): Parameters<CompareAudioArgs>) -> CallToolResult {
        run_blocking(move || {
            let (a_start, b_start) = (args.a_start_seconds.unwrap_or(0.0), args.b_start_seconds.unwrap_or(0.0));
            let end = |start: f64| args.duration_seconds.map(|d| start + d);
            let a = audio::read(std::path::Path::new(&args.a), Some(a_start), end(a_start))?;
            let b = audio::read(std::path::Path::new(&args.b), Some(b_start), end(b_start))?;
            // Compare the same length when no duration was given.
            let frames = a.frames().min(b.frames());
            let trim = |mut x: audio::Audio| {
                for channel in &mut x.channels {
                    channel.truncate(frames);
                }
                x
            };
            Ok(audio::compare(&trim(a), &trim(b)))
        })
        .await
    }

    #[tool(description = "Everything that sets the level and sound of one track: fader, pan, whole-chain FX enable state, FX chain with pin mappings, automation envelopes (volume, pan, mute, plugin parameters), sends, receives, parent. Notes call out what acts after the plugins, such as an active volume envelope that shifts a limiter's ceiling.")]
    async fn describe_track(&self, Parameters(args): Parameters<TrackRef>) -> CallToolResult {
        bridge_tool_timeout(&self.bridge, "describe_track", to_args(&args), LONG_TIMEOUT).await
    }

    #[tool(description = "List automation envelopes of a track: volume, trim, pan, width and mute envelopes and plugin parameter envelopes, with active state, point count and value range (dB for volume).")]
    async fn list_track_envelopes(&self, Parameters(args): Parameters<ListTrackEnvelopesArgs>) -> CallToolResult {
        bridge_tool_timeout(&self.bridge, "list_track_envelopes", to_args(&args), LONG_TIMEOUT).await
    }

    #[tool(description = "Switch an automation envelope on or off without deleting its points, to check what it does to the level.")]
    async fn set_envelope_active(&self, Parameters(args): Parameters<SetEnvelopeActiveArgs>) -> CallToolResult {
        bridge_tool(&self.bridge, "set_envelope_active", to_args(&args)).await
    }

    #[tool(description = "Run a REAPER action by numeric ID or named command. Escape hatch for features without a dedicated tool; prefer adding a real tool.")]
    async fn run_action(&self, Parameters(args): Parameters<RunActionArgs>) -> CallToolResult {
        bridge_tool_timeout(&self.bridge, "run_action", to_args(&args), LONG_TIMEOUT).await
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let service = ReaperMcp {
        bridge: BridgeClient::new()?,
    }
    .serve(stdio())
    .await
    .context("cannot start MCP stdio server")?;
    service.waiting().await.context("MCP server stopped")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::AudioDeviceArgs;

    #[test]
    fn audio_device_action_schema_accepts_only_supported_values() {
        for action in ["status", "close", "open"] {
            let args = serde_json::from_str::<AudioDeviceArgs>(&format!("{{\"action\":\"{action}\"}}"))
                .unwrap();
            assert_eq!(serde_json::to_value(args).unwrap()["action"], action);
        }
        assert!(serde_json::from_str::<AudioDeviceArgs>(r#"{"action":"restart"}"#).is_err());
    }
}
