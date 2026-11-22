#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
resource_dir="${1:-${REAPER_RESOURCE_PATH:-$HOME/.config/REAPER}}"
scripts_dir="$resource_dir/Scripts"

mkdir -p "$scripts_dir/vendor"
install -m 0644 "$repo_root/lua/reaper_mcp_bridge.lua" "$scripts_dir/reaper_mcp_bridge.lua"
install -m 0644 "$repo_root/lua/reaper_mcp_handlers.lua" "$scripts_dir/reaper_mcp_handlers.lua"
install -m 0644 "$repo_root/lua/vendor/json.lua" "$scripts_dir/vendor/json.lua"
printf 'Installed bridge: %s\n' "$scripts_dir/reaper_mcp_bridge.lua"
printf 'Load it from REAPER Actions > Show action list > New action > Load ReaScript.\n'
