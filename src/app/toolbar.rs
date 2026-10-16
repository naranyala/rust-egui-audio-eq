//! Top panel: source picker, bypass, master gain, reset.

use eframe::egui;

use crate::dsp::eq::NUM_BANDS;
use crate::dsp::source::SourceKind;
use crate::state::{MASTER_MAX_DB, MASTER_MIN_DB, Params};

pub fn show(ui: &mut egui::Ui, params: &mut Params) -> bool {
    let mut dirty = false;
    egui::Panel::top("controls").show(ui, |ui| {
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.heading("Audio EQ");
            ui.separator();

            ui.label("Source:");
            for kind in SourceKind::ALL {
                if ui
                    .selectable_value(&mut params.source, kind, kind.label())
                    .changed()
                {
                    dirty = true;
                }
            }

            ui.separator();
            if ui.checkbox(&mut params.bypass, "Bypass").changed() {
                dirty = true;
            }

            ui.separator();
            if ui
                .add(
                    egui::Slider::new(&mut params.master_db, MASTER_MIN_DB..=MASTER_MAX_DB)
                        .text("Master dB")
                        .fixed_decimals(1),
                )
                .changed()
            {
                dirty = true;
            }

            if ui.button("Reset").clicked() {
                params.gains_db = [0.0; NUM_BANDS];
                params.master_db = 0.0;
                dirty = true;
            }
        });
        ui.add_space(4.0);
    });
    dirty
}
