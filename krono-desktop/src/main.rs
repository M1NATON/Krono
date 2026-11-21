#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cli;
mod config;
mod i18n;
mod notifications;
#[cfg(debug_assertions)]
mod shot_runner;
mod storage;
mod sync;
mod tray;
mod ui;

use app::KronoApp;
use eframe::egui;
use std::io::Write;

fn append_crash_log(kind: &str, details: &str) {
    let path = config::crash_log_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "[{ts}] {kind}: {details}");
    }
}

fn main() -> eframe::Result<()> {
    if let Some(exit_code) = cli::try_handle_cli() {
        std::process::exit(exit_code);
    }

    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic payload".to_string()
        };
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let bt = std::backtrace::Backtrace::force_capture();
        append_crash_log("PANIC", &format!("{msg} at {loc}\n{bt}"));
        prev_hook(info);
    }));

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([360.0, 560.0])
            .with_icon(ui::icons::build_window_icon())
            .with_title("Krono"),
        ..Default::default()
    };

    let res = eframe::run_native(
        "Krono",
        options,
        Box::new(|cc| Ok(Box::new(KronoApp::new(cc)))),
    );
    if let Err(ref err) = res {
        append_crash_log("EFRAME_ERROR", &format!("{err:?}"));
    }
    res
}
