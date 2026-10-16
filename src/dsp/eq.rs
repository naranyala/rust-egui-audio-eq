use super::biquad::Biquad;

pub const NUM_BANDS: usize = 8;
pub const GAIN_LIMIT: f32 = 12.0;
pub const PEAKING_Q: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BandKind {
    LowShelf,
    Peaking,
    HighShelf,
}

/// One band's fixed design. `q` applies to peaking bands only (the shelving
/// recipes take no Q); per-band overrides arrive with parametric editing (P1)
/// and layer on top of this table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandSpec {
    pub freq: f32,
    pub kind: BandKind,
    pub q: f32,
}

/// Single source of truth for the graphic EQ layout: shelves at the ends so
/// the outer bands have no dead space, peaking Q = 1.0 between them so
/// adjacent bands blend without comb ripples.
pub const BANDS: [BandSpec; NUM_BANDS] = [
    BandSpec {
        freq: 60.0,
        kind: BandKind::LowShelf,
        q: PEAKING_Q,
    },
    BandSpec {
        freq: 150.0,
        kind: BandKind::Peaking,
        q: PEAKING_Q,
    },
    BandSpec {
        freq: 400.0,
        kind: BandKind::Peaking,
        q: PEAKING_Q,
    },
    BandSpec {
        freq: 1_000.0,
        kind: BandKind::Peaking,
        q: PEAKING_Q,
    },
    BandSpec {
        freq: 2_400.0,
        kind: BandKind::Peaking,
        q: PEAKING_Q,
    },
    BandSpec {
        freq: 6_000.0,
        kind: BandKind::Peaking,
        q: PEAKING_Q,
    },
    BandSpec {
        freq: 12_000.0,
        kind: BandKind::Peaking,
        q: PEAKING_Q,
    },
    BandSpec {
        freq: 16_000.0,
        kind: BandKind::HighShelf,
        q: PEAKING_Q,
    },
];

fn make_filter(sample_rate: f32, spec: &BandSpec, gain_db: f32) -> Biquad {
    match spec.kind {
        BandKind::LowShelf => Biquad::low_shelf(sample_rate, spec.freq, gain_db),
        BandKind::Peaking => Biquad::peaking(sample_rate, spec.freq, spec.q, gain_db),
        BandKind::HighShelf => Biquad::high_shelf(sample_rate, spec.freq, gain_db),
    }
}

/// Cascaded 8-band graphic EQ chain.
pub struct EqChain {
    filters: [Biquad; NUM_BANDS],
    applied: [f32; NUM_BANDS],
    sample_rate: f32,
}

impl EqChain {
    pub fn new(sample_rate: f32) -> Self {
        let sample_rate = if sample_rate.is_finite() && sample_rate > 0.0 {
            sample_rate
        } else {
            48_000.0
        };
        Self {
            filters: std::array::from_fn(|i| make_filter(sample_rate, &BANDS[i], 0.0)),
            applied: [0.0; NUM_BANDS],
            sample_rate,
        }
    }

    /// Update band gains (dB). Gains are clamped to ±`GAIN_LIMIT`; a band's
    /// coefficients are rebuilt only when its gain actually moved.
    pub fn set_gains(&mut self, gains: &[f32; NUM_BANDS]) {
        for (i, filter) in self.filters.iter_mut().enumerate() {
            let g = gains[i].clamp(-GAIN_LIMIT, GAIN_LIMIT);
            if (g - self.applied[i]).abs() > 0.01 {
                *filter = make_filter(self.sample_rate, &BANDS[i], g);
                self.applied[i] = g;
            }
        }
    }

    #[allow(dead_code)]
    pub fn gains(&self) -> [f32; NUM_BANDS] {
        self.applied
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let mut y = x;
        for filter in &mut self.filters {
            y = filter.process(y);
        }
        y
    }

    /// Composite magnitude of all bands at `freq`, in dB.
    pub fn magnitude_db(&self, freq: f32) -> f32 {
        self.filters
            .iter()
            .map(|f| f.magnitude_db(freq, self.sample_rate))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FS: f32 = 48_000.0;

    #[test]
    fn band_specs_are_wired_as_designed() {
        assert_eq!(BANDS[0].kind, BandKind::LowShelf);
        assert_eq!(BANDS[NUM_BANDS - 1].kind, BandKind::HighShelf);
        for (i, band) in BANDS.iter().enumerate().take(NUM_BANDS - 1).skip(1) {
            assert_eq!(band.kind, BandKind::Peaking, "band {i}");
        }
        for pair in BANDS.windows(2) {
            assert!(pair[0].freq < pair[1].freq, "frequencies must ascend");
        }
    }

    #[test]
    fn flat_chain_is_transparent() {
        let eq = EqChain::new(FS);
        for freq in [20.0, 60.0, 150.0, 1_000.0, 6_000.0, 16_000.0, 20_000.0] {
            let db = eq.magnitude_db(freq);
            assert!(db.abs() < 1e-3, "freq {freq} -> {db}");
        }
    }

    #[test]
    fn single_band_boost_matches_its_gain() {
        let mut eq = EqChain::new(FS);
        let mut gains = [0.0f32; NUM_BANDS];
        gains[3] = 6.0;
        eq.set_gains(&gains);
        let at_center = eq.magnitude_db(BANDS[3].freq);
        assert!((at_center - 6.0).abs() < 0.05, "{at_center}");
        let far_away = eq.magnitude_db(30.0);
        assert!(far_away.abs() < 0.5, "{far_away}");
    }

    #[test]
    fn gains_are_clamped_to_the_limit() {
        let mut eq = EqChain::new(FS);
        let mut gains = [0.0f32; NUM_BANDS];
        gains[3] = 50.0;
        gains[4] = -99.0;
        eq.set_gains(&gains);
        assert_eq!(eq.gains()[3], GAIN_LIMIT);
        assert_eq!(eq.gains()[4], -GAIN_LIMIT);

        let mut only3 = [0.0f32; NUM_BANDS];
        only3[3] = 50.0;
        eq.set_gains(&only3);
        let at_center = eq.magnitude_db(BANDS[3].freq);
        assert!((at_center - GAIN_LIMIT).abs() < 0.05, "{at_center}");

        let mut only4 = [0.0f32; NUM_BANDS];
        only4[4] = -99.0;
        eq.set_gains(&only4);
        let at_band4 = eq.magnitude_db(BANDS[4].freq);
        assert!((at_band4 + GAIN_LIMIT).abs() < 0.05, "{at_band4}");
    }

    #[test]
    fn process_agrees_with_magnitude_response() {
        let mut eq = EqChain::new(FS);
        let mut gains = [0.0f32; NUM_BANDS];
        gains[3] = 6.0;
        gains[1] = -4.0;
        eq.set_gains(&gains);

        let freq = BANDS[3].freq;
        let predicted = eq.magnitude_db(freq);

        let dt = 1.0 / FS;
        let mut phase = 0.0f32;
        let mut in_sq = 0.0f64;
        let mut out_sq = 0.0f64;
        for i in 0..48_000usize {
            let x = 0.5 * phase.sin();
            phase = (phase + std::f32::consts::TAU * freq * dt) % std::f32::consts::TAU;
            let y = eq.process(x);
            if i >= 8_160 {
                in_sq += (x as f64) * (x as f64);
                out_sq += (y as f64) * (y as f64);
            }
        }
        let measured = 20.0 * ((out_sq / in_sq).sqrt() as f32).log10();
        assert!(
            (measured - predicted).abs() < 0.1,
            "measured {measured} vs predicted {predicted}"
        );
    }

    #[test]
    fn rebuilding_on_same_gain_keeps_filters() {
        let mut eq = EqChain::new(FS);
        let mut gains = [0.0f32; NUM_BANDS];
        gains[2] = 3.0;
        eq.set_gains(&gains);
        let first = eq.magnitude_db(BANDS[2].freq);
        eq.set_gains(&gains);
        eq.set_gains(&gains);
        let again = eq.magnitude_db(BANDS[2].freq);
        assert_eq!(first, again);
    }
}
