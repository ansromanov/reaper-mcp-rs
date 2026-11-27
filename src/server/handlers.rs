//! Tool bodies that do more than forward their arguments to the Lua bridge.

use anyhow::{anyhow, Context, Result};
use rmcp::model::CallToolResult;
use serde_json::{json, Value};

use super::{round_json, run_blocking, tool_result, ReaperMcp};
use crate::{args::*, audio, bridge::LONG_TIMEOUT, track_templates};

impl ReaperMcp {
    pub(super) async fn apply_fx_template(&self, args: ApplyTrackFxTemplateArgs) -> CallToolResult {
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
                .ok_or_else(|| {
                    anyhow!("REAPER did not return the destination track state chunk")
                })?;
            let replacement = match mode.as_str() {
                "append" => {
                    track_templates::append_track_fx_chain(current_chunk, source_chain.as_deref())
                }
                _ => {
                    track_templates::replace_track_fx_chain(current_chunk, source_chain.as_deref())
                }
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

    pub(super) async fn probe_outputs(&self, args: ProbeInstrumentOutputsArgs) -> CallToolResult {
        let bridge = self.bridge.clone();
        let result = tokio::task::spawn_blocking(move || -> Result<Value> {
            let mut probe =
                bridge.call("probe_instrument_outputs", to_args(&args), LONG_TIMEOUT)?;
            if let Some(pairs) = probe.get_mut("pairs").and_then(Value::as_array_mut) {
                for pair in pairs.iter_mut() {
                    let Some(path) = pair.get("file").and_then(Value::as_str).map(str::to_owned)
                    else {
                        continue;
                    };
                    match audio::read(std::path::Path::new(&path), None, None) {
                        Ok(audio) => {
                            let analysis = audio::analyze(&audio);
                            let silent = analysis.peak_dbfs < -90.0;
                            let loudest = analysis
                                .bands
                                .iter()
                                .max_by(|a, b| {
                                    a.db.partial_cmp(&b.db).unwrap_or(std::cmp::Ordering::Equal)
                                })
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

    pub(super) async fn analyze(&self, args: AnalyzeAudioArgs) -> CallToolResult {
        run_blocking(move || {
            let audio = audio::read(
                std::path::Path::new(&args.path),
                args.start_seconds,
                args.end_seconds,
            )?;
            Ok(audio::analyze(&audio))
        })
        .await
    }

    pub(super) async fn compare(&self, args: CompareAudioArgs) -> CallToolResult {
        run_blocking(move || {
            let (a_start, b_start) = (
                args.a_start_seconds.unwrap_or(0.0),
                args.b_start_seconds.unwrap_or(0.0),
            );
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
}
