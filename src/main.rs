use anyhow::{Context, Result};
use reaper_mcp_rs::{bridge::BridgeClient, server::ReaperMcp};
use rmcp::{transport::stdio, ServiceExt};

#[tokio::main]
async fn main() -> Result<()> {
    let service = ReaperMcp::new(BridgeClient::new()?)
        .serve(stdio())
        .await
        .context("cannot start MCP stdio server")?;
    service.waiting().await.context("MCP server stopped")?;
    Ok(())
}
