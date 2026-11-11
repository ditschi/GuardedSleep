//! Talk-back decisions of the core (`talk-back`, `audio-monitoring` and `link-supervision` specs of
//! the `add-talk-back` change): the vocabulary shared by the `Link` and its bindings, the baby's
//! arbitration (one talker, rate, cooldown, budget, the noise pause) and the playback check.
//!
//! Sans-IO like the rest of the core: time only comes in as arguments, nothing here sends or plays
//! anything. The arbitration lives in [`TalkArbiter`], one value that belongs to the noise gate,
//! so `multi-parent-pairings` can lift it from the single `Link` into the hub unchanged.

// Time and levels flow through here: return values instead of panicking (AGENTS.md section 5.3).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

use serde::{Deserialize, Serialize};

use crate::protocol::{PlaybackResult, TalkDenyReason};

/// The baby unit ends a talk by itself this long after granting it, whatever the parent does.
pub const TALK_MAX_MS: u64 = 30_000;
/// At most one talk request per link per this long.
pub const TALK_RATE_MS: u64 = 1_000;
/// The parent gives up on a request that got neither a grant nor a denial after this long.
pub const TALK_ANSWER_TIMEOUT_MS: u64 = 3_000;
/// The baby starts the voice after this long even if the chime never reported its end.
pub const TALK_CHIME_TIMEOUT_MS: u64 = 1_500;
/// After `talk_end` the baby keeps playing this long, so the last words are not cut.
pub const TALK_DRAIN_MS: u64 = 300;
/// Noise detection stays paused this long after playback ended (the speaker's tail).
pub const TALK_NOISE_TAIL_MS: u64 = 2_000;
/// After every pause, new talks are denied until the gate ran unpaused for at least this long.
pub const TALK_COOLDOWN_MIN_MS: u64 = 10_000;
/// All talk pauses together stay within this budget ...
pub const TALK_BUDGET_MS: u64 = 90_000;
/// ... in any window of this length.
pub const TALK_BUDGET_WINDOW_MS: u64 = 600_000;
/// A parent that was not listening listens this long after releasing the talk button.
pub const TALK_AFTER_LISTEN_MS: u64 = 10_000;
/// The voice fades in over this long on the baby unit.
pub const TALK_FADE_IN_MS: u64 = 800;

/// The level rise (dB) over the room before the talk that counts as "heard". Tuned in the hardware
/// round (`add-talk-back` task 2.1 (f)).
pub const TALK_HEARD_MIN_DB: f64 = 6.0;
/// The room level before a talk is the median of the samples of this long before the chime.
pub const TALK_BASELINE_MS: u64 = 2_000;
/// Fewer samples than this before the chime (the gate had no 2 s of unpaused level yet): unknown.
pub const TALK_BASELINE_MIN_SAMPLES: usize = 8;
/// A baseline whose samples spread more than this (dB, highest to lowest) is unstable: unknown.
pub const TALK_BASELINE_UNSTABLE_DB: f64 = 10.0;
/// With the chime: the voice part of the check window is the first this long of playback.
pub const TALK_CHECK_VOICE_MS: u64 = 3_000;
/// Without the chime: the check window is the first this long of playback.
pub const TALK_CHECK_NO_CHIME_MS: u64 = 5_000;
/// A talk level (the incoming voice, `talk_level`) at or below this is silence: the parent said
/// nothing, so the voice part of the check does not judge.
pub const TALK_VOICE_FLOOR: f64 = 0.005;
/// Levels are compared in dB with this floor, so digital silence has a finite level.
pub const TALK_LEVEL_FLOOR: f64 = 0.0001;
/// This many unconfirmed talks in a row set `talk_audible: uncertain`.
pub const TALK_UNCONFIRMED_STREAK: u32 = 3;

/// Talk-back volume of the baby unit, a setting of the baby platform (`set_talk_back`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub enum TalkLevel {
    Low,
    Medium,
    High,
}

/// Why a talk ended on the parent (`TalkState::Ended`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub enum TalkEndReason {
    /// The parent let go of the button.
    Released,
    /// The 30 second limit.
    Limit,
    /// Neither a grant nor a denial arrived in time.
    NoAnswer,
    /// The attempt closed or was declared lost.
    LinkLost,
    /// The microphone failed or its permission was revoked.
    MicFailed,
    /// An alert sound on the parent unit took precedence.
    Alert,
    /// The baby unit ended it early (talk-back switched off there).
    SwitchedOff,
    /// This link was stopped.
    Stopped,
}

/// The talk state a parent UI shows (`LinkEvent::TalkChanged`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub enum TalkState {
    /// "One moment": the request is on its way.
    Requesting,
    /// "Speak now": the microphone is attached; `until_ms` is the end of the limit on the core's
    /// clock.
    Talking {
        until_ms: u64,
    },
    Ended {
        reason: TalkEndReason,
    },
    /// The baby unit said no; the reason is `None` when it was unknown.
    Denied {
        reason: Option<TalkDenyReason>,
    },
}

/// The pause of the noise gate that one talk causes, and what limits how often it may happen.
///
/// A pause starts when the baby accepts a request (the chime), ends [`TALK_NOISE_TAIL_MS`] after
/// playback ended or the talk was abandoned, and is followed by a cooldown. A talk released before
/// its grant is a short pause like any other and counts toward the budget.
#[derive(Debug, Clone, Default)]
pub struct TalkArbiter {
    /// The last request, for the per-link rate limit.
    last_request: Option<u64>,
    /// The running pause.
    pause: Option<Pause>,
    /// Finished pauses `(start, end)` of the last budget window, oldest first.
    done: Vec<(u64, u64)>,
    /// New talks are denied until then (set when a pause ends).
    cooldown_until: u64,
}

#[derive(Debug, Clone, Copy)]
struct Pause {
    start: u64,
    /// `None` while the talk runs; the end of the tail once playback ended.
    end_at: Option<u64>,
}

/// `max(10 s, hold + 1 s)`: how long the gate must have run unpaused before the next talk.
pub fn cooldown_ms(hold_ms: u64) -> u64 {
    TALK_COOLDOWN_MIN_MS.max(hold_ms.saturating_add(1_000))
}

impl TalkArbiter {
    /// Rate limit per link: at most one request per [`TALK_RATE_MS`]. Every request counts, also a
    /// denied one, so a flood stays denied.
    pub fn rate(&mut self, now: u64) -> Result<(), TalkDenyReason> {
        let ok = self
            .last_request
            .is_none_or(|t| now.saturating_sub(t) >= TALK_RATE_MS);
        self.last_request = Some(now);
        if ok {
            Ok(())
        } else {
            Err(TalkDenyReason::Rate)
        }
    }

    /// One talker, then the cooldown, then the budget. `Ok` means the request may start a pause.
    pub fn admit(&mut self, now: u64, hold_ms: u64) -> Result<(), TalkDenyReason> {
        self.prune(now);
        match self.pause {
            Some(Pause { end_at: None, .. }) => return Err(TalkDenyReason::Busy),
            // The speaker's tail still counts as the pause; the cooldown starts when it ends.
            Some(Pause {
                end_at: Some(end), ..
            }) => {
                return Err(TalkDenyReason::Cooldown {
                    retry_in_ms: end.saturating_sub(now).saturating_add(cooldown_ms(hold_ms)),
                });
            }
            None => {}
        }
        if now < self.cooldown_until {
            return Err(TalkDenyReason::Cooldown {
                retry_in_ms: self.cooldown_until - now,
            });
        }
        if self.used_ms(now) >= TALK_BUDGET_MS {
            return Err(TalkDenyReason::Budget {
                retry_in_ms: self.budget_retry_ms(now),
            });
        }
        Ok(())
    }

    /// Why a request would not pass right now, ignoring the rate and a running talk: the status the
    /// baby shows (`talk_retry_in_ms`).
    pub fn blocked(&self, now: u64, hold_ms: u64) -> Option<TalkDenyReason> {
        let mut probe = self.clone();
        match probe.admit(now, hold_ms) {
            Err(TalkDenyReason::Busy) | Ok(()) => None,
            Err(reason) => Some(reason),
        }
    }

    /// A talk was accepted: the noise gate is paused from now on.
    pub fn begin_pause(&mut self, now: u64) {
        self.pause = Some(Pause {
            start: now,
            end_at: None,
        });
    }

    /// Playback ended (or the talk was abandoned): the gate stays paused for the tail.
    pub fn playback_ended(&mut self, now: u64) {
        if let Some(p) = self.pause.as_mut()
            && p.end_at.is_none()
        {
            p.end_at = Some(now.saturating_add(TALK_NOISE_TAIL_MS));
        }
    }

    /// The noise gate is paused (a talk runs or its tail has not passed).
    pub fn is_paused(&self) -> bool {
        self.pause.is_some()
    }

    /// A talk runs (a pause without a tail yet).
    pub fn talk_running(&self) -> bool {
        matches!(self.pause, Some(Pause { end_at: None, .. }))
    }

    /// Ends the pause when its tail has passed and starts the cooldown. `true` exactly once per
    /// pause: the caller restarts the gate's hold time and tells the parents.
    pub fn tick(&mut self, now: u64, hold_ms: u64) -> bool {
        let Some(Pause {
            start,
            end_at: Some(end),
        }) = self.pause
        else {
            return false;
        };
        if now < end {
            return false;
        }
        self.pause = None;
        self.done.push((start, end));
        self.cooldown_until = end.saturating_add(cooldown_ms(hold_ms));
        self.prune(now);
        true
    }

    /// Milliseconds of noise detection paused within the budget window ending at `now`.
    pub fn used_ms(&self, now: u64) -> u64 {
        let from = now.saturating_sub(TALK_BUDGET_WINDOW_MS);
        let overlap = |start: u64, end: u64| end.min(now).saturating_sub(start.max(from));
        let running = self
            .pause
            .map_or(0, |p| overlap(p.start, p.end_at.unwrap_or(now)));
        self.done
            .iter()
            .fold(running, |sum, (s, e)| sum.saturating_add(overlap(*s, *e)))
    }

    /// How long until the used budget drops below [`TALK_BUDGET_MS`] as old pauses age out of the
    /// window.
    fn budget_retry_ms(&self, now: u64) -> u64 {
        let from = now.saturating_sub(TALK_BUDGET_WINDOW_MS);
        let mut excess = self
            .used_ms(now)
            .saturating_sub(TALK_BUDGET_MS)
            .saturating_add(1);
        for (s, e) in &self.done {
            let start = (*s).max(from);
            let len = (*e).saturating_sub(start);
            if len == 0 {
                continue;
            }
            if excess <= len {
                // The piece starting at `start` leaves the window one millisecond per millisecond
                // from `start + window` on.
                return (start + TALK_BUDGET_WINDOW_MS + excess).saturating_sub(now);
            }
            excess -= len;
        }
        TALK_BUDGET_WINDOW_MS
    }

    fn prune(&mut self, now: u64) {
        let from = now.saturating_sub(TALK_BUDGET_WINDOW_MS);
        self.done.retain(|(_, end)| *end > from);
    }
}

fn db(rms: f64) -> f64 {
    let x = if rms.is_finite() { rms.max(0.0) } else { 0.0 };
    20.0 * x.max(TALK_LEVEL_FLOOR).log10()
}

fn median(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return 0.0;
    }
    let lo = sorted.get((n - 1) / 2).copied().unwrap_or(0.0);
    let hi = sorted.get(n / 2).copied().unwrap_or(lo);
    (lo + hi) / 2.0
}

/// The playback check of one talk (`talk-back` "Playback check"): the baby's own microphone against
/// the room before the chime. Level numbers only; nothing is kept beyond the peaks.
#[derive(Debug, Clone)]
pub struct PlaybackCheck {
    chime: bool,
    /// The room level before the talk in dB; `None` when the check cannot decide.
    baseline_db: Option<f64>,
    /// The voice has started (the chime, if any, is over).
    voice_at: Option<u64>,
    chime_peak_db: Option<f64>,
    voice_peak_db: Option<f64>,
    result: Option<PlaybackResult>,
}

impl PlaybackCheck {
    /// Starts the check when the chime would start. `recent` are the `(time, rms)` samples of the
    /// gate before now (the caller keeps them while the gate is not paused), `threshold` the noise
    /// threshold, `noise_event` whether the gate is loud, `raw` whether the baby platform captures
    /// without echo cancellation, `chime` whether a chime plays.
    pub fn start(
        now: u64,
        recent: &[(u64, f64)],
        threshold: f64,
        noise_event: bool,
        raw: bool,
        chime: bool,
    ) -> Self {
        let from = now.saturating_sub(TALK_BASELINE_MS);
        let mut levels: Vec<f64> = recent
            .iter()
            .filter(|(t, _)| *t >= from && *t <= now)
            .map(|(_, r)| db(*r))
            .collect();
        levels.sort_by(|a, b| a.total_cmp(b));
        let spread = match (levels.first(), levels.last()) {
            (Some(lo), Some(hi)) => hi - lo,
            _ => f64::INFINITY,
        };
        let baseline = median(&levels);
        let decidable = raw
            && !noise_event
            && levels.len() >= TALK_BASELINE_MIN_SAMPLES
            && spread <= TALK_BASELINE_UNSTABLE_DB
            && baseline < db(threshold);
        Self {
            chime,
            baseline_db: decidable.then_some(baseline),
            voice_at: None,
            chime_peak_db: None,
            voice_peak_db: None,
            result: None,
        }
    }

    /// The chime is over and the voice starts now.
    pub fn voice_started(&mut self, now: u64) {
        self.voice_at.get_or_insert(now);
    }

    /// One level sample of the baby's microphone (about 10 Hz) with the latest level of the
    /// incoming talk, if one was reported.
    pub fn sample(&mut self, now: u64, rms: f64, talk_level: Option<f64>) {
        if self.result.is_some() {
            return;
        }
        let level = db(rms);
        let peak = |slot: &mut Option<f64>| *slot = Some(slot.map_or(level, |p| p.max(level)));
        match self.voice_at {
            None => {
                if self.chime {
                    peak(&mut self.chime_peak_db);
                }
            }
            Some(at) => {
                let window = if self.chime {
                    TALK_CHECK_VOICE_MS
                } else {
                    TALK_CHECK_NO_CHIME_MS
                };
                // A sample after the window belongs to nobody; `poll` decides.
                if now.saturating_sub(at) <= window
                    && talk_level.is_some_and(|l| l.is_finite() && l > TALK_VOICE_FLOOR)
                {
                    peak(&mut self.voice_peak_db);
                }
            }
        }
    }

    /// The result once the window is over; `None` before. Decides at most once.
    pub fn poll(&mut self, now: u64) -> Option<PlaybackResult> {
        if self.result.is_none()
            && let Some(at) = self.voice_at
        {
            let window = if self.chime {
                TALK_CHECK_VOICE_MS
            } else {
                TALK_CHECK_NO_CHIME_MS
            };
            if now.saturating_sub(at) >= window {
                self.result = Some(self.decide());
            }
        }
        self.result
    }

    /// The talk ended: decides with what was seen so far (once).
    pub fn finish(&mut self) -> PlaybackResult {
        let decided = self.result.unwrap_or_else(|| self.decide());
        self.result = Some(decided);
        decided
    }

    fn decide(&self) -> PlaybackResult {
        let Some(baseline) = self.baseline_db else {
            return PlaybackResult::Unknown;
        };
        let peak = match (self.chime_peak_db, self.voice_peak_db) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        match peak {
            // A parent who said nothing (no chime either) is not judged.
            None => PlaybackResult::Unknown,
            Some(p) if p - baseline >= TALK_HEARD_MIN_DB => PlaybackResult::Heard,
            Some(_) => PlaybackResult::Unconfirmed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOLD: u64 = 800;

    #[test]
    fn constants_match_the_design() {
        assert_eq!(TALK_MAX_MS, 30_000);
        assert_eq!(TALK_RATE_MS, 1_000);
        assert_eq!(TALK_ANSWER_TIMEOUT_MS, 3_000);
        assert_eq!(TALK_CHIME_TIMEOUT_MS, 1_500);
        assert_eq!(TALK_DRAIN_MS, 300);
        assert_eq!(TALK_NOISE_TAIL_MS, 2_000);
        assert_eq!(TALK_COOLDOWN_MIN_MS, 10_000);
        assert_eq!(TALK_BUDGET_MS, 90_000);
        assert_eq!(TALK_BUDGET_WINDOW_MS, 600_000);
        assert_eq!(TALK_AFTER_LISTEN_MS, 10_000);
        assert_eq!(TALK_FADE_IN_MS, 800);
        assert_eq!(TALK_HEARD_MIN_DB, 6.0);
    }

    #[test]
    fn the_cooldown_is_ten_seconds_or_hold_plus_one() {
        assert_eq!(cooldown_ms(800), 10_000);
        assert_eq!(cooldown_ms(9_000), 10_000);
        assert_eq!(cooldown_ms(9_001), 10_001);
        assert_eq!(cooldown_ms(15_000), 16_000);
    }

    #[test]
    fn the_rate_limit_counts_denied_requests_too() {
        let mut a = TalkArbiter::default();
        assert_eq!(a.rate(0), Ok(()));
        assert_eq!(a.rate(999), Err(TalkDenyReason::Rate));
        // The denied request at 999 restarts the second.
        assert_eq!(a.rate(1_500), Err(TalkDenyReason::Rate));
        assert_eq!(a.rate(2_500), Ok(()));
    }

    #[test]
    fn one_talker_a_tail_then_the_cooldown() {
        let mut a = TalkArbiter::default();
        assert_eq!(a.admit(0, HOLD), Ok(()));
        a.begin_pause(0);
        assert!(a.is_paused() && a.talk_running());
        assert_eq!(a.admit(1_000, HOLD), Err(TalkDenyReason::Busy));
        // Playback ends at 5 s; the tail runs to 7 s, the cooldown to 17 s.
        a.playback_ended(5_000);
        assert!(!a.talk_running() && a.is_paused());
        assert_eq!(
            a.admit(5_500, HOLD),
            Err(TalkDenyReason::Cooldown {
                retry_in_ms: 1_500 + 10_000
            })
        );
        assert!(!a.tick(6_999, HOLD));
        assert!(a.tick(7_000, HOLD));
        assert!(!a.tick(7_001, HOLD), "the end of a pause is reported once");
        assert!(!a.is_paused());
        assert_eq!(
            a.admit(8_000, HOLD),
            Err(TalkDenyReason::Cooldown { retry_in_ms: 9_000 })
        );
        assert_eq!(a.admit(17_000, HOLD), Ok(()));
    }

    #[test]
    fn a_long_hold_time_lengthens_the_cooldown() {
        let mut a = TalkArbiter::default();
        a.begin_pause(0);
        a.playback_ended(0);
        assert!(a.tick(2_000, 15_000));
        assert_eq!(
            a.admit(2_000, 15_000),
            Err(TalkDenyReason::Cooldown {
                retry_in_ms: 16_000
            })
        );
        assert_eq!(a.admit(18_000, 15_000), Ok(()));
    }

    #[test]
    fn a_talk_released_at_once_still_counts_toward_the_budget() {
        let mut a = TalkArbiter::default();
        a.begin_pause(1_000);
        a.playback_ended(1_000);
        assert!(a.tick(3_000, HOLD));
        // 1 s of pause plus the 2 s tail.
        assert_eq!(a.used_ms(3_000), 2_000);
        assert_eq!(a.used_ms(60_000), 2_000);
    }

    #[test]
    fn the_budget_denies_after_ninety_seconds_and_says_when_it_frees_up() {
        let mut a = TalkArbiter::default();
        // Three 30 s pauses (28 s of talk plus the 2 s tail), 20 s apart: 90 s at 100 s.
        let mut t = 0;
        for _ in 0..3 {
            assert_eq!(a.admit(t, HOLD), Ok(()));
            a.begin_pause(t);
            a.playback_ended(t + 28_000);
            assert!(a.tick(t + 30_000, HOLD));
            t += 50_000;
        }
        // Now = 150 s; used 90 s: the oldest pause (0..30 s) frees up first.
        let now = 150_000;
        assert_eq!(a.used_ms(now), 90_000);
        let Err(TalkDenyReason::Budget { retry_in_ms }) = a.admit(now, HOLD) else {
            panic!("budget")
        };
        // One millisecond below the budget is reached 1 ms after the window passed the start
        // of the oldest pause.
        assert_eq!(retry_in_ms, 600_000 + 1 - now);
        assert!(a.used_ms(now + retry_in_ms) < TALK_BUDGET_MS);
        assert_eq!(a.admit(now + retry_in_ms, HOLD), Ok(()));
        assert!(a.used_ms(now + retry_in_ms - 1) >= TALK_BUDGET_MS);
    }

    #[test]
    fn the_budget_hint_walks_over_several_old_pauses() {
        let mut a = TalkArbiter::default();
        // 23 pauses of 4 s (2 s of talk and the tail), 20 s apart: 92 s in the window.
        for i in 0..23 {
            let t = i * 20_000;
            a.begin_pause(t);
            a.playback_ended(t + 2_000);
            assert!(a.tick(t + 4_000, HOLD));
        }
        let now = 460_000;
        let Err(TalkDenyReason::Budget { retry_in_ms }) = a.admit(now, HOLD) else {
            panic!("budget");
        };
        // 2 s over the budget (92 s used): the first pause leaves the window one millisecond per
        // millisecond from `start + window` on, so it takes 2 001 ms of it to get below.
        assert_eq!(a.used_ms(now), 92_000);
        assert_eq!(retry_in_ms, 600_000 + 2_001 - now);
        assert!(a.used_ms(now + retry_in_ms) < TALK_BUDGET_MS);
        assert!(a.used_ms(now + retry_in_ms - 1) >= TALK_BUDGET_MS);
    }

    #[test]
    fn old_pauses_leave_the_window() {
        let mut a = TalkArbiter::default();
        a.begin_pause(0);
        a.playback_ended(28_000);
        assert!(a.tick(30_000, HOLD));
        assert_eq!(a.used_ms(30_000), 30_000);
        assert_eq!(a.used_ms(630_000), 0);
        assert_eq!(a.admit(630_000, HOLD), Ok(()));
        assert!(a.done.is_empty(), "pruned");
    }

    #[test]
    fn blocked_reports_the_cooldown_and_the_budget_but_not_a_running_talk() {
        let mut a = TalkArbiter::default();
        assert_eq!(a.blocked(0, HOLD), None);
        a.begin_pause(0);
        assert_eq!(a.blocked(1_000, HOLD), None, "a running talk is not a wait");
        a.playback_ended(1_000);
        assert!(matches!(
            a.blocked(1_500, HOLD),
            Some(TalkDenyReason::Cooldown { .. })
        ));
        assert!(a.tick(3_000, HOLD));
        assert_eq!(
            a.blocked(3_000, HOLD),
            Some(TalkDenyReason::Cooldown {
                retry_in_ms: 10_000
            })
        );
        assert_eq!(a.blocked(13_000, HOLD), None);
    }

    /// Ten samples of `rms` in the 2 s before 10 000 ms.
    fn room(rms: f64) -> Vec<(u64, f64)> {
        (0..=10).map(|i| (8_000 + i * 200, rms)).collect()
    }

    fn check(room: &[(u64, f64)], chime: bool) -> PlaybackCheck {
        PlaybackCheck::start(10_000, room, 0.12, false, true, chime)
    }

    /// Feeds `rms` for `ms` after `from` in 100 ms steps, with a loud incoming voice.
    fn feed(c: &mut PlaybackCheck, from: u64, ms: u64, rms: f64, voice: Option<f64>) {
        for i in 0..=ms / 100 {
            c.sample(from + i * 100, rms, voice);
        }
    }

    #[test]
    fn a_louder_room_during_the_chime_is_heard() {
        let mut c = check(&room(0.01), true);
        feed(&mut c, 10_000, 600, 0.05, None);
        c.voice_started(10_600);
        feed(&mut c, 10_600, 3_000, 0.01, Some(0.3));
        assert_eq!(c.poll(13_600), Some(PlaybackResult::Heard));
        assert_eq!(c.finish(), PlaybackResult::Heard);
    }

    #[test]
    fn the_voice_alone_can_make_a_talk_heard() {
        let mut c = check(&room(0.01), true);
        feed(&mut c, 10_000, 600, 0.01, None);
        c.voice_started(10_600);
        feed(&mut c, 10_600, 3_000, 0.04, Some(0.3));
        assert_eq!(c.poll(13_600), Some(PlaybackResult::Heard));
    }

    #[test]
    fn a_room_that_stays_as_quiet_is_unconfirmed() {
        let mut c = check(&room(0.01), true);
        feed(&mut c, 10_000, 600, 0.011, None);
        c.voice_started(10_600);
        feed(&mut c, 10_600, 3_000, 0.012, Some(0.3));
        assert_eq!(c.poll(13_500), None, "the window is still open");
        assert_eq!(c.poll(13_600), Some(PlaybackResult::Unconfirmed));
    }

    #[test]
    fn without_a_chime_a_silent_parent_is_not_judged() {
        let mut c = check(&room(0.01), false);
        c.voice_started(10_000);
        feed(&mut c, 10_000, 5_000, 0.011, Some(0.001));
        assert_eq!(c.poll(14_999), None);
        assert_eq!(c.poll(15_000), Some(PlaybackResult::Unknown));
    }

    #[test]
    fn without_a_chime_the_voice_samples_decide() {
        let mut quiet = check(&room(0.01), false);
        quiet.voice_started(10_000);
        feed(&mut quiet, 10_000, 5_000, 0.011, Some(0.3));
        assert_eq!(quiet.poll(15_000), Some(PlaybackResult::Unconfirmed));
        let mut loud = check(&room(0.01), false);
        loud.voice_started(10_000);
        feed(&mut loud, 10_000, 5_000, 0.05, Some(0.3));
        assert_eq!(loud.poll(15_000), Some(PlaybackResult::Heard));
    }

    #[test]
    fn the_check_cannot_decide_without_raw_capture_or_in_a_loud_or_unstable_room() {
        let loud_after = |c: &mut PlaybackCheck| {
            feed(c, 10_000, 600, 0.5, None);
            c.voice_started(10_600);
            feed(c, 10_600, 3_000, 0.5, Some(0.3));
            c.finish()
        };
        // not raw
        let mut c = PlaybackCheck::start(10_000, &room(0.01), 0.12, false, false, true);
        assert_eq!(loud_after(&mut c), PlaybackResult::Unknown);
        // a noise event runs
        let mut c = PlaybackCheck::start(10_000, &room(0.01), 0.12, true, true, true);
        assert_eq!(loud_after(&mut c), PlaybackResult::Unknown);
        // the room is at the noise threshold (a white-noise machine)
        let mut c = check(&room(0.12), true);
        assert_eq!(loud_after(&mut c), PlaybackResult::Unknown);
        // unstable: a crying child
        let crying: Vec<_> = (0..=10)
            .map(|i| (8_000 + i * 200, if i % 2 == 0 { 0.02 } else { 0.1 }))
            .collect();
        let mut c = check(&crying, true);
        assert_eq!(loud_after(&mut c), PlaybackResult::Unknown);
        // too few samples before the chime
        let mut c = check(&room(0.01)[..4], true);
        assert_eq!(loud_after(&mut c), PlaybackResult::Unknown);
        // samples older than the baseline window do not count
        let old: Vec<_> = (0..=10).map(|i| (i * 200, 0.01)).collect();
        let mut c = check(&old, true);
        assert_eq!(loud_after(&mut c), PlaybackResult::Unknown);
    }

    #[test]
    fn a_digital_silence_baseline_still_decides() {
        let mut c = check(&room(0.0), true);
        feed(&mut c, 10_000, 600, 0.02, None);
        assert_eq!(c.finish(), PlaybackResult::Heard);
    }

    #[test]
    fn a_talk_that_ends_early_decides_with_what_was_seen() {
        let mut c = check(&room(0.01), true);
        feed(&mut c, 10_000, 300, 0.011, None);
        assert_eq!(c.finish(), PlaybackResult::Unconfirmed);
        // A result is final.
        feed(&mut c, 10_400, 300, 0.5, None);
        assert_eq!(c.finish(), PlaybackResult::Unconfirmed);
        assert_eq!(c.poll(99_000), Some(PlaybackResult::Unconfirmed));
        // Aborted before any sample: nothing to judge.
        let mut c = check(&room(0.01), true);
        assert_eq!(c.finish(), PlaybackResult::Unknown);
    }

    #[test]
    fn samples_after_the_window_do_not_count() {
        let mut c = check(&room(0.01), true);
        feed(&mut c, 10_000, 600, 0.011, None);
        c.voice_started(10_600);
        feed(&mut c, 10_600, 3_000, 0.011, Some(0.3));
        // A loud sample 4 s into the voice is outside the 3 s window.
        c.sample(14_700, 0.9, Some(0.3));
        assert_eq!(c.poll(14_700), Some(PlaybackResult::Unconfirmed));
    }

    #[test]
    fn levels_are_in_db_with_a_floor() {
        assert!((db(1.0) - 0.0).abs() < 1e-9);
        assert!((db(0.1) + 20.0).abs() < 1e-9);
        assert!((db(0.0) + 80.0).abs() < 1e-9);
        assert!((db(f64::NAN) + 80.0).abs() < 1e-9);
        assert_eq!(median(&[]), 0.0);
        assert_eq!(median(&[1.0, 3.0]), 2.0);
        assert_eq!(median(&[1.0, 2.0, 9.0]), 2.0);
    }

    #[test]
    fn the_vocabulary_round_trips_through_json() {
        let states = [
            TalkState::Requesting,
            TalkState::Talking { until_ms: 5 },
            TalkState::Ended {
                reason: TalkEndReason::NoAnswer,
            },
            TalkState::Denied {
                reason: Some(TalkDenyReason::Budget { retry_in_ms: 3 }),
            },
            TalkState::Denied { reason: None },
        ];
        for s in states {
            let json = serde_json::to_string(&s).unwrap();
            assert_eq!(
                serde_json::from_str::<TalkState>(&json).unwrap(),
                s,
                "{json}"
            );
        }
        assert_eq!(
            serde_json::to_string(&TalkState::Talking { until_ms: 5 }).unwrap(),
            r#"{"state":"talking","until_ms":5}"#
        );
        assert_eq!(
            serde_json::to_string(&TalkLevel::High).unwrap(),
            r#""high""#
        );
    }
}
