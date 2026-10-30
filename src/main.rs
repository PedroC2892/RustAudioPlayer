//! Rust Audio Player: a small desktop audio player built on rodio and egui.
//!
//! Usage: `RustAudioPlayer [FILE_OR_FOLDER]...`

mod app;
mod audio;
mod instance;
mod playlist;
mod store;
mod theme;

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;

fn main() -> eframe::Result {
    let paths: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();

    // Already running? Hand the files over and exit without a second window.
    let server = match instance::claim(&paths) {
        instance::Claim::Forwarded => return Ok(()),
        instance::Claim::Primary(server) => Some(server),
        instance::Claim::Standalone => None,
    };

    // Taskbar and title-bar icon, drawn in code (see theme::icon_rgba).
    let icon_size = 256;
    let icon = egui::IconData {
        rgba: theme::icon_rgba(icon_size),
        width: icon_size,
        height: icon_size,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Rust Audio Player")
            .with_app_id("rust-audio-player")
            .with_icon(Arc::new(icon))
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([780.0, 520.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "Rust Audio Player",
        options,
        Box::new(|cc| {
            theme::install(&cc.egui_ctx);
            let audio = audio::Audio::new()?;
            let repaint = cc.egui_ctx.clone();
            let opened = server.map(|s| s.serve(move || repaint.request_repaint()));
            Ok(Box::new(app::App::new(&cc.egui_ctx, audio, &paths, opened)))
        }),
    )
}
