//! Arguments of the MCP tools, with the JSON schemas clients see.

use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

mod fx;
mod items;
mod project;
mod render;
mod tracks;

pub(crate) use fx::*;
pub(crate) use items::*;
pub(crate) use project::*;
pub(crate) use render::*;
pub(crate) use tracks::*;

pub(crate) fn to_args<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or_else(|_| json!({}))
}
