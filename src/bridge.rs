//! File-based IPC with the Lua bridge running inside REAPER.
//!
//! The server writes `request.json` atomically, the bridge answers in `response.json`, and a
//! `heartbeat` file tells whether the bridge is alive.

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
use rmcp::model::CallToolResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::server::tool_result;

pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const HEARTBEAT_GRACE: Duration = Duration::from_secs(5);
pub(crate) const LONG_TIMEOUT: Duration = Duration::from_secs(300);
pub(crate) const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct BridgeClient {
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
    /// Uses `REAPER_MCP_DIR`, or the platform REAPER resource directory plus `reaper-mcp-bridge`.
    pub fn new() -> Result<Self> {
        let dir = match env::var_os("REAPER_MCP_DIR") {
            Some(path) => PathBuf::from(path),
            None => {
                let base = BaseDirs::new().context("cannot locate user configuration directory")?;
                base.config_dir().join("REAPER").join("reaper-mcp-bridge")
            }
        };
        Self::with_dir(dir)
    }

    /// Uses the given IPC directory, creating it when missing.
    pub fn with_dir(dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = dir.into();
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

    /// Sends one command and waits up to `timeout` for its response.
    pub fn call(&self, command: &str, args: Value, timeout: Duration) -> Result<Value> {
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

    /// Reports whether the bridge heartbeat is fresh.
    pub fn health(&self) -> Value {
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

pub(crate) async fn bridge_tool(
    bridge: &BridgeClient,
    command: &'static str,
    args: Value,
) -> CallToolResult {
    bridge_tool_timeout(bridge, command, args, REQUEST_TIMEOUT).await
}

pub(crate) async fn bridge_tool_timeout(
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
