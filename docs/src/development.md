# Development

## Layout

| Path | What |
|---|---|
| `src/main.rs` | Binary: flags, then the MCP server on stdio. |
| `src/server.rs` | The MCP tools. Most forward their arguments to a Lua handler. |
| `src/args.rs` | Tool arguments; their doc comments become the JSON schema descriptions. |
| `src/bridge.rs` | File-based IPC client for the Lua bridge. |
| `src/track_templates.rs` | Track template and FX chain manipulation on REAPER state chunks. |
| `src/audio.rs` | WAV reading, loudness (BS.1770), spectrum and comparison. |
| `src/reaper_host.rs` | Windows-only helpers that work without the bridge. |
| `lua/reaper_mcp_bridge.lua` | The loop inside REAPER: reads requests, runs handlers, writes responses and the heartbeat. |
| `lua/reaper_mcp_handlers.lua` | One handler per command; reloadable with `reload_bridge`. |
| `tests/lua/fake_reaper.lua` | In-memory fake of the ReaScript API for tests. |
| `docs/` | This book (mdBook). |

## Build and check

```sh
cargo build
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

CI runs the same on Linux, macOS and Windows, plus a Lua syntax check and ShellCheck. A pull
request can be merged only when CI passes.

## Tests

- **Unit tests** next to the code: state chunk edits, argument schemas, JSON rounding.
- **`tests/bridge_ipc.rs`** runs the real bridge and handlers in a Lua 5.4 interpreter against
  `tests/lua/fake_reaper.lua`, and talks to them through `BridgeClient`, the IPC the server uses.
  Install `lua5.4` (or a `lua` that is 5.4) to run them; without one they are skipped locally,
  and CI requires them (`REAPER_MCP_REQUIRE_LUA=1`). The fake raises an error naming any
  ReaScript function it does not implement, so extend it when a test needs more.
- **`tests/audio_analysis.rs`** checks the analysis against generated sines with known levels.
- **Consistency checks** in `src/server.rs`: every tool has a unique name, a description and an
  object schema; every command the server sends has a Lua handler; `docs/src/reference.md` matches
  the tool definitions.

## Adding a tool

1. Add the arguments to `src/args.rs` with a doc comment on every field.
2. Add the tool to `src/server.rs` with a description that says what it changes and what it
   returns.
3. Add the handler to `lua/reaper_mcp_handlers.lua` as `handlers.<name> = function(args)`.
   Validate arguments first, then change the project inside `undo("…", function() … end)`.
4. Add a test in `tests/bridge_ipc.rs` and the ReaScript functions it needs to the fake.
5. Regenerate the reference: `REAPER_MCP_UPDATE_DOCS=1 cargo test`.
6. If the tool closes a gap, update [REAPER API coverage](api-coverage.md) and the
   [roadmap](roadmap.md).

To try a handler change in a running REAPER without restarting it, copy the Lua files with the
install script and call `reload_bridge`.

## Documentation

```sh
cargo install mdbook
mdbook serve docs
```

The book is published to GitHub Pages when `docs/` changes on `main`.

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org): `feat:` and `fix:` appear in the
changelog and choose the next version; `docs:`, `test:`, `refactor:`, `ci:` and `chore:` do not.
See [Releases](releases.md).
