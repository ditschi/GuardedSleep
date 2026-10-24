//! RMS threshold + hold gate.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GateChange {
    Loud { rms: f64 },
    Quiet,
}

#[derive(Debug, Clone)]
pub struct NoiseGate {
    threshold: f64,
    hold_ms: u64,
    above_since: Option<u64>,
    loud: bool,
    last_rms: f64,
}

impl NoiseGate {
    pub fn new(threshold: f64, hold_ms: u64) -> Self {
        Self {
            threshold,
            hold_ms,
            above_since: None,
            loud: false,
            last_rms: 0.0,
        }
    }

    pub fn last_rms(&self) -> f64 {
        self.last_rms
    }

    pub fn threshold(&self) -> f64 {
        self.threshold
    }

    pub fn hold_ms(&self) -> u64 {
        self.hold_ms
    }

    /// Applies new parameters to the running gate. The loud/quiet state and the hold timer are
    /// kept, so a change never fires a fresh alert by itself: a lowered threshold or shortened
    /// hold can only make the next sample fire sooner, a raised threshold ends an ongoing loud
    /// state with the usual `Quiet` on the next sample below it.
    pub fn set_params(&mut self, threshold: f64, hold_ms: u64) {
        self.threshold = threshold;
        self.hold_ms = hold_ms;
    }

    /// Feed one RMS sample (0..1). Returns a change only on transitions.
    pub fn sample(&mut self, now_ms: u64, rms: f64) -> Option<GateChange> {
        let rms = if rms.is_finite() {
            rms.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.last_rms = rms;
        if rms >= self.threshold {
            let since = *self.above_since.get_or_insert(now_ms);
            if !self.loud && now_ms.saturating_sub(since) >= self.hold_ms {
                self.loud = true;
                return Some(GateChange::Loud { rms });
            }
            None
        } else {
            self.above_since = None;
            if self.loud {
                self.loud = false;
                return Some(GateChange::Quiet);
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_after_hold_and_resets_on_quiet() {
        let mut g = NoiseGate::new(0.2, 300);
        assert_eq!(g.sample(0, 0.5), None);
        assert_eq!(g.sample(200, 0.5), None);
        assert_eq!(g.sample(300, 0.6), Some(GateChange::Loud { rms: 0.6 }));
        assert_eq!(g.sample(400, 0.6), None);
        assert_eq!(g.sample(500, 0.1), Some(GateChange::Quiet));
    }

    #[test]
    fn short_spike_does_not_fire() {
        let mut g = NoiseGate::new(0.2, 300);
        g.sample(0, 0.9);
        g.sample(100, 0.0);
        assert_eq!(g.sample(350, 0.9), None);
    }

    #[test]
    fn live_parameter_change_keeps_state() {
        let mut g = NoiseGate::new(0.2, 300);
        g.sample(0, 0.5);
        g.set_params(0.2, 100);
        assert_eq!(g.sample(100, 0.5), Some(GateChange::Loud { rms: 0.5 }));
        g.set_params(0.9, 100);
        assert_eq!(g.sample(200, 0.5), Some(GateChange::Quiet));
        g.set_params(0.2, 5_000);
        assert_eq!(g.sample(300, 0.5), None);
        assert_eq!(g.threshold(), 0.2);
        assert_eq!(g.hold_ms(), 5_000);
    }

    #[test]
    fn loud_state_is_not_reported_again_after_a_change() {
        let mut g = NoiseGate::new(0.2, 0);
        assert!(matches!(g.sample(0, 0.5), Some(GateChange::Loud { .. })));
        g.set_params(0.1, 1_000);
        assert_eq!(g.sample(100, 0.5), None);
    }

    #[test]
    fn non_finite_samples_are_silence() {
        let mut g = NoiseGate::new(0.2, 0);
        assert_eq!(g.sample(0, f64::NAN), None);
        assert_eq!(g.last_rms(), 0.0);
    }
}
