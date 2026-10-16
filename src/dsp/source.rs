#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SourceKind {
    Silence,
    #[default]
    Tone,
    Sweep,
    WhiteNoise,
    PinkNoise,
}

impl SourceKind {
    pub const ALL: [SourceKind; 5] = [
        SourceKind::Silence,
        SourceKind::Tone,
        SourceKind::Sweep,
        SourceKind::WhiteNoise,
        SourceKind::PinkNoise,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SourceKind::Silence => "Silence",
            SourceKind::Tone => "Tone",
            SourceKind::Sweep => "Sweep",
            SourceKind::WhiteNoise => "White",
            SourceKind::PinkNoise => "Pink",
        }
    }
}

const TONE_HZ: f32 = 440.0;
const SWEEP_S: f32 = 10.0;
const SWEEP_START_HZ: f32 = 200.0;
const SWEEP_END_HZ: f32 = 8_000.0;
const LEVEL: f32 = 0.3;

/// Mono test-signal generator.
pub struct Source {
    kind: SourceKind,
    phase: f32,
    t: f32,
    rng: u32,
    pink: [f32; 3],
}

impl Default for Source {
    fn default() -> Self {
        Self::new(SourceKind::default())
    }
}

impl Source {
    pub fn new(kind: SourceKind) -> Self {
        Self {
            kind,
            phase: 0.0,
            t: 0.0,
            rng: 0x2545_F491,
            pink: [0.0; 3],
        }
    }

    pub fn set_kind(&mut self, kind: SourceKind) {
        if kind != self.kind {
            self.kind = kind;
            self.phase = 0.0;
            self.t = 0.0;
            self.pink = [0.0; 3];
        }
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    fn next_white(&mut self) -> f32 {
        (self.next_u32() as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    fn next_pink(&mut self) -> f32 {
        let w = self.next_white();
        let p = &mut self.pink;
        p[0] = 0.997_65 * p[0] + w * 0.099_046;
        p[1] = 0.963_00 * p[1] + w * 0.296_516_4;
        p[2] = 0.570_00 * p[2] + w * 1.052_691_3;
        (p[0] + p[1] + p[2] + w * 0.184_8) * 0.35
    }

    /// Generate one sample; `dt` is one sample period in seconds.
    pub fn next(&mut self, dt: f32) -> f32 {
        match self.kind {
            SourceKind::Silence => 0.0,
            SourceKind::Tone => {
                self.phase = (self.phase + std::f32::consts::TAU * TONE_HZ * dt)
                    .rem_euclid(std::f32::consts::TAU);
                LEVEL * self.phase.sin()
            }
            SourceKind::Sweep => {
                self.t += dt;
                let cycle = (self.t % SWEEP_S) / SWEEP_S;
                let freq = SWEEP_START_HZ * (SWEEP_END_HZ / SWEEP_START_HZ).powf(cycle);
                self.phase = (self.phase + std::f32::consts::TAU * freq * dt)
                    .rem_euclid(std::f32::consts::TAU);
                LEVEL * self.phase.sin()
            }
            SourceKind::WhiteNoise => LEVEL * self.next_white(),
            SourceKind::PinkNoise => LEVEL * self.next_pink().clamp(-1.0, 1.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 48_000.0;

    #[test]
    fn silence_is_exactly_zero() {
        let mut s = Source::new(SourceKind::Silence);
        for _ in 0..1_000 {
            assert_eq!(s.next(DT), 0.0);
        }
    }

    #[test]
    fn tone_stays_bounded_and_is_not_silent() {
        let mut s = Source::new(SourceKind::Tone);
        let mut peak = 0.0f32;
        for _ in 0..48_000 {
            let v = s.next(DT);
            assert!(v.is_finite());
            peak = peak.max(v.abs());
        }
        assert!((peak - 0.3).abs() < 0.01, "peak {peak}");
    }

    #[test]
    fn noise_stays_bounded() {
        for kind in [SourceKind::WhiteNoise, SourceKind::PinkNoise] {
            let mut s = Source::new(kind);
            for _ in 0..20_000 {
                let v = s.next(DT);
                assert!(v.is_finite(), "{kind:?}");
                assert!(v.abs() <= 0.31, "{kind:?} peak {}", v.abs());
            }
        }
    }

    #[test]
    fn changing_kind_resets_phase() {
        let mut s = Source::new(SourceKind::Tone);
        for _ in 0..10_000 {
            s.next(DT);
        }
        s.set_kind(SourceKind::Silence);
        assert_eq!(s.next(DT), 0.0);
    }

    #[test]
    fn sweep_produces_signal() {
        let mut s = Source::new(SourceKind::Sweep);
        let mut peak = 0.0f32;
        for _ in 0..480_000 {
            peak = peak.max(s.next(DT).abs());
        }
        assert!((peak - 0.3).abs() < 0.02, "peak {peak}");
    }
}
