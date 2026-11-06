# Installation & Setup

## Prerequisites

- **Rust** (stable, via [rustup](https://rustup.rs)). No Node.js, no webview, no GTK.
- Linux: a Wayland or X11 session with OpenGL (Mesa). The usual runtime libraries (libxkbcommon, libwayland / libX11, libEGL, libGL) must be installed. Linux is the platform the app has been run and tested on.

Verify your installation:

```bash
cargo --version
```

## Clone & Run

```bash
git clone https://github.com/biswajith/nexus.git
cd nexus
./start.sh
```

`start.sh` checks for cargo and runs `cargo run --release -p nexus-gui`. The first run compiles the workspace, which takes a few minutes.

## Production Build

```bash
cargo build --release -p nexus-gui   # binary: target/release/nexus
```

The release binary is self-contained. To get a launcher entry, copy `crates/nexus-gui/assets/nexus.desktop` to `~/.local/share/applications/` and `crates/nexus-gui/assets/nexus.png` to `~/.local/share/icons/hicolor/512x512/apps/`, and put `nexus` on your `PATH`.

## Development Commands

| Command | Description |
|---|---|
| `cargo run -p nexus-gui` | Debug build and run |
| `cargo test --workspace` | Run all tests (engine and GUI) |
| `cargo clippy --workspace --all-targets -- -D warnings` | Lint |
| `cargo fmt --all --check` | Check formatting |

## Project Structure

```
nexus/
├── crates/nexus-core/   # Rust engine library (no UI)
├── crates/nexus-gui/    # egui app, binary `nexus`
├── docs/                # Product documentation
└── Cargo.toml           # Rust workspace
```

## System Requirements

| Requirement | Minimum |
|---|---|
| OS | Linux with Wayland or X11 and OpenGL |
| RAM | 1 GB (the app itself uses well under 100 MB when idle) |
