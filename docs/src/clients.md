# Connecting an MCP client

reaper-mcp-rs is a stdio MCP server: the client starts the binary and talks to it over stdin and
stdout. It takes no arguments. Use the full path printed by the installer.

## Claude Code

```sh
claude mcp add reaper -- ~/.local/bin/reaper-mcp-rs
```

On Windows:

```powershell
claude mcp add reaper -- "$env:LOCALAPPDATA\Programs\reaper-mcp-rs\reaper-mcp-rs.exe"
```

Add `--scope user` to make it available in every project.

## Claude Desktop

Open **Settings > Developer > Edit Config** and add the server to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "reaper": {
      "command": "/home/you/.local/bin/reaper-mcp-rs"
    }
  }
}
```

On Windows, escape the backslashes:
`"command": "C:\\Users\\you\\AppData\\Local\\Programs\\reaper-mcp-rs\\reaper-mcp-rs.exe"`.
Restart Claude Desktop afterwards.

## Other clients

Any client that runs stdio MCP servers works. The usual configuration is the same
`mcpServers` entry as above: a name, the command, and optionally environment variables.

## A different IPC directory

The server and the bridge find each other through an IPC directory, by default
`<REAPER resource directory>/reaper-mcp-bridge`. To move it, set `REAPER_MCP_DIR` to the same
path for both REAPER and the server:

```json
{
  "mcpServers": {
    "reaper": {
      "command": "/home/you/.local/bin/reaper-mcp-rs",
      "env": { "REAPER_MCP_DIR": "/tmp/reaper-mcp" }
    }
  }
}
```

See [Configuration and IPC](configuration.md).
