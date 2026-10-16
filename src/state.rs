use std::sync::{
    RwLock,
    atomic::{AtomicU32, Ordering},
};

use crate::dsp::eq::{GAIN_LIMIT, NUM_BANDS};
use crate::dsp::source::SourceKind;

pub const MASTER_MIN_DB: f32 = -24.0;
pub const MASTER_MAX_DB: f32 = 6.0;
pub const METER_FLOOR_DB: f32 = -120.0;

/// Everything the UI can change while audio is running. `Copy` so the audio
/// callback can snapshot it without allocating or holding the lock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub gains_db: [f32; NUM_BANDS],
    pub master_db: f32,
    pub bypass: bool,
    pub source: SourceKind,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            gains_db: [0.0; NUM_BANDS],
            master_db: 0.0,
            bypass: false,
            source: SourceKind::default(),
        }
    }
}

impl Params {
    pub fn sanitized(mut self) -> Self {
        for g in &mut self.gains_db {
            *g = g.clamp(-GAIN_LIMIT, GAIN_LIMIT);
        }
        self.master_db = self.master_db.clamp(MASTER_MIN_DB, MASTER_MAX_DB);
        self
    }

    /// Apply the headless test hooks: `EQ_INITIAL_GAINS=0,6,6,0,0,0,0,0`,
    /// `EQ_INITIAL_BYPASS=1`. `get` abstracts the environment so parsing is
    /// unit-testable without process-global state.
    pub fn with_env_overrides<F: Fn(&str) -> Option<String>>(mut self, get: F) -> Self {
        if let Some(list) = get("EQ_INITIAL_GAINS") {
            let parsed: Option<Vec<f32>> = list
                .split(',')
                .map(|item| item.trim().parse().ok())
                .collect();
            if let Some(values) = parsed
                && values.len() == NUM_BANDS
            {
                self.gains_db.copy_from_slice(&values);
            }
        }
        if let Some(value) = get("EQ_INITIAL_BYPASS") {
            self.bypass = matches!(value.as_str(), "1" | "true" | "yes");
        }
        self
    }
}

/// Shared between the UI thread and the audio callback.
#[derive(Default)]
pub struct SharedState {
    params: RwLock<Params>,
    rms: AtomicU32,
    peak: AtomicU32,
}

impl SharedState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Lock-free-ish read: one short read, returned by value.
    pub fn snapshot(&self) -> Params {
        self.params
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .sanitized()
    }

    pub fn update<F: FnOnce(&mut Params)>(&self, mutate: F) {
        let mut guard = self.params.write().unwrap_or_else(|e| e.into_inner());
        mutate(&mut guard);
        *guard = guard.sanitized();
    }

    pub fn set_meter(&self, rms: f32, peak: f32) {
        self.rms.store(rms.to_bits(), Ordering::Relaxed);
        self.peak.store(peak.to_bits(), Ordering::Relaxed);
    }

    pub fn meter(&self) -> (f32, f32) {
        (
            f32::from_bits(self.rms.load(Ordering::Relaxed)),
            f32::from_bits(self.peak.load(Ordering::Relaxed)),
        )
    }
}

pub fn to_db(linear: f32) -> f32 {
    if linear > 1e-9 {
        20.0 * linear.log10()
    } else {
        METER_FLOOR_DB
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_flat() {
        let p = Params::default();
        assert_eq!(p.gains_db, [0.0; NUM_BANDS]);
        assert_eq!(p.master_db, 0.0);
        assert!(!p.bypass);
    }

    #[test]
    fn snapshot_and_update_round_trip() {
        let state = SharedState::new();
        assert_eq!(state.snapshot(), Params::default());
        state.update(|p| {
            p.gains_db[3] = 6.0;
            p.bypass = true;
            p.source = SourceKind::PinkNoise;
        });
        let p = state.snapshot();
        assert_eq!(p.gains_db[3], 6.0);
        assert!(p.bypass);
        assert_eq!(p.source, SourceKind::PinkNoise);
    }

    #[test]
    fn update_sanitizes_out_of_range_values() {
        let state = SharedState::new();
        state.update(|p| {
            p.gains_db[0] = 500.0;
            p.master_db = -900.0;
        });
        let p = state.snapshot();
        assert_eq!(p.gains_db[0], GAIN_LIMIT);
        assert_eq!(p.master_db, MASTER_MIN_DB);
    }

    #[test]
    fn meter_round_trips_bits() {
        let state = SharedState::new();
        state.set_meter(0.25, 0.5);
        let (rms, peak) = state.meter();
        assert_eq!(rms, 0.25);
        assert_eq!(peak, 0.5);
    }

    #[test]
    fn to_db_floors_silence() {
        assert!((to_db(1.0) - 0.0).abs() < 1e-6);
        assert!((to_db(0.5) + 6.020_6).abs() < 0.01);
        assert_eq!(to_db(0.0), METER_FLOOR_DB);
    }

    fn fake_env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| {
            pairs
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| (*value).to_string())
        }
    }

    #[test]
    fn env_overrides_apply_gains_and_bypass() {
        let p = Params::default().with_env_overrides(fake_env(&[
            ("EQ_INITIAL_GAINS", "0, 6, -3, 0,0,0,0,0"),
            ("EQ_INITIAL_BYPASS", "1"),
        ]));
        assert_eq!(p.gains_db, [0.0, 6.0, -3.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert!(p.bypass);
    }

    #[test]
    fn env_overrides_ignore_wrong_gain_count_or_format() {
        let short =
            Params::default().with_env_overrides(fake_env(&[("EQ_INITIAL_GAINS", "1,2,3")]));
        assert_eq!(short.gains_db, [0.0; NUM_BANDS]);
        let garbage = Params::default()
            .with_env_overrides(fake_env(&[("EQ_INITIAL_GAINS", "a,b,c,d,e,f,g,h")]));
        assert_eq!(garbage.gains_db, [0.0; NUM_BANDS]);
    }

    #[test]
    fn env_bypass_only_accepts_truthy_values() {
        for truthy in ["1", "true", "yes"] {
            let p =
                Params::default().with_env_overrides(fake_env(&[("EQ_INITIAL_BYPASS", truthy)]));
            assert!(p.bypass, "{truthy}");
        }
        let falsy = Params::default().with_env_overrides(fake_env(&[("EQ_INITIAL_BYPASS", "0")]));
        assert!(!falsy.bypass);
    }

    #[test]
    fn env_overrides_leave_params_untouched_when_vars_absent() {
        let p = Params::default().with_env_overrides(fake_env(&[]));
        assert_eq!(p, Params::default());
    }
}
