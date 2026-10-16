//! UI composition: one file per panel, this module wires them to state.

mod bands;
mod curve;
mod status;
mod toolbar;

use std::sync::Arc;
use std::time::Duration;

use eframe::egui;

use crate::engine::{EqEngine, LiveEngine};
use crate::state::{Params, SharedState};

use curve::CurveCache;

const REPAINT_MS: u64 = 33;
const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

pub struct EqApp {
    state: Arc<SharedState>,
    engine: Option<Box<dyn EqEngine>>,
    start_error: Option<String>,
    curve: CurveCache,
}

impl EqApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let state = Arc::new(SharedState::new());
        state.update(|p| {
            *p = Params::default().with_env_overrides(|name| std::env::var(name).ok());
        });
        let (engine, start_error) = match LiveEngine::start(state.clone()) {
            Ok(engine) => (Some(Box::new(engine) as Box<dyn EqEngine>), None),
            Err(err) => (None, Some(err)),
        };
        let mut app = Self {
            state,
            engine,
            start_error,
            curve: CurveCache::new(),
        };
        let params = app.state.snapshot();
        app.curve.rebuild(&params.gains_db, app.sample_rate());
        app
    }

    fn sample_rate(&self) -> f32 {
        self.engine
            .as_ref()
            .map_or(FALLBACK_SAMPLE_RATE, |e| e.sample_rate() as f32)
    }
}

impl eframe::App for EqApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut params = self.state.snapshot();
        let mut dirty = false;

        dirty |= toolbar::show(ui, &mut params);
        dirty |= bands::show(ui, &mut params);

        if dirty {
            self.state.update(|current| *current = params);
            self.curve.mark_dirty();
        }
        if self.curve.is_dirty() {
            self.curve.rebuild(&params.gains_db, self.sample_rate());
        }

        status::show(
            ui,
            self.engine.as_deref(),
            self.start_error.as_deref(),
            params.bypass,
            self.state.meter(),
        );
        self.curve.show(ui);

        ui.ctx()
            .request_repaint_after(Duration::from_millis(REPAINT_MS));
    }
}
