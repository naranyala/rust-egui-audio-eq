//! Center panel: the response curve, cached and recomputed only when a
//! parameter changes.

use eframe::egui;
use egui_plot::{GridInput, GridMark, Line, Plot, PlotPoints};

use crate::dsp::eq::{EqChain, NUM_BANDS};

const F_MIN: f32 = 20.0;
const F_MAX: f32 = 20_000.0;
const CURVE_POINTS: usize = 200;
const Y_MIN_DB: f64 = -18.0;
const Y_MAX_DB: f64 = 18.0;

const HZ_GRID: [f64; 18] = [
    20.0, 30.0, 50.0, 100.0, 150.0, 200.0, 300.0, 500.0, 700.0, 1_000.0, 1_500.0, 2_000.0, 3_000.0,
    5_000.0, 7_000.0, 10_000.0, 15_000.0, 20_000.0,
];

pub fn fmt_hz(hz: f64) -> String {
    if hz >= 1_000.0 {
        format!("{}k", hz / 1_000.0)
    } else {
        format!("{hz}")
    }
}

fn hz_grid(input: GridInput) -> Vec<GridMark> {
    let (lo, hi) = input.bounds;
    HZ_GRID
        .iter()
        .map(|&hz| hz.log10())
        .filter(|value| *value >= lo && *value <= hi)
        .map(|value| {
            let is_decade = (value.round() - value).abs() < 1e-9;
            GridMark {
                value,
                step_size: if is_decade { 1.0 } else { 0.3 },
            }
        })
        .collect()
}

/// Cache of the composite EQ response, one point per log-spaced frequency.
pub struct CurveCache {
    points: Vec<[f64; 2]>,
    dirty: bool,
}

impl CurveCache {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            dirty: true,
        }
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Recompute from the same DSP that renders the audio (bypass does not
    /// affect the drawn response).
    pub fn rebuild(&mut self, gains: &[f32; NUM_BANDS], sample_rate: f32) {
        let mut eq = EqChain::new(sample_rate);
        eq.set_gains(gains);
        let lo = F_MIN.log10();
        let hi = F_MAX.log10();
        self.points = (0..=CURVE_POINTS)
            .map(|i| {
                let t = i as f32 / CURVE_POINTS as f32;
                let log_hz = lo + t * (hi - lo);
                [
                    f64::from(log_hz),
                    f64::from(eq.magnitude_db(10f32.powf(log_hz))),
                ]
            })
            .collect();
        self.dirty = false;
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(4.0);
            Plot::new("response")
                .height(ui.available_height() - 8.0)
                .x_axis_label("Hz (log)")
                .y_axis_label("dB")
                .default_x_bounds(f64::from(F_MIN.log10()), f64::from(F_MAX.log10()))
                .default_y_bounds(Y_MIN_DB, Y_MAX_DB)
                .auto_bounds(egui::Vec2b::new(false, false))
                .x_grid_spacer(hz_grid)
                .x_axis_formatter(|mark, _| fmt_hz(10f64.powf(mark.value)))
                .show(ui, |plot_ui| {
                    plot_ui.line(
                        Line::new("EQ", PlotPoints::from_iter(self.points.iter().copied())).stroke(
                            egui::Stroke::new(2.0, egui::Color32::from_rgb(90, 170, 255)),
                        ),
                    );
                });
        });
    }
}

impl Default for CurveCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_hz_shows_plain_hz_below_1k_and_khz_above() {
        assert_eq!(fmt_hz(20.0), "20");
        assert_eq!(fmt_hz(150.0), "150");
        assert_eq!(fmt_hz(1_000.0), "1k");
        assert_eq!(fmt_hz(2_400.0), "2.4k");
        assert_eq!(fmt_hz(16_000.0), "16k");
    }

    #[test]
    fn grid_marks_decades_distinctly_and_clips_to_bounds() {
        let marks = hz_grid(GridInput {
            bounds: (1.0, 4.0),
            base_step_size: 1.0,
        });
        assert!(!marks.is_empty());
        let decade = marks.iter().find(|m| (m.value - 2.0).abs() < 1e-9).unwrap();
        assert_eq!(decade.step_size, 1.0);
        let mid = marks
            .iter()
            .find(|m| (m.value - 2.176_091).abs() < 1e-3)
            .unwrap();
        assert_eq!(mid.step_size, 0.3);
        assert!(marks.iter().all(|m| m.value >= 1.0 && m.value <= 4.0));
    }

    #[test]
    fn rebuild_fills_points_and_clears_dirty() {
        let mut cache = CurveCache::new();
        assert!(cache.is_dirty());
        cache.rebuild(&[6.0; NUM_BANDS], 48_000.0);
        assert!(!cache.is_dirty());
        assert_eq!(cache.points.len(), CURVE_POINTS + 1);
    }
}
