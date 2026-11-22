//! Safe, balanced parsing and replacement of track chunks from REAPER templates.

use std::{fs, path::Path};

#[derive(Clone, Debug)]
struct Block {
    tag: String,
    start: usize,
    end: usize,
    depth: usize,
}

fn scan_blocks(input: &str) -> Result<Vec<Block>, String> {
    let mut blocks = Vec::new();
    let mut stack: Vec<(String, usize)> = Vec::new();
    let mut offset = 0;

    for raw_line in input.split_inclusive('\n') {
        let line = raw_line.trim();
        if line == ">" {
            let (tag, start) = stack
                .pop()
                .ok_or_else(|| "unexpected closing chunk delimiter".to_string())?;
            blocks.push(Block {
                tag,
                start,
                end: offset + raw_line.len(),
                depth: stack.len(),
            });
        } else if let Some(open) = line.strip_prefix('<') {
            let tag = open
                .split(|ch: char| ch.is_whitespace() || ch == '>')
                .next()
                .unwrap_or_default();
            if tag.is_empty() {
                return Err("empty chunk tag".to_string());
            }
            stack.push((tag.to_string(), offset));
        }
        offset += raw_line.len();
    }

    if !stack.is_empty() {
        return Err(format!("unclosed {} chunk", stack.last().unwrap().0));
    }
    blocks.sort_by_key(|block| block.start);
    Ok(blocks)
}

fn top_level_tracks(template: &str) -> Result<Vec<Block>, String> {
    let tracks: Vec<_> = scan_blocks(template)?
        .into_iter()
        .filter(|block| block.depth == 0 && block.tag == "TRACK")
        .collect();
    if tracks.is_empty() {
        return Err("template contains no <TRACK> chunks".to_string());
    }
    Ok(tracks)
}

fn parse_track_name(line: &str) -> Option<String> {
    let field = line.trim().strip_prefix("NAME")?;
    if !field.is_empty() && !field.starts_with(char::is_whitespace) {
        return None;
    }
    let value = field.trim_start();
    if !value.starts_with('"') {
        return Some(value.to_string());
    }
    let mut result = String::new();
    let mut escaped = false;
    for ch in value[1..].chars() {
        if escaped {
            result.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(result);
        } else {
            result.push(ch);
        }
    }
    None
}

fn track_name(track: &str) -> Result<String, String> {
    let mut depth = 0usize;
    for line in track.lines() {
        let trimmed = line.trim();
        if trimmed == ">" {
            depth = depth.saturating_sub(1);
        } else if trimmed.starts_with('<') {
            depth += 1;
        } else if depth == 1
            && (trimmed == "NAME"
                || trimmed
                    .strip_prefix("NAME")
                    .is_some_and(|rest| rest.starts_with(char::is_whitespace)))
        {
            return parse_track_name(trimmed)
                .ok_or_else(|| "track has a malformed NAME field".to_string());
        }
    }
    Err("track has no direct NAME field".to_string())
}

fn named_track(template: &str, name: &str) -> Result<String, String> {
    let mut found = None;
    for block in top_level_tracks(template)? {
        let track = &template[block.start..block.end];
        if track_name(track)? == name {
            if found.is_some() {
                return Err(format!("track name '{name}' is ambiguous in the template"));
            }
            found = Some(track.to_string());
        }
    }
    found.ok_or_else(|| format!("track '{name}' was not found in the template"))
}

fn direct_children(track: &str) -> Result<Vec<Block>, String> {
    let blocks = scan_blocks(track)?;
    let root = blocks
        .iter()
        .find(|block| block.depth == 0 && matches!(block.tag.as_str(), "TRACK" | "MASTERTRACK"))
        .ok_or_else(|| "state chunk must contain one <TRACK> or <MASTERTRACK> chunk".to_string())?;
    if !track[..root.start].trim().is_empty() || !track[root.end..].trim().is_empty() {
        return Err("state chunk must contain exactly one track chunk".to_string());
    }
    Ok(blocks
        .into_iter()
        .filter(|block| block.depth == 1)
        .collect())
}

fn unique_fx_ids(chain: &str) -> String {
    chain
        .split_inclusive('\n')
        .map(|line| {
            let trimmed = line.trim_start();
            if let Some(guid) = trimmed.strip_prefix("FXID ") {
                let guid = guid.trim();
                if guid.starts_with('{') && guid.ends_with('}') {
                    let indent = &line[..line.len() - trimmed.len()];
                    let newline = if line.ends_with("\r\n") {
                        "\r\n"
                    } else if line.ends_with('\n') {
                        "\n"
                    } else {
                        ""
                    };
                    return format!("{indent}FXID {{{}}}{newline}", uuid::Uuid::new_v4());
                }
            }
            line.to_string()
        })
        .collect()
}

/// Remove editor window/session state from copied chains so applying a static
/// template cannot reopen plugin editors or inherit their source layout.
fn normalize_fx_ui_state(chain: &str) -> String {
    let mut output = String::with_capacity(chain.len());
    let mut depth = 0usize;
    for line in chain.split_inclusive('\n') {
        let trimmed = line.trim();
        if depth == 1 {
            let key = trimmed.split_whitespace().next().unwrap_or_default();
            if matches!(
                key.to_ascii_uppercase().as_str(),
                "SHOW" | "LASTSEL" | "DOCKED" | "FLOATPOS" | "WNDRECT" | "CLOSEFLAGS"
            ) {
                continue;
            }
        }
        if trimmed.starts_with('<') {
            depth += 1;
            output.push_str(line);
            if depth == 1 {
                let indent = &line[..line.len() - line.trim_start().len()];
                let newline = if line.ends_with("\r\n") {
                    "\r\n"
                } else if line.ends_with('\n') {
                    "\n"
                } else {
                    ""
                };
                for field in ["SHOW -1", "LASTSEL 0", "DOCKED 0"] {
                    output.push_str(indent);
                    output.push_str("  ");
                    output.push_str(field);
                    output.push_str(newline);
                }
            }
        } else {
            if trimmed == ">" {
                depth = depth.saturating_sub(1);
            }
            output.push_str(line);
        }
    }
    output
}

fn relabel_chain_root(chain: &str, desired_tag: &str) -> String {
    let Some((line, rest)) = chain.split_once('\n') else {
        return chain.to_string();
    };
    let trimmed = line.trim_start();
    let current_tag = if trimmed.starts_with("<FXCHAIN") {
        "FXCHAIN"
    } else if trimmed.starts_with("<MASTERFXLIST") {
        "MASTERFXLIST"
    } else {
        return chain.to_string();
    };
    if current_tag == desired_tag {
        return chain.to_string();
    }
    let indent = &line[..line.len() - trimmed.len()];
    let suffix = trimmed.strip_prefix(&format!("<{current_tag}")).unwrap();
    format!("{indent}<{desired_tag}{suffix}\n{rest}")
}

/// Read the static FX chain for an exactly named track in a track-template file.
/// FX parameter envelopes live outside FXCHAIN and are intentionally excluded.
pub fn read_template_fx_chain(path: &Path, track_name: &str) -> Result<Option<String>, String> {
    let template = fs::read_to_string(path)
        .map_err(|error| format!("cannot read template {}: {error}", path.display()))?;
    let blocks = scan_blocks(&template)?;
    let has_track = blocks
        .iter()
        .any(|block| block.depth == 0 && block.tag == "TRACK");
    if !has_track {
        if track_name != "MASTER" {
            return Err("an .RfxChain file uses source_track 'MASTER'".to_string());
        }
        let chains: Vec<_> = blocks
            .iter()
            .filter(|block| {
                block.depth == 0 && matches!(block.tag.as_str(), "FXCHAIN" | "MASTERFXLIST")
            })
            .collect();
        if chains.len() != 1 {
            return Err("master FX template must contain one top-level FX chain".to_string());
        }
        return Ok(Some(normalize_fx_ui_state(&unique_fx_ids(
            &template[chains[0].start..chains[0].end],
        ))));
    }
    let track = named_track(&template, track_name)?;
    let children = direct_children(&track)?;
    let chains: Vec<_> = children
        .iter()
        .filter(|block| block.tag == "FXCHAIN")
        .collect();
    if chains.len() > 1 {
        return Err("source track has multiple <FXCHAIN> chunks".to_string());
    }
    Ok(chains
        .first()
        .map(|block| normalize_fx_ui_state(&unique_fx_ids(&track[block.start..block.end]))))
}

/// Replace only a destination track's FXCHAIN and FX-linked automation blocks.
/// Media items, sends, fader/pan, routing and non-FX envelopes stay byte-for-byte.
pub fn replace_track_fx_chain(
    destination: &str,
    source_chain: Option<&str>,
) -> Result<String, String> {
    let children = direct_children(destination)?;
    let blocks = scan_blocks(destination)?;
    let root = blocks
        .iter()
        .find(|block| block.depth == 0 && matches!(block.tag.as_str(), "TRACK" | "MASTERTRACK"))
        .expect("direct_children validated the root");
    let chain_tag = if root.tag == "MASTERTRACK" {
        "MASTERFXLIST"
    } else {
        "FXCHAIN"
    };
    let source_chain = source_chain.map(|chain| relabel_chain_root(chain, chain_tag));
    let old_chains: Vec<_> = children
        .iter()
        .filter(|block| block.tag == chain_tag)
        .collect();
    if old_chains.len() > 1 {
        return Err(format!("destination has multiple <{chain_tag}> chunks"));
    }
    let mut insert_before = None;
    if source_chain.is_some() && old_chains.is_empty() {
        insert_before = children
            .iter()
            .find(|block| block.tag == "ITEM")
            .map(|block| block.start);
        if insert_before.is_none() {
            let close = destination
                .rfind('>')
                .ok_or_else(|| "destination track has no closing delimiter".to_string())?;
            insert_before = Some(
                destination[..close]
                    .rfind('\n')
                    .map_or(close, |idx| idx + 1),
            );
        }
    }

    let mut result =
        String::with_capacity(destination.len() + source_chain.as_deref().map_or(0, str::len));
    let mut cursor = 0;
    let mut inserted = false;
    for block in children {
        if insert_before == Some(block.start) && !inserted {
            result.push_str(&destination[cursor..block.start]);
            result.push_str(source_chain.as_deref().unwrap());
            if !source_chain.as_deref().unwrap().ends_with('\n') {
                result.push('\n');
            }
            cursor = block.start;
            inserted = true;
        }

        if block.tag == chain_tag {
            result.push_str(&destination[cursor..block.start]);
            if let Some(chain) = source_chain.as_deref() {
                result.push_str(chain);
                if !chain.ends_with('\n') {
                    result.push('\n');
                }
                inserted = true;
            }
            cursor = block.end;
        } else if matches!(block.tag.as_str(), "PARMENV" | "PROGRAMENV") {
            result.push_str(&destination[cursor..block.start]);
            cursor = block.end;
        }
    }

    if insert_before.is_some() && !inserted {
        let position = insert_before.unwrap();
        result.push_str(&destination[cursor..position]);
        result.push_str(source_chain.as_deref().unwrap());
        if !source_chain.as_deref().unwrap().ends_with('\n') {
            result.push('\n');
        }
        cursor = position;
    }
    result.push_str(&destination[cursor..]);
    Ok(result)
}

/// Append static source FX instances after the destination chain without
/// changing existing FX state or FX parameter/program automation.
pub fn append_track_fx_chain(
    destination: &str,
    source_chain: Option<&str>,
) -> Result<String, String> {
    let children = direct_children(destination)?;
    let blocks = scan_blocks(destination)?;
    let root = blocks
        .iter()
        .find(|block| block.depth == 0 && matches!(block.tag.as_str(), "TRACK" | "MASTERTRACK"))
        .expect("direct_children validated the root");
    let chain_tag = if root.tag == "MASTERTRACK" {
        "MASTERFXLIST"
    } else {
        "FXCHAIN"
    };
    let Some(source_chain) = source_chain else {
        return Ok(destination.to_string());
    };
    let source_chain = relabel_chain_root(source_chain, chain_tag);
    let destination_chains: Vec<_> = children
        .iter()
        .filter(|block| block.tag == chain_tag)
        .collect();
    if destination_chains.len() > 1 {
        return Err(format!("destination has multiple <{chain_tag}> chunks"));
    }
    let source_blocks = scan_blocks(&source_chain)?;
    let source_root = source_blocks
        .iter()
        .find(|block| block.depth == 0 && block.tag == chain_tag)
        .ok_or_else(|| format!("source must contain one <{chain_tag}> chunk"))?;
    let Some(first_fx) = source_blocks
        .iter()
        .filter(|block| block.depth == 1)
        .min_by_key(|block| block.start)
    else {
        return Ok(destination.to_string());
    };

    let mut close_start = None;
    let mut offset = source_root.start;
    for line in source_chain[source_root.start..source_root.end].split_inclusive('\n') {
        if offset + line.len() == source_root.end && line.trim() == ">" {
            close_start = Some(offset);
        }
        offset += line.len();
    }
    let source_end =
        close_start.ok_or_else(|| "source FX chain has no closing delimiter".to_string())?;
    let instances = &source_chain[first_fx.start..source_end];
    if instances.trim().is_empty() {
        return Ok(destination.to_string());
    }

    let Some(destination_chain) = destination_chains.first().copied() else {
        let insert_at = children
            .iter()
            .find(|block| block.tag == "ITEM")
            .map(|block| block.start)
            .unwrap_or_else(|| {
                destination[..root.end]
                    .rfind('\n')
                    .map_or(root.end, |index| index + 1)
            });
        let mut result = String::with_capacity(destination.len() + source_chain.len());
        result.push_str(&destination[..insert_at]);
        if !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(&source_chain);
        if !source_chain.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(&destination[insert_at..]);
        return Ok(result);
    };

    let mut destination_close_start = None;
    let mut offset = destination_chain.start;
    for line in destination[destination_chain.start..destination_chain.end].split_inclusive('\n') {
        if offset + line.len() == destination_chain.end && line.trim() == ">" {
            destination_close_start = Some(offset);
        }
        offset += line.len();
    }
    let close_start = destination_close_start
        .ok_or_else(|| "destination FX chain has no closing delimiter".to_string())?;

    let mut result = String::with_capacity(destination.len() + instances.len());
    result.push_str(&destination[..close_start]);
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result.push_str(instances);
    if !instances.ends_with('\n') {
        result.push('\n');
    }
    result.push_str(&destination[close_start..]);
    Ok(result)
}

/// Remap retained AUXRECV source indexes to the track order in a template and
/// omit receives whose source track is outside that template.
#[cfg(test)]
fn remap_template_receives(
    track: &str,
    local_track_indices: &std::collections::HashMap<usize, usize>,
) -> Result<(String, usize), String> {
    let receives: Vec<_> = direct_children(track)?
        .into_iter()
        .filter(|block| block.tag == "AUXRECV")
        .collect();
    let mut result = String::with_capacity(track.len());
    let mut cursor = 0;
    let mut omitted = 0;
    for block in receives {
        let line_end = track[block.start..block.end]
            .find('\n')
            .map_or(block.end, |end| block.start + end);
        let opening = &track[block.start..line_end];
        let trimmed = opening.trim_start();
        let after_tag = trimmed
            .strip_prefix("<AUXRECV")
            .ok_or_else(|| "malformed AUXRECV chunk".to_string())?;
        let spacing_len = after_tag
            .find(|ch: char| !ch.is_whitespace())
            .ok_or_else(|| "AUXRECV is missing its source track index".to_string())?;
        let spacing = &after_tag[..spacing_len];
        let index_text = &after_tag[spacing_len..];
        let index_end = index_text
            .find(char::is_whitespace)
            .unwrap_or(index_text.len());
        let source_index: usize = index_text[..index_end]
            .parse()
            .map_err(|_| "AUXRECV source track index is invalid".to_string())?;
        let indent = &opening[..opening.len() - trimmed.len()];

        result.push_str(&track[cursor..block.start]);
        if let Some(local_index) = local_track_indices.get(&source_index) {
            result.push_str(indent);
            result.push_str("<AUXRECV");
            result.push_str(spacing);
            result.push_str(&local_index.to_string());
            result.push_str(&index_text[index_end..]);
            result.push_str(&track[line_end..block.end]);
        } else {
            omitted += 1;
        }
        cursor = block.end;
    }
    result.push_str(&track[cursor..]);
    Ok((result, omitted))
}

#[cfg(test)]
mod tests {
    use super::{append_track_fx_chain, read_template_fx_chain, replace_track_fx_chain};
    use std::path::Path;

    const TEMPLATE: &str = include_str!("../tests/fixtures/mix-template.RTrackTemplate");
    const DESTINATION: &str = include_str!("../tests/fixtures/destination-track.chunk");

    #[test]
    fn extracts_named_static_fx_without_timeline_envelopes_and_freshens_fxids() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/mix-template.RTrackTemplate");
        let chain = read_template_fx_chain(&path, "GUITARS").unwrap().unwrap();
        let normalized = chain.replace("\r\n", "\n");
        assert!(chain.contains("VST: Source EQ"));
        assert!(normalized.contains("SHOW -1\n"));
        assert!(normalized.contains("LASTSEL 0\n"));
        assert!(normalized.contains("DOCKED 0\n"));
        assert!(!normalized.contains("FLOATPOS"));
        assert!(!normalized.contains("closeflags"));
        assert!(!chain.contains("<PARMENV"));
        assert!(!chain.contains("11111111-1111-4111-8111-111111111111"));
        assert!(read_template_fx_chain(&path, "BASS").unwrap().is_none());
        assert!(read_template_fx_chain(&path, "").unwrap().is_none());
    }

    #[test]
    fn replaces_only_fx_and_old_fx_envelopes() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/mix-template.RTrackTemplate");
        let chain = read_template_fx_chain(&path, "GUITARS").unwrap().unwrap();
        let result = replace_track_fx_chain(DESTINATION, Some(&chain)).unwrap();
        assert!(result.contains("VST: Source EQ"));
        assert!(!result.contains("Old Compressor"));
        assert!(!result.contains("<PARMENV"));
        assert!(!result.contains("<PROGRAMENV"));
        assert!(result.contains("VOLPAN 0.75 -0.2 1"));
        assert!(result.contains("<VOLENV2"));
        assert!(result.contains("<AUXRECV"));
        assert!(result.contains("FILE \"preserve.wav\""));
    }

    #[test]
    fn appends_static_fx_while_preserving_existing_chain_and_automation() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/mix-template.RTrackTemplate");
        let chain = read_template_fx_chain(&path, "GUITARS").unwrap().unwrap();
        let result = append_track_fx_chain(DESTINATION, Some(&chain)).unwrap();
        assert!(result.contains("VST: Old Compressor"));
        assert!(result.contains("VST: Source EQ"));
        assert!(result.contains("VST: Source Saturator"));
        assert!(result.contains("<PARMENV"));
        assert!(result.contains("<PROGRAMENV"));
        assert_eq!(result.matches("<PARMENV").count(), 1);
        assert_eq!(result.matches("<PROGRAMENV").count(), 1);
        assert_eq!(result.matches("SHOW").count(), 1);
        assert!(result.contains("VOLPAN 0.75 -0.2 1"));
        assert!(result.contains("FILE \"preserve.wav\""));
        assert!(!result.contains("11111111-1111-4111-8111-111111111111"));
        assert!(!result.contains("44444444-4444-4444-8444-444444444444"));
    }

    #[test]
    fn append_installs_chain_when_destination_has_none_without_dropping_envelopes() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/mix-template.RTrackTemplate");
        let chain = read_template_fx_chain(&path, "GUITARS").unwrap().unwrap();
        let destination = "<TRACK\n  NAME \"Empty\"\n  <PARMENV\n    PARM 0 1\n  >\n>\n";
        let result = append_track_fx_chain(destination, Some(&chain)).unwrap();
        assert!(result.contains("VST: Source EQ"));
        assert!(result.contains("<PARMENV"));
        assert_eq!(result.matches("<PARMENV").count(), 1);
    }

    #[test]
    fn empty_source_chain_clears_fx_but_keeps_non_fx_track_state() {
        let result = replace_track_fx_chain(DESTINATION, None).unwrap();
        assert!(!result.contains("<FXCHAIN"));
        assert!(!result.contains("<PARMENV"));
        assert!(!result.contains("<PROGRAMENV"));
        assert!(result.contains("<VOLENV2"));
        assert!(result.contains("FILE \"preserve.wav\""));
    }

    #[test]
    fn inserts_source_chain_when_destination_has_none_and_allows_trailing_whitespace() {
        let destination = "<TRACK\n  NAME \"Empty\"\n  VOLPAN 1 0 1\n>\n\n";
        let source = "<FXCHAIN\n  SHOW 0\n>";
        let result = replace_track_fx_chain(destination, Some(source)).unwrap();
        assert!(result.contains("<FXCHAIN\n  SHOW 0\n>"));
        assert!(result.contains("VOLPAN 1 0 1"));
        assert!(result.ends_with(">\n\n"));
    }

    #[test]
    fn applies_exported_master_chain_to_master_state_chunk() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/master.RfxChain");
        let chain = read_template_fx_chain(&path, "MASTER").unwrap().unwrap();
        let destination = "<MASTERTRACK\n  VOLPAN 1 0 1\n  <MASTERFXLIST\n    SHOW 0\n  >\n>\n";
        let result = replace_track_fx_chain(destination, Some(&chain)).unwrap();
        let normalized = result.replace("\r\n", "\n");
        assert!(normalized.contains("<MASTERFXLIST\n  SHOW -1\n  LASTSEL 0\n  DOCKED 0"));
        assert!(!normalized.contains("FLOATPOS"));
        assert!(result.contains("VST: Master Limiter"));
        assert!(!result.contains("<FXCHAIN"));
        assert!(result.contains("VOLPAN 1 0 1"));
    }

    #[test]
    fn remaps_internal_receives_and_drops_external_sources() {
        let mut local = std::collections::HashMap::new();
        local.insert(4, 0);
        let (result, omitted) = super::remap_template_receives(DESTINATION, &local).unwrap();
        assert!(result.contains("<AUXRECV 0 1"));
        assert!(!result.contains("<AUXRECV 4 "));
        assert!(!result.contains("<AUXRECV 7 "));
        assert_eq!(omitted, 1);
        assert!(result.contains("FILE \"preserve.wav\""));
    }

    #[test]
    fn rejects_malformed_balanced_chunks() {
        assert!(replace_track_fx_chain("<TRACK\n NAME \"broken\"\n", None).is_err());
    }

    #[test]
    fn template_fixture_contains_the_expected_track_chunks() {
        assert!(TEMPLATE.contains("NAME \"GUITARS\""));
        assert!(TEMPLATE.contains("NAME BASS"));
    }
}
