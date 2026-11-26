default:
    @just --list

# The full local gate; CI runs the same checks.
check: fmt-check file-size lint test scripts

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

# Production files stay at or below 800 lines (the baseline can only shrink).
file-size:
    python3 scripts/check-file-size.py

lint:
    cargo clippy --all-targets -- -D warnings

test:
    cargo test --all-targets

# Fast deterministic gate for local iteration.
fast-test:
    cargo test --all-targets

# Lua syntax and shell scripts, as in CI.
scripts:
    for f in lua/*.lua tests/lua/*.lua; do luac5.4 -p "$f"; done
    shellcheck scripts/*.sh
