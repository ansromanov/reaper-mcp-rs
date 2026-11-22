use anyhow::{Context, Result};
use reaper_mcp_rs::{bridge::BridgeClient, server::ReaperMcp};
use rmcp::{transport::stdio, ServiceExt};

const USAGE: &str = "\
reaper-mcp-rs: MCP server for REAPER over stdio.

Start it from an MCP client, for example:
  claude mcp add reaper -- reaper-mcp-rs

The Lua bridge must be running inside REAPER. Set REAPER_MCP_DIR in both REAPER and
the server to move the IPC directory. Documentation:
  https://ansromanov.github.io/reaper-mcp-rs/

Options:
  -h, --help     print this help
  -V, --version  print the version
";

#[tokio::main]
async fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("-V" | "--version") => {
            println!("reaper-mcp-rs {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("-h" | "--help") => {
            print!("{USAGE}");
            return Ok(());
        }
        Some(other) => anyhow::bail!("unknown argument {other}; see --help"),
        None => {}
    }
    let service = ReaperMcp::new(BridgeClient::new()?)
        .serve(stdio())
        .await
        .context("cannot start MCP stdio server")?;
    service.waiting().await.context("MCP server stopped")?;
    Ok(())
}
