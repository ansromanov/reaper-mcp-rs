# Security

reaper-mcp-rs gives an AI agent real control over REAPER. Treat connecting it like giving someone
your keyboard while REAPER is in front.

## What the agent can do

- Change any open project: tracks, routing, FX, items, automation. Edits run inside undo blocks,
  so they can be undone, but an agent can make many changes quickly.
- Open, save, render and close projects, and write files where REAPER can (renders, templates,
  FX chains).
- Read files it is given a path to (`analyze_audio`, `insert_media`, `import_fx_chain`).
- Run any REAPER action with `run_action`, including actions from extensions and custom scripts
  you have installed.

## What it cannot do

- Run code it sends: the bridge dispatches a fixed set of handlers and never evaluates Lua from the
  client.
- Reach the network: the server talks only to its MCP client over stdio and to the bridge through
  local files.

## Recommendations

- Back up projects before letting an agent work on them, or work on a copy.
- Connect the server only to MCP clients and agents you trust, and keep tool approval on in the
  client for project-changing tools if you are unsure.
- The IPC directory is a plain local directory: any program running as your user can write
  requests to it. Do not point `REAPER_MCP_DIR` at a shared or network location.

## Reporting a vulnerability

Please report security problems privately through
[GitHub security advisories](https://github.com/ansromanov/reaper-mcp-rs/security/advisories/new)
rather than in a public issue.
