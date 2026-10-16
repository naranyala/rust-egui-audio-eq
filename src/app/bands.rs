//! Left panel: the eight vertical band sliders.

use eframe::egui;

use crate::app::curve::fmt_hz;
use crate::dsp::eq::{BANDS, BandKind, GAIN_LIMIT};
use crate::state::Params;

const BANDS_PANEL_WIDTH: f32 = 420.0;

fn kind_short(kind: BandKind) -> &'static str {
    match kind {
        BandKind::LowShelf => "LS",
        BandKind::Peaking => "PK",
        BandKind::HighShelf => "HS",
    }
}

pub fn show(ui: &mut egui::Ui, params: &mut Params) -> bool {
    let mut dirty = false;
    egui::Panel::left("bands")
        .resizable(false)
        .exact_size(BANDS_PANEL_WIDTH)
        .show(ui, |ui| {
            ui.add_space(6.0);
            ui.vertical_centered(|ui| {
                ui.strong("Bands (dB)");
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                for (band, gain) in BANDS.iter().zip(&mut params.gains_db) {
                    ui.vertical(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.small(fmt_hz(f64::from(band.freq)));
                            ui.small(kind_short(band.kind));
                        });
                        let response = ui.add(
                            egui::Slider::new(gain, -GAIN_LIMIT..=GAIN_LIMIT)
                                .vertical()
                                .fixed_decimals(1),
                        );
                        if response.changed() {
                            dirty = true;
                        }
                    });
                }
            });
            ui.separator();
            ui.small("band 1 = low shelf · band 8 = high shelf · peaking Q = 1.0");
        });
    dirty
}
