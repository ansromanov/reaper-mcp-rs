use std::collections::BTreeSet;

use serde_json::json;

use super::*;

#[test]
fn round_json_rounds_floats_and_drops_non_finite_values() {
    let value = json!({"a": 1.2345, "b": [2, -0.06], "c": "x", "d": 7});
    assert_eq!(
        round_json(value, 1),
        json!({"a": 1.2, "b": [2, -0.1], "c": "x", "d": 7})
    );
    assert_eq!(
        round_json(serde_json::to_value(f64::NAN).unwrap(), 1),
        json!(null)
    );
}

#[test]
fn every_tool_has_a_unique_name_a_description_and_an_object_schema() {
    let tools = ReaperMcp::tool_router().list_all();
    assert!(tools.len() > 70, "only {} tools registered", tools.len());
    let mut names = BTreeSet::new();
    for tool in &tools {
        assert!(
            names.insert(tool.name.to_string()),
            "duplicate tool {}",
            tool.name
        );
        assert!(
            tool.description.as_deref().is_some_and(|d| d.len() > 20),
            "tool {} needs a description",
            tool.name
        );
        assert_eq!(
            tool.input_schema.get("type"),
            Some(&json!("object")),
            "tool {} input schema",
            tool.name
        );
    }
}

/// `docs/src/reference.md` lists every tool with its arguments. It is generated from the tool
/// definitions; run `REAPER_MCP_UPDATE_DOCS=1 cargo test` to rewrite it after changing a tool.
#[test]
fn tool_reference_is_up_to_date() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/docs/src/reference.md");
    let generated = tool_reference();
    if std::env::var_os("REAPER_MCP_UPDATE_DOCS").is_some() {
        std::fs::write(path, &generated).unwrap();
        return;
    }
    // A Windows checkout may turn the file's line endings into CRLF.
    let current = std::fs::read_to_string(path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        current == generated,
        "docs/src/reference.md is out of date; run REAPER_MCP_UPDATE_DOCS=1 cargo test"
    );
}

fn tool_reference() -> String {
    let mut tools = ReaperMcp::tool_router().list_all();
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    let mut out = format!(
        "# Tool reference\n\n\
         All {} tools with their arguments, generated from the tool definitions. To change a\n\
         description, edit `src/server/mod.rs` or `src/args/` and run\n\
         `REAPER_MCP_UPDATE_DOCS=1 cargo test`.\n\n\
         Track indexes start at 0; `-1` is the master track.\n",
        tools.len()
    );
    for tool in &tools {
        out.push_str(&format!("\n## `{}`\n\n", tool.name));
        out.push_str(&escape_html(
            tool.description.as_deref().unwrap_or("").trim(),
        ));
        out.push('\n');
        let schema = &tool.input_schema;
        let required: BTreeSet<&str> = schema
            .get("required")
            .and_then(|r| r.as_array())
            .map(|r| r.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();
        let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) else {
            out.push_str("\nNo arguments.\n");
            continue;
        };
        if properties.is_empty() {
            out.push_str("\nNo arguments.\n");
            continue;
        }
        out.push_str("\n| Argument | Type | Description |\n|---|---|---|\n");
        for (name, property) in properties {
            let mark = if required.contains(name.as_str()) {
                " (required)"
            } else {
                ""
            };
            let description = property
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .replace('\n', " ")
                .replace('|', "\\|");
            let description = escape_html(&description);
            out.push_str(&format!(
                "| `{name}`{mark} | {} | {description} |\n",
                type_name(property)
            ));
        }
    }
    out
}

/// Keeps placeholders such as `<ext>` visible in the rendered book.
fn escape_html(text: &str) -> String {
    text.replace('<', "&lt;").replace('>', "&gt;")
}

fn type_name(property: &serde_json::Value) -> String {
    if let Some(kind) = property.get("type") {
        return match kind {
            serde_json::Value::Array(kinds) => kinds
                .iter()
                .filter_map(|k| k.as_str())
                .filter(|k| *k != "null")
                .collect::<Vec<_>>()
                .join(" or "),
            other => other.as_str().unwrap_or("value").to_string(),
        };
    }
    if let Some(reference) = property.get("$ref").and_then(|r| r.as_str()) {
        return reference.rsplit('/').next().unwrap_or("object").to_string();
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(options) = property.get(key).and_then(|o| o.as_array()) {
            return options
                .iter()
                .map(type_name)
                .filter(|t| t != "null")
                .collect::<Vec<_>>()
                .join(" or ");
        }
    }
    "value".to_string()
}

/// Every command the server sends must have a handler in the Lua bridge.
#[test]
fn every_bridge_command_has_a_lua_handler() {
    let server = include_str!("mod.rs");
    let lua = [
        include_str!("../../lua/handlers/support.lua"),
        include_str!("../../lua/handlers/project.lua"),
        include_str!("../../lua/handlers/tracks.lua"),
        include_str!("../../lua/handlers/items.lua"),
        include_str!("../../lua/handlers/fx.lua"),
        include_str!("../../lua/handlers/envelopes.lua"),
        include_str!("../../lua/handlers/render.lua"),
    ]
    .join("\n");
    let handlers: BTreeSet<&str> = lua
        .lines()
        .filter_map(|line| {
            let name = line
                .strip_prefix("handlers.")
                .or_else(|| line.strip_prefix("  "))?;
            let (name, rest) = name.split_once(" = ")?;
            (rest.starts_with("function")
                && name.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
            .then_some(name)
        })
        .collect();
    let mut missing = Vec::new();
    for marker in ["bridge_tool(", "bridge_tool_timeout("] {
        for chunk in server.split(marker).skip(1) {
            let Some(command) = chunk.split('"').nth(1) else {
                continue;
            };
            // Only literal command names, not the helper definitions.
            if !command.is_empty()
                && command.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                && command != "reload"
                && !handlers.contains(command)
            {
                missing.push(command.to_string());
            }
        }
    }
    assert!(
        missing.is_empty(),
        "commands without a Lua handler: {missing:?}"
    );
}
