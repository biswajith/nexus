#!/usr/bin/env bash
# Builds and starts Nexus (native Rust + egui app, no Node/webview needed).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Make rustup's default location visible if cargo isn't on PATH yet.
if ! command -v cargo &>/dev/null && [ -d "$HOME/.cargo/bin" ]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi

if ! command -v cargo &>/dev/null; then
    echo "Error: cargo not found on PATH. Install Rust from https://rustup.rs and re-run." >&2
    exit 1
fi
echo "Nexus API Client - local-first, offline, yours."
echo "$(cargo --version)"
echo "Starting Nexus (the first run compiles the workspace in release mode, which takes a while)..."
echo "Press Ctrl+K in the app for the command palette, F1 for all shortcuts."

exec cargo run --release -p nexus-gui -- "$@"
