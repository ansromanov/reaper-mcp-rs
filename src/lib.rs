//! MCP server for REAPER. The server talks to a Lua bridge inside REAPER through files in an IPC
//! directory; see `bridge`.

mod args;
pub mod audio;
pub mod bridge;
mod reaper_host;
pub mod server;
pub mod track_templates;
