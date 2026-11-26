#!/usr/bin/env bash
# Installs the Lua bridge into REAPER's Scripts/reaper-mcp folder on Linux and macOS.
#
#   scripts/install-bridge.sh [--startup] [REAPER resource directory]
#
# --startup makes REAPER start the bridge by itself through Scripts/__startup.lua.
# The resource directory defaults to $REAPER_RESOURCE_PATH, then the platform default.
set -euo pipefail

startup=false
resource_dir=""
for arg in "$@"; do
  case "$arg" in
    --startup) startup=true ;;
    -h | --help)
      sed -n '2,7p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) resource_dir="$arg" ;;
  esac
done

if [[ -z "$resource_dir" ]]; then
  if [[ -n "${REAPER_RESOURCE_PATH:-}" ]]; then
    resource_dir="$REAPER_RESOURCE_PATH"
  elif [[ "$(uname -s)" == "Darwin" ]]; then
    resource_dir="$HOME/Library/Application Support/REAPER"
  else
    resource_dir="$HOME/.config/REAPER"
  fi
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target="$resource_dir/Scripts/reaper-mcp"

mkdir -p "$target/vendor" "$target/handlers"
install -m 0644 "$repo_root/lua/reaper_mcp_bridge.lua" "$target/reaper_mcp_bridge.lua"
install -m 0644 "$repo_root/lua/reaper_mcp_handlers.lua" "$target/reaper_mcp_handlers.lua"
for handler in "$repo_root"/lua/handlers/*.lua; do
  install -m 0644 "$handler" "$target/handlers/$(basename "$handler")"
done
install -m 0644 "$repo_root/lua/vendor/json.lua" "$target/vendor/json.lua"
install -m 0644 "$repo_root/lua/vendor/json.LICENSE" "$target/vendor/json.LICENSE"
printf 'Installed bridge to %s\n' "$target"

if [[ "$startup" == true ]]; then
  startup_file="$resource_dir/Scripts/__startup.lua"
  if ! grep -qsF 'reaper-mcp/reaper_mcp_bridge.lua' "$startup_file"; then
    printf '\n-- Starts the reaper-mcp-rs bridge.\ndofile(reaper.GetResourcePath() .. "/Scripts/reaper-mcp/reaper_mcp_bridge.lua")\n' >>"$startup_file"
    printf 'Added the bridge to %s\n' "$startup_file"
  fi
  printf 'Restart REAPER, or run the bridge once from the action list.\n'
else
  printf 'Load it from REAPER: Actions > Show action list > New action > Load ReaScript,\n'
  printf 'or re-run with --startup to start it with REAPER.\n'
fi
