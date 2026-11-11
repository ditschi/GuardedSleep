//! Talk-back end to end between two `Link`s (`add-talk-back` specs `talk-back`, `audio-monitoring`,
//! `link-supervision`, `parent-alerts`): grant and every denial, the trust gate, the noise pause,
//! the cooldown and the budget, the playback check, and teardown on every path that drops the live
//! attempt.

mod common;

use common::*;
use guardedsleep_core::link::{LinkAction, LinkEvent, LinkInput, LinkState};
use guardedsleep_core::protocol::{
    DeviceStatus, PlaybackResult, TalkAudible, TalkDenyReason, TalkStatus, Talker,
};
use guardedsleep_core::talk::{TalkEndReason, TalkLevel, TalkState};
use serde_json::{Value, json};

const HOLD_MS: u64 = 300;

// ---- drivers ----------------------------------------------------------------------------------

/// A connected pair whose baby platform captures raw and has heard 3 s of a quiet room.
fn talk_world() -> World {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.input(BABY, LinkInput::SetRawCapture { raw: true });
    w.connect_pair();
    room(&mut w, 3_000, 0.01);
    w
}

/// The baby's microphone level for `ms`, in 100 ms steps, with the parent's voice at `voice`.
fn level(w: &mut World, ms: u64, room: f64, voice: Option<f64>) {
    for _ in 0..ms / 100 {
        w.advance_by(100, 100);
        w.input(BABY, LinkInput::AudioLevel { rms: room });
        if let Some(v) = voice {
            w.input(BABY, LinkInput::TalkLevel { rms: v });
        }
    }
}

fn room(w: &mut World, ms: u64, rms: f64) {
    level(w, ms, rms, None);
}

fn attempt(w: &World, n: usize) -> u32 {
    w.nodes[n].link.live_attempt().expect("live attempt")
}

fn press(w: &mut World) {
    w.input(PARENT, LinkInput::SetTalk { pressed: true });
}

fn release(w: &mut World) {
    w.input(PARENT, LinkInput::SetTalk { pressed: false });
}

fn chime_done(w: &mut World) {
    let attempt = attempt(w, BABY);
    w.input(BABY, LinkInput::TalkChimeDone { attempt });
}

/// Press, let the chime play for 600 ms at `chime_room`, finish it and attach the microphone. The
/// talk is running afterwards; returns the time of the grant.
fn start_talk(w: &mut World, chime_room: f64) -> u64 {
    press(w);
    level(w, 600, chime_room, None);
    chime_done(w);
    let granted = w.now;
    w.input(PARENT, LinkInput::TalkMicAttached);
    granted
}

fn parent_states(w: &World) -> Vec<TalkState> {
    w.nodes[PARENT]
        .events
        .iter()
        .filter_map(|e| match e {
            LinkEvent::TalkChanged { state } => Some(*state),
            _ => None,
        })
        .collect()
}

fn last_state(w: &World) -> Option<TalkState> {
    parent_states(w).last().copied()
}

fn actions(w: &World, n: usize) -> Vec<LinkAction> {
    w.nodes[n]
        .talk_actions
        .iter()
        .map(|(_, a)| a.clone())
        .collect()
}

fn playback_calls(w: &World) -> Vec<(u64, bool, u64)> {
    w.nodes[BABY]
        .talk_actions
        .iter()
        .filter_map(|(t, a)| match a {
            LinkAction::SetTalkPlayback {
                enabled, until_ms, ..
            } => Some((*t, *enabled, *until_ms)),
            _ => None,
        })
        .collect()
}

fn mic_calls(w: &World) -> Vec<bool> {
    actions(w, PARENT)
        .iter()
        .filter_map(|a| match a {
            LinkAction::SetTalkMic { enabled, .. } => Some(*enabled),
            _ => None,
        })
        .collect()
}

fn data(w: &World, n: usize, ty: &str) -> Vec<(u64, Value)> {
    w.nodes[n]
        .data_sent
        .iter()
        .filter(|(_, v)| v["type"] == ty)
        .cloned()
        .collect()
}

fn denied(w: &World) -> Vec<Option<TalkDenyReason>> {
    parent_states(w)
        .into_iter()
        .filter_map(|s| match s {
            TalkState::Denied { reason } => Some(reason),
            _ => None,
        })
        .collect()
}

fn baby_events(w: &World, f: impl Fn(&LinkEvent) -> bool) -> usize {
    w.nodes[BABY].events.iter().filter(|e| f(e)).count()
}

fn noise_alerts(w: &World) -> usize {
    w.nodes[PARENT]
        .events
        .iter()
        .filter(|e| matches!(e, LinkEvent::NoiseAlert { probe: false, .. }))
        .count()
}

fn last_peer_status(w: &World) -> DeviceStatus {
    w.nodes[PARENT]
        .events
        .iter()
        .rev()
        .find_map(|e| match e {
            LinkEvent::PeerStatus { status } => Some(status.clone()),
            _ => None,
        })
        .expect("a peer status")
}

fn settings(w: &mut World, enabled: bool, chime: bool) {
    w.input(
        BABY,
        LinkInput::SetTalkBack {
            enabled,
            level: TalkLevel::Medium,
            chime,
        },
    );
}

fn raw(msg: Value) -> String {
    msg.to_string()
}

// ---- grant and release --------------------------------------------------------------------------

#[test]
fn a_held_talk_runs_from_the_chime_to_the_release_and_the_baby_stays_silent_otherwise() {
    let mut w = talk_world();
    // Before any talk: no talk action on either side, and the baby reports talk-back as on.
    assert!(w.nodes[BABY].talk_actions.is_empty());
    let status = last_peer_status(&w);
    assert_eq!(status.talk, Some(TalkStatus::On));
    assert_eq!(status.talker, Some(Talker::None));
    assert_eq!(status.noise_paused, None);

    press(&mut w);
    assert_eq!(last_state(&w), Some(TalkState::Requesting));
    // The press ducks the baby's audio and the baby plays the chime.
    assert!(matches!(
        actions(&w, PARENT).as_slice(),
        [LinkAction::SetPlaybackDucked { ducked: true, .. }]
    ));
    assert!(matches!(
        actions(&w, BABY).as_slice(),
        [LinkAction::PlayTalkChime {
            level: TalkLevel::Low,
            ..
        }]
    ));
    assert_eq!(
        data(&w, PARENT, "talk_request").len(),
        1,
        "one request, with the parent's id"
    );
    // Nothing plays and no grant goes out until the chime reported its end.
    assert!(playback_calls(&w).is_empty());
    assert!(data(&w, BABY, "talk_grant").is_empty());

    level(&mut w, 600, 0.01, None);
    chime_done(&mut w);
    let granted_at = w.now;
    let calls = playback_calls(&w);
    assert_eq!(
        calls,
        vec![(granted_at, true, granted_at + 30_000)],
        "a lease"
    );
    let Some(LinkAction::SetTalkPlayback {
        level: lvl,
        fade_in_ms,
        attempt: a,
        ..
    }) = actions(&w, BABY).pop()
    else {
        panic!("playback action");
    };
    assert_eq!(
        (lvl, fade_in_ms, a),
        (TalkLevel::Low, 800, attempt(&w, BABY))
    );
    let grants = data(&w, BABY, "talk_grant");
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].1["max_ms"], 30_000);
    assert_eq!(
        grants[0].1["id"],
        data(&w, PARENT, "talk_request")[0].1["id"]
    );
    assert_eq!(
        mic_calls(&w),
        vec![true],
        "the grant opens the parent's microphone"
    );
    // "Speak now" only once the microphone is attached.
    assert_eq!(last_state(&w), Some(TalkState::Requesting));
    w.input(PARENT, LinkInput::TalkMicAttached);
    assert_eq!(
        last_state(&w),
        Some(TalkState::Talking {
            until_ms: granted_at + 30_000
        })
    );
    assert_eq!(
        baby_events(&w, |e| matches!(
            e,
            LinkEvent::TalkActive { active: true, .. }
        )),
        1
    );
    let status = last_peer_status(&w);
    assert_eq!(status.talker, Some(Talker::You));
    assert_eq!(status.noise_paused, Some(true));

    // Release: talk_end, the microphone closes, the baby's audio resumes, a 10 s listen starts.
    level(&mut w, 5_000, 0.01, Some(0.3));
    let released_at = w.now;
    release(&mut w);
    assert_eq!(
        last_state(&w),
        Some(TalkState::Ended {
            reason: TalkEndReason::Released
        })
    );
    assert_eq!(data(&w, PARENT, "talk_end").len(), 1);
    assert_eq!(mic_calls(&w), vec![true, false]);
    assert!(matches!(
        actions(&w, PARENT).last(),
        Some(LinkAction::SetPlaybackDucked { ducked: false, .. })
    ));
    assert_eq!(
        data(&w, PARENT, "listen").last().unwrap().1["enabled"],
        true
    );
    // The baby plays for the drain, then closes the lease.
    level(&mut w, 200, 0.01, Some(0.3));
    assert_eq!(playback_calls(&w).len(), 1, "still draining");
    level(&mut w, 200, 0.01, Some(0.3));
    let calls = playback_calls(&w);
    assert_eq!(calls.len(), 2);
    assert!(!calls[1].1 && calls[1].0 >= released_at + 300);
    assert_eq!(
        baby_events(&w, |e| matches!(
            e,
            LinkEvent::TalkActive {
                active: false,
                duration_ms
            } if *duration_ms >= 5_000
        )),
        1
    );
    // The listen ends 10 s after the release.
    let before = w.mark(PARENT);
    room(&mut w, 9_000, 0.01);
    assert_eq!(
        w.count_events(PARENT, before, |e| matches!(
            e,
            LinkEvent::ListenChanged { enabled: false }
        )),
        0
    );
    room(&mut w, 2_000, 0.01);
    assert_eq!(
        w.count_events(PARENT, before, |e| matches!(
            e,
            LinkEvent::ListenChanged { enabled: false }
        )),
        1
    );
    assert_eq!(
        data(&w, PARENT, "listen").last().unwrap().1["enabled"],
        false
    );
}

// ---- trust gate ---------------------------------------------------------------------------------

#[test]
fn a_verified_attempt_before_the_pin_gets_no_talk() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.nodes[PARENT].drop_parent_pin = true;
    w.input(BABY, LinkInput::SetRawCapture { raw: true });
    w.connect_pair();
    room(&mut w, 3_000, 0.01);
    press(&mut w);
    room(&mut w, 4_000, 0.01);
    // The baby ignores the request: no chime, no grant, no denial, nothing plays.
    assert!(w.nodes[BABY].talk_actions.is_empty());
    assert!(data(&w, BABY, "talk_grant").is_empty() && data(&w, BABY, "talk_deny").is_empty());
    // The parent gives up after 3 s with "no answer" and tells the baby to stop.
    assert_eq!(
        parent_states(&w),
        vec![
            TalkState::Requesting,
            TalkState::Ended {
                reason: TalkEndReason::NoAnswer
            }
        ]
    );
    assert_eq!(data(&w, PARENT, "talk_end").len(), 1);
    assert_eq!(mic_calls(&w), vec![false]);
}

#[test]
fn a_press_without_a_link_sends_nothing() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.start(PARENT, 2);
    press(&mut w);
    assert!(w.nodes[PARENT].talk_actions.is_empty());
    assert!(parent_states(&w).is_empty());
    w.input(PARENT, LinkInput::SetTalk { pressed: false });
    assert!(parent_states(&w).is_empty());
}

// ---- denials ------------------------------------------------------------------------------------

fn assert_denied(w: &World, reason: TalkDenyReason) {
    assert_eq!(denied(w), vec![Some(reason)]);
    assert!(
        w.nodes[BABY].talk_actions.is_empty(),
        "a denial plays nothing: {:?}",
        w.nodes[BABY].talk_actions
    );
    assert!(data(w, BABY, "talk_grant").is_empty());
    // A denial leaves the parent's microphone and ducking off again.
    assert_eq!(mic_calls(w), vec![false]);
    assert!(matches!(
        actions(w, PARENT).last(),
        Some(LinkAction::SetPlaybackDucked { ducked: false, .. })
    ));
}

#[test]
fn a_baby_with_talk_back_off_denies_and_says_off() {
    let mut w = talk_world();
    settings(&mut w, false, true);
    assert_eq!(last_peer_status(&w).talk, Some(TalkStatus::Off));
    press(&mut w);
    assert_denied(&w, TalkDenyReason::Off);
    assert_eq!(
        baby_events(&w, |e| matches!(e, LinkEvent::NoisePaused { .. })),
        0
    );
}

#[test]
fn a_parent_without_a_talk_line_is_unsupported() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.nodes[PARENT].parent_talk_direction = "inactive";
    w.input(BABY, LinkInput::SetRawCapture { raw: true });
    w.connect_pair();
    // The attempt verified as usual.
    assert_eq!(w.state(BABY), LinkState::Connected);
    assert_eq!(last_peer_status(&w).talk, Some(TalkStatus::Unsupported));
    press(&mut w);
    assert_denied(&w, TalkDenyReason::Unsupported);
}

#[test]
fn a_baby_that_cannot_play_sound_says_muted_and_denies() {
    // The user's own Do Not Disturb blocks alarm sounds.
    let mut w = talk_world();
    w.status_input(
        BABY,
        DeviceStatus {
            alarms_allowed: Some(false),
            ..Default::default()
        },
    );
    assert_eq!(last_peer_status(&w).talk, Some(TalkStatus::Muted));
    press(&mut w);
    assert_denied(&w, TalkDenyReason::Muted);

    // The platform could not set the volume.
    let mut w = talk_world();
    w.input(BABY, LinkInput::SetTalkMuted { muted: true });
    assert_eq!(last_peer_status(&w).talk, Some(TalkStatus::Muted));
    press(&mut w);
    assert_denied(&w, TalkDenyReason::Muted);
    w.input(BABY, LinkInput::SetTalkMuted { muted: false });
    assert_eq!(last_peer_status(&w).talk, Some(TalkStatus::On));
}

#[test]
fn a_second_request_while_a_talk_runs_is_busy_and_the_talk_continues() {
    let mut w = talk_world();
    start_talk(&mut w, 0.01);
    w.inject_data(BABY, &raw(json!({"type":"talk_request","v":3,"id":900})));
    let denies = data(&w, BABY, "talk_deny");
    // One second apart from the first request is needed for the rate limit to pass.
    assert_eq!(denies.len(), 1);
    assert_eq!(denies[0].1["reason"], "rate");
    level(&mut w, 1_000, 0.01, Some(0.3));
    w.inject_data(BABY, &raw(json!({"type":"talk_request","v":3,"id":901})));
    let denies = data(&w, BABY, "talk_deny");
    assert_eq!(denies.len(), 2);
    assert_eq!(
        (denies[1].1["id"].as_u64(), denies[1].1["reason"].as_str()),
        (Some(901), Some("busy"))
    );
    assert_eq!(playback_calls(&w).len(), 1, "A's talk goes on");
}

#[test]
fn requests_come_at_most_once_a_second_and_a_held_button_retries_a_rate_denial_once_silently() {
    let mut w = talk_world();
    // A request the baby accepts and the parent withdraws at once (id 100); a press 500 ms later
    // runs into the rate limit.
    w.inject_data(BABY, &raw(json!({"type":"talk_request","v":3,"id":100})));
    w.inject_data(BABY, &raw(json!({"type":"talk_end","v":3,"id":100})));
    assert!(data(&w, BABY, "talk_deny").is_empty());
    level(&mut w, 500, 0.01, None);
    press(&mut w);
    assert_eq!(data(&w, BABY, "talk_deny")[0].1["reason"], "rate");
    // The parent keeps showing "one moment" and asks again after a second.
    assert_eq!(parent_states(&w), vec![TalkState::Requesting]);
    assert_eq!(data(&w, PARENT, "talk_request").len(), 1);
    level(&mut w, 1_000, 0.01, None);
    assert_eq!(data(&w, PARENT, "talk_request").len(), 2);
    // The second answer is the cooldown that follows the withdrawn talk: no retry loop.
    assert!(matches!(
        denied(&w).as_slice(),
        [Some(TalkDenyReason::Cooldown { .. })]
    ));
    level(&mut w, 3_000, 0.01, None);
    assert_eq!(data(&w, PARENT, "talk_request").len(), 2);
}

// ---- timers: deadline, answer timeout, release before the grant -----------------------------------

#[test]
fn the_baby_ends_a_talk_30_seconds_after_the_grant_whatever_the_parent_does() {
    let mut w = talk_world();
    let granted = start_talk(&mut w, 0.01);
    // The parent never lets go; 29.7 s later the speaker is still on, then it is not.
    level(&mut w, 29_700, 0.01, Some(0.3));
    assert_eq!(playback_calls(&w).len(), 1);
    level(&mut w, 600, 0.01, Some(0.3));
    let calls = playback_calls(&w);
    assert_eq!(calls.len(), 2);
    assert!(!calls[1].1);
    assert!(calls[1].0 >= granted + 30_000 && calls[1].0 <= granted + 30_500);
    // The baby tells the parent, who ends with the limit; the microphone closes.
    assert_eq!(data(&w, BABY, "talk_end").len(), 1);
    assert_eq!(
        last_state(&w),
        Some(TalkState::Ended {
            reason: TalkEndReason::Limit
        })
    );
    assert_eq!(mic_calls(&w), vec![true, false]);
}

#[test]
fn a_parent_that_releases_before_the_grant_never_sends_audio_and_the_chime_stops() {
    let mut w = talk_world();
    press(&mut w);
    level(&mut w, 200, 0.01, None);
    release(&mut w);
    // The baby cancels the chime and never grants.
    let calls = playback_calls(&w);
    assert_eq!(calls.len(), 1);
    assert!(!calls[0].1);
    assert!(data(&w, BABY, "talk_grant").is_empty());
    assert_eq!(
        baby_events(&w, |e| matches!(e, LinkEvent::TalkActive { .. })),
        0
    );
    // The parent's microphone was only ever told to close.
    assert_eq!(mic_calls(&w), vec![false]);
    // A late chime report starts nothing.
    chime_done(&mut w);
    assert_eq!(playback_calls(&w).len(), 1);
    assert!(data(&w, BABY, "talk_grant").is_empty());
    // The talk is a short pause like any other, with its tail and cooldown.
    level(&mut w, 1_000, 0.01, None);
    assert_eq!(last_peer_status(&w).noise_paused, Some(true));
    level(&mut w, 2_000, 0.01, None);
    assert_eq!(last_peer_status(&w).noise_paused, None);
    assert!(last_peer_status(&w).talk_retry_in_ms.is_some());
}

#[test]
fn the_chime_that_never_reports_its_end_is_cut_after_1500_ms() {
    let mut w = talk_world();
    press(&mut w);
    level(&mut w, 1_400, 0.01, None);
    assert!(data(&w, BABY, "talk_grant").is_empty());
    level(&mut w, 200, 0.01, None);
    assert_eq!(data(&w, BABY, "talk_grant").len(), 1);
}

#[test]
fn without_a_chime_the_voice_starts_at_once() {
    let mut w = talk_world();
    settings(&mut w, true, false);
    press(&mut w);
    assert!(
        !actions(&w, BABY)
            .iter()
            .any(|a| matches!(a, LinkAction::PlayTalkChime { .. }))
    );
    assert_eq!(data(&w, BABY, "talk_grant").len(), 1);
}

// ---- teardown on every path that drops the live attempt ------------------------------------------

/// A running talk, then `trigger`; the baby's playback closes before its attempt does, and the
/// parent's microphone closes with its attempt.
fn teardown_case(trigger: impl FnOnce(&mut World), parent_first: bool) {
    let mut w = talk_world();
    start_talk(&mut w, 0.01);
    level(&mut w, 1_000, 0.01, Some(0.3));
    let (baby_ix, parent_ix) = (
        w.nodes[BABY].talk_actions.len(),
        w.nodes[PARENT].talk_actions.len(),
    );
    trigger(&mut w);
    let baby: Vec<_> = w.nodes[BABY].talk_actions[baby_ix..]
        .iter()
        .map(|(_, a)| a.clone())
        .collect();
    let close = baby
        .iter()
        .position(|a| matches!(a, LinkAction::CloseAttempt { .. }))
        .expect("the baby's attempt closed");
    let stop = baby
        .iter()
        .position(|a| matches!(a, LinkAction::SetTalkPlayback { enabled: false, .. }))
        .expect("the baby's playback stopped");
    assert!(stop < close, "stop first: {baby:?}");
    let parent: Vec<_> = w.nodes[PARENT].talk_actions[parent_ix..]
        .iter()
        .map(|(_, a)| a.clone())
        .collect();
    assert!(
        parent
            .iter()
            .any(|a| matches!(a, LinkAction::SetTalkMic { enabled: false, .. })),
        "{parent:?}"
    );
    assert!(matches!(last_state(&w), Some(TalkState::Ended { .. })));
    let _ = parent_first;
}

#[test]
fn every_path_that_drops_the_live_attempt_stops_the_speaker_and_the_microphone() {
    // The data channel closes on the baby.
    teardown_case(
        |w| {
            let a = attempt(w, BABY);
            w.input(BABY, LinkInput::AttemptDataClosed { attempt: a });
        },
        false,
    );
    // The peer connection fails on the baby.
    teardown_case(
        |w| {
            let a = attempt(w, BABY);
            w.input(
                BABY,
                LinkInput::AttemptPcState {
                    attempt: a,
                    state: "failed".into(),
                },
            );
        },
        false,
    );
    // The parent stops on purpose: the baby gets the stop notice.
    teardown_case(|w| w.stop(PARENT), true);
    // The baby stops on purpose.
    teardown_case(|w| w.stop(BABY), false);
    // Liveness: the parent's network dies, nothing arrives for 25 s.
    teardown_case(
        |w| {
            w.set_net(PARENT, false);
            level(w, 40_000, 0.01, None);
        },
        false,
    );
}

#[test]
fn a_lost_link_ends_the_parent_side_with_link_lost_and_no_listen() {
    let mut w = talk_world();
    start_talk(&mut w, 0.01);
    let a = attempt(&w, PARENT);
    w.input(PARENT, LinkInput::AttemptDataClosed { attempt: a });
    assert_eq!(
        last_state(&w),
        Some(TalkState::Ended {
            reason: TalkEndReason::LinkLost
        })
    );
    assert_eq!(mic_calls(&w), vec![true, false]);
    assert!(
        !w.nodes[PARENT]
            .events
            .iter()
            .any(|e| matches!(e, LinkEvent::ListenChanged { enabled: true }))
    );
}

#[test]
fn a_stop_ends_the_talk_in_the_open_with_talk_end() {
    let mut w = talk_world();
    start_talk(&mut w, 0.01);
    w.stop(PARENT);
    assert_eq!(data(&w, PARENT, "talk_end").len(), 1);
    assert_eq!(
        last_state(&w),
        Some(TalkState::Ended {
            reason: TalkEndReason::Stopped
        })
    );
}

// ---- interruptions on the parent -------------------------------------------------------------------

#[test]
fn an_alert_a_failed_microphone_and_a_switch_off_end_the_talk() {
    // An alert sound on the parent takes precedence: talk_end, microphone off, no listen.
    let mut w = talk_world();
    start_talk(&mut w, 0.01);
    w.input(PARENT, LinkInput::AlertPreemptsTalk);
    assert_eq!(
        last_state(&w),
        Some(TalkState::Ended {
            reason: TalkEndReason::Alert
        })
    );
    assert_eq!(data(&w, PARENT, "talk_end").len(), 1);
    assert_eq!(mic_calls(&w), vec![true, false]);
    assert!(
        !data(&w, PARENT, "listen")
            .iter()
            .any(|(_, m)| m["enabled"] == true)
    );
    // A permission revoked or a failed microphone ends it too, and the listen follows.
    let mut w = talk_world();
    start_talk(&mut w, 0.01);
    w.input(PARENT, LinkInput::TalkMicFailed);
    assert_eq!(
        last_state(&w),
        Some(TalkState::Ended {
            reason: TalkEndReason::MicFailed
        })
    );
    // Talk-back switched off on the baby during the talk: the playback stops at once.
    let mut w = talk_world();
    start_talk(&mut w, 0.01);
    level(&mut w, 2_000, 0.01, Some(0.3));
    settings(&mut w, false, true);
    let calls = playback_calls(&w);
    assert!(!calls.last().unwrap().1);
    assert_eq!(
        last_state(&w),
        Some(TalkState::Ended {
            reason: TalkEndReason::SwitchedOff
        })
    );
    assert_eq!(last_peer_status(&w).talk, Some(TalkStatus::Off));
}

#[test]
fn a_test_alert_is_unavailable_while_a_talk_runs() {
    let mut w = talk_world();
    start_talk(&mut w, 0.01);
    w.input(PARENT, LinkInput::RequestProbe);
    assert_eq!(w.nodes[BABY].probe_tones, 0);
    release(&mut w);
    w.input(PARENT, LinkInput::RequestProbe);
    assert_eq!(w.nodes[BABY].probe_tones, 1);
}

#[test]
fn a_parent_that_was_listening_keeps_listening_after_the_talk() {
    let mut w = talk_world();
    w.input(PARENT, LinkInput::SetListen { enabled: true });
    start_talk(&mut w, 0.01);
    release(&mut w);
    let before = w.mark(PARENT);
    room(&mut w, 15_000, 0.01);
    assert_eq!(
        w.count_events(PARENT, before, |e| matches!(
            e,
            LinkEvent::ListenChanged { .. }
        )),
        0
    );
    assert_eq!(
        data(&w, PARENT, "listen").last().unwrap().1["enabled"],
        true
    );
    // Taking over the after-talk listen keeps it as well.
    let mut w = talk_world();
    start_talk(&mut w, 0.01);
    release(&mut w);
    w.input(PARENT, LinkInput::SetListen { enabled: true });
    room(&mut w, 15_000, 0.01);
    assert_eq!(
        data(&w, PARENT, "listen").last().unwrap().1["enabled"],
        true
    );
}

// ---- stale ids, settings that cannot be changed from afar ------------------------------------------

#[test]
fn stale_ids_never_have_an_effect() {
    let mut w = talk_world();
    press(&mut w);
    let id = data(&w, PARENT, "talk_request")[0].1["id"]
        .as_u64()
        .unwrap();
    // The baby's answers for another id do nothing to the parent.
    w.inject_data(
        PARENT,
        &raw(json!({"type":"talk_grant","v":3,"id":id + 7,"max_ms":30000})),
    );
    w.inject_data(
        PARENT,
        &raw(json!({"type":"talk_deny","v":3,"id":id + 7,"reason":"off"})),
    );
    w.inject_data(PARENT, &raw(json!({"type":"talk_end","v":3,"id":id + 7})));
    assert!(mic_calls(&w).is_empty());
    assert_eq!(parent_states(&w), vec![TalkState::Requesting]);
    // A request, then an end for another id: the baby's talk runs on.
    chime_done(&mut w);
    w.inject_data(BABY, &raw(json!({"type":"talk_end","v":3,"id":id + 7})));
    level(&mut w, 1_000, 0.01, Some(0.3));
    assert_eq!(playback_calls(&w).len(), 1);
    // After the talk ended, its old messages are stale too.
    release(&mut w);
    level(&mut w, 1_000, 0.01, None);
    let mics = mic_calls(&w);
    w.inject_data(
        PARENT,
        &raw(json!({"type":"talk_grant","v":3,"id":id,"max_ms":30000})),
    );
    assert_eq!(mic_calls(&w), mics);
}

#[test]
fn nothing_on_the_wire_changes_the_baby_talk_back_settings() {
    let mut w = talk_world();
    settings(&mut w, false, false);
    for msg in [
        json!({"type":"set_talk_back","v":3,"enabled":true,"level":"high","chime":true}),
        json!({"type":"talk_request","v":3,"id":55,"enabled":true,"level":"high"}),
    ] {
        w.inject_data(BABY, &raw(msg));
    }
    // The request is only a request: denied as off, nothing plays.
    assert_eq!(data(&w, BABY, "talk_deny")[0].1["reason"], "off");
    assert!(w.nodes[BABY].talk_actions.is_empty());
    assert_eq!(last_peer_status(&w).talk, Some(TalkStatus::Off));
}

#[test]
fn settings_are_accepted_before_start_and_survive_a_restart() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    settings(&mut w, false, true);
    w.connect_pair();
    assert_eq!(last_peer_status(&w).talk, Some(TalkStatus::Off));
    w.stop(BABY);
    w.start(BABY, 9);
    w.advance(8_000);
    assert_eq!(last_peer_status(&w).talk, Some(TalkStatus::Off));
}

#[test]
fn talk_audio_outside_a_grant_warns_once_per_attempt_and_never_during_a_talk() {
    let mut w = talk_world();
    let a = attempt(&w, BABY);
    w.input(BABY, LinkInput::TalkRtpOutsideGrant { attempt: a });
    w.input(BABY, LinkInput::TalkRtpOutsideGrant { attempt: a });
    let warnings = |w: &World| {
        baby_events(
            w,
            |e| matches!(e, LinkEvent::SecurityWarning { key } if key == "talk_audio_outside_grant"),
        )
    };
    assert_eq!(warnings(&w), 1);
    // Another attempt number is nobody's.
    w.input(BABY, LinkInput::TalkRtpOutsideGrant { attempt: a + 50 });
    assert_eq!(warnings(&w), 1);
    let mut w = talk_world();
    start_talk(&mut w, 0.01);
    let a = attempt(&w, BABY);
    w.input(BABY, LinkInput::TalkRtpOutsideGrant { attempt: a });
    assert_eq!(warnings(&w), 0);
}

// ---- the noise pause --------------------------------------------------------------------------------

fn heartbeat_rms(w: &World, from: u64, to: u64) -> Vec<bool> {
    data(w, BABY, "heartbeat")
        .iter()
        .filter(|(t, _)| *t >= from && *t < to)
        .map(|(_, m)| m.get("rms").is_some())
        .collect()
}

#[test]
fn a_loud_talk_in_a_quiet_room_raises_no_alert_and_hides_the_level_from_the_parent() {
    let mut w = talk_world();
    let alerts = noise_alerts(&w);
    let talk_from = w.now;
    start_talk(&mut w, 0.01);
    // The baby's own speaker at a high level for 20 s: nothing reaches the gate.
    level(&mut w, 20_000, 0.6, Some(0.5));
    assert!(
        w.nodes[BABY].mic_calls.is_empty(),
        "no audio for noise during the talk"
    );
    release(&mut w);
    level(&mut w, 2_500, 0.6, Some(0.5));
    let talk_to = w.now;
    // Afterwards the room is quiet again.
    room(&mut w, 15_000, 0.01);
    assert_eq!(
        noise_alerts(&w),
        alerts,
        "no noise alert, during or after the talk"
    );
    // Audio flows afterwards only for the parent's 10 s listen, not for noise.
    assert_eq!(w.nodes[BABY].mic_calls, vec![true, false]);
    // The baby's heartbeats carry no level during the pause and do afterwards.
    let during = heartbeat_rms(&w, talk_from, talk_to);
    assert!(
        !during.is_empty() && during.iter().all(|has| !has),
        "{during:?}"
    );
    let after = heartbeat_rms(&w, talk_to + 1_000, w.now);
    assert!(
        !after.is_empty() && after.iter().all(|has| *has),
        "{after:?}"
    );
    assert_eq!(
        baby_events(&w, |e| matches!(e, LinkEvent::NoisePaused { paused: true })),
        1
    );
    assert_eq!(
        baby_events(&w, |e| matches!(
            e,
            LinkEvent::NoisePaused { paused: false }
        )),
        1
    );
    // The parent saw the pause, and the note went away after the tail.
    let seen: Vec<bool> = w.nodes[PARENT]
        .events
        .iter()
        .filter_map(|e| match e {
            LinkEvent::NoisePaused { paused } => Some(*paused),
            _ => None,
        })
        .collect();
    assert_eq!(seen, vec![true, false]);
}

#[test]
fn a_cry_that_starts_during_a_talk_is_reported_within_the_limit_plus_tail_plus_hold() {
    let mut w = talk_world();
    let granted = start_talk(&mut w, 0.01);
    level(&mut w, 5_000, 0.01, Some(0.3));
    let cry_at = w.now;
    // The child cries and keeps crying; the parent holds until the baby cuts the talk.
    let alerts = noise_alerts(&w);
    while noise_alerts(&w) == alerts && w.now < cry_at + 60_000 {
        level(&mut w, 100, 0.6, Some(0.3));
    }
    let alert_at = w.now;
    assert!(noise_alerts(&w) > alerts, "the cry is reported");
    assert!(alert_at > granted + 30_000, "not during the talk");
    assert!(
        alert_at <= granted + 30_000 + 2_000 + HOLD_MS + 300,
        "{} ms after the cry started",
        alert_at - cry_at
    );
    // The paused talk was shown to the parent until then.
    assert!(
        w.nodes[PARENT]
            .events
            .iter()
            .any(|e| matches!(e, LinkEvent::NoisePaused { paused: true }))
    );
}

#[test]
fn a_noise_event_that_is_running_continues_through_a_talk_without_a_new_alert() {
    let mut w = talk_world();
    // The baby cries: one alert, microphone open.
    level(&mut w, 1_000, 0.6, None);
    assert_eq!(noise_alerts(&w), 1);
    assert_eq!(w.nodes[BABY].mic_calls, vec![true]);
    // The parent talks while the child still cries (a talk press is an acknowledgment).
    press(&mut w);
    level(&mut w, 600, 0.6, None);
    chime_done(&mut w);
    w.input(PARENT, LinkInput::TalkMicAttached);
    level(&mut w, 10_000, 0.6, Some(0.3));
    release(&mut w);
    level(&mut w, 3_000, 0.6, Some(0.3));
    assert_eq!(noise_alerts(&w), 1, "no new alert");
    assert_eq!(
        w.nodes[BABY].mic_calls,
        vec![true],
        "audio to the parents keeps flowing as before"
    );
    // Afterwards the gate is still loud and ends the event the usual way once the parent's
    // after-talk listen is over as well.
    room(&mut w, 12_000, 0.01);
    assert_eq!(w.nodes[BABY].mic_calls, vec![true, false]);
}

fn heartbeat(rms: f64) -> String {
    raw(json!({"type":"heartbeat","v":3,"t":1,"seq":1,"rms":rms}))
}

/// The parent has a snoozed noise event; loud heartbeats arrive while the baby says whether its
/// noise detection is paused. Returns the number of escalations after the snooze ended.
fn escalations_after_snooze(baby_paused: bool) -> usize {
    let mut w = talk_world();
    w.input(
        PARENT,
        LinkInput::SetNoiseEscalation {
            after_ms: Some(1_000),
        },
    );
    level(&mut w, 1_000, 0.6, None);
    assert_eq!(noise_alerts(&w), 1);
    room(&mut w, 200, 0.01);
    w.input(
        PARENT,
        LinkInput::SetSnooze {
            until_ms: Some(w.now + 60_000),
        },
    );
    if baby_paused {
        w.inject_data(
            PARENT,
            &raw(json!({"type":"device_status","v":3,"noise_paused":true,"talk":"on","talker":"other"})),
        );
    }
    // Loud heartbeats at 20, 39 and 58 s; the snooze ends at 61 s.
    for _ in 0..3 {
        room(&mut w, 19_000, 0.01);
        w.inject_data(PARENT, &heartbeat(0.9));
    }
    let mark = w.mark(PARENT);
    room(&mut w, 20_000, 0.01);
    w.count_events(PARENT, mark, |e| {
        matches!(e, LinkEvent::NoiseEscalated { .. })
    })
}

#[test]
fn levels_from_the_pause_do_not_extend_a_noise_event_on_the_parent() {
    // Control: loud heartbeats of a baby that is not paused keep the event alive; when the snooze
    // ends it restarts and, unacknowledged, escalates.
    assert_eq!(escalations_after_snooze(false), 1);
    // While the baby says that its detection is paused, the same heartbeats are ignored: the event
    // is long over when the snooze ends.
    assert_eq!(escalations_after_snooze(true), 0);
}

#[test]
fn a_talk_press_acknowledges_the_running_noise_event() {
    let escalations = |press_talk: bool| {
        let mut w = talk_world();
        w.input(
            PARENT,
            LinkInput::SetNoiseEscalation {
                after_ms: Some(5_000),
            },
        );
        level(&mut w, 1_000, 0.6, None);
        level(&mut w, 2_000, 0.6, None);
        if press_talk {
            press(&mut w);
            release(&mut w);
        }
        level(&mut w, 20_000, 0.6, None);
        w.nodes[PARENT]
            .events
            .iter()
            .filter(|e| matches!(e, LinkEvent::NoiseEscalated { .. }))
            .count()
    };
    assert_eq!(escalations(false), 1);
    assert_eq!(escalations(true), 0);
}

#[test]
fn the_pause_leaves_mic_silent_alone() {
    let mut w = talk_world();
    let mic_status = |w: &World| {
        w.nodes[PARENT].events.iter().filter(|e| matches!(e, LinkEvent::PeerStatus { status } if status.mic_silent == Some(true))).count()
    };
    start_talk(&mut w, 0.01);
    // Digital silence for longer than the 30 s the mic check needs, all inside the pause.
    level(&mut w, 29_000, 0.0, Some(0.3));
    release(&mut w);
    level(&mut w, 2_800, 0.0, Some(0.3));
    assert_eq!(mic_status(&w), 0);
    // After the pause the count starts over: 30 s of silence later it reports.
    room(&mut w, 25_000, 0.0);
    assert_eq!(mic_status(&w), 0);
    room(&mut w, 8_000, 0.0);
    assert_eq!(mic_status(&w), 1);
}

// ---- cooldown and budget ------------------------------------------------------------------------------

/// A short talk: press, chime, grant, hold `ms`, release; then `after` ms of quiet room.
fn short_talk(w: &mut World, ms: u64, after: u64) {
    start_talk(w, 0.01);
    level(w, ms, 0.01, Some(0.3));
    release(w);
    room(w, after, 0.01);
}

#[test]
fn after_a_talk_the_baby_denies_until_the_gate_ran_unpaused_for_ten_seconds() {
    let mut w = talk_world();
    short_talk(&mut w, 2_000, 1_000);
    // Tail until 2 s after the playback ended, then 10 s cooldown.
    press(&mut w);
    let Some(Some(TalkDenyReason::Cooldown { retry_in_ms })) = denied(&w).first().copied() else {
        panic!("{:?}", denied(&w));
    };
    assert!((10_000..=12_000).contains(&retry_in_ms), "{retry_in_ms}");
    assert!(last_peer_status(&w).talk_retry_in_ms.is_some());
    // The status goes back to no wait when the cooldown is over, and a new talk passes.
    room(&mut w, retry_in_ms + 500, 0.01);
    assert_eq!(last_peer_status(&w).talk_retry_in_ms, None);
    let grants = data(&w, BABY, "talk_grant").len();
    start_talk(&mut w, 0.01);
    assert_eq!(data(&w, BABY, "talk_grant").len(), grants + 1);
}

#[test]
fn a_long_hold_time_lengthens_the_cooldown() {
    let mut w = talk_world();
    w.input(
        BABY,
        LinkInput::SetNoiseGate {
            threshold: 0.2,
            hold_ms: 15_000,
        },
    );
    short_talk(&mut w, 1_000, 3_000);
    press(&mut w);
    let Some(Some(TalkDenyReason::Cooldown { retry_in_ms })) = denied(&w).first().copied() else {
        panic!("{:?}", denied(&w));
    };
    assert!(retry_in_ms > 11_000, "hold + 1 s, not 10 s: {retry_in_ms}");
}

#[test]
fn the_budget_is_ninety_seconds_of_pause_in_ten_minutes_and_the_parent_sees_when_it_frees_up() {
    let mut w = talk_world();
    // Three talks of the full 30 s with the cooldown between them: 3 x 32 s of pause.
    for _ in 0..3 {
        start_talk(&mut w, 0.01);
        level(&mut w, 31_000, 0.01, Some(0.3));
        room(&mut w, 14_000, 0.01);
    }
    press(&mut w);
    let Some(Some(TalkDenyReason::Budget { retry_in_ms })) = denied(&w).last().copied() else {
        panic!("{:?}", denied(&w));
    };
    assert!(
        retry_in_ms > 100_000 && retry_in_ms <= 600_000,
        "{retry_in_ms}"
    );
    assert!(last_peer_status(&w).talk_retry_in_ms.is_some());
    // Noise detection keeps running while the talk is denied.
    let alerts = noise_alerts(&w);
    level(&mut w, 1_000, 0.6, None);
    assert_eq!(noise_alerts(&w), alerts + 1);
}

#[test]
fn chained_presses_by_one_or_two_parents_cannot_blind_the_gate_for_long() {
    let mut w = talk_world();
    let cry_at = w.now + 180_000;
    let mut first_alert: Option<u64> = None;
    let mut pause_ms = 0u64;
    let mut cycles = 0;
    while w.now < cry_at + 420_000 {
        // Press, let go after a second, press again as soon as the button is up.
        press(&mut w);
        let cry = w.now >= cry_at;
        let rms = if cry { 0.6 } else { 0.01 };
        level(&mut w, 700, rms, None);
        chime_done(&mut w);
        w.input(PARENT, LinkInput::TalkMicAttached);
        let started = w.now;
        level(&mut w, 1_000, rms, Some(0.3));
        release(&mut w);
        level(&mut w, 200, rms, Some(0.3));
        pause_ms += w.now - started;
        if first_alert.is_none() && noise_alerts(&w) > 0 {
            first_alert = Some(w.now);
        }
        cycles += 1;
    }
    let first_alert = first_alert.expect("the cry reached the parents");
    assert!(cycles > 20, "{cycles} cycles");
    // One talk limit plus the tail plus the cooldown after the cry started (plus the hold time).
    assert!(first_alert >= cry_at);
    assert!(
        first_alert - cry_at <= 30_000 + 2_000 + 10_000 + HOLD_MS + 1_000,
        "{} ms",
        first_alert - cry_at
    );
    let _ = pause_ms;
}

// ---- the playback check ---------------------------------------------------------------------------------

/// One complete talk of 5 s with the room at `room_rms` during the chime and `voice_room` during
/// the voice, then enough quiet for the cooldown. Returns the result the parent got, if any.
fn checked_talk(
    w: &mut World,
    chime_room: f64,
    voice_room: f64,
    voice: f64,
) -> Option<PlaybackResult> {
    let mark = w.mark(PARENT);
    press(w);
    level(w, 600, chime_room, None);
    chime_done(w);
    w.input(PARENT, LinkInput::TalkMicAttached);
    level(w, 5_000, voice_room, Some(voice));
    release(w);
    room(w, 14_000, 0.01);
    w.events_since(PARENT, mark).iter().find_map(|e| match e {
        LinkEvent::TalkPlayback { result } => Some(*result),
        _ => None,
    })
}

#[test]
fn a_talk_that_the_baby_microphone_hears_is_heard() {
    let mut w = talk_world();
    assert_eq!(
        checked_talk(&mut w, 0.05, 0.01, 0.3),
        Some(PlaybackResult::Heard)
    );
    // The voice alone, with the chime off, is enough as well.
    settings(&mut w, true, false);
    assert_eq!(
        checked_talk(&mut w, 0.01, 0.05, 0.3),
        Some(PlaybackResult::Heard)
    );
}

#[test]
fn a_talk_that_the_room_does_not_show_is_unconfirmed() {
    let mut w = talk_world();
    assert_eq!(
        checked_talk(&mut w, 0.011, 0.011, 0.3),
        Some(PlaybackResult::Unconfirmed)
    );
}

#[test]
fn a_silent_parent_without_a_chime_is_not_judged() {
    let mut w = talk_world();
    settings(&mut w, true, false);
    assert_eq!(
        checked_talk(&mut w, 0.011, 0.011, 0.001),
        Some(PlaybackResult::Unknown)
    );
}

#[test]
fn the_check_says_unknown_without_raw_capture() {
    let mut w = talk_world();
    w.input(BABY, LinkInput::SetRawCapture { raw: false });
    assert_eq!(
        checked_talk(&mut w, 0.05, 0.05, 0.3),
        Some(PlaybackResult::Unknown)
    );
}

#[test]
fn a_white_noise_machine_and_a_crying_child_keep_the_check_at_unknown() {
    // A steady level at the noise threshold.
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.input(BABY, LinkInput::SetRawCapture { raw: true });
    w.connect_pair();
    room(&mut w, 3_000, 0.2);
    assert_eq!(
        checked_talk(&mut w, 0.5, 0.5, 0.3),
        Some(PlaybackResult::Unknown)
    );
    // A crying child: the baseline is unstable and a noise event runs.
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.input(BABY, LinkInput::SetRawCapture { raw: true });
    w.connect_pair();
    for i in 0..30 {
        w.advance_by(100, 100);
        w.input(
            BABY,
            LinkInput::AudioLevel {
                rms: if i % 2 == 0 { 0.3 } else { 0.9 },
            },
        );
    }
    assert_eq!(
        checked_talk(&mut w, 0.95, 0.95, 0.3),
        Some(PlaybackResult::Unknown)
    );
}

#[test]
fn three_unconfirmed_talks_warn_but_never_deny_and_a_heard_one_clears_the_warning() {
    let mut w = talk_world();
    for n in 1..=3 {
        assert_eq!(
            checked_talk(&mut w, 0.011, 0.011, 0.3),
            Some(PlaybackResult::Unconfirmed)
        );
        let audible = last_peer_status(&w).talk_audible;
        assert_eq!(
            audible,
            (n == 3).then_some(TalkAudible::Uncertain),
            "after talk {n}"
        );
    }
    // The warning does not deny a request: the next talk is granted.
    let grants = data(&w, BABY, "talk_grant").len();
    assert_eq!(
        checked_talk(&mut w, 0.05, 0.01, 0.3),
        Some(PlaybackResult::Heard)
    );
    assert_eq!(data(&w, BABY, "talk_grant").len(), grants + 1);
    assert!(denied(&w).is_empty());
    assert_eq!(last_peer_status(&w).talk_audible, None);
    // Unknown results count for nothing in either direction; an adult at the settings clears it.
    let mut w = talk_world();
    for _ in 0..3 {
        checked_talk(&mut w, 0.011, 0.011, 0.3);
    }
    assert_eq!(
        last_peer_status(&w).talk_audible,
        Some(TalkAudible::Uncertain)
    );
    w.input(BABY, LinkInput::TalkSettingsOpened);
    assert_eq!(last_peer_status(&w).talk_audible, None);
}

#[test]
fn a_crying_child_who_is_talked_to_three_times_gets_no_warning() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.input(BABY, LinkInput::SetRawCapture { raw: true });
    w.connect_pair();
    for _ in 0..3 {
        // Loud, jumpy room before and during every talk.
        for i in 0..30 {
            w.advance_by(100, 100);
            w.input(
                BABY,
                LinkInput::AudioLevel {
                    rms: if i % 2 == 0 { 0.3 } else { 0.9 },
                },
            );
        }
        assert_eq!(
            checked_talk(&mut w, 0.5, 0.5, 0.3),
            Some(PlaybackResult::Unknown)
        );
        assert!(denied(&w).is_empty());
    }
    assert_eq!(last_peer_status(&w).talk_audible, None);
}
