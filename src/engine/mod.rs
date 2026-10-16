//! Engine layer: everything that turns parameters into sound.
//!
//! [`EqEngine`] is the app-facing contract (epic E1). Today the only
//! implementation is the live cpal stream ([`LiveEngine`]); the offline
//! renderer and mock engines land against the same trait so the conformance
//! suite can run against all of them.
//!
//! [`Renderer`] is the pure per-sample core shared by every future engine —
//! no cpal, no locks, no allocation in the hot path.

pub mod live;
pub mod render;

pub use live::LiveEngine;

/// What the UI needs from any audio engine: a label for the status bar, the
/// sample rate for the response curve, and drained runtime errors.
pub trait EqEngine {
    /// Nominal sample rate of the running stream (Hz).
    fn sample_rate(&self) -> u32;

    /// One-line description, e.g. `alsa_output… · 48000 Hz · F32 · 2 ch`.
    fn device_label(&self) -> String;

    /// Take the most recent runtime error, if any (drains it).
    fn take_error(&self) -> Option<String>;
}
