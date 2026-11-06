//! `nexus` — the Nexus desktop app.
//!
//! Dev/test flags: `--screenshot <png>` (render, save, exit), `--home <dir>` (data dir instead
//! of `~/.nexus`), `--size WxH`, `--autosend`, `--url <u>`, `--req-tab params|headers|body|auth|scripts`,
//! `--resp-tab body|headers|tests|timing|visualize`, `--console`, `--search <text>`, `--view-mode pretty|raw|preview`, `--sidebar collections|history`,
//! `--panel http|graphql|ws|mcp|runner`, `--expand-all`, `--open-first`, `--open <request name>`, `--seed-demo` (write demo data into `--home` first),
//! (debug builds only: `--seed-demo`, `--input`.) `--body-file <f>` / `--script-file <f>` (load text into the editor), `--focus-editor`, `--input "text:abc|key:ctrl+f"`.

use nexus_gui::{NexusApp, Opts};
use std::path::PathBuf;

fn parse_args() -> Opts {
    let mut opts = Opts::default();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--screenshot" => opts.screenshot = args.next(),
            "--home" => opts.home = args.next().map(PathBuf::from),
            "--size" => {
                opts.size = args.next().and_then(|s| {
                    let (w, h) = s.split_once('x')?;
                    Some((w.parse().ok()?, h.parse().ok()?))
                })
            }
            "--autosend" => opts.autosend = true,
            "--url" => opts.url = args.next(),
            "--req-tab" => opts.req_tab = args.next(),
            "--resp-tab" => opts.resp_tab = args.next(),
            "--sidebar" => opts.sidebar_tab = args.next(),
            "--panel" => opts.panel = args.next(),
            "--expand-all" => opts.expand_all = true,
            "--open-first" => opts.open_first = true,
            "--open" => opts.open_request = args.next(),
            #[cfg(debug_assertions)]
            "--seed-demo" => opts.seed_demo = true,
            "--theme" => opts.theme = args.next(),
            "--console" => opts.console = true,
            "--search" => opts.search = args.next(),
            "--view-mode" => opts.view_mode = args.next(),
            "--body-file" => opts.body_file = args.next(),
            "--script-file" => opts.script_file = args.next(),
            "--focus-editor" => opts.focus_editor = true,
            #[cfg(debug_assertions)]
            "--input" => opts.input = args.next(),
            _ => {}
        }
    }
    opts
}

fn main() -> eframe::Result {
    let opts = parse_args();
    let home = match opts
        .home
        .clone()
        .or_else(nexus_core::service::App::default_home)
    {
        Some(h) => h,
        None => {
            eprintln!("nexus: cannot determine the data directory (HOME is not set); use --home");
            std::process::exit(1);
        }
    };
    #[cfg(debug_assertions)]
    if opts.seed_demo {
        if let Err(e) = nexus_gui::demo::seed(&home) {
            eprintln!("nexus: cannot seed demo data: {e}");
            std::process::exit(1);
        }
    }
    let (w, h) = opts.size.unwrap_or((1400.0, 900.0));
    let native = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Nexus")
            .with_app_id("nexus")
            .with_inner_size([w, h])
            .with_min_inner_size([960.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Nexus",
        native,
        Box::new(move |cc| {
            nexus_gui::theme::install(&cc.egui_ctx);
            let app = NexusApp::new(home, opts.clone()).map_err(|e| e.to_string())?;
            Ok(Box::new(app))
        }),
    )
}
