# AGENTS.md — reaper-mcp-rs

> Canonical instructions for this repository, read by Claude Code, Codex and other
> agents. Keep it short: no file tree and no symbol index, they rot. Use `ls`, `grep`
> and the module docs instead.

reaper-mcp-rs is an MCP server that lets an AI agent work on a real REAPER project. Licence: MIT. This repository is public: nothing private
(credentials, host names, internal paths, other people's licensed material) goes in.

## Layout

- `src/` — the Rust MCP server (`server/` tool surface, `args/` argument types, `bridge.rs` IPC to REAPER, `audio.rs` analysis).
- `lua/` — the ReaScript bridge that runs inside REAPER (`reaper_mcp_bridge.lua` loop, `reaper_mcp_handlers.lua` loader, `handlers/` one file per area).
- `docs/` — the mdBook; the tool reference is generated, see below. `install.sh`, `install.ps1`, `scripts/` — installers and the release script.

## Rules worth knowing

- A new or changed tool needs a test in `tests/bridge_ipc.rs`; regenerate the reference with `REAPER_MCP_UPDATE_DOCS=1 cargo test`.
- Set `REAPER_MCP_REQUIRE_LUA=1` so the bridge tests fail instead of skipping when Lua is missing.
- Nothing private to a person, studio or plugin collection belongs in the repository.

## Gate

Run this before opening a pull request. CI runs the same checks.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets   # needs lua5.4
python3 scripts/check-file-size.py
```

## Testing

Rust tests live in `tests/`; Lua is tested through the real bridge in `tests/lua/`. Tool changes are not done until REAPER has been tried or the pull request says why not. Add or update tests with every behaviour change; say in the pull request why
something cannot be tested.

## Workflow

- Work in a git worktree on a branch, never on `main`. Fetch and base the branch on the latest `origin/main`.
- `main` is protected: every change is a pull request with green CI. Never push to `main`.
- Right after opening a pull request, turn on auto-merge: `gh pr merge --auto --squash`. GitHub merges it when the
  required checks pass; do not merge by hand and do not use `--admin`. Dependabot's pull requests get this from a workflow.
- Commit messages and pull request titles are [Conventional Commits](https://www.conventionalcommits.org)
  (`feat:`, `fix:`, `docs:`, `refactor:`, `ci:`, `chore:`); the changelog and versions come from them.
- Files stay at or below 800 production lines (`scripts/check-file-size.py`). Split the module; never raise
  the limit or add to `scripts/file-size-baseline.txt` (it can only shrink).
- Update the documentation in the same pull request as the behaviour: `docs/src` (mdBook) and README.md.
- Releases go through a pull request: `scripts/release.sh` opens `chore(release): vX`, and merging it triggers `release.yml`. Never push to `main`.
- Never commit secrets. CI scans for them; a finding is a blocker, not noise.
