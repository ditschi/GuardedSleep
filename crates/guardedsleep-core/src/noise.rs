//! RMS threshold + hold gate with a sliding window (`audio-monitoring` "Local noise detection").
//!
//! Speech and crying come in bursts, so a single sample below the threshold must not restart the
//! hold time. Each sample counts for the time since the previous one (at most
//! [`MAX_SAMPLE_COVER_MS`]). A noisy stretch starts at the first loud sample and ends after
//! [`RELEASE_MS`] without one. The gate turns loud on a sample at or above the threshold when the
//! stretch lasted at least the hold time and the loud samples cover at least half of the last hold
//! time; only that last hold time counts, whatever came before.

use std::collections::VecDeque;

/// Lowest noise threshold any message or setting may carry (about -56 dBFS). One constant for the
/// `DeviceStatus` parser, the link config clamp, the vectors and the clients (through the
/// bindings), so a status with a quiet threshold is never rejected whole.
pub const MIN_NOISE_THRESHOLD: f64 = 0.0015;
pub const MAX_NOISE_THRESHOLD: f64 = 1.0;

/// A sample covers at most this long, so a throttled sampler cannot make one sample count for
/// minutes.
pub const MAX_SAMPLE_COVER_MS: u64 = 1_000;
/// Time without a loud sample that ends a noisy stretch and releases a loud gate.
pub const RELEASE_MS: u64 = 2_000;
/// What the first sample after a start or restart covers: the platform's nominal interval.
const NOMINAL_SAMPLE_MS: u64 = 100;
/// Hard bound on the window buffer (15 s at 100 ms is 150 entries).
const MAX_WINDOW_ENTRIES: usize = 512;

// The release must outlast a full sample cover, or one sparse sample would end a stretch.
const _: () = assert!(RELEASE_MS >= MAX_SAMPLE_COVER_MS);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GateChange {
    Loud { rms: f64 },
    Quiet,
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    at_ms: u64,
    covered_ms: u64,
    loud: bool,
}

#[derive(Debug, Clone)]
pub struct NoiseGate {
    threshold: f64,
    hold_ms: u64,
    window: VecDeque<Entry>,
    prev_at: Option<u64>,
    stretch_since: Option<u64>,
    last_loud_at: Option<u64>,
    loud: bool,
    last_rms: f64,
}

impl NoiseGate {
    pub fn new(threshold: f64, hold_ms: u64) -> Self {
        Self {
            threshold,
            hold_ms,
            window: VecDeque::new(),
            prev_at: None,
            stretch_since: None,
            last_loud_at: None,
            loud: false,
            last_rms: 0.0,
        }
    }

    pub fn last_rms(&self) -> f64 {
        self.last_rms
    }

    /// True while the gate is in its loud state (a change to loud was reported, quiet has not
    /// followed).
    pub fn is_loud(&self) -> bool {
        self.loud
    }

    pub fn threshold(&self) -> f64 {
        self.threshold
    }

    pub fn hold_ms(&self) -> u64 {
        self.hold_ms
    }

    /// Applies new parameters to the running gate. The loud/quiet state and the window are kept,
    /// so a change never fires a fresh alert by itself: a lowered threshold or shortened hold can
    /// only make the next sample fire sooner, a raised threshold ends an ongoing loud state with
    /// the usual `Quiet` once no sample has reached it for [`RELEASE_MS`].
    pub fn set_params(&mut self, threshold: f64, hold_ms: u64) {
        self.threshold = threshold;
        self.hold_ms = hold_ms;
    }

    /// Forgets the counted history and keeps the loud/quiet state: the gate was not fed for a
    /// while (a talk paused it) and the next loud sample starts a fresh stretch. A loud gate is
    /// not released at once, the release time starts now.
    pub fn restart_hold(&mut self, now_ms: u64) {
        self.window.clear();
        self.prev_at = None;
        self.stretch_since = None;
        self.last_loud_at = Some(now_ms);
    }

    /// Feed one RMS sample (0..1). Returns a change only on transitions.
    pub fn sample(&mut self, now_ms: u64, rms: f64) -> Option<GateChange> {
        let rms = if rms.is_finite() {
            rms.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.last_rms = rms;
        let covered_ms = self.prev_at.map_or(NOMINAL_SAMPLE_MS, |p| {
            now_ms.saturating_sub(p).min(MAX_SAMPLE_COVER_MS)
        });
        self.prev_at = Some(now_ms);
        let is_loud_sample = rms >= self.threshold;

        // A stretch that saw no loud sample for the release time is over, before this sample
        // may start a new one.
        let expired = self
            .last_loud_at
            .is_none_or(|t| now_ms.saturating_sub(t) >= RELEASE_MS);
        if expired {
            self.stretch_since = None;
            self.window.clear();
        }
        if is_loud_sample {
            self.last_loud_at = Some(now_ms);
            self.stretch_since.get_or_insert(now_ms);
        }
        self.window.push_back(Entry {
            at_ms: now_ms,
            covered_ms,
            loud: is_loud_sample,
        });
        while self
            .window
            .front()
            .is_some_and(|e| e.at_ms.saturating_add(self.hold_ms) <= now_ms)
            || self.window.len() > MAX_WINDOW_ENTRIES
        {
            self.window.pop_front();
        }

        if is_loud_sample {
            if self.loud {
                return None;
            }
            let lasted = self
                .stretch_since
                .is_some_and(|s| now_ms.saturating_sub(s) >= self.hold_ms);
            let loud_cover: u64 = self
                .window
                .iter()
                .filter(|e| e.loud)
                .map(|e| e.covered_ms)
                .sum();
            if lasted && loud_cover.saturating_mul(2) >= self.hold_ms {
                self.loud = true;
                return Some(GateChange::Loud { rms });
            }
            None
        } else if self.loud && expired {
            self.loud = false;
            Some(GateChange::Quiet)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const T: f64 = 0.2;

    /// Feeds `rms` every `step` ms from `from` (exclusive) up to and including `to`.
    fn feed(g: &mut NoiseGate, from: u64, to: u64, step: u64, rms: f64) -> Vec<(u64, GateChange)> {
        let mut changes = Vec::new();
        let mut t = from + step;
        while t <= to {
            if let Some(c) = g.sample(t, rms) {
                changes.push((t, c));
            }
            t += step;
        }
        changes
    }

    #[test]
    fn fires_after_hold_and_releases_after_two_seconds() {
        let mut g = NoiseGate::new(T, 300);
        assert_eq!(g.sample(0, 0.5), None);
        assert_eq!(g.sample(200, 0.5), None);
        assert_eq!(g.sample(300, 0.6), Some(GateChange::Loud { rms: 0.6 }));
        assert_eq!(g.sample(400, 0.6), None);
        assert_eq!(g.sample(500, 0.1), None);
        assert_eq!(g.sample(2_399, 0.1), None);
        assert_eq!(g.sample(2_400, 0.1), Some(GateChange::Quiet));
        assert!(!g.is_loud());
    }

    #[test]
    fn a_single_slam_does_not_fire() {
        let mut g = NoiseGate::new(T, 5_000);
        assert_eq!(g.sample(0, 0.9), None);
        assert!(feed(&mut g, 0, 20_000, 100, 0.0).is_empty());
        assert!(!g.is_loud());
    }

    #[test]
    fn continuous_cry_fires_at_the_hold_and_not_before() {
        let mut g = NoiseGate::new(T, 5_000);
        let ch = feed(&mut g, 0, 10_000, 100, 0.5);
        // The first sample at 100 ms starts the stretch, so the hold is over 5 s later.
        assert_eq!(ch, vec![(5_100, GateChange::Loud { rms: 0.5 })]);
    }

    #[test]
    fn continuous_sound_shorter_than_the_hold_does_not_fire() {
        let mut g = NoiseGate::new(T, 5_000);
        assert!(feed(&mut g, 0, 4_000, 100, 0.5).is_empty());
        assert!(feed(&mut g, 4_000, 15_000, 100, 0.0).is_empty());
    }

    #[test]
    fn a_three_second_coughing_fit_then_quiet_does_not_fire() {
        let mut g = NoiseGate::new(T, 5_000);
        // Cough bursts: 200 ms loud, 100 ms pause.
        let mut t = 0;
        while t < 3_000 {
            g.sample(t, 0.6);
            g.sample(t + 100, 0.6);
            g.sample(t + 200, 0.0);
            t += 300;
        }
        assert!(feed(&mut g, t, t + 15_000, 100, 0.0).is_empty());
        assert!(!g.is_loud());
    }

    #[test]
    fn talking_with_half_second_pauses_fires() {
        let mut g = NoiseGate::new(T, 5_000);
        let mut fired = None;
        let mut t = 0;
        // 1 s of speech, 0.5 s pause, for 6 s.
        while t < 6_000 {
            for k in 0..15 {
                let rms = if k < 10 { 0.5 } else { 0.0 };
                if g.sample(t + k * 100, rms).is_some() {
                    fired = Some(t + k * 100);
                }
            }
            t += 1_500;
        }
        assert!(fired.is_some_and(|f| f <= 6_100), "{fired:?}");
    }

    #[test]
    fn knocks_below_half_coverage_do_not_fire() {
        let mut g = NoiseGate::new(T, 5_000);
        // 100 ms knock every 300 ms: a third of every window.
        let mut t = 0;
        while t < 60_000 {
            assert_eq!(g.sample(t, 0.9), None, "at {t}");
            g.sample(t + 100, 0.0);
            g.sample(t + 200, 0.0);
            t += 300;
        }
    }

    #[test]
    fn dense_knocking_over_half_the_window_fires() {
        let mut g = NoiseGate::new(T, 5_000);
        let mut fired = false;
        let mut t = 0;
        while t < 20_000 {
            fired |= g.sample(t, 0.9).is_some();
            g.sample(t + 100, 0.0);
            t += 200;
        }
        assert!(fired);
    }

    #[test]
    fn one_second_cadence_with_continuous_cry_fires() {
        let mut g = NoiseGate::new(T, 5_000);
        let ch = feed(&mut g, 0, 10_000, 1_000, 0.5);
        assert_eq!(ch.len(), 1);
        assert!(ch[0].0 <= 6_000, "{ch:?}");
    }

    #[test]
    fn a_tv_third_of_the_time_does_not_delay_a_cry() {
        let mut g = NoiseGate::new(T, 5_000);
        // A minute of TV: loud for the first 3 s of every 9 s, never a 2 s gap shorter than 6 s.
        let mut t = 0;
        let mut fired_in_tv = false;
        while t < 60_000 {
            let rms = if t % 9_000 < 3_000 { 0.5 } else { 0.0 };
            fired_in_tv |= g.sample(t, rms).is_some();
            t += 100;
        }
        assert!(!fired_in_tv);
        // Then the child cries continuously: loud no later than hold + one sample.
        let ch = feed(&mut g, 60_000 - 100, 60_000 + 5_100, 100, 0.5);
        assert!(g.is_loud(), "{ch:?}");
    }

    #[test]
    fn restart_during_a_loud_gate_does_not_release_it_at_once() {
        let mut g = NoiseGate::new(T, 300);
        g.sample(0, 0.5);
        assert!(matches!(g.sample(300, 0.5), Some(GateChange::Loud { .. })));
        g.restart_hold(10_000);
        assert!(g.is_loud());
        assert_eq!(g.sample(10_100, 0.0), None);
        assert_eq!(g.sample(11_900, 0.0), None);
        assert_eq!(g.sample(12_000, 0.0), Some(GateChange::Quiet));
    }

    #[test]
    fn restart_needs_a_fresh_hold() {
        let mut g = NoiseGate::new(T, 300);
        g.sample(0, 0.5);
        g.restart_hold(100);
        // 400 ms after the first loud sample, but the stretch starts over at the restart.
        assert_eq!(g.sample(400, 0.5), None);
        assert_eq!(g.sample(600, 0.5), None);
        assert!(matches!(g.sample(700, 0.5), Some(GateChange::Loud { .. })));
    }

    #[test]
    fn live_parameter_change_keeps_state() {
        let mut g = NoiseGate::new(T, 300);
        g.sample(0, 0.5);
        g.set_params(T, 100);
        assert_eq!(g.sample(100, 0.5), Some(GateChange::Loud { rms: 0.5 }));
        g.set_params(0.9, 100);
        assert_eq!(g.sample(200, 0.5), None);
        assert_eq!(g.sample(2_300, 0.5), Some(GateChange::Quiet));
        g.set_params(T, 5_000);
        assert_eq!(g.sample(2_400, 0.5), None);
        assert_eq!(g.threshold(), T);
        assert_eq!(g.hold_ms(), 5_000);
    }

    #[test]
    fn loud_state_is_not_reported_again_after_a_change() {
        let mut g = NoiseGate::new(T, 0);
        assert!(matches!(g.sample(0, 0.5), Some(GateChange::Loud { .. })));
        g.set_params(0.1, 1_000);
        assert_eq!(g.sample(100, 0.5), None);
    }

    #[test]
    fn is_loud_follows_the_reported_changes() {
        let mut g = NoiseGate::new(T, 100);
        assert!(!g.is_loud());
        assert_eq!(g.sample(0, 0.5), None);
        assert!(!g.is_loud());
        assert!(matches!(g.sample(100, 0.5), Some(GateChange::Loud { .. })));
        assert!(g.is_loud());
        assert_eq!(g.sample(200, 0.0), None);
        assert!(g.is_loud());
        assert_eq!(g.sample(2_100, 0.0), Some(GateChange::Quiet));
        assert!(!g.is_loud());
    }

    #[test]
    fn the_reported_level_is_at_or_above_the_threshold() {
        let mut g = NoiseGate::new(T, 200);
        g.sample(0, 0.3);
        g.sample(100, 0.1);
        match g.sample(200, 0.25) {
            Some(GateChange::Loud { rms }) => assert!(rms >= T),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn non_finite_samples_are_silence() {
        let mut g = NoiseGate::new(T, 0);
        assert_eq!(g.sample(0, f64::NAN), None);
        assert_eq!(g.last_rms(), 0.0);
    }

    proptest! {
        /// Whatever came before, `hold` of continuous loud sound turns the gate loud within the
        /// hold plus one sample.
        #[test]
        fn continuous_loud_fires_within_hold_plus_a_sample(
            prefix in proptest::collection::vec((1u64..=1_000, 0.0f64..1.0), 0..200),
            hold_steps in 10u64..=150,
            gap in 1u64..=1_000,
        ) {
            let hold = hold_steps * 100;
            let mut g = NoiseGate::new(0.1, hold);
            let mut t = 0;
            for (dt, rms) in prefix {
                t += dt;
                g.sample(t, rms);
            }
            let start = t;
            t += gap;
            g.sample(t, 0.9);
            let mut now = t;
            while now < t + hold + 100 {
                now += 100;
                g.sample(now, 0.9);
            }
            prop_assert!(g.is_loud(), "start {start} end {now}");
        }

        /// The gate never turns loud before `hold` has passed since the noisy stretch started.
        #[test]
        fn never_loud_before_the_hold_since_the_stretch_started(
            samples in proptest::collection::vec((1u64..=1_500, any::<bool>()), 1..400),
            hold_steps in 10u64..=150,
        ) {
            let hold = hold_steps * 100;
            let mut g = NoiseGate::new(0.1, hold);
            let mut t = 0;
            let mut last_loud: Option<u64> = None;
            let mut stretch: Option<u64> = None;
            for (dt, loud) in samples {
                t += dt;
                if last_loud.is_none_or(|l| t - l >= RELEASE_MS) {
                    stretch = None;
                }
                if loud {
                    last_loud = Some(t);
                    stretch.get_or_insert(t);
                }
                let c = g.sample(t, if loud { 0.9 } else { 0.0 });
                if matches!(c, Some(GateChange::Loud { .. })) {
                    prop_assert!(t - stretch.unwrap_or(t) >= hold);
                }
            }
        }
    }
}
