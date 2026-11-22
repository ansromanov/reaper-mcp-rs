//! Runs the real Lua bridge and handlers against an in-memory fake of REAPER's API
//! (`tests/lua/fake_reaper.lua`) and talks to it through `BridgeClient`, the same IPC the MCP
//! server uses. Needs a Lua 5.4 interpreter (`lua5.4` or `lua`); REAPER embeds Lua 5.4.
#![cfg(unix)]

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use reaper_mcp_rs::bridge::BridgeClient;
use serde_json::{json, Value};

const TIMEOUT: Duration = Duration::from_secs(5);

struct FakeReaper {
    child: Child,
    dir: PathBuf,
    client: BridgeClient,
}

impl FakeReaper {
    /// Starts the bridge, or returns None when no Lua interpreter is installed. CI sets
    /// `REAPER_MCP_REQUIRE_LUA` so that a missing interpreter fails instead of skipping.
    fn start() -> Option<Self> {
        let Some(lua) = find_lua() else {
            assert!(
                std::env::var_os("REAPER_MCP_REQUIRE_LUA").is_none(),
                "REAPER_MCP_REQUIRE_LUA is set but no Lua 5.4 interpreter was found"
            );
            eprintln!("skipping: no Lua 5.4 interpreter found");
            return None;
        };
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let dir = std::env::temp_dir().join(format!("reaper-mcp-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let child = Command::new(lua)
            .arg(root.join("tests/lua/fake_reaper.lua"))
            .arg(root.join("lua/reaper_mcp_bridge.lua"))
            .arg("60")
            .env("REAPER_MCP_DIR", &dir)
            .stdin(Stdio::null())
            .spawn()
            .expect("cannot start the Lua interpreter");
        let client = BridgeClient::with_dir(&dir).unwrap();
        let fake = Self { child, dir, client };
        fake.wait_until_connected();
        Some(fake)
    }

    fn wait_until_connected(&self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if self.client.health()["connected"] == json!(true) {
                return;
            }
            thread::sleep(Duration::from_millis(50));
        }
        panic!("the fake bridge did not start: {}", self.client.health());
    }

    fn call(&self, command: &str, args: Value) -> anyhow::Result<Value> {
        self.client.call(command, args, TIMEOUT)
    }
}

impl Drop for FakeReaper {
    fn drop(&mut self) {
        let _ = fs::write(self.dir.join("stop"), "");
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.child.try_wait() {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn find_lua() -> Option<&'static str> {
    ["lua5.4", "lua"].into_iter().find(|candidate| {
        Command::new(candidate)
            .arg("-v")
            .output()
            .is_ok_and(|out| String::from_utf8_lossy(&out.stdout).contains("Lua 5.4"))
    })
}

#[test]
fn tracks_round_trip_through_the_bridge() {
    let Some(reaper) = FakeReaper::start() else {
        return;
    };

    let created = reaper
        .call("create_track", json!({"name": "Bass"}))
        .unwrap();
    assert_eq!(created["index"], 0);
    assert_eq!(created["name"], "Bass");
    reaper
        .call("create_track", json!({"name": "Drums"}))
        .unwrap();

    let renamed = reaper
        .call("rename_track", json!({"index": 1, "name": "Kit"}))
        .unwrap();
    assert_eq!(renamed["old_name"], "Drums");
    assert_eq!(renamed["name"], "Kit");

    let set = reaper
        .call(
            "set_track",
            json!({"track": 0, "volume_db": -6.0, "pan": -0.5, "mute": true}),
        )
        .unwrap();
    assert!((set["volume_db"].as_f64().unwrap() + 6.0).abs() < 1e-9);
    assert_eq!(set["pan"], -0.5);
    assert_eq!(set["mute"], true);

    let tracks = reaper.call("list_tracks", json!({})).unwrap();
    let names: Vec<_> = tracks
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Bass", "Kit"]);

    let deleted = reaper.call("delete_track", json!({"track": 0})).unwrap();
    assert_eq!(deleted["deleted"], json!(["Bass"]));
    assert_eq!(deleted["track_count"], 1);
}

#[test]
fn markers_keep_positions_and_colors() {
    let Some(reaper) = FakeReaper::start() else {
        return;
    };

    let added = reaper
        .call(
            "add_markers",
            json!({"markers": [
                {"name": "Chorus", "position_seconds": 30.0, "end_seconds": 45.0, "color": "#ff8800"},
                {"name": "Intro", "position_seconds": 0.0},
            ]}),
        )
        .unwrap();
    assert_eq!(added["added"], 2);

    let markers = reaper.call("list_markers", json!({})).unwrap();
    assert_eq!(markers[0]["name"], "Intro");
    assert_eq!(markers[1]["name"], "Chorus");
    assert_eq!(markers[1]["is_region"], true);
    assert_eq!(markers[1]["region_end"], 45.0);
    // ColorToNative(0xff, 0x88, 0x00) plus REAPER's "custom color" flag.
    assert_eq!(markers[1]["color"], 0xff | (0x88 << 8) | 0x0100_0000);

    let cleared = reaper
        .call(
            "add_markers",
            json!({"clear": true, "markers": [{"name": "Outro", "position_seconds": 90.0}]}),
        )
        .unwrap();
    assert_eq!(cleared["total"], 1);
}

#[test]
fn errors_come_back_as_messages() {
    let Some(reaper) = FakeReaper::start() else {
        return;
    };

    let error = reaper
        .call("create_track", json!({"name": "   "}))
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("name must contain 1 to 255 bytes"));

    let error = reaper
        .call("rename_track", json!({"index": 3, "name": "x"}))
        .unwrap_err();
    assert!(error.to_string().contains("track index is out of range"));

    let error = reaper.call("no_such_command", json!({})).unwrap_err();
    assert!(error
        .to_string()
        .contains("unknown command: no_such_command"));

    let error = reaper
        .call(
            "add_markers",
            json!({"markers": [{"name": "x", "position_seconds": 1.0, "color": "orange"}]}),
        )
        .unwrap_err();
    assert!(error.to_string().contains("color must be a hex string"));
}

#[test]
fn reload_rereads_the_handlers() {
    let Some(reaper) = FakeReaper::start() else {
        return;
    };
    let reloaded = reaper.call("reload", json!({})).unwrap();
    assert_eq!(reloaded["reloaded"], true);
    reaper
        .call("create_track", json!({"name": "After reload"}))
        .unwrap();
}

#[test]
fn a_stopped_bridge_is_reported() {
    let dir = std::env::temp_dir().join(format!("reaper-mcp-test-{}", uuid::Uuid::new_v4()));
    let client = BridgeClient::with_dir(&dir).unwrap();
    let health = client.health();
    assert_eq!(health["connected"], false);
    assert!(health["error"]
        .as_str()
        .unwrap()
        .contains("heartbeat missing"));
    let _ = fs::remove_dir_all(&dir);
}
