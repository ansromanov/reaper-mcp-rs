#!/bin/sh
# Installs reaper-mcp-rs from the latest GitHub release on Linux and macOS:
#
#   curl -fsSL https://raw.githubusercontent.com/ansromanov/reaper-mcp-rs/main/install.sh | sh
#
# Settings (environment variables):
#   REAPER_MCP_VERSION   release tag such as v0.2.0 (default: the latest release)
#   REAPER_MCP_BIN_DIR   where the server binary goes (default: ~/.local/bin)
#   REAPER_RESOURCE_PATH REAPER resource directory (default: the platform default)
#   REAPER_MCP_STARTUP   1 to start the bridge with REAPER through Scripts/__startup.lua
set -eu

repo="ansromanov/reaper-mcp-rs"
version="${REAPER_MCP_VERSION:-latest}"
bin_dir="${REAPER_MCP_BIN_DIR:-$HOME/.local/bin}"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target="x86_64-unknown-linux-gnu" ;;
  Linux-aarch64 | Linux-arm64) target="aarch64-unknown-linux-gnu" ;;
  Darwin-arm64) target="aarch64-apple-darwin" ;;
  Darwin-x86_64) target="x86_64-apple-darwin" ;;
  *)
    echo "No prebuilt reaper-mcp-rs for $(uname -s) $(uname -m); build it from source." >&2
    exit 1
    ;;
esac

if [ -n "${REAPER_MCP_BASE_URL:-}" ]; then
  base="$REAPER_MCP_BASE_URL"
elif [ "$version" = "latest" ]; then
  base="https://github.com/$repo/releases/latest/download"
else
  base="https://github.com/$repo/releases/download/$version"
fi
archive="reaper-mcp-rs-$target.tar.gz"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
echo "Downloading $archive ($version)"
curl -fsSL -o "$work/$archive" "$base/$archive"
curl -fsSL -o "$work/SHA256SUMS" "$base/SHA256SUMS"

expected="$(grep " $archive\$" "$work/SHA256SUMS" | cut -d' ' -f1)"
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$work/$archive" | cut -d' ' -f1)"
else
  actual="$(shasum -a 256 "$work/$archive" | cut -d' ' -f1)"
fi
if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
  echo "Checksum mismatch for $archive" >&2
  exit 1
fi

tar -xzf "$work/$archive" -C "$work"
mkdir -p "$bin_dir"
install -m 0755 "$work/reaper-mcp-rs/reaper-mcp-rs" "$bin_dir/reaper-mcp-rs"
echo "Installed server to $bin_dir/reaper-mcp-rs"

set --
if [ "${REAPER_MCP_STARTUP:-0}" = "1" ]; then set -- --startup; fi
if [ -n "${REAPER_RESOURCE_PATH:-}" ]; then set -- "$@" "$REAPER_RESOURCE_PATH"; fi
bash "$work/reaper-mcp-rs/scripts/install-bridge.sh" "$@"

cat <<EOF

Next: register the server with your MCP client.
  Claude Code:    claude mcp add reaper -- "$bin_dir/reaper-mcp-rs"
  Claude Desktop: add to claude_desktop_config.json
    { "mcpServers": { "reaper": { "command": "$bin_dir/reaper-mcp-rs" } } }
EOF
