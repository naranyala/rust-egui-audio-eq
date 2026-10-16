#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod dsp;
mod engine;
mod state;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1_100.0, 640.0])
            .with_min_inner_size([860.0, 480.0])
            .with_title("Audio EQ"),
        ..Default::default()
    };
    eframe::run_native(
        "rust-egui-audio-eq",
        options,
        Box::new(|cc| Ok(Box::new(app::EqApp::new(cc)))),
    )
}
