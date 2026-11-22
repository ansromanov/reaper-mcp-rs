# Installation

You need a current REAPER and an MCP client. The bridge is a Lua ReaScript, tested with Lua 5.4,
the version current REAPER releases embed.
Installing puts two things in place: the server binary and the Lua bridge in REAPER's `Scripts`
folder.

## One-line installer

**Linux and macOS:**

```sh
curl -fsSL https://raw.githubusercontent.com/ansromanov/reaper-mcp-rs/main/install.sh | sh
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/ansromanov/reaper-mcp-rs/main/install.ps1 | iex
```

The installer downloads the latest release for your platform, checks its SHA-256 checksum,
installs the server and the bridge, and prints the command that registers the server with your
MCP client.

| Platform | Server binary | Bridge |
|---|---|---|
| Linux | `~/.local/bin/reaper-mcp-rs` | `~/.config/REAPER/Scripts/reaper-mcp/` |
| macOS | `~/.local/bin/reaper-mcp-rs` | `~/Library/Application Support/REAPER/Scripts/reaper-mcp/` |
| Windows | `%LOCALAPPDATA%\Programs\reaper-mcp-rs\reaper-mcp-rs.exe` | `%APPDATA%\REAPER\Scripts\reaper-mcp\` |

Environment variables change what the installer does:

| Variable | Effect |
|---|---|
| `REAPER_MCP_STARTUP=1` | Start the bridge with REAPER (adds one line to `Scripts/__startup.lua`). |
| `REAPER_MCP_VERSION=v0.2.0` | Install that release instead of the latest. |
| `REAPER_MCP_BIN_DIR=…` | Put the server binary somewhere else. |
| `REAPER_RESOURCE_PATH=…` | REAPER resource directory, for a portable REAPER install. |

For example, to install and start the bridge with REAPER on Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/ansromanov/reaper-mcp-rs/main/install.sh | REAPER_MCP_STARTUP=1 sh
```

## Start the bridge in REAPER

If you did not choose to start it with REAPER, start it once per REAPER session:
**Actions > Show action list > New action > Load ReaScript**, choose
`Scripts/reaper-mcp/reaper_mcp_bridge.lua` and run it. Keep REAPER open.

Starting the bridge again replaces the running copy, and the `reload_bridge` tool re-reads the
handlers after an update without restarting REAPER.

## From a release archive

Every [release](https://github.com/ansromanov/reaper-mcp-rs/releases) has an archive per
platform with the binary, the bridge (`lua/`), the install scripts and `SHA256SUMS`. Unpack it
and run:

- Linux, macOS: `scripts/install-bridge.sh [--startup] [REAPER resource directory]`
- Windows: `scripts\install-bridge.ps1 [-Startup] [-ResourceDir <dir>]`

Then put the binary anywhere and point your MCP client at it.

## From source

```sh
git clone https://github.com/ansromanov/reaper-mcp-rs
cd reaper-mcp-rs
cargo build --release
scripts/install-bridge.sh            # Windows: scripts\install-bridge.ps1
```

The binary is `target/release/reaper-mcp-rs`. Building needs a stable Rust toolchain.

## Check the installation

```sh
reaper-mcp-rs --version
```

Then [connect an MCP client](clients.md) and call the `health` tool: it reports whether the
bridge heartbeat is fresh and which IPC directory the server uses.

## Updating and removing

Run the installer again to update; restart the bridge (or call `reload_bridge`) afterwards. To
remove, delete the server binary, the `Scripts/reaper-mcp` folder and, if you added it, the
bridge line in `Scripts/__startup.lua`.
