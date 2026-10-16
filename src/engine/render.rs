//! Pure per-sample render core — the part every engine shares.
//!
//! Owns: the EQ chain, the test-signal source, parameter smoothers, the
//! clipping guard, and level metering. Knows nothing about cpal, threads, or
//! locks: the caller feeds it [`Params`] snapshots and drains meters per
//! buffer.

use crate::dsp::eq::{EqChain, NUM_BANDS};
use crate::dsp::smooth::Smoother;
use crate::dsp::source::Source;
use crate::state::{MASTER_MAX_DB, MASTER_MIN_DB, Params};

/// Coefficient-update granularity in frames (~0.7 ms at 48 kHz): smoothers
/// advance every sample but filter coefficients rebuild only at this rate.
pub const BLOCK: usize = 32;
const BAND_TAU_S: f32 = 0.02;
const MASTER_TAU_S: f32 = 0.03;

/// The real-time DSP state: source → 8-band EQ → master → clamp, with
/// meters accumulated per buffer.
pub struct Renderer {
    eq: EqChain,
    source: Source,
    bands: [Smoother; NUM_BANDS],
    master: Smoother,
    gains: [f32; NUM_BANDS],
    dt: f32,
    bypass: bool,
    master_lin: f32,
    block_left: usize,
    sum_sq: f64,
    frames: usize,
    peak: f32,
}

impl Renderer {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            eq: EqChain::new(sample_rate),
            source: Source::default(),
            bands: std::array::from_fn(|_| Smoother::new(0.0, sample_rate, BAND_TAU_S)),
            master: Smoother::new(0.0, sample_rate, MASTER_TAU_S),
            gains: [0.0; NUM_BANDS],
            dt: 1.0 / sample_rate,
            bypass: false,
            master_lin: 1.0,
            block_left: 0,
            sum_sq: 0.0,
            frames: 0,
            peak: 0.0,
        }
    }

    /// Adopt new targets from a parameter snapshot (once per buffer).
    pub fn sync(&mut self, params: &Params) {
        self.source.set_kind(params.source);
        for (i, smoother) in self.bands.iter_mut().enumerate() {
            smoother.set(params.gains_db[i]);
        }
        self.master
            .set(params.master_db.clamp(MASTER_MIN_DB, MASTER_MAX_DB));
        self.bypass = params.bypass;
    }

    /// Advance smoothers one block and refresh changed coefficients.
    fn run_block(&mut self) {
        for _ in 0..BLOCK {
            for (i, smoother) in self.bands.iter_mut().enumerate() {
                self.gains[i] = smoother.next();
            }
            self.master.next();
        }
        self.eq.set_gains(&self.gains);
        self.master_lin = if self.bypass {
            1.0
        } else {
            db_to_lin(self.master.value())
        };
    }

    /// Process one output sample and accumulate its meter contribution.
    pub fn next_sample(&mut self) -> f32 {
        if self.block_left == 0 {
            self.run_block();
            self.block_left = BLOCK;
        }
        self.block_left -= 1;

        let mut sample = self.source.next(self.dt);
        if !self.bypass {
            sample = self.eq.process(sample) * self.master_lin;
        }
        sample = sample.clamp(-1.0, 1.0);

        self.sum_sq += f64::from(sample) * f64::from(sample);
        self.frames += 1;
        self.peak = self.peak.max(sample.abs());
        sample
    }

    /// RMS and peak of everything rendered since the last call; restarts the
    /// metering window.
    pub fn take_levels(&mut self) -> (f32, f32) {
        let rms = if self.frames > 0 {
            (self.sum_sq / self.frames as f64).sqrt() as f32
        } else {
            0.0
        };
        let peak = self.peak;
        self.sum_sq = 0.0;
        self.frames = 0;
        self.peak = 0.0;
        (rms, peak)
    }
}

pub fn db_to_lin(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::source::SourceKind;

    const FS: f32 = 48_000.0;

    fn tone_renderer() -> Renderer {
        let mut r = Renderer::new(FS);
        r.sync(&Params {
            source: SourceKind::Tone,
            ..Default::default()
        });
        r
    }

    #[test]
    fn flat_chain_passes_the_source_unchanged() {
        let mut r = tone_renderer();
        for _ in 0..48_000 {
            r.next_sample();
        }
        let (rms, peak) = r.take_levels();
        assert!((rms - 0.3 / 2.0f32.sqrt()).abs() < 0.01, "rms {rms}");
        assert!((peak - 0.3).abs() < 0.002, "peak {peak}");
    }

    #[test]
    fn bypass_is_bit_identical_to_flat_processing() {
        let mut flat = tone_renderer();
        let mut bypassed = tone_renderer();
        bypassed.sync(&Params {
            source: SourceKind::Tone,
            bypass: true,
            ..Default::default()
        });
        for i in 0..2_000 {
            let a = flat.next_sample();
            let b = bypassed.next_sample();
            assert_eq!(a.to_bits(), b.to_bits(), "sample {i}");
        }
    }

    #[test]
    fn master_gain_scales_the_tone() {
        let mut r = Renderer::new(FS);
        r.sync(&Params {
            source: SourceKind::Tone,
            master_db: -6.0,
            ..Default::default()
        });
        let mut peak = 0.0f32;
        for i in 0..48_000 {
            let s = r.next_sample();
            if i >= 24_000 {
                peak = peak.max(s.abs());
            }
        }
        let expected = 0.3 * 10.0f32.powf(-6.0 / 20.0);
        assert!((peak - expected).abs() < 0.005, "peak {peak} vs {expected}");
    }
}
