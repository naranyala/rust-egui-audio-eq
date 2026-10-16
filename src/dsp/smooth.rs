/// One-pole parameter smoother (exponential approach, no overshoot).
#[derive(Clone, Copy, Debug)]
pub struct Smoother {
    current: f32,
    target: f32,
    coeff: f32,
}

impl Smoother {
    pub fn new(initial: f32, sample_rate: f32, time_constant_s: f32) -> Self {
        let sample_rate = sample_rate.max(1.0);
        let time_constant_s = time_constant_s.max(1e-4);
        Self {
            current: initial,
            target: initial,
            coeff: (-1.0 / (time_constant_s * sample_rate)).exp(),
        }
    }

    pub fn set(&mut self, target: f32) {
        self.target = target;
    }

    pub fn value(&self) -> f32 {
        self.current
    }

    /// Advance one sample and return the new value.
    pub fn next(&mut self) -> f32 {
        self.current = self.target + (self.current - self.target) * self.coeff;
        if (self.current - self.target).abs() < 1e-6 {
            self.current = self.target;
        }
        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converges_to_target() {
        let mut s = Smoother::new(0.0, 48_000.0, 0.02);
        s.set(12.0);
        for _ in 0..48_000 {
            s.next();
        }
        assert!((s.value() - 12.0).abs() < 1e-3);
    }

    #[test]
    fn step_is_monotonic_no_overshoot() {
        let mut s = Smoother::new(-6.0, 48_000.0, 0.02);
        s.set(6.0);
        let mut last = s.value();
        for _ in 0..4_800 {
            let v = s.next();
            assert!(v >= last - 1e-6, "decreased: {v} after {last}");
            assert!(v <= 6.0 + 1e-6, "overshoot: {v}");
            last = v;
        }
        assert!(last > 5.9, "should be close to target, got {last}");
    }

    #[test]
    fn stationary_input_stays_put() {
        let mut s = Smoother::new(3.5, 48_000.0, 0.02);
        for _ in 0..1_000 {
            assert_eq!(s.next(), 3.5);
        }
        assert_eq!(s.value(), 3.5);
    }
}
