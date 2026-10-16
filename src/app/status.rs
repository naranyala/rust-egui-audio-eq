//! Bottom panel: device/stream status, errors, meters, bypass badge.

use eframe::egui;

use crate::engine::EqEngine;
use crate::state::to_db;

pub fn show(
    ui: &mut egui::Ui,
    engine: Option<&dyn EqEngine>,
    start_error: Option<&str>,
    bypass: bool,
    (rms, peak): (f32, f32),
) {
    egui::Panel::bottom("status").show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            match engine {
                Some(engine) => {
                    ui.label(engine.device_label());
                    if let Some(err) = engine.take_error() {
                        ui.colored_label(
                            egui::Color32::from_rgb(230, 90, 90),
                            format!("stream error: {err}"),
                        );
                    }
                }
                None => {
                    ui.colored_label(
                        egui::Color32::from_rgb(230, 90, 90),
                        start_error.unwrap_or("audio unavailable"),
                    );
                }
            }
            ui.separator();
            ui.label(format!(
                "RMS {:+.1} dB · peak {:+.1} dB",
                to_db(rms),
                to_db(peak)
            ));
            if bypass {
                ui.colored_label(
                    egui::Color32::from_rgb(240, 170, 60),
                    "BYPASS — signal untouched",
                );
            }
        });
    });
}
