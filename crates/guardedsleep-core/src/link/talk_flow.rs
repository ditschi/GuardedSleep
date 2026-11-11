//! The talk-back flow of the [`Link`] (`add-talk-back`, specs `talk-back`, `audio-monitoring`,
//! `link-supervision`, `parent-alerts`): the parent's press, request and microphone, the baby's
//! trust gate, arbitration, chime, playback lease, noise pause and playback check.
//!
//! The decisions of the arbitration live in [`TalkArbiter`] and [`PlaybackCheck`] (`talk.rs`); this
//! file wires them to the link's inputs, actions and events. All baby-side arbitration state is in
//! one [`TalkCore`] value next to the noise gate, so `multi-parent-pairings` can lift it into the
//! hub. Every path that drops the live attempt passes `Link::close_live`, which calls
//! [`Link::teardown_talk`] first.

use std::collections::VecDeque;

use super::*;
use crate::protocol::{PlaybackResult, TalkAudible, TalkDenyReason, TalkStatus, Talker};
use crate::talk::{
    PlaybackCheck, TALK_AFTER_LISTEN_MS, TALK_ANSWER_TIMEOUT_MS, TALK_BASELINE_MS,
    TALK_CHIME_TIMEOUT_MS, TALK_DRAIN_MS, TALK_FADE_IN_MS, TALK_MAX_MS, TALK_RATE_MS,
    TALK_UNCONFIRMED_STREAK, TalkArbiter, TalkEndReason, TalkLevel, TalkState,
};

/// The warning key of talk audio that arrives outside a grant (`talk-back` "Playback only for the
/// granted link"). Its copy belongs to `add-talk-back` task 3.4.
const WARN_TALK_AUDIO_OUTSIDE_GRANT: &str = "talk_audio_outside_grant";
/// A talk level older than this does not describe the voice that plays now.
const TALK_LEVEL_FRESH_MS: u64 = 1_000;
/// A `talk_end` this close before the deadline is the baby's limit, not an early stop.
const TALK_LIMIT_SLACK_MS: u64 = 1_000;

/// The baby's talk-back settings (`set_talk_back`): local to the baby, no message changes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TalkSettings {
    enabled: bool,
    level: TalkLevel,
    chime: bool,
}

impl TalkSettings {
    pub(super) fn new(enabled: bool, level: TalkLevel, chime: bool) -> Self {
        Self {
            enabled,
            level,
            chime,
        }
    }
}

impl Default for TalkSettings {
    fn default() -> Self {
        // On, low and with the chime: the defaults of the `talk-back` settings requirement.
        Self {
            enabled: true,
            level: TalkLevel::Low,
            chime: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParentPhase {
    /// The request is out; neither a grant nor a denial yet.
    Requesting { deadline: u64 },
    /// The baby said `rate` while the button is still held: one silent retry at `at`.
    RateWait { at: u64 },
    /// Granted; the microphone was asked to attach.
    Granted { until: u64 },
    /// The microphone is attached ("Speak now").
    Talking { until: u64 },
}

#[derive(Debug, Clone, Copy)]
struct ParentTalk {
    id: u64,
    phase: ParentPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BabyPhase {
    /// The chime plays; the voice starts at `talk_chime_done` or at the deadline.
    Chime { deadline: u64 },
    /// The playback lease runs until `until`; `drain_until` is set once the parent ended the talk.
    Playing {
        until: u64,
        drain_until: Option<u64>,
    },
}

struct BabyTalk {
    attempt: u32,
    id: u64,
    phase: BabyPhase,
    /// When the voice started (the grant), for the duration the baby unit shows.
    voice_at: Option<u64>,
    check: PlaybackCheck,
    result_sent: bool,
}

/// What ended a parent talk, as the UI sees it.
#[derive(Debug, Clone, Copy)]
enum Outcome {
    Ended(TalkEndReason),
    Denied(Option<TalkDenyReason>),
}

/// Everything the link keeps for talk-back, both roles.
#[derive(Default)]
pub(super) struct TalkCore {
    // ---- baby ----
    settings: TalkSettings,
    /// The baby platform cannot make a talk audible right now (`set_talk_muted`).
    muted: bool,
    /// The baby platform captures without echo cancellation (`set_raw_capture`).
    raw: bool,
    arb: TalkArbiter,
    baby: Option<BabyTalk>,
    /// Gate samples of the last seconds while the gate is not paused: the baseline of the check.
    levels: VecDeque<(u64, f64)>,
    talk_level: Option<(u64, f64)>,
    unconfirmed_streak: u32,
    /// The attempt whose parent proved the pinned secret (or sent it). Talk needs it.
    pinned_attempt: Option<u32>,
    warned_attempt: Option<u32>,
    /// Whether a cooldown or budget wait was shown in the last status.
    blocked_shown: bool,
    // ---- parent ----
    parent: Option<ParentTalk>,
    next_id: u64,
    last_id: Option<u64>,
    rate_retried: bool,
    /// The after-talk listen of the parent ends at this time.
    listen_until: Option<u64>,
}

impl TalkCore {
    /// What survives a `stop` and `start` of the link: settings and the arbitration. Anything that
    /// belongs to an attempt is gone with it.
    pub(super) fn carry_over(&self) -> TalkCore {
        TalkCore {
            settings: self.settings,
            muted: self.muted,
            raw: self.raw,
            arb: self.arb.clone(),
            unconfirmed_streak: self.unconfirmed_streak,
            next_id: self.next_id,
            ..TalkCore::default()
        }
    }

    pub(super) fn paused(&self) -> bool {
        self.arb.is_paused()
    }
}

impl Link {
    fn live_handle(&self) -> Option<u32> {
        self.live.as_ref().map(|l| l.attempt.handle())
    }

    /// Remember that the live attempt's parent proved the pinned secret (or sent it): only then it
    /// may talk (design 3, trust gate).
    pub(super) fn mark_talk_trusted(&mut self) {
        self.talk.pinned_attempt = self.live_handle();
    }

    /// The baby accepts talk requests only from a verified attempt of a pinned parent whose proof
    /// passed (or whose pin arrived) on this attempt.
    fn talk_trusted(&self) -> bool {
        self.cfg.role == Role::Baby
            && self.live_verified()
            && self.pin.is_some()
            && self.live_handle().is_some()
            && self.live_handle() == self.talk.pinned_attempt
    }

    fn talk_line_ok(&self) -> bool {
        self.live
            .as_ref()
            .is_some_and(|l| l.attempt.talk_line_accepted() == Some(true))
    }

    fn talk_muted(&self) -> bool {
        self.talk.muted || self.platform_status.alarms_allowed == Some(false)
    }

    fn talk_status(&self) -> TalkStatus {
        if !self.talk_line_ok() {
            TalkStatus::Unsupported
        } else if !self.talk.settings.enabled {
            TalkStatus::Off
        } else if self.talk_muted() {
            TalkStatus::Muted
        } else {
            TalkStatus::On
        }
    }

    /// The talk-back fields of the baby's `device_status`.
    pub(super) fn talk_status_fields(&self, s: &mut DeviceStatus) {
        s.talk = Some(self.talk_status());
        s.talker = Some(if self.talk.baby.is_some() {
            Talker::You
        } else {
            Talker::None
        });
        s.noise_paused = self.talk.paused().then_some(true);
        s.talk_retry_in_ms = match self.talk.arb.blocked(self.now, self.gate.hold_ms()) {
            Some(TalkDenyReason::Cooldown { retry_in_ms })
            | Some(TalkDenyReason::Budget { retry_in_ms }) => Some(retry_in_ms),
            _ => None,
        };
        s.talk_audible = (self.talk.unconfirmed_streak >= TALK_UNCONFIRMED_STREAK)
            .then_some(TalkAudible::Uncertain);
    }

    // ---- inputs ----------------------------------------------------------------------------

    /// Parent: the talk button went down or up.
    pub(super) fn on_set_talk(&mut self, now: u64, pressed: bool, out: &mut Vec<LinkAction>) {
        if self.cfg.role != Role::Parent {
            return;
        }
        if pressed {
            self.press_talk(now, out);
        } else if let Some(t) = self.talk.parent {
            // A request that the baby already denied has nothing to end on its side.
            let send_end = !matches!(t.phase, ParentPhase::RateWait { .. });
            self.end_parent_talk(
                now,
                Outcome::Ended(TalkEndReason::Released),
                send_end,
                true,
                out,
            );
        }
    }

    fn press_talk(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        // Not connected, or a talk is already on: a press sends nothing.
        if self.talk.parent.is_some() || !self.live_verified() {
            return;
        }
        let Some(attempt) = self.live_handle() else {
            return;
        };
        // A press counts as acknowledgment of the running noise event, like Listen.
        if let Some(e) = self.noise_event.as_mut() {
            e.acked = true;
        }
        self.talk.next_id += 1;
        let id = self.talk.next_id;
        self.talk.rate_retried = false;
        self.talk.parent = Some(ParentTalk {
            id,
            phase: ParentPhase::Requesting {
                deadline: now + TALK_ANSWER_TIMEOUT_MS,
            },
        });
        out.push(LinkAction::SetPlaybackDucked {
            attempt,
            ducked: true,
        });
        self.send_app(&DataMessage::TalkRequest { id }, out);
        emit(
            out,
            LinkEvent::TalkChanged {
                state: TalkState::Requesting,
            },
        );
    }

    /// Parent: the platform attached the microphone to the talk sender.
    pub(super) fn on_talk_mic_attached(&mut self, out: &mut Vec<LinkAction>) {
        if let Some(t) = self.talk.parent.as_mut()
            && let ParentPhase::Granted { until } = t.phase
        {
            t.phase = ParentPhase::Talking { until };
            emit(
                out,
                LinkEvent::TalkChanged {
                    state: TalkState::Talking { until_ms: until },
                },
            );
        }
    }

    /// Parent: the microphone failed or its permission was revoked.
    pub(super) fn on_talk_mic_failed(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        if let Some(t) = self.talk.parent {
            let send_end = !matches!(t.phase, ParentPhase::RateWait { .. });
            self.end_parent_talk(
                now,
                Outcome::Ended(TalkEndReason::MicFailed),
                send_end,
                true,
                out,
            );
        }
    }

    /// Parent: an alert sound or vibration is due; the talk ends first.
    pub(super) fn on_alert_preempts_talk(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        if let Some(t) = self.talk.parent {
            let send_end = !matches!(t.phase, ParentPhase::RateWait { .. });
            self.end_parent_talk(
                now,
                Outcome::Ended(TalkEndReason::Alert),
                send_end,
                false,
                out,
            );
        }
    }

    /// Whether a parent talk runs (a test alert is unavailable meanwhile).
    pub(super) fn parent_talking(&self) -> bool {
        self.talk.parent.is_some()
    }

    /// Parent: the user took over listening, so the after-talk timer must not end it.
    pub(super) fn talk_listen_taken_over(&mut self) {
        self.talk.listen_until = None;
    }

    /// Baby: the chime ended.
    pub(super) fn on_talk_chime_done(&mut self, now: u64, attempt: u32, out: &mut Vec<LinkAction>) {
        if self
            .talk
            .baby
            .as_ref()
            .is_some_and(|t| t.attempt == attempt && matches!(t.phase, BabyPhase::Chime { .. }))
        {
            self.start_talk_playback(now, out);
        }
    }

    /// Baby: new talk-back settings. Switching it off ends a running talk at once.
    pub(super) fn on_set_talk_back(
        &mut self,
        now: u64,
        settings: TalkSettings,
        out: &mut Vec<LinkAction>,
    ) {
        if self.cfg.role != Role::Baby {
            return;
        }
        self.talk.settings = settings;
        // An adult is at the settings: the warning about three unconfirmed talks has done its job.
        self.talk.unconfirmed_streak = 0;
        if !settings.enabled && self.talk.baby.is_some() {
            self.stop_baby_talk(now, true, true, out);
        }
        self.flush_status(out);
    }

    pub(super) fn on_talk_settings_opened(&mut self, out: &mut Vec<LinkAction>) {
        if self.cfg.role == Role::Baby && self.talk.unconfirmed_streak != 0 {
            self.talk.unconfirmed_streak = 0;
            self.flush_status(out);
        }
    }

    pub(super) fn on_set_talk_muted(&mut self, muted: bool, out: &mut Vec<LinkAction>) {
        if self.cfg.role == Role::Baby && self.talk.muted != muted {
            self.talk.muted = muted;
            self.flush_status(out);
        }
    }

    pub(super) fn on_set_raw_capture(&mut self, raw: bool) {
        if self.cfg.role == Role::Baby {
            self.talk.raw = raw;
        }
    }

    pub(super) fn on_talk_level(&mut self, now: u64, rms: f64) {
        if self.cfg.role == Role::Baby {
            self.talk.talk_level = Some((now, rms));
        }
    }

    /// Baby: the media layer saw talk RTP outside a lease. Once per attempt, without content.
    pub(super) fn on_talk_rtp_outside_grant(&mut self, attempt: u32, out: &mut Vec<LinkAction>) {
        if self.cfg.role != Role::Baby
            || self.live_handle() != Some(attempt)
            || self.talk.warned_attempt == Some(attempt)
            || self
                .talk
                .baby
                .as_ref()
                .is_some_and(|t| t.attempt == attempt)
        {
            return;
        }
        self.talk.warned_attempt = Some(attempt);
        emit(
            out,
            LinkEvent::SecurityWarning {
                key: WARN_TALK_AUDIO_OUTSIDE_GRANT.to_owned(),
            },
        );
    }

    // ---- parent: messages from the baby ----------------------------------------------------

    pub(super) fn on_talk_grant(
        &mut self,
        now: u64,
        id: u64,
        max_ms: u64,
        out: &mut Vec<LinkAction>,
    ) {
        let Some(attempt) = self.live_handle() else {
            return;
        };
        let Some(t) = self.talk.parent.as_mut() else {
            return;
        };
        if t.id != id || !matches!(t.phase, ParentPhase::Requesting { .. }) {
            return;
        }
        let until = now + max_ms.clamp(1, TALK_MAX_MS);
        t.phase = ParentPhase::Granted { until };
        out.push(LinkAction::SetTalkMic {
            attempt,
            enabled: true,
        });
    }

    pub(super) fn on_talk_deny(
        &mut self,
        now: u64,
        id: u64,
        reason: Option<TalkDenyReason>,
        out: &mut Vec<LinkAction>,
    ) {
        let Some(t) = self.talk.parent.as_mut() else {
            return;
        };
        if t.id != id || !matches!(t.phase, ParentPhase::Requesting { .. }) {
            return;
        }
        // `rate` while the button is still held: one silent retry a second later.
        if reason == Some(TalkDenyReason::Rate) && !self.talk.rate_retried {
            self.talk.rate_retried = true;
            t.phase = ParentPhase::RateWait {
                at: now + TALK_RATE_MS,
            };
            return;
        }
        self.end_parent_talk(now, Outcome::Denied(reason), false, false, out);
    }

    /// The baby ended the talk (its limit, or talk-back was switched off there).
    pub(super) fn on_parent_talk_end(&mut self, now: u64, id: u64, out: &mut Vec<LinkAction>) {
        let Some(t) = self.talk.parent else {
            return;
        };
        if t.id != id {
            return;
        }
        let reason = match t.phase {
            ParentPhase::Granted { until } | ParentPhase::Talking { until }
                if now + TALK_LIMIT_SLACK_MS >= until =>
            {
                TalkEndReason::Limit
            }
            _ => TalkEndReason::SwitchedOff,
        };
        self.end_parent_talk(now, Outcome::Ended(reason), false, true, out);
    }

    pub(super) fn on_talk_playback(
        &mut self,
        id: u64,
        result: PlaybackResult,
        out: &mut Vec<LinkAction>,
    ) {
        // The result comes seconds into a talk, so the parent may have let go already.
        if self.talk.parent.is_some_and(|t| t.id == id) || self.talk.last_id == Some(id) {
            emit(out, LinkEvent::TalkPlayback { result });
        }
    }

    // ---- parent: ending a talk -------------------------------------------------------------

    /// Ends the parent's talk: `talk_end` if the baby may still play, microphone and ducking off,
    /// the after-talk listen, the event. Every ending goes through here.
    fn end_parent_talk(
        &mut self,
        now: u64,
        outcome: Outcome,
        send_end: bool,
        listen: bool,
        out: &mut Vec<LinkAction>,
    ) {
        let Some(t) = self.talk.parent.take() else {
            return;
        };
        self.talk.last_id = Some(t.id);
        let Some(attempt) = self.live_handle() else {
            return;
        };
        if send_end {
            self.send_app(&DataMessage::TalkEnd { id: t.id }, out);
        }
        out.push(LinkAction::SetTalkMic {
            attempt,
            enabled: false,
        });
        out.push(LinkAction::SetPlaybackDucked {
            attempt,
            ducked: false,
        });
        if listen {
            self.start_after_listen(now, out);
        }
        let state = match outcome {
            Outcome::Ended(reason) => TalkState::Ended { reason },
            Outcome::Denied(reason) => TalkState::Denied { reason },
        };
        emit(out, LinkEvent::TalkChanged { state });
    }

    /// A parent that was not listening listens for [`TALK_AFTER_LISTEN_MS`] after a talk, as a
    /// normal listen intent that the user can keep or end.
    fn start_after_listen(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        if !self.live_verified() {
            return;
        }
        let ours = self.talk.listen_until.is_some();
        if self.listen_intent && !ours {
            // The parent listens on purpose: the previous state simply resumes.
            return;
        }
        if !self.listen_intent {
            self.listen_intent = true;
            if let Some(e) = self.noise_event.as_mut() {
                e.acked = true;
            }
            self.send_app(&DataMessage::Listen { enabled: true }, out);
            emit(out, LinkEvent::ListenChanged { enabled: true });
        }
        self.talk.listen_until = Some(now + TALK_AFTER_LISTEN_MS);
    }

    // ---- baby: requests --------------------------------------------------------------------

    /// Baby: a `talk_request` from the (trusted) parent.
    pub(super) fn on_talk_request(&mut self, now: u64, id: u64, out: &mut Vec<LinkAction>) {
        if !self.talk_trusted() {
            return;
        }
        let Some(attempt) = self.live_handle() else {
            return;
        };
        // A repeat of the running talk's request is nothing new.
        if self.talk.baby.as_ref().is_some_and(|t| t.id == id) {
            return;
        }
        let verdict = self.talk_verdict(now);
        if let Err(reason) = verdict {
            self.send_app(
                &DataMessage::TalkDeny {
                    id,
                    reason: Some(reason),
                },
                out,
            );
            // The cooldown or budget wait just changed what the parents may see.
            self.flush_status(out);
            return;
        }
        // Accepted: the noise gate pauses from the chime on. The room before the talk is the
        // baseline of the playback check; the `mic_silent` count starts over.
        self.talk.arb.begin_pause(now);
        self.zero_since = None;
        let recent: Vec<(u64, f64)> = self.talk.levels.iter().copied().collect();
        let check = PlaybackCheck::start(
            now,
            &recent,
            self.gate.threshold(),
            self.gate.is_loud(),
            self.talk.raw,
            self.talk.settings.chime,
        );
        let chime = self.talk.settings.chime;
        self.talk.baby = Some(BabyTalk {
            attempt,
            id,
            phase: BabyPhase::Chime {
                deadline: now + TALK_CHIME_TIMEOUT_MS,
            },
            voice_at: None,
            check,
            result_sent: false,
        });
        emit(out, LinkEvent::NoisePaused { paused: true });
        if chime {
            out.push(LinkAction::PlayTalkChime {
                attempt,
                level: self.talk.settings.level,
            });
            self.flush_status(out);
        } else {
            self.start_talk_playback(now, out);
        }
    }

    /// The baby's answer to a request, in this order: the per-link rate, a parent without a talk
    /// line, the switch, honest playability, then the arbitration (one talker, cooldown, budget).
    fn talk_verdict(&mut self, now: u64) -> Result<(), TalkDenyReason> {
        self.talk.arb.rate(now)?;
        if !self.talk_line_ok() {
            return Err(TalkDenyReason::Unsupported);
        }
        if !self.talk.settings.enabled {
            return Err(TalkDenyReason::Off);
        }
        if self.talk_muted() {
            return Err(TalkDenyReason::Muted);
        }
        self.talk.arb.admit(now, self.gate.hold_ms())
    }

    /// The voice starts: the lease, the grant and the indicator.
    fn start_talk_playback(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        let level = self.talk.settings.level;
        let Some(t) = self.talk.baby.as_mut() else {
            return;
        };
        let until = now + TALK_MAX_MS;
        t.phase = BabyPhase::Playing {
            until,
            drain_until: None,
        };
        t.voice_at = Some(now);
        t.check.voice_started(now);
        let (attempt, id) = (t.attempt, t.id);
        out.push(LinkAction::SetTalkPlayback {
            attempt,
            enabled: true,
            level,
            fade_in_ms: TALK_FADE_IN_MS,
            until_ms: until,
        });
        self.send_app(
            &DataMessage::TalkGrant {
                id,
                max_ms: TALK_MAX_MS,
            },
            out,
        );
        emit(
            out,
            LinkEvent::TalkActive {
                active: true,
                duration_ms: 0,
            },
        );
        self.flush_status(out);
    }

    /// Baby: the parent ended the talk.
    pub(super) fn on_baby_talk_end(&mut self, now: u64, id: u64, out: &mut Vec<LinkAction>) {
        let Some(t) = self.talk.baby.as_mut() else {
            return;
        };
        if t.id != id {
            return;
        }
        match t.phase {
            // Released before the grant: the chime stops, nothing else plays.
            BabyPhase::Chime { .. } => self.stop_baby_talk(now, true, false, out),
            BabyPhase::Playing { until, drain_until } => {
                if drain_until.is_none() {
                    t.phase = BabyPhase::Playing {
                        until,
                        drain_until: Some(now + TALK_DRAIN_MS),
                    };
                }
            }
        }
    }

    /// Stops the baby's talk: playback off (this also cancels a running chime), `talk_end` to the
    /// parent when asked and the link is up, the playback result, the indicator, and the 2 s tail
    /// of the noise pause.
    fn stop_baby_talk(&mut self, now: u64, live: bool, send_end: bool, out: &mut Vec<LinkAction>) {
        let Some(mut t) = self.talk.baby.take() else {
            return;
        };
        out.push(LinkAction::SetTalkPlayback {
            attempt: t.attempt,
            enabled: false,
            level: self.talk.settings.level,
            fade_in_ms: 0,
            until_ms: now,
        });
        if live {
            if send_end {
                self.send_app(&DataMessage::TalkEnd { id: t.id }, out);
            }
            if t.voice_at.is_some() && !t.result_sent {
                let result = t.check.finish();
                self.report_playback(t.id, result, out);
            }
        }
        if let Some(at) = t.voice_at {
            emit(
                out,
                LinkEvent::TalkActive {
                    active: false,
                    duration_ms: now.saturating_sub(at),
                },
            );
        }
        self.talk.arb.playback_ended(now);
        if live {
            self.flush_status(out);
        }
    }

    /// Sends the playback result of a talk and counts it: three unconfirmed talks in a row make
    /// the warning, a heard one clears it, an unknown one counts for nothing.
    fn report_playback(&mut self, id: u64, result: PlaybackResult, out: &mut Vec<LinkAction>) {
        self.send_app(&DataMessage::TalkPlayback { id, result }, out);
        match result {
            PlaybackResult::Heard => self.talk.unconfirmed_streak = 0,
            PlaybackResult::Unconfirmed => {
                self.talk.unconfirmed_streak = self.talk.unconfirmed_streak.saturating_add(1);
            }
            PlaybackResult::Unknown => {}
        }
    }

    // ---- baby: levels ----------------------------------------------------------------------

    /// One level sample of the baby's microphone. While the gate is paused it feeds the playback
    /// check and nothing else; otherwise it is kept as the baseline of the next check. Returns
    /// `true` when the pause consumed the sample.
    pub(super) fn talk_audio_level(&mut self, now: u64, rms: f64) -> bool {
        if self.talk.arb.is_paused() {
            let voice = self
                .talk
                .talk_level
                .filter(|(t, _)| now.saturating_sub(*t) <= TALK_LEVEL_FRESH_MS)
                .map(|(_, l)| l);
            if let Some(t) = self.talk.baby.as_mut() {
                t.check.sample(now, rms, voice);
            }
            return true;
        }
        self.talk.levels.push_back((now, rms));
        while self
            .talk
            .levels
            .front()
            .is_some_and(|(t, _)| now.saturating_sub(*t) > TALK_BASELINE_MS + 500)
        {
            self.talk.levels.pop_front();
        }
        false
    }

    // ---- housekeeping and teardown ---------------------------------------------------------

    /// Timers of both roles: the answer timeout, the limit, the silent `rate` retry, the end of the
    /// after-talk listen (parent); the chime timeout, the drain, the lease deadline, the playback
    /// window and the end of the pause (baby).
    pub(super) fn talk_housekeeping(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        match self.cfg.role {
            Role::Parent => self.parent_talk_housekeeping(now, out),
            Role::Baby => self.baby_talk_housekeeping(now, out),
        }
    }

    fn parent_talk_housekeeping(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        if let Some(t) = self.talk.parent {
            match t.phase {
                ParentPhase::Requesting { deadline } if now >= deadline => self.end_parent_talk(
                    now,
                    Outcome::Ended(TalkEndReason::NoAnswer),
                    true,
                    false,
                    out,
                ),
                ParentPhase::Granted { until } | ParentPhase::Talking { until } if now >= until => {
                    self.end_parent_talk(now, Outcome::Ended(TalkEndReason::Limit), true, true, out)
                }
                ParentPhase::RateWait { at } if now >= at => {
                    self.talk.next_id += 1;
                    let id = self.talk.next_id;
                    if let Some(t) = self.talk.parent.as_mut() {
                        t.id = id;
                        t.phase = ParentPhase::Requesting {
                            deadline: now + TALK_ANSWER_TIMEOUT_MS,
                        };
                    }
                    self.send_app(&DataMessage::TalkRequest { id }, out);
                }
                _ => {}
            }
        }
        if self.talk.listen_until.is_some_and(|u| now >= u) {
            self.talk.listen_until = None;
            if self.listen_intent {
                self.listen_intent = false;
                self.send_app(&DataMessage::Listen { enabled: false }, out);
                emit(out, LinkEvent::ListenChanged { enabled: false });
            }
        }
    }

    fn baby_talk_housekeeping(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        let live = self.live_verified();
        let snapshot = self
            .talk
            .baby
            .as_ref()
            .map(|t| (t.phase, t.id, t.result_sent));
        if let Some((phase, id, result_sent)) = snapshot {
            match phase {
                BabyPhase::Chime { deadline } if now >= deadline => {
                    self.start_talk_playback(now, out);
                }
                BabyPhase::Playing { until, .. } if now >= until => {
                    // The deadline: the parent's `talk_end` may never come.
                    self.stop_baby_talk(now, live, true, out);
                }
                BabyPhase::Playing {
                    drain_until: Some(d),
                    ..
                } if now >= d => self.stop_baby_talk(now, live, false, out),
                _ if !result_sent => {
                    // The result of the check comes once its window is over, not at the end.
                    let result = self
                        .talk
                        .baby
                        .as_mut()
                        .and_then(|t| t.check.poll(now).inspect(|_| t.result_sent = true));
                    if let Some(result) = result {
                        self.report_playback(id, result, out);
                    }
                }
                _ => {}
            }
        }
        let hold = self.gate.hold_ms();
        if self.talk.arb.tick(now, hold) {
            // The gate restarts its hold time, and `mic_silent` counts again from here.
            self.gate.restart_hold();
            self.zero_since = None;
            self.talk.talk_level = None;
            emit(out, LinkEvent::NoisePaused { paused: false });
            self.flush_status(out);
        }
        let blocked = self.talk.arb.blocked(now, hold).is_some();
        if blocked != self.talk.blocked_shown {
            self.talk.blocked_shown = blocked;
            self.flush_status(out);
        }
    }

    /// The link is being stopped on purpose: a running talk ends in the open, with `talk_end`.
    pub(super) fn end_talk_for_stop(&mut self, out: &mut Vec<LinkAction>) {
        let now = self.now;
        match self.cfg.role {
            Role::Parent => self.end_parent_talk(
                now,
                Outcome::Ended(TalkEndReason::Stopped),
                true,
                false,
                out,
            ),
            Role::Baby => {
                let live = self.live_verified();
                self.stop_baby_talk(now, live, true, out);
            }
        }
    }

    /// Called first thing by `close_live`, the one place every teardown of the live attempt passes:
    /// the speaker stops, the parent's microphone closes, the talk is gone on both sides. Nothing is
    /// sent (the attempt is going away); the noise pause keeps its tail.
    pub(super) fn teardown_talk(&mut self, out: &mut Vec<LinkAction>) {
        let now = self.now;
        if self.talk.baby.is_some() {
            self.stop_baby_talk(now, false, false, out);
        }
        if self.talk.parent.is_some() {
            self.end_parent_talk(
                now,
                Outcome::Ended(TalkEndReason::LinkLost),
                false,
                false,
                out,
            );
        }
        self.talk.pinned_attempt = None;
        // A pause shown by the baby ends with the link, for the parent that reads it.
        if self.cfg.role == Role::Parent
            && let Some(s) = self.peer_status.as_mut()
            && s.noise_paused == Some(true)
        {
            s.noise_paused = None;
            emit(out, LinkEvent::NoisePaused { paused: false });
        }
    }
}
