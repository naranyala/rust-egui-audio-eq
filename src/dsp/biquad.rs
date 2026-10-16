/// RBJ Audio EQ Cookbook biquad (transposed direct form II).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
}

const MIN_Q: f32 = 0.05;
const MAX_Q: f32 = 30.0;

fn clamp_freq(freq: f32, sample_rate: f32) -> f32 {
    freq.clamp(1.0, sample_rate * 0.49)
}

fn clamp_q(q: f32) -> f32 {
    q.clamp(MIN_Q, MAX_Q)
}

impl Biquad {
    pub fn identity() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            x1: 0.0,
            x2: 0.0,
        }
    }

    fn from_normalized(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> Self {
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            x1: 0.0,
            x2: 0.0,
        }
    }

    pub fn peaking(sample_rate: f32, freq: f32, q: f32, gain_db: f32) -> Self {
        if gain_db.abs() < 1e-4 {
            return Self::identity();
        }
        let w0 = std::f32::consts::TAU * clamp_freq(freq, sample_rate) / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * clamp_q(q));
        let a = 10.0f32.powf(gain_db / 40.0);
        Self::from_normalized(
            1.0 + alpha * a,
            -2.0 * cos,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cos,
            1.0 - alpha / a,
        )
    }

    pub fn low_shelf(sample_rate: f32, freq: f32, gain_db: f32) -> Self {
        if gain_db.abs() < 1e-4 {
            return Self::identity();
        }
        let w0 = std::f32::consts::TAU * clamp_freq(freq, sample_rate) / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let a = 10.0f32.powf(gain_db / 40.0);
        let alpha = sin / 2.0 * 2.0_f32.sqrt();
        let sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
        Self::from_normalized(
            a * ((a + 1.0) - (a - 1.0) * cos + sqrt_a_alpha),
            2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
            a * ((a + 1.0) - (a - 1.0) * cos - sqrt_a_alpha),
            (a + 1.0) + (a - 1.0) * cos + sqrt_a_alpha,
            -2.0 * ((a - 1.0) + (a + 1.0) * cos),
            (a + 1.0) + (a - 1.0) * cos - sqrt_a_alpha,
        )
    }

    pub fn high_shelf(sample_rate: f32, freq: f32, gain_db: f32) -> Self {
        if gain_db.abs() < 1e-4 {
            return Self::identity();
        }
        let w0 = std::f32::consts::TAU * clamp_freq(freq, sample_rate) / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let a = 10.0f32.powf(gain_db / 40.0);
        let alpha = sin / 2.0 * 2.0_f32.sqrt();
        let sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
        Self::from_normalized(
            a * ((a + 1.0) + (a - 1.0) * cos + sqrt_a_alpha),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
            a * ((a + 1.0) + (a - 1.0) * cos - sqrt_a_alpha),
            (a + 1.0) - (a - 1.0) * cos + sqrt_a_alpha,
            2.0 * ((a - 1.0) - (a + 1.0) * cos),
            (a + 1.0) - (a - 1.0) * cos - sqrt_a_alpha,
        )
    }

    #[allow(dead_code)]
    pub fn lowpass(sample_rate: f32, freq: f32, q: f32) -> Self {
        let w0 = std::f32::consts::TAU * clamp_freq(freq, sample_rate) / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * clamp_q(q));
        Self::from_normalized(
            (1.0 - cos) / 2.0,
            1.0 - cos,
            (1.0 - cos) / 2.0,
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        )
    }

    #[allow(dead_code)]
    pub fn highpass(sample_rate: f32, freq: f32, q: f32) -> Self {
        let w0 = std::f32::consts::TAU * clamp_freq(freq, sample_rate) / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * clamp_q(q));
        Self::from_normalized(
            (1.0 + cos) / 2.0,
            -(1.0 + cos),
            (1.0 + cos) / 2.0,
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        )
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.x1;
        self.x1 = self.b1 * x - self.a1 * y + self.x2;
        self.x2 = self.b2 * x - self.a2 * y;
        y
    }

    /// Magnitude of the transfer function at `freq`, in dB.
    pub fn magnitude_db(&self, freq: f32, sample_rate: f32) -> f32 {
        let w = std::f32::consts::TAU * clamp_freq(freq, sample_rate) / sample_rate;
        let (sin, cos) = w.sin_cos();
        let (sin2, cos2) = (2.0 * sin * cos, 2.0 * cos * cos - 1.0);
        let n_re = self.b0 + self.b1 * cos + self.b2 * cos2;
        let n_im = -(self.b1 * sin + self.b2 * sin2);
        let d_re = 1.0 + self.a1 * cos + self.a2 * cos2;
        let d_im = -(self.a1 * sin + self.a2 * sin2);
        let num = n_re * n_re + n_im * n_im;
        let den = d_re * d_re + d_im * d_im;
        10.0 * (num / den).max(f32::MIN_POSITIVE).log10()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FS: f32 = 48_000.0;

    fn sine_rms_gain_db(filter_gain: f32, freq: f32, make: impl Fn(f32) -> Biquad) -> f32 {
        let mut filter = make(filter_gain);
        let dt = 1.0 / FS;
        let mut in_sq = 0.0f64;
        let mut out_sq = 0.0f64;
        let total = 48_000usize;
        let skip = 8_160usize;
        let mut phase = 0.0f32;
        for i in 0..total {
            let x = 0.5 * (phase).sin();
            phase = (phase + std::f32::consts::TAU * freq * dt) % std::f32::consts::TAU;
            let y = filter.process(x);
            if i >= skip {
                in_sq += (x as f64) * (x as f64);
                out_sq += (y as f64) * (y as f64);
            }
        }
        let ratio = (out_sq / in_sq).sqrt() as f32;
        20.0 * ratio.log10()
    }

    #[test]
    fn flat_peaking_is_transparent() {
        let filter = Biquad::peaking(FS, 1_000.0, 1.0, 0.0);
        for freq in [20.0, 100.0, 1_000.0, 10_000.0, 20_000.0] {
            assert!(filter.magnitude_db(freq, FS).abs() < 1e-3, "freq {freq}");
        }
    }

    #[test]
    fn peaking_gain_at_center_frequency() {
        for (gain, q) in [(6.0f32, 1.0f32), (-12.0, 0.7), (3.0, 4.0)] {
            let filter = Biquad::peaking(FS, 1_000.0, q, gain);
            let measured = filter.magnitude_db(1_000.0, FS);
            assert!(
                (measured - gain).abs() < 0.01,
                "gain {gain} q {q} -> {measured}"
            );
        }
    }

    #[test]
    fn plus_6db_doubles_amplitude() {
        let measured = sine_rms_gain_db(6.0, 1_000.0, |g| Biquad::peaking(FS, 1_000.0, 1.0, g));
        let linear = 10.0f32.powf(measured / 20.0);
        assert!(
            (linear - 2.0).abs() < 0.02,
            "expected ~2.0x, got {linear} ({measured} dB)"
        );
    }

    #[test]
    fn low_shelf_boosts_bass_only() {
        let filter = Biquad::low_shelf(FS, 200.0, 6.0);
        let low = filter.magnitude_db(10.0, FS);
        let high = filter.magnitude_db(15_000.0, FS);
        assert!((low - 6.0).abs() < 0.2, "low end {low}");
        assert!(high.abs() < 0.3, "high end {high}");
    }

    #[test]
    fn high_shelf_boosts_treble_only() {
        let filter = Biquad::high_shelf(FS, 4_000.0, 6.0);
        let high = filter.magnitude_db(15_000.0, FS);
        let low = filter.magnitude_db(10.0, FS);
        assert!((high - 6.0).abs() < 0.3, "high end {high}");
        assert!(low.abs() < 0.3, "low end {low}");
    }

    #[test]
    fn shelf_cut_reduces() {
        let low = Biquad::low_shelf(FS, 200.0, -12.0);
        assert!((low.magnitude_db(10.0, FS) + 12.0).abs() < 0.3);
        let high = Biquad::high_shelf(FS, 4_000.0, -12.0);
        assert!((high.magnitude_db(15_000.0, FS) + 12.0).abs() < 0.4);
    }

    #[test]
    fn extreme_settings_stay_bounded() {
        let mut filters = [
            Biquad::peaking(FS, 1_000.0, 30.0, 12.0),
            Biquad::peaking(FS, 16_000.0, 30.0, -12.0),
            Biquad::lowpass(FS, 20.0, 30.0),
            Biquad::highpass(FS, 20_000.0, 30.0),
        ];
        for filter in &mut filters {
            let mut x = 1.0f32;
            for _ in 0..48_000 {
                let y = filter.process(x);
                assert!(y.is_finite());
                x = if x == 1.0 { -1.0 } else { 1.0 };
            }
        }
    }
}
