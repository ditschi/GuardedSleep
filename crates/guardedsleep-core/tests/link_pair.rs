//! Two `Link`s (baby + parent) against a fake signaling server and fake WebRTC (ADR-017 §12).

mod common;

use common::*;
use guardedsleep_core::envelope::MessageClass;
use guardedsleep_core::link::Link;
use guardedsleep_core::link::{LinkAction, LinkEvent, LinkInput, LinkState};
use guardedsleep_core::pairing::encode_psk;
use guardedsleep_core::protocol::{DeviceStatus, Dnd, Ringer, Role, UnitWarning};
use serde_json::{Value, json};

fn hb(seq: u64) -> String {
    json!({"type":"heartbeat","v":3,"t":1,"seq":seq}).to_string()
}

fn is_link_lost(e: &LinkEvent) -> bool {
    matches!(e, LinkEvent::LinkLost { .. })
}

fn is_restored(e: &LinkEvent) -> bool {
    matches!(e, LinkEvent::LinkRestored { .. })
}

fn loud(w: &mut World, ms: u64) {
    let steps = ms / 100;
    for _ in 0..steps {
        w.advance_by(100, 100);
        w.input(BABY, LinkInput::AudioLevel { rms: 0.5 });
    }
}

fn quiet(w: &mut World, ms: u64) {
    for _ in 0..(ms / 100) {
        w.advance_by(100, 100);
        w.input(BABY, LinkInput::AudioLevel { rms: 0.01 });
    }
}

// ---- happy path --------------------------------------------------------------------------------

#[test]
fn happy_path_connects_and_exchanges_heartbeats_status_and_noise() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    assert_eq!(
        w.nodes[BABY].link.room_id(),
        w.nodes[PARENT].link.room_id(),
        "same PSK gives the same room"
    );
    w.start(BABY, 1);
    w.status_input(
        BABY,
        DeviceStatus {
            battery_pct: Some(87),
            charging: Some(true),
            ringer: Some(Ringer::Normal),
            dnd: Some(Dnd::Off),
            ..Default::default()
        },
    );
    w.start(PARENT, 2);
    w.advance(5_000);
    assert_eq!(w.state(BABY), LinkState::Connected);
    assert_eq!(w.state(PARENT), LinkState::Connected);
    assert_eq!(w.live_attempts(BABY), 1);
    assert_eq!(w.live_attempts(PARENT), 1);

    // Baby status arrives once after verification, with the battery rounded to 5.
    let statuses: Vec<_> = w.nodes[PARENT]
        .events
        .iter()
        .filter_map(|e| match e {
            LinkEvent::PeerStatus { status } => Some(status.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].battery_pct, Some(85));
    assert_eq!(statuses[0].charging, Some(true));

    let m = w.mark(PARENT);
    w.advance(30_000);
    let beats = w.count_events(PARENT, m, |e| matches!(e, LinkEvent::PeerHeartbeat { .. }));
    assert!(beats >= 2, "parent saw {beats} baby heartbeats");

    let m = w.mark(PARENT);
    loud(&mut w, 1_000);
    assert!(
        w.count_events(PARENT, m, |e| matches!(
            e,
            LinkEvent::NoiseAlert {
                probe: false,
                snoozed: false,
                ..
            }
        )) >= 1
    );
    assert_eq!(w.nodes[BABY].mic_calls.first(), Some(&true));
}

#[test]
fn status_is_sent_only_on_change() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let count = |w: &World| {
        w.nodes[PARENT]
            .events
            .iter()
            .filter(|e| matches!(e, LinkEvent::PeerStatus { .. }))
            .count()
    };
    let before = count(&w);
    let s = DeviceStatus {
        battery_pct: Some(80),
        ..Default::default()
    };
    w.status_input(BABY, s.clone());
    assert_eq!(count(&w), before + 1);
    w.status_input(BABY, s.clone());
    w.status_input(
        BABY,
        DeviceStatus {
            battery_pct: Some(81),
            ..Default::default()
        },
    );
    assert_eq!(count(&w), before + 1, "81 rounds to the same 80");
    w.status_input(
        BABY,
        DeviceStatus {
            battery_pct: Some(80),
            ringer: Some(Ringer::Silent),
            ..Default::default()
        },
    );
    assert_eq!(count(&w), before + 2);
}

#[test]
fn unit_warnings_reach_the_parent_and_are_sent_only_on_change() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let statuses = |w: &World| -> Vec<DeviceStatus> {
        w.nodes[PARENT]
            .events
            .iter()
            .filter_map(|e| match e {
                LinkEvent::PeerStatus { status } => Some(status.clone()),
                _ => None,
            })
            .collect()
    };
    let before = statuses(&w).len();
    let warn = |v: Vec<UnitWarning>| DeviceStatus {
        unit_warnings: v,
        ..Default::default()
    };
    w.status_input(
        BABY,
        warn(vec![UnitWarning::NotCharging, UnitWarning::PageHidden]),
    );
    let got = statuses(&w);
    assert_eq!(got.len(), before + 1);
    assert_eq!(
        got.last().unwrap().unit_warnings,
        vec![UnitWarning::PageHidden, UnitWarning::NotCharging]
    );
    // Same set in another order is not a change.
    w.status_input(
        BABY,
        warn(vec![UnitWarning::PageHidden, UnitWarning::NotCharging]),
    );
    assert_eq!(statuses(&w).len(), before + 1);
    w.status_input(BABY, warn(vec![]));
    let got = statuses(&w);
    assert_eq!(got.len(), before + 2);
    assert!(got.last().unwrap().unit_warnings.is_empty());
}

// ---- security ----------------------------------------------------------------------------------

#[test]
fn forged_new_aid_offer_does_not_close_a_verified_attempt() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    // No mailbox is open while the attempt is verified; the injection stands for a frame that
    // reaches the link anyway (a platform that ignored `close_mailbox`).
    let baby_id = "b".to_owned();
    let offer = |aid: &str| json!({"type":"sdp","aid":aid,"sdp":{"type":"offer","sdp":"v=0\r\nm=audio 9 X 1\r\na=sendonly\r\na=fingerprint:sha-256 AA\r\n"}});
    let handle = w.nodes[PARENT].link.live_attempt().unwrap();
    // From the real baby id with a brand-new aid, and from a stranger.
    for (from, aid) in [
        (baby_id.as_str(), "11111111111111111111111111111111"),
        ("stranger", "22222222222222222222222222222222"),
    ] {
        let m = w.mark(PARENT);
        w.inject_signal(PARENT, from, offer(aid));
        w.advance(500);
        assert_eq!(w.nodes[PARENT].link.live_attempt(), Some(handle));
        assert_eq!(w.state(PARENT), LinkState::Connected);
        assert_eq!(
            w.count_events(PARENT, m, |e| matches!(e, LinkEvent::Failed { .. })),
            0
        );
    }
    // A request_offer from the baby-side stranger must not touch the baby's verified attempt.
    let baby_handle = w.nodes[BABY].link.live_attempt().unwrap();
    w.inject_signal(
        BABY,
        "stranger",
        json!({"type":"request_offer","aid":"33333333333333333333333333333333"}),
    );
    w.inject_signal(
        BABY,
        "p",
        json!({"type":"request_offer","aid":"44444444444444444444444444444444"}),
    );
    w.advance(500);
    assert_eq!(w.nodes[BABY].link.live_attempt(), Some(baby_handle));
}

#[test]
fn stranger_ice_and_answers_are_ignored() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let handle = w.nodes[BABY].link.live_attempt().unwrap();
    let ice = json!({"type":"ice","aid":"55555555555555555555555555555555","candidate":{"candidate":"c","sdpMid":"0","sdpMLineIndex":0}});
    let text = json!({"type":"signal","from":"stranger","payload":ice}).to_string();
    let actions = w.nodes[BABY]
        .link
        .handle(w.now, LinkInput::MailboxFrame { id: 0, bytes: text });
    assert!(
        !actions
            .iter()
            .any(|a| matches!(a, LinkAction::AddIceCandidate { .. }))
    );
    assert_eq!(w.nodes[BABY].link.live_attempt(), Some(handle));
}

#[test]
fn reflected_proof_never_verifies_and_baby_stays_unverified() {
    // A parent-side attacker that holds the PSK (so its envelopes authenticate) answers the baby's
    // offer and reflects the baby's own proof back.
    let mut w = World::pair(&PSK_A, &PSK_A);
    let atk = w.add_impostor("attacker", Role::Parent, &PSK_A, 9);
    w.start(BABY, 1);
    w.advance(500);
    w.impostor_greets(atk, BABY);
    // No request comes from the attacker: the baby offers on its own after its grace period.
    w.advance(3_000);
    let h = w.nodes[BABY]
        .link
        .live_attempt()
        .expect("baby offers to the attacker");
    w.advance(100);
    // The baby's offer has been given the fake local description by the harness; answer it.
    let answer_sdp = format!(
        "v=0\r\nm=audio 9 X 1\r\na=recvonly\r\nm=audio 9 X 1\r\na=sendonly\r\na=fingerprint:sha-256 {}\r\n",
        colon_fp(&"ab".repeat(32))
    );
    let answer = json!({"type":"sdp","aid":"66666666666666666666666666666666","sdp":{"type":"answer","sdp":answer_sdp}});
    w.impostor_says(atk, BABY, MessageClass::Description, &answer);
    let acts = w.nodes[BABY]
        .link
        .handle(w.now, LinkInput::AttemptDataOpen { attempt: h });
    let own_proof = acts
        .iter()
        .find_map(|a| match a {
            LinkAction::SendData { text, .. } => Some(text.clone()),
            _ => None,
        })
        .expect("baby sends its proof");
    let acts = w.nodes[BABY].link.handle(
        w.now,
        LinkInput::AttemptDataMessage {
            attempt: h,
            text: own_proof,
        },
    );
    assert!(acts
        .iter()
        .any(|a| matches!(a, LinkAction::Emit { event: LinkEvent::Failed { reason } } if reason == "pair_proof_mismatch")));
    assert_ne!(w.state(BABY), LinkState::Connected);
    assert!(
        !acts
            .iter()
            .any(|a| matches!(a, LinkAction::SendData { text, .. } if text.contains("heartbeat")))
    );
}

#[test]
fn mismatching_certificates_at_first_pairing_are_terminal_after_two_mismatches_on_both_sides() {
    // Both sides hold the PSK, but a man in the middle terminates DTLS: each reports a certificate
    // that the SDP does not commit to.
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.set_dtls(BABY, Dtls::Mismatch);
    w.set_dtls(PARENT, Dtls::Mismatch);
    w.start(BABY, 1);
    w.start(PARENT, 2);
    w.advance(10_000);
    for n in [BABY, PARENT] {
        let s = w.status(n);
        assert_eq!(s.state, LinkState::FailedNeedsUser, "node {n}");
        assert_eq!(s.key.as_deref(), Some("pairing_key_mismatch"));
        assert_eq!(w.live_attempts(n), 0);
        assert_eq!(
            w.count_events(
                n,
                0,
                |e| matches!(e, LinkEvent::Failed { reason } if reason == "dtls_fingerprint_mismatch")
            ),
            2,
            "exactly two consecutive mismatches"
        );
        assert_eq!(
            w.count_events(n, 0, |e| matches!(e, LinkEvent::SecurityWarning { .. })),
            0
        );
    }
    // Terminal means silent: no further attempts or signaling.
    let created = w.nodes[BABY].created.len();
    w.advance(60_000);
    assert_eq!(w.nodes[BABY].created.len(), created);
    // The user can stop (and later start again with a new code).
    w.stop(BABY);
    assert_eq!(w.state(BABY), LinkState::Stopped);
}

#[test]
fn devices_with_different_keys_never_confirm_a_session_and_start_no_attempt() {
    // Two devices that share a room but not the PSK (a wrongly scanned code in a relay that does not
    // separate rooms): their envelopes do not authenticate, so nothing is ever offered or proven.
    let mut w = World::pair(&PSK_A, &PSK_B);
    w.start(BABY, 1);
    w.start(PARENT, 2);
    w.advance(70_000);
    for n in [BABY, PARENT] {
        let s = w.status(n);
        assert_eq!(s.state, LinkState::Connecting, "node {n}");
        assert!(s.key.is_none());
        assert!(s.level >= 2, "the silence still escalates");
        assert_eq!(s.cause.as_deref(), Some("peer_absent"));
        assert!(!w.nodes[n].link.peer_confirmed());
        assert!(w.nodes[n].created.is_empty(), "no attempt for node {n}");
        assert_eq!(
            w.count_events(n, 0, |e| matches!(e, LinkEvent::Failed { .. })),
            0
        );
        assert_eq!(
            w.count_events(n, 0, |e| matches!(e, LinkEvent::SecurityWarning { .. })),
            0
        );
        // Presence is all that was ever sent; no offer, request or stop leaked out.
        assert!(
            w.nodes[n].sent_signal_types.iter().all(|t| t == "here"),
            "{:?}",
            w.nodes[n].sent_signal_types
        );
    }
}

#[test]
fn spoofed_mismatch_after_pairing_is_not_terminal_and_slows_retries() {
    let mut w = World::empty();
    // A baby that verified a pairing before but holds no parent secret (the first-pairing window:
    // nothing pinned yet), so any parent that holds the PSK is let through to the DTLS check.
    let mut baby_cfg = cfg(Role::Baby, &PSK_A);
    baby_cfg.paired_before = true;
    // ... and its first-pairing window is still open (the invite is valid until 2_000_000 s).
    baby_cfg.pin_window_until_s = Some(2_000_000);
    let baby = w.add_node(baby_cfg);
    let parent = w.add_node(cfg(Role::Parent, &PSK_A));
    // The outsider holds the PSK (it reads the invite) but terminates DTLS itself, so the baby
    // sees a certificate the SDP does not commit to. It never gives up on its side.
    let mut atk_cfg = cfg(Role::Parent, &PSK_A);
    atk_cfg.paired_before = true;
    let atk = w.add_node(atk_cfg);
    w.start(baby, 1);
    w.input(baby, LinkInput::WallClock { unix_s: 1_000_000 });
    w.advance(1_000);
    let m = w.mark(baby);
    let t0 = w.now;
    w.set_dtls(baby, Dtls::Mismatch);
    w.start(atk, 3);
    w.advance(120_000);

    let mismatches = w.count_events(
        baby,
        m,
        |e| matches!(e, LinkEvent::Failed { reason } if reason == "dtls_fingerprint_mismatch"),
    );
    let warnings = w.count_events(
        baby,
        m,
        |e| matches!(e, LinkEvent::SecurityWarning { key } if key == "wrong_key_attempt"),
    );
    assert!(mismatches >= 3, "mismatches: {mismatches}");
    assert_eq!(warnings, mismatches);
    assert_ne!(w.state(baby), LinkState::FailedNeedsUser);
    // Attempts after a mismatch are at least 15 s apart.
    let times: Vec<u64> = w.nodes[baby]
        .create_times
        .iter()
        .copied()
        .filter(|t| *t >= t0)
        .collect();
    // A request that crosses the baby's own offer replaces the attempt within the same instant.
    let mut episodes: Vec<u64> = Vec::new();
    for t in times {
        if episodes.last().is_none_or(|last| t - last > 1_000) {
            episodes.push(t);
        }
    }
    assert!(episodes.len() >= 4, "{episodes:?}");
    for pair in episodes.windows(2) {
        assert!(
            pair[1] - pair[0] >= 15_000,
            "attempts too close: {episodes:?}"
        );
    }
    // The outsider leaves; the real parent comes back and is verified once the hold expired.
    w.stop(atk);
    w.set_dtls(baby, Dtls::Honest);
    w.start(parent, 4);
    w.advance(40_000);
    assert_eq!(w.state(parent), LinkState::Connected);
}

#[test]
fn paired_before_makes_the_first_mismatch_non_terminal() {
    let mut c = cfg(Role::Baby, &PSK_A);
    c.paired_before = true;
    let mut w = World::empty();
    w.add_node(c);
    w.add_node(cfg(Role::Parent, &PSK_A));
    w.set_dtls(BABY, Dtls::Mismatch);
    w.set_dtls(PARENT, Dtls::Mismatch);
    w.start(BABY, 1);
    w.start(PARENT, 2);
    w.advance(120_000);
    assert_ne!(w.state(BABY), LinkState::FailedNeedsUser);
    // The parent never verified anything, so it still ends terminal after two mismatches.
    assert_eq!(w.state(PARENT), LinkState::FailedNeedsUser);
}

#[test]
fn baby_sdp_that_does_not_send_on_the_monitoring_line_fails_and_no_offer_leaves_the_device() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.nodes[BABY].baby_direction = "sendrecv";
    w.start(BABY, 1);
    w.start(PARENT, 2);
    w.advance(30_000);
    assert!(
        w.count_events(
            BABY,
            0,
            |e| matches!(e, LinkEvent::Failed { reason } if reason == "baby_sdp_bad_direction")
        ) >= 1
    );
    assert!(!w.nodes[BABY].sent_signal_types.iter().any(|t| t == "sdp"));
    assert_ne!(w.state(PARENT), LinkState::Connected);
    assert_ne!(w.state(BABY), LinkState::Connected);
    assert_ne!(w.state(BABY), LinkState::FailedNeedsUser);
}

// ---- liveness and recovery ---------------------------------------------------------------------

fn outage(who: usize, secs: u64) -> (World, u8, u8) {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.advance(3_000);
    w.reset_max_level(BABY);
    w.reset_max_level(PARENT);
    w.set_net(who, false);
    w.advance(secs * 1_000);
    w.set_net(who, true);
    let (lb, lp) = (w.nodes[BABY].max_level, w.nodes[PARENT].max_level);
    w.advance(40_000);
    (w, lb, lp)
}

#[test]
fn outage_of_20s_never_reaches_level_2_and_recovers() {
    for who in [BABY, PARENT] {
        let (w, lb, lp) = outage(who, 20);
        assert!(lb < 2 && lp < 2, "levels {lb}/{lp} when {who} was offline");
        assert_eq!(w.state(BABY), LinkState::Connected);
        assert_eq!(w.state(PARENT), LinkState::Connected);
        assert_eq!(w.count_events(PARENT, 0, is_link_lost), 0);
        assert_eq!(w.count_events(BABY, 0, is_link_lost), 0);
    }
}

#[test]
fn outage_of_90s_raises_level_2_then_restores() {
    for who in [BABY, PARENT] {
        let mut w = World::pair(&PSK_A, &PSK_A);
        w.connect_pair();
        w.advance(3_000);
        let t_down = w.now;
        let m = w.mark(PARENT);
        w.set_net(who, false);
        let mut level2_at = None;
        while w.now < t_down + 90_000 {
            w.advance(500);
            if level2_at.is_none() && w.status(PARENT).level >= 2 {
                level2_at = Some(w.now);
            }
        }
        let l2 = level2_at.expect("parent reached level 2");
        // The parent's last sign of life is at most one heartbeat before the outage started.
        assert!(
            l2 - t_down <= 60_000 + 2_000,
            "level 2 after {} ms (offline: {who})",
            l2 - t_down
        );
        assert_eq!(
            w.count_events(PARENT, m, is_link_lost),
            1,
            "alarm raised once"
        );
        assert_eq!(w.status(PARENT).state, LinkState::Reconnecting);
        w.set_net(who, true);
        w.advance(30_000);
        assert_eq!(w.state(PARENT), LinkState::Connected);
        assert_eq!(w.state(BABY), LinkState::Connected);
        let restored: Vec<u64> = w
            .events_since(PARENT, m)
            .iter()
            .filter_map(|e| match e {
                LinkEvent::LinkRestored { down_ms } => Some(*down_ms),
                _ => None,
            })
            .collect();
        assert_eq!(restored.len(), 1);
        assert!(restored[0] >= 85_000, "down_ms {}", restored[0]);
        assert_eq!(w.status(PARENT).level, 0);
    }
}

#[test]
fn level_1_between_20_and_60_seconds_and_no_link_lost() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.reset_max_level(PARENT);
    w.set_net(BABY, false);
    w.advance(45_000);
    assert_eq!(w.status(PARENT).level, 1);
    assert_eq!(w.count_events(PARENT, 0, is_link_lost), 0);
    w.set_net(BABY, true);
    w.advance(30_000);
    assert_eq!(w.count_events(PARENT, 0, is_restored), 1);
}

#[test]
fn level_3_after_180_seconds() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.set_net(BABY, false);
    let t0 = w.now;
    w.advance(200_000);
    assert_eq!(w.status(PARENT).level, 3);
    assert!(w.now - t0 >= 180_000);
}

#[test]
fn level_3_is_reached_at_180s_after_last_alive() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.set_net(BABY, false);
    w.advance(170_000);
    assert_eq!(w.status(PARENT).level, 2);
    w.advance(20_000);
    assert_eq!(w.status(PARENT).level, 3);
}

#[test]
fn listen_intent_is_resent_after_a_short_outage_and_reset_after_a_long_one() {
    let listen_texts = |w: &World| w.nodes[BABY].mic_calls.iter().filter(|e| **e).count();
    // Short outage: the baby receives `listen` again and keeps the mic on.
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.input(PARENT, LinkInput::SetListen { enabled: true });
    assert_eq!(w.nodes[BABY].mic_calls, vec![true]);
    w.set_net(BABY, false);
    w.advance(30_000);
    assert_eq!(
        w.nodes[BABY].mic_calls,
        vec![true, false],
        "mic off while detached"
    );
    w.set_net(BABY, true);
    w.advance(30_000);
    assert_eq!(w.nodes[BABY].mic_calls.last(), Some(&true));
    assert_eq!(listen_texts(&w), 2);
    assert_eq!(
        w.count_events(PARENT, 0, |e| matches!(e, LinkEvent::ListenChanged { .. })),
        0
    );

    // Long outage: the intent is reset and announced.
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.input(PARENT, LinkInput::SetListen { enabled: true });
    w.set_net(BABY, false);
    w.advance(90_000);
    w.set_net(BABY, true);
    w.advance(30_000);
    assert_eq!(w.state(PARENT), LinkState::Connected);
    assert_eq!(
        w.count_events(PARENT, 0, |e| matches!(
            e,
            LinkEvent::ListenChanged { enabled: false }
        )),
        1
    );
    assert_eq!(w.nodes[BABY].mic_calls.last(), Some(&false));
}

#[test]
fn the_baby_is_told_when_the_parent_listens_and_when_the_intent_is_reset() {
    let listen_changes = |w: &World, mark: usize| -> Vec<bool> {
        w.events_since(BABY, mark)
            .iter()
            .filter_map(|e| match e {
                LinkEvent::ListenChanged { enabled } => Some(*enabled),
                _ => None,
            })
            .collect()
    };
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let m = w.mark(BABY);
    w.input(PARENT, LinkInput::SetListen { enabled: true });
    assert_eq!(listen_changes(&w, m), vec![true]);
    // The same intent again is no change.
    w.input(PARENT, LinkInput::SetListen { enabled: true });
    assert_eq!(listen_changes(&w, m), vec![true]);
    w.input(PARENT, LinkInput::SetListen { enabled: false });
    assert_eq!(listen_changes(&w, m), vec![true, false]);
    // Nothing is announced for a baby that was not listened to when the link drops.
    w.set_net(BABY, false);
    w.advance(40_000);
    assert_eq!(listen_changes(&w, m), vec![true, false]);
    w.set_net(BABY, true);
    w.advance(30_000);
    assert_eq!(w.state(BABY), LinkState::Connected);

    // A lost link resets the intent on the baby, and a re-sent one shows again after recovery.
    let m = w.mark(BABY);
    w.input(PARENT, LinkInput::SetListen { enabled: true });
    assert_eq!(listen_changes(&w, m), vec![true]);
    w.set_net(BABY, false);
    w.advance(30_000);
    assert_eq!(
        listen_changes(&w, m),
        vec![true, false],
        "reset by the loss"
    );
    w.set_net(BABY, true);
    w.advance(30_000);
    assert_eq!(
        listen_changes(&w, m),
        vec![true, false, true],
        "the parent re-sent its intent after a short outage"
    );
    // The parent side is unchanged: it hears nothing about its own request.
    assert_eq!(
        w.count_events(PARENT, 0, |e| matches!(e, LinkEvent::ListenChanged { .. })),
        0
    );
}

#[test]
fn server_restart_while_connected_changes_no_state_and_signaling_returns() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let m = w.mark(PARENT);
    let attempts = w.nodes[BABY].created.len();
    w.server_restart();
    w.advance(10_000);
    assert_eq!(w.state(BABY), LinkState::Connected);
    assert_eq!(w.state(PARENT), LinkState::Connected);
    assert_eq!(
        w.count_events(PARENT, m, |e| matches!(e, LinkEvent::Status { .. })),
        0,
        "no status change at all"
    );
    assert_eq!(w.nodes[BABY].created.len(), attempts, "no new attempt");
    // The mailboxes are closed while verified: a restart of the server is invisible.
    assert_eq!(
        w.nodes[BABY].open_signaling_calls, 1,
        "baby does not reconnect"
    );
    assert!(w.client_id(BABY).is_none() && w.client_id(PARENT).is_none());
    // On loss the mailboxes return at once and the pair recovers through the restarted server.
    w.set_net(BABY, false);
    w.advance(30_000);
    w.set_net(BABY, true);
    w.advance(20_000);
    assert_eq!(w.state(BABY), LinkState::Connected);
    assert_eq!(w.state(PARENT), LinkState::Connected);
}

#[test]
fn server_restart_while_disconnected_recovers() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.set_net(BABY, false);
    w.advance(40_000);
    w.server_restart();
    w.set_net(BABY, true);
    w.advance(30_000);
    assert_eq!(w.state(BABY), LinkState::Connected);
    assert_eq!(w.state(PARENT), LinkState::Connected);
}

#[test]
fn baby_taken_is_retried_until_the_stale_socket_is_gone() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    let ghost = w.add_ghost_baby();
    w.start(BABY, 1);
    w.start(PARENT, 2);
    w.advance(12_000);
    assert!(
        w.nodes[BABY].open_signaling_calls >= 4,
        "retries with backoff"
    );
    assert_ne!(w.state(BABY), LinkState::FailedNeedsUser);
    assert_ne!(w.state(BABY), LinkState::Connected);
    w.remove_ghost(ghost);
    w.advance(30_000);
    assert_eq!(w.state(BABY), LinkState::Connected);
    assert_eq!(w.state(PARENT), LinkState::Connected);
}

#[test]
fn parent_cap_is_retryable_too() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    let second = w.add_node(cfg(Role::Parent, &PSK_A));
    w.start(PARENT, 2);
    w.start(second, 3);
    w.start(BABY, 1);
    w.advance(15_000);
    assert_ne!(w.state(second), LinkState::FailedNeedsUser);
    assert!(w.nodes[second].open_signaling_calls >= 2);
}

fn error_text(code: &str) -> LinkInput {
    LinkInput::MailboxFrame {
        id: 0,
        bytes: json!({"type":"error","code":code,"message":"x",
        })
        .to_string(),
    }
}

#[test]
fn unsupported_protocol_as_the_reply_to_hello_is_terminal() {
    let mut link = Link::new(cfg(Role::Baby, &PSK_A)).unwrap();
    link.handle(0, LinkInput::Start { entropy: 1 });
    link.handle(1, LinkInput::MailboxOpened { id: 0 });
    let acts = link.handle(2, error_text("UNSUPPORTED_PROTOCOL"));
    assert!(acts.contains(&LinkAction::CloseMailbox { id: 0 }));
    let s = link.status().unwrap();
    assert_eq!(s.state, LinkState::FailedNeedsUser);
    assert_eq!(s.key.as_deref(), Some("update_required"));
}

#[test]
fn forged_unsupported_protocol_after_welcome_does_nothing_destructive() {
    // Not yet verified: joined but no peer.
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.start(BABY, 1);
    w.advance(500);
    w.input(BABY, error_text("UNSUPPORTED_PROTOCOL"));
    assert_eq!(w.state(BABY), LinkState::Connecting);
    assert!(w.status(BABY).key.is_none());

    // Verified: the live attempt, the state and the signaling socket are untouched.
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let handle = w.nodes[PARENT].link.live_attempt().unwrap();
    let opens = w.nodes[PARENT].open_signaling_calls;
    let m = w.mark(PARENT);
    w.input(PARENT, error_text("UNSUPPORTED_PROTOCOL"));
    w.advance(5_000);
    assert_eq!(w.state(PARENT), LinkState::Connected);
    assert_eq!(w.nodes[PARENT].link.live_attempt(), Some(handle));
    assert_eq!(w.nodes[PARENT].open_signaling_calls, opens);
    assert_eq!(w.status(PARENT).cause, None);
    assert_eq!(
        w.count_events(PARENT, m, |e| matches!(e, LinkEvent::Failed { .. })),
        0
    );
}

#[test]
fn unsupported_protocol_on_reconnect_keeps_a_verified_link_and_leaves_a_sticky_hint() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let handle = w.nodes[PARENT].link.live_attempt().unwrap();
    w.reject_hello = true;
    w.server_restart();
    w.advance(10_000);
    // Media does not need the relay, and no mailbox is open while verified, so the rejected
    // `hello` never happens: still connected on the same attempt, with no hint yet.
    for n in [BABY, PARENT] {
        let s = w.status(n);
        assert_eq!(s.state, LinkState::Connected, "node {n}");
        assert_eq!(s.cause, None, "node {n}");
        assert!(s.key.is_none());
    }
    assert_eq!(w.nodes[PARENT].link.live_attempt(), Some(handle));

    // Once the verified attempt is gone the link stays running and keeps escalating: a relay
    // that answers hello with UNSUPPORTED_PROTOCOL cannot silence the alarm.
    w.set_net(BABY, false);
    let m = w.mark(PARENT);
    w.advance(200_000);
    let s = w.status(PARENT);
    assert_eq!(s.state, LinkState::Reconnecting);
    assert!(s.key.is_none());
    assert_eq!(s.level, 3);
    assert_eq!(s.cause.as_deref(), Some("update_required"));
    assert_eq!(w.count_events(PARENT, m, is_link_lost), 1);
}

#[test]
fn unsupported_protocol_retries_signaling_on_the_long_backoff_for_a_verified_link() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.set_net(BABY, false);
    w.advance(27_000);
    w.reject_hello = true;
    w.server_restart();
    w.advance(5_000);
    let opens = w.nodes[PARENT].open_signaling_calls;
    w.advance(20_000);
    assert_eq!(
        w.nodes[PARENT].open_signaling_calls, opens,
        "no retry before 30 s"
    );
    w.advance(15_000);
    assert!(w.nodes[PARENT].open_signaling_calls > opens);
    assert_ne!(w.state(PARENT), LinkState::FailedNeedsUser);
}

#[test]
fn unsupported_protocol_is_never_terminal_for_a_paired_before_link() {
    for role in [Role::Baby, Role::Parent] {
        let mut c = cfg(role, &PSK_A);
        c.paired_before = true;
        let mut link = Link::new(c).unwrap();
        link.handle(0, LinkInput::Start { entropy: 1 });
        link.handle(1, LinkInput::MailboxOpened { id: 0 });
        let acts = link.handle(2, error_text("UNSUPPORTED_PROTOCOL"));
        assert!(acts.contains(&LinkAction::CloseMailbox { id: 0 }));
        let s = link.status().unwrap();
        assert_eq!(s.state, LinkState::Connecting, "{role:?}");
        assert_eq!(s.cause.as_deref(), Some("update_required"));
        // The outage keeps escalating from the start of the run.
        link.handle(70_000, LinkInput::Tick);
        assert!(link.status().unwrap().level >= 2);
    }
}

#[test]
fn stale_signaling_socket_is_detected_after_35_seconds_without_traffic() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    // The path dies silently: server messages stop arriving but no close is reported.
    let opens = w.nodes[BABY].open_signaling_calls;
    w.nodes[BABY].net_up = false;
    w.advance(19_000);
    assert_eq!(
        w.nodes[BABY].open_signaling_calls, opens,
        "no pong for < 20 s is fine"
    );
    w.advance(20_000);
    assert!(
        w.nodes[BABY].open_signaling_calls > opens,
        "reopened after the dead timeout"
    );
}

#[test]
fn request_offer_after_parent_restart() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.advance(3_000);
    let t0 = w.now;
    w.restart(PARENT, 99);
    // The baby holds no mailbox while its attempt is verified, so the restarted parent cannot ask
    // for an offer yet: it waits in the room until the baby's liveness timeout reopens the baby's
    // mailbox.
    w.advance(40_000);
    assert_eq!(w.state(PARENT), LinkState::Connected);
    assert_eq!(w.state(BABY), LinkState::Connected);
    // The baby kept its verified attempt until liveness fired (<= 25 s after the last message),
    // then re-offered without further delay.
    let baby_new = *w.nodes[BABY].create_times.last().unwrap();
    assert!(
        baby_new - t0 <= 27_000,
        "re-offered after {} ms",
        baby_new - t0
    );
    assert_eq!(
        w.count_events(PARENT, 0, |e| matches!(e, LinkEvent::Failed { .. })),
        0
    );
}

#[test]
fn baby_without_live_attempt_answers_request_offer_immediately() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    // Both attempts die (baby offline > 30 s, pc failed); the parent keeps requesting.
    w.set_net(BABY, false);
    w.advance(50_000);
    w.set_net(BABY, true);
    let t = w.now;
    w.advance(10_000);
    assert_eq!(w.state(BABY), LinkState::Connected);
    assert!(*w.nodes[BABY].create_times.last().unwrap() - t <= 20_000);
}

#[test]
fn baby_restart_is_recovered_by_the_parent() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.restart(BABY, 77);
    w.advance(60_000);
    assert_eq!(w.state(BABY), LinkState::Connected);
    assert_eq!(w.state(PARENT), LinkState::Connected);
}

#[test]
fn network_available_retries_immediately_and_resets_backoff() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.block_media = true;
    w.start(BABY, 1);
    w.start(PARENT, 2);
    w.advance(100_000);
    let before = w.nodes[BABY].created.len();
    let t = w.now;
    w.block_media = false;
    w.input(BABY, LinkInput::NetworkAvailable);
    w.input(PARENT, LinkInput::NetworkAvailable);
    w.advance(3_000);
    assert!(w.nodes[BABY].created.len() > before, "fresh attempt at {t}");
    assert_eq!(w.state(BABY), LinkState::Connected);
}

#[test]
fn media_blocked_cause_after_three_failed_attempts() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.block_media = true;
    w.start(BABY, 1);
    w.start(PARENT, 2);
    w.advance(70_000);
    let s = w.status(PARENT);
    assert!(s.level >= 2);
    assert_eq!(s.cause.as_deref(), Some("media_blocked"));
}

#[test]
fn peer_absent_and_server_unreachable_cause_hints() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.start(PARENT, 2);
    w.advance(65_000);
    let s = w.status(PARENT);
    assert_eq!(s.state, LinkState::Connecting);
    assert!(s.level >= 2);
    assert_eq!(s.cause.as_deref(), Some("peer_absent"));

    let mut w = World::pair(&PSK_A, &PSK_A);
    w.set_net(PARENT, false);
    w.start(PARENT, 2);
    w.advance(65_000);
    let s = w.status(PARENT);
    assert!(s.level >= 2);
    assert_eq!(s.cause.as_deref(), Some("server_unreachable"));
}

#[test]
fn backoff_between_failed_attempts_follows_the_schedule() {
    // The parent confirmed its session and then hung: every offer goes unanswered and times out
    // after 20 s. (A parent that never confirms gets no offer at all.)
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.start(PARENT, 2);
    w.advance(1_000);
    w.start(BABY, 1);
    while !w.nodes[BABY].link.peer_confirmed() {
        w.advance_by(50, 50);
    }
    w.nodes[PARENT].alive = false;
    w.advance_by(200_000, 100);
    // A request that crosses the baby's own offer replaces that attempt within the same instant.
    let mut t: Vec<u64> = Vec::new();
    for c in &w.nodes[BABY].create_times {
        if t.last().is_none_or(|last| c - last > 1_000) {
            t.push(*c);
        }
    }
    assert!(t.len() >= 6, "{t:?}");
    // Each attempt lives 20 s; gaps after failure are 0, 1, 2, 4, 8, 15, 15, ...
    let gaps: Vec<u64> = t.windows(2).map(|p| p[1] - p[0] - 20_000).collect();
    let expected = [0u64, 1_000, 2_000, 4_000, 8_000, 15_000];
    for (g, e) in gaps.iter().zip(expected) {
        assert!(*g >= e && *g <= e + 600, "gaps {gaps:?}");
    }
}

// ---- time jumps --------------------------------------------------------------------------------

#[test]
fn time_jump_of_ten_minutes_without_peer_loss_keeps_the_attempt() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let handle = w.nodes[BABY].link.live_attempt().unwrap();
    let parent_handle = w.nodes[PARENT].link.live_attempt().unwrap();
    w.jump(600_000);
    // Traffic that arrived while the phones slept is processed before the first tick (fed
    // straight into the links so the signaling reconnect of one phone cannot interleave).
    for (n, attempt) in [(BABY, handle), (PARENT, parent_handle)] {
        let _ = w.nodes[n].link.handle(
            w.now,
            LinkInput::AttemptDataMessage {
                attempt,
                text: hb(1),
            },
        );
    }
    w.advance(5_000);
    assert_eq!(w.nodes[BABY].link.live_attempt(), Some(handle));
    assert_eq!(w.state(BABY), LinkState::Connected);
    assert_eq!(w.state(PARENT), LinkState::Connected);
    assert_eq!(w.count_events(PARENT, 0, is_link_lost), 0);
}

#[test]
fn time_jump_of_ten_minutes_with_peer_loss_alarms_and_recovers() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.jump(600_000);
    let m = w.mark(PARENT);
    w.advance(5_000);
    assert_eq!(w.count_events(PARENT, m, is_link_lost), 1);
    let down = w
        .events_since(PARENT, m)
        .iter()
        .find_map(|e| match e {
            LinkEvent::LinkRestored { down_ms } => Some(*down_ms),
            _ => None,
        })
        .expect("restored after the jump");
    assert!(down >= 590_000);
    assert_eq!(w.state(PARENT), LinkState::Connected);
    // Level 3 was reached on the very first tick after the jump.
    assert!(w.nodes[PARENT].max_level >= 3);
}

#[test]
fn clock_never_runs_backwards() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let t = w.now;
    w.now = t - 4_000;
    w.advance(500);
    assert_eq!(w.state(PARENT), LinkState::Connected);
}

// ---- noise, snooze, probe, quiet, mic ------------------------------------------------------------

#[test]
fn snooze_suppresses_noise_alerts_only_and_marks_them() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let until = w.now + 20_000;
    w.input(
        PARENT,
        LinkInput::SetSnooze {
            until_ms: Some(until),
        },
    );
    let m = w.mark(PARENT);
    loud(&mut w, 1_000);
    assert!(
        w.count_events(PARENT, m, |e| matches!(
            e,
            LinkEvent::NoiseAlert { snoozed: true, .. }
        )) >= 1
    );
    assert_eq!(
        w.count_events(PARENT, m, |e| matches!(
            e,
            LinkEvent::NoiseAlert { snoozed: false, .. }
        )),
        0
    );
    // Probe alerts are user-initiated and never snoozed.
    quiet(&mut w, 1_000);
    let m = w.mark(PARENT);
    w.input(PARENT, LinkInput::RequestProbe);
    assert!(
        w.count_events(PARENT, m, |e| matches!(
            e,
            LinkEvent::NoiseAlert {
                probe: true,
                snoozed: false,
                ..
            }
        )) == 1
    );
    // Link escalation ignores snooze completely.
    w.set_net(BABY, false);
    w.advance(70_000);
    assert!(w.status(PARENT).level >= 2);
    assert!(w.count_events(PARENT, 0, is_link_lost) >= 1);
    // After the snooze ends, alerts are normal again.
    w.set_net(BABY, true);
    w.advance(30_000);
    quiet(&mut w, 1_000);
    let m = w.mark(PARENT);
    loud(&mut w, 1_000);
    assert!(
        w.count_events(PARENT, m, |e| matches!(
            e,
            LinkEvent::NoiseAlert { snoozed: false, .. }
        )) >= 1
    );
}

#[test]
fn probe_makes_the_baby_send_a_tone_and_a_probe_alert() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let m = w.mark(PARENT);
    w.input(PARENT, LinkInput::RequestProbe);
    assert_eq!(w.nodes[BABY].probe_tones, 1);
    assert_eq!(w.nodes[BABY].mic_calls, vec![true]);
    assert_eq!(
        w.count_events(PARENT, m, |e| matches!(
            e,
            LinkEvent::NoiseAlert { probe: true, .. }
        )),
        1
    );
    w.advance(2_500);
    assert_eq!(
        w.nodes[BABY].mic_calls,
        vec![true, false],
        "mic closes after the tone"
    );
}

#[test]
fn probe_before_verification_does_nothing() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.start(PARENT, 2);
    w.input(PARENT, LinkInput::RequestProbe);
    w.input(PARENT, LinkInput::RequestQuiet);
    assert_eq!(w.nodes[BABY].probe_tones, 0);
    assert_eq!(w.nodes[BABY].apply_quiet_calls, 0);
}

// ---- noise escalation (discreet alert style) ---------------------------------------------------

fn escalations(w: &World, m: usize) -> usize {
    w.count_events(PARENT, m, |e| matches!(e, LinkEvent::NoiseEscalated { .. }))
}

fn escalation_delay(w: &mut World, after_ms: Option<u64>) {
    w.input(PARENT, LinkInput::SetNoiseEscalation { after_ms });
}

fn snooze_for(w: &mut World, ms: u64) {
    let until = w.now + ms;
    w.input(
        PARENT,
        LinkInput::SetSnooze {
            until_ms: Some(until),
        },
    );
}

#[test]
fn unacknowledged_noise_escalates_once_after_the_delay() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    escalation_delay(&mut w, Some(60_000));
    let m = w.mark(PARENT);
    loud(&mut w, 50_000);
    assert_eq!(escalations(&w, m), 0, "not before the delay");
    loud(&mut w, 20_000);
    assert_eq!(escalations(&w, m), 1);
    assert!(matches!(
        w.events_since(PARENT, m)
            .iter()
            .find(|e| matches!(e, LinkEvent::NoiseEscalated { .. })),
        Some(LinkEvent::NoiseEscalated { after_ms: 60_000 })
    ));
    loud(&mut w, 60_000);
    assert_eq!(escalations(&w, m), 1, "once per event");
}

#[test]
fn listen_stop_and_snooze_acknowledge_the_event() {
    for ack in 0..3 {
        let mut w = World::pair(&PSK_A, &PSK_A);
        w.connect_pair();
        escalation_delay(&mut w, Some(60_000));
        let m = w.mark(PARENT);
        loud(&mut w, 20_000);
        match ack {
            0 => w.input(PARENT, LinkInput::SetListen { enabled: true }),
            1 => w.input(PARENT, LinkInput::AcknowledgeNoise),
            _ => snooze_for(&mut w, 600_000),
        }
        loud(&mut w, 90_000);
        assert_eq!(escalations(&w, m), 0, "acknowledgment kind {ack}");
    }
}

#[test]
fn an_event_that_starts_while_listening_is_acknowledged() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    escalation_delay(&mut w, Some(30_000));
    w.input(PARENT, LinkInput::SetListen { enabled: true });
    let m = w.mark(PARENT);
    loud(&mut w, 90_000);
    assert_eq!(escalations(&w, m), 0, "the parent was already listening");
}

#[test]
fn escalation_off_never_fires() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    escalation_delay(&mut w, None);
    let m = w.mark(PARENT);
    loud(&mut w, 150_000);
    assert_eq!(escalations(&w, m), 0);
}

#[test]
fn a_delay_set_during_an_event_counts_from_then() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let m = w.mark(PARENT);
    loud(&mut w, 90_000);
    escalation_delay(&mut w, Some(60_000));
    loud(&mut w, 30_000);
    assert_eq!(escalations(&w, m), 0, "armed late: no immediate switch");
    loud(&mut w, 40_000);
    assert_eq!(escalations(&w, m), 1);
}

#[test]
fn a_new_event_after_two_quiet_minutes_restarts_the_count() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    escalation_delay(&mut w, Some(30_000));
    let m = w.mark(PARENT);
    loud(&mut w, 40_000);
    assert_eq!(escalations(&w, m), 1);
    quiet(&mut w, 130_000);
    let m2 = w.mark(PARENT);
    loud(&mut w, 20_000);
    assert_eq!(escalations(&w, m2), 0, "fresh count");
    loud(&mut w, 20_000);
    assert_eq!(escalations(&w, m2), 1);
}

#[test]
fn a_snooze_that_ends_in_noise_starts_the_count_over() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    escalation_delay(&mut w, Some(30_000));
    snooze_for(&mut w, 20_000);
    let m = w.mark(PARENT);
    loud(&mut w, 40_000);
    assert_eq!(
        escalations(&w, m),
        0,
        "the snooze acknowledged it until it ended"
    );
    loud(&mut w, 40_000);
    assert_eq!(escalations(&w, m), 1);
}

#[test]
fn a_test_alert_never_escalates() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    escalation_delay(&mut w, Some(5_000));
    let m = w.mark(PARENT);
    w.input(PARENT, LinkInput::RequestProbe);
    w.advance(60_000);
    assert_eq!(escalations(&w, m), 0);
}

#[test]
fn quiet_request_needs_a_verified_attempt_and_is_rate_limited() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.start(PARENT, 2);
    w.input(PARENT, LinkInput::RequestQuiet);
    w.start(BABY, 1);
    w.advance(5_000);
    assert_eq!(
        w.nodes[BABY].apply_quiet_calls, 0,
        "request before verification is not queued"
    );
    w.input(PARENT, LinkInput::RequestQuiet);
    assert_eq!(w.nodes[BABY].apply_quiet_calls, 1);
    w.advance(5_000);
    w.input(PARENT, LinkInput::RequestQuiet);
    assert_eq!(
        w.nodes[BABY].apply_quiet_calls, 1,
        "second request inside 10 s is dropped"
    );
    w.advance(6_000);
    w.input(PARENT, LinkInput::RequestQuiet);
    assert_eq!(w.nodes[BABY].apply_quiet_calls, 2);

    // The platform answers; the parent sees the new state.
    let m = w.mark(PARENT);
    w.input(
        BABY,
        LinkInput::QuietApplied {
            state: DeviceStatus {
                quiet_active: Some(true),
                ringer: Some(Ringer::Silent),
                ..Default::default()
            },
        },
    );
    assert!(w.events_since(PARENT, m).iter().any(|e| matches!(
        e,
        LinkEvent::PeerStatus { status } if status.quiet_active == Some(true)
    )));
}

#[test]
fn baby_never_applies_quiet_when_the_request_arrives_unverified() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.start(BABY, 1);
    w.advance(500);
    let acts = w.nodes[BABY].link.handle(
        w.now,
        LinkInput::AttemptDataMessage {
            attempt: 1,
            text: json!({"type":"quiet_request","v":3}).to_string(),
        },
    );
    assert!(!acts.iter().any(|a| matches!(a, LinkAction::ApplyQuiet)));
}

#[test]
fn mic_silence_for_30_seconds_is_reported_and_clears_on_signal() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let silent = |w: &World| {
        w.nodes[PARENT]
            .events
            .iter()
            .rev()
            .find_map(|e| match e {
                LinkEvent::PeerStatus { status } => Some(status.mic_silent),
                _ => None,
            })
            .flatten()
    };
    for _ in 0..290 {
        w.advance_by(100, 100);
        w.input(BABY, LinkInput::AudioLevel { rms: 0.0 });
    }
    assert_ne!(silent(&w), Some(true), "not yet after 29 s");
    for _ in 0..20 {
        w.advance_by(100, 100);
        w.input(BABY, LinkInput::AudioLevel { rms: 0.0 });
    }
    assert_eq!(silent(&w), Some(true));
    w.advance_by(100, 100);
    w.input(BABY, LinkInput::AudioLevel { rms: 0.0001 });
    assert_eq!(silent(&w), Some(false));
}

#[test]
fn near_zero_but_nonzero_audio_is_not_silent() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    for _ in 0..400 {
        w.advance_by(100, 100);
        w.input(BABY, LinkInput::AudioLevel { rms: 0.000_001 });
    }
    assert!(!w.nodes[PARENT].events.iter().any(|e| matches!(
        e,
        LinkEvent::PeerStatus { status } if status.mic_silent == Some(true)
    )));
}

// ---- missed events -----------------------------------------------------------------------------

#[test]
fn missed_events_after_an_outage_cover_the_whole_gap() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.advance(3_000);
    w.set_net(BABY, false);
    quiet(&mut w, 35_000);
    quiet(&mut w, 500);
    loud(&mut w, 1_000);
    quiet(&mut w, 1_000);
    w.input(BABY, LinkInput::AudioLevel { rms: 0.9 });
    loud(&mut w, 1_000);
    w.set_net(BABY, true);
    quiet(&mut w, 30_000);
    let ev = w.nodes[PARENT]
        .events
        .iter()
        .find_map(|e| match e {
            LinkEvent::MissedEvents {
                count,
                max_rms,
                complete,
                down_ms,
            } => Some((*count, *max_rms, *complete, *down_ms)),
            _ => None,
        })
        .expect("parent got missed_events");
    assert!(ev.0 >= 1);
    assert!(ev.1 >= 0.5);
    assert!(ev.2, "complete");
    assert!(ev.3 >= 30_000);
}

fn missed_summary(w: &World) -> Option<(u32, bool)> {
    w.nodes[PARENT].events.iter().find_map(|e| match e {
        LinkEvent::MissedEvents {
            count, complete, ..
        } => Some((*count, *complete)),
        _ => None,
    })
}

#[test]
fn missed_events_report_zero_complete_for_a_quiet_outage_with_continuous_samples() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.set_net(BABY, false);
    quiet(&mut w, 40_000);
    w.set_net(BABY, true);
    quiet(&mut w, 30_000);
    assert_eq!(missed_summary(&w), Some((0, true)));
}

#[test]
fn missed_events_without_any_samples_during_the_outage_are_incomplete() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    quiet(&mut w, 2_000);
    w.set_net(BABY, false);
    w.advance(40_000);
    w.set_net(BABY, true);
    w.advance(30_000);
    assert_eq!(missed_summary(&w), Some((0, false)));
}

#[test]
fn missed_events_with_one_three_second_gap_are_incomplete() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.set_net(BABY, false);
    quiet(&mut w, 30_000);
    w.advance_by(3_000, 100);
    quiet(&mut w, 10_000);
    w.set_net(BABY, true);
    quiet(&mut w, 30_000);
    assert_eq!(missed_summary(&w), Some((0, false)));
}

#[test]
fn missed_events_gap_is_reset_per_outage() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.set_net(BABY, false);
    w.advance(40_000);
    w.set_net(BABY, true);
    quiet(&mut w, 30_000);
    assert_eq!(missed_summary(&w), Some((0, false)));
    w.nodes[PARENT].events.clear();
    w.set_net(BABY, false);
    quiet(&mut w, 40_000);
    w.set_net(BABY, true);
    quiet(&mut w, 30_000);
    assert_eq!(missed_summary(&w), Some((0, true)));
}

#[test]
fn missed_events_after_a_baby_restart_are_incomplete() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.nodes[PARENT].events.clear();
    w.set_net(PARENT, false);
    w.restart(BABY, 5);
    loud(&mut w, 1_000);
    w.set_net(PARENT, true);
    w.advance(40_000);
    assert!(
        w.nodes[PARENT].events.iter().any(|e| matches!(
            e,
            LinkEvent::MissedEvents { complete: false, count, .. } if *count >= 1
        )),
        "{:?}",
        w.nodes[PARENT].events
    );
}

// ---- parent events -----------------------------------------------------------------------------

fn battery(pct: u8, charging: bool) -> DeviceStatus {
    DeviceStatus {
        battery_pct: Some(pct),
        charging: Some(charging),
        ..Default::default()
    }
}

fn low_events(w: &World) -> Vec<(u8, bool)> {
    w.nodes[PARENT]
        .events
        .iter()
        .filter_map(|e| match e {
            LinkEvent::LowBattery { pct, critical } => Some((*pct, *critical)),
            _ => None,
        })
        .collect()
}

#[test]
fn low_battery_fires_once_per_crossing() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.status_input(BABY, battery(40, false));
    w.status_input(BABY, battery(20, false));
    assert!(low_events(&w).is_empty());
    w.status_input(BABY, battery(10, false));
    w.status_input(BABY, battery(10, false));
    w.status_input(BABY, battery(15, false));
    assert_eq!(low_events(&w), vec![(10, false)]);
    w.status_input(BABY, battery(0, false));
    assert_eq!(low_events(&w), vec![(10, false), (0, true)]);
    w.status_input(BABY, battery(0, false));
    w.status_input(BABY, battery(5, false));
    assert_eq!(
        low_events(&w).len(),
        2,
        "no repeat inside the same crossing"
    );
    // Re-arm above 15 %, then cross again.
    w.status_input(BABY, battery(60, false));
    w.status_input(BABY, battery(10, false));
    assert_eq!(low_events(&w).len(), 3);
    assert_eq!(low_events(&w)[2], (10, false));
}

#[test]
fn critical_battery_fires_even_while_charging_low_does_not() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.status_input(BABY, battery(10, true));
    assert!(low_events(&w).is_empty(), "10 % while charging is fine");
    w.status_input(BABY, battery(0, true));
    assert_eq!(low_events(&w), vec![(0, true)]);
}

#[test]
fn charger_unplugged_fires_on_the_true_to_false_edge_only() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let n = |w: &World| {
        w.nodes[PARENT]
            .events
            .iter()
            .filter(|e| matches!(e, LinkEvent::ChargerUnplugged))
            .count()
    };
    w.status_input(BABY, battery(80, false));
    assert_eq!(n(&w), 0, "unknown to not charging is not an unplug");
    w.status_input(BABY, battery(80, true));
    assert_eq!(n(&w), 0);
    w.status_input(BABY, battery(80, false));
    assert_eq!(n(&w), 1);
    w.status_input(BABY, battery(75, false));
    assert_eq!(n(&w), 1);
}

// ---- deliberate stop ---------------------------------------------------------------------------

#[test]
fn baby_stop_is_peer_stopped_for_the_parent_not_an_outage() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.advance(3_000);
    let m = w.mark(PARENT);
    w.stop(BABY);
    assert_eq!(w.state(BABY), LinkState::Stopped);
    assert_eq!(w.state(PARENT), LinkState::PeerStopped);
    w.advance(300_000);
    assert_eq!(w.state(PARENT), LinkState::PeerStopped);
    assert_eq!(w.status(PARENT).level, 0);
    assert_eq!(
        w.count_events(PARENT, m, |e| matches!(e, LinkEvent::PeerStopped)),
        1
    );
    assert_eq!(w.count_events(PARENT, m, is_link_lost), 0);
    assert_eq!(w.nodes[PARENT].max_level, 0);
    // No attempts are made against a peer that announced it left.
    let created = w.nodes[PARENT].created.len();
    w.advance(30_000);
    assert_eq!(w.nodes[PARENT].created.len(), created);
    // Starting the baby again returns the parent to connected without an alarm.
    w.start(BABY, 8);
    w.advance(20_000);
    assert_eq!(w.state(PARENT), LinkState::Connected);
    assert_eq!(w.state(BABY), LinkState::Connected);
    assert_eq!(w.count_events(PARENT, m, is_link_lost), 0);
    assert_eq!(w.count_events(PARENT, m, is_restored), 0);
}

#[test]
fn parent_stop_is_peer_stopped_for_the_baby_not_an_outage() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.advance(3_000);
    let m = w.mark(BABY);
    w.stop(PARENT);
    assert_eq!(w.state(BABY), LinkState::PeerStopped);
    w.advance(300_000);
    assert_eq!(w.state(BABY), LinkState::PeerStopped);
    assert_eq!(w.status(BABY).level, 0);
    assert_eq!(
        w.count_events(BABY, m, |e| matches!(e, LinkEvent::PeerStopped)),
        1
    );
    assert_eq!(w.count_events(BABY, m, is_link_lost), 0);
    w.start(PARENT, 9);
    w.advance(20_000);
    assert_eq!(w.state(BABY), LinkState::Connected);
    assert_eq!(w.state(PARENT), LinkState::Connected);
}

#[test]
fn a_crash_is_an_outage_not_a_stop() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.nodes[BABY].alive = false;
    w.advance(90_000);
    assert_eq!(w.state(PARENT), LinkState::Reconnecting);
    assert!(w.status(PARENT).level >= 2);
}

/// The sealed stop that `node` would put on the relay when it stops on purpose, taken straight
/// from the core so that nothing else (the DataChannel notice, the socket close) reaches the peer.
/// The node is stopped afterwards.
fn stop_envelope(w: &mut World, node: usize) -> Value {
    let acts = w.nodes[node].link.handle(w.now, LinkInput::Stop);
    let stops: Vec<_> = signals_in(&acts)
        .into_iter()
        .filter(|(_, p)| {
            decode_message(&psk_of(w, node), p).is_some_and(|m| m["type"] == "stop_notice")
        })
        .collect();
    assert_eq!(stops.len(), 1, "exactly one stop on the relay");
    stops[0].1.clone()
}

/// A pair that verified and then lost the baby's attempt without losing the network: both devices
/// reopen their mailboxes (they were closed while verified) and are back in the room with the
/// peer's session confirmed, so a sealed stop can be taken from the baby's relay traffic.
fn pair_with_open_mailboxes() -> World {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let handle = w.nodes[BABY].link.live_attempt().unwrap();
    // New attempts cannot verify, so the mailboxes stay open.
    w.block_media = true;
    w.input(
        BABY,
        LinkInput::AttemptPcState {
            attempt: handle,
            state: "failed".into(),
        },
    );
    w.advance(1_000);
    assert!(w.client_id(BABY).is_some(), "the baby is back in the room");
    assert!(w.nodes[BABY].link.peer_confirmed());
    w.block_media = false;
    w
}

fn psk_of(w: &World, node: usize) -> Vec<u8> {
    guardedsleep_core::pairing::decode_psk(&w.nodes[node].cfg.psk_b64url).unwrap()
}

/// Baby unreachable for long enough that the parent has no live attempt but still sees the baby in
/// the room.
fn parent_without_attempt() -> World {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.set_net(BABY, false);
    w.advance(27_000);
    assert_eq!(w.state(PARENT), LinkState::Reconnecting);
    assert_eq!(w.nodes[PARENT].link.live_attempt(), None);
    w
}

#[test]
fn a_real_stop_over_signaling_reaches_a_parent_that_lost_its_attempt() {
    let mut w = parent_without_attempt();
    w.set_net(BABY, true);
    // The baby's mailbox was closed while it was verified and reopened on the loss; it needs a
    // moment to be back in the room with the parent's session confirmed.
    w.advance(20_000);
    w.stop(BABY);
    w.advance(500);
    assert_eq!(w.state(PARENT), LinkState::PeerStopped);
    assert_eq!(w.live_attempts(PARENT), 0);
}

#[test]
fn a_verified_link_holds_no_mailbox_and_the_stop_goes_over_the_data_channel() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    // Both mailboxes were closed when the attempt became a link (design section 1).
    for n in [BABY, PARENT] {
        assert!(w.client_id(n).is_none(), "node {n} is not in the room");
        assert_eq!(
            w.nodes[n].open_signaling_calls, 1,
            "node {n} never reconnects"
        );
    }
    w.advance(120_000);
    for n in [BABY, PARENT] {
        assert_eq!(w.nodes[n].open_signaling_calls, 1, "node {n}");
    }
    // A deliberate stop reaches the parent over the DataChannel only: no mailbox is open.
    let m = w.mark(PARENT);
    let before = w.server_signal_log.len();
    w.stop(BABY);
    assert_eq!(w.state(PARENT), LinkState::PeerStopped);
    assert_eq!(
        w.server_signal_log.len(),
        before,
        "nothing went over a mailbox"
    );
    assert_eq!(
        w.count_events(PARENT, m, |e| matches!(e, LinkEvent::PeerStopped)),
        1
    );
    // Entering `peer_stopped` reopens the parent's mailbox: it has to hear the baby's next session.
    w.advance(2_000);
    assert!(w.client_id(PARENT).is_some());
    w.advance(120_000);
    assert_eq!(w.state(PARENT), LinkState::PeerStopped);
    assert_eq!(w.count_events(PARENT, m, is_link_lost), 0);
}

#[test]
fn a_stored_stop_after_a_parent_restart_is_ignored() {
    // The baby stopped on purpose once; a relay kept the notice. The parent has restarted since,
    // so the notice names a session that no longer exists.
    let mut w = pair_with_open_mailboxes();
    let stored = stop_envelope(&mut w, BABY);
    w.restart(PARENT, 4);
    w.restart(BABY, 3);
    w.advance(30_000);
    assert_eq!(w.state(PARENT), LinkState::Connected);
    w.set_net(BABY, false);
    w.advance(27_000);
    assert_eq!(w.state(PARENT), LinkState::Reconnecting);
    let m = w.mark(PARENT);
    w.inject_signal(PARENT, "whoever", stored);
    assert_eq!(w.state(PARENT), LinkState::Reconnecting);
    w.advance(60_000);
    assert!(w.status(PARENT).level >= 2);
    assert_eq!(w.count_events(PARENT, m, is_link_lost), 1);
    assert_eq!(
        w.count_events(PARENT, m, |e| matches!(e, LinkEvent::PeerStopped)),
        0
    );
}

#[test]
fn a_stored_baby_stop_after_the_baby_restarted_is_ignored() {
    // The stop is delivered once and honoured; the baby starts again with a new session and the
    // pair verifies. When the link is lost later, the relay replays the old notice to the
    // unchanged parent: it names a retired baby session.
    let mut w = pair_with_open_mailboxes();
    let stored = stop_envelope(&mut w, BABY);
    w.inject_signal(PARENT, "b", stored.clone());
    assert_eq!(w.state(PARENT), LinkState::PeerStopped);
    w.restart(BABY, 3);
    w.advance(30_000);
    assert_eq!(w.state(PARENT), LinkState::Connected);
    w.set_net(BABY, false);
    w.advance(27_000);
    assert_eq!(w.state(PARENT), LinkState::Reconnecting);
    let m = w.mark(PARENT);
    w.inject_signal(PARENT, "b", stored);
    assert_eq!(w.state(PARENT), LinkState::Reconnecting);
    w.advance(60_000);
    assert!(w.status(PARENT).level >= 2);
    assert_eq!(w.count_events(PARENT, m, is_link_lost), 1);
}

#[test]
fn a_stored_stop_is_ignored_when_the_baby_restarted_before_it_was_delivered() {
    // The notice was never delivered; the baby came back as a new session that the parent
    // confirmed, which retired the old one.
    let mut w = pair_with_open_mailboxes();
    let stored = stop_envelope(&mut w, BABY);
    w.restart(BABY, 3);
    w.advance(30_000);
    assert_eq!(w.state(PARENT), LinkState::Connected);
    let handle = w.nodes[PARENT].link.live_attempt().unwrap();
    w.inject_signal(PARENT, "b", stored);
    assert_eq!(w.state(PARENT), LinkState::Connected);
    assert_eq!(w.nodes[PARENT].link.live_attempt(), Some(handle));
}

#[test]
fn a_replayed_stop_changes_nothing() {
    let mut w = parent_without_attempt();
    // New attempts cannot verify, so the baby's mailbox stays open for the stop.
    w.block_media = true;
    w.set_net(BABY, true);
    w.advance(20_000);
    let stop = stop_envelope(&mut w, BABY);
    w.block_media = false;
    w.inject_signal(PARENT, "b", stop.clone());
    assert_eq!(w.state(PARENT), LinkState::PeerStopped);
    let m = w.mark(PARENT);
    let before = w.status(PARENT);
    w.inject_signal(PARENT, "b", stop.clone());
    assert_eq!(w.status(PARENT), before);
    // A reopened mailbox does not make it count again (the replay state covers the whole run).
    w.server_restart();
    w.advance(5_000);
    w.inject_signal(PARENT, "b", stop);
    assert_eq!(
        w.count_events(PARENT, m, |e| matches!(e, LinkEvent::PeerStopped)),
        0
    );
}

#[test]
fn forged_stops_are_ignored_and_escalation_continues() {
    let mut w = parent_without_attempt();
    let good = {
        // A genuine stop of the baby's current session, kept back.
        w.block_media = true;
        w.set_net(BABY, true);
        w.advance(20_000);
        let stop = stop_envelope(&mut w, BABY);
        w.block_media = false;
        stop
    };
    w.set_net(BABY, false);
    let mut flipped = good.clone();
    let env = good["env"].as_str().unwrap();
    let mut chars: Vec<char> = env.chars().collect();
    let mid = chars.len() / 2;
    chars[mid] = if chars[mid] == 'A' { 'B' } else { 'A' };
    flipped["env"] = json!(chars.into_iter().collect::<String>());
    // A stop of the parent's own role, reflected back at the parent.
    let own_role = {
        let acts = w.nodes[PARENT].link.handle(w.now, LinkInput::Tick);
        let _ = acts;
        let mut other = SealedPeer::new(Role::Parent, &PSK_A, 77);
        other.say_to(
            MessageClass::Control,
            guardedsleep_core::envelope::SessionId::new([1; 16]),
            &json!({"type":"stop_notice"}),
        )
    };
    // Sealed under another pairing's key.
    let other_key = {
        let mut other = SealedPeer::new(Role::Baby, &PSK_B, 78);
        other.say_to(
            MessageClass::Control,
            guardedsleep_core::envelope::SessionId::new([1; 16]),
            &json!({"type":"stop_notice"}),
        )
    };
    // A PSK holder that never confirmed a session names the parent's session guess.
    let unconfirmed = {
        let mut other = SealedPeer::new(Role::Baby, &PSK_A, 79);
        other.say_to(
            MessageClass::Control,
            guardedsleep_core::envelope::SessionId::new([2; 16]),
            &json!({"type":"stop_notice"}),
        )
    };
    let forged = [
        json!({"type":"stop_notice"}),
        json!({"type":"stop_notice","aid":"5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a","mac":"0".repeat(64)}),
        flipped,
        own_role,
        other_key,
        unconfirmed,
    ];
    let m = w.mark(PARENT);
    for p in forged {
        w.inject_signal(PARENT, "anyone", p);
        assert_eq!(w.state(PARENT), LinkState::Reconnecting);
    }
    // Escalation is untouched and still reaches the alarm.
    w.advance(60_000);
    assert!(w.status(PARENT).level >= 2);
    assert_eq!(w.count_events(PARENT, m, is_link_lost), 1);
    assert_eq!(
        w.count_events(PARENT, m, |e| matches!(e, LinkEvent::PeerStopped)),
        0
    );
    // The genuine one still works afterwards: nothing above was recorded as seen.
    w.inject_signal(PARENT, "b", good);
    assert_eq!(w.state(PARENT), LinkState::PeerStopped);
}

#[test]
fn a_stranger_stop_notice_never_counts() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.inject_signal(PARENT, "stranger", json!({"type":"stop_notice"}));
    assert_eq!(w.state(PARENT), LinkState::Connected);
}

// ---- confirmation handshake ----------------------------------------------------------------------

/// A baby `Link` in a room with a parent member "p", joined and waiting for its `here`.
fn baby_with_a_parent_member() -> (Link, SealedPeer, Value) {
    let mut link = Link::new(cfg(Role::Baby, &PSK_A)).unwrap();
    let room = link.room_id().to_owned();
    link.handle(0, LinkInput::Start { entropy: 5 });
    link.handle(1, LinkInput::MailboxOpened { id: 0 });
    let text = |v: Value| LinkInput::MailboxFrame {
        id: 0,
        bytes: v.to_string(),
    };
    link.handle(
        2,
        text(json!({"type":"welcome","clientId":"b","roomId":room})),
    );
    let acts = link.handle(
        3,
        text(json!({"type":"members","peers":[
            {"clientId":"b","role":"baby"},{"clientId":"p","role":"parent"}]})),
    );
    let here = signals_in(&acts)
        .into_iter()
        .find(|(to, _)| to == "p")
        .expect("the baby announces itself to the parent member")
        .1;
    (link, SealedPeer::new(Role::Parent, &PSK_A, 21), here)
}

#[test]
fn a_lost_confirming_echo_is_repaired_by_repeating_the_pending_challenge() {
    let (mut link, mut parent, here) = baby_with_a_parent_member();
    // The parent answers the baby's `here` with a challenge; the baby answers with its own
    // challenge and the echo of the parent's.
    let (_, r1) = parent.accept(3, &here).unwrap();
    let acts = link.handle(4, signal_frame("p", &r1.expect("a challenge")));
    let second = signals_in(&acts)
        .into_iter()
        .next()
        .expect("baby replies")
        .1;
    // The parent confirms the baby and echoes; that echo is lost.
    let (opened, lost_echo) = parent.accept(4, &second).unwrap();
    assert!(opened.confirmed && lost_echo.is_some());
    assert!(!link.peer_confirmed(), "the baby never saw the echo");
    // Nothing repeats too early...
    assert!(signals_in(&link.handle(4_000, LinkInput::Tick)).is_empty());
    // ...and after 5 s the baby sends its `here` again, with the challenge it still waits on.
    let acts = link.handle(5_003, LinkInput::Tick);
    let repeats = signals_in(&acts);
    assert_eq!(repeats.len(), 2, "a plain here and the pending challenge");
    let mut echoed = None;
    for (_, payload) in &repeats {
        if let Ok((_, Some(reply))) = parent.accept(5_003, payload) {
            echoed = Some(reply);
        }
    }
    let echo = echoed.expect("the confirmed parent echoes again");
    link.handle(5_100, signal_frame("p", &echo));
    assert!(link.peer_confirmed());
}

#[test]
fn a_member_that_never_answers_gets_presence_on_a_backoff_and_no_attempt() {
    let (mut link, _silent, _) = baby_with_a_parent_member();
    let mut sent_at = Vec::new();
    for t in (500..400_000).step_by(500) {
        let acts = link.handle(t, LinkInput::Tick);
        if !signals_in(&acts).is_empty() {
            sent_at.push(t);
        }
        // The relay answers every ping, so the mailbox stays alive.
        link.handle(
            t,
            LinkInput::MailboxFrame {
                id: 0,
                bytes: json!({"type":"pong"}).to_string(),
            },
        );
        assert!(
            !acts
                .iter()
                .any(|a| matches!(a, LinkAction::CreateAttempt { .. })),
            "no attempt for a member that never confirmed"
        );
    }
    // 5 s, 15 s and 60 s after the first, then every 5 minutes.
    let gaps: Vec<u64> = sent_at.windows(2).map(|p| p[1] - p[0]).collect();
    assert_eq!(gaps, vec![15_000, 60_000, 300_000], "{sent_at:?}");
}

// ---- envelope sizes ------------------------------------------------------------------------------

#[test]
fn browser_sized_descriptions_fit_their_envelope_class() {
    // Measured in Chrome 152 (audio sendonly plus a DataChannel, four host candidates in the SDP):
    // the offer message is 1,940 bytes without and 2,460 with candidates. The class holds 4,026.
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.nodes[BABY].sdp_pad = 2_300;
    w.nodes[PARENT].sdp_pad = 1_900;
    w.connect_pair();
    let sizes: Vec<usize> = w
        .sent_of(BABY, "sdp")
        .iter()
        .chain(w.sent_of(PARENT, "sdp").iter())
        .map(|(m, _)| m.to_string().len())
        .collect();
    assert!(sizes.iter().any(|n| *n > 2_300), "{sizes:?}");
    assert!(sizes.iter().all(|n| *n < 4_026), "{sizes:?}");
}

#[test]
fn an_sdp_that_does_not_fit_its_class_is_never_sent_and_never_truncated() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.nodes[BABY].sdp_pad = 4_100;
    w.start(BABY, 1);
    w.start(PARENT, 2);
    w.advance(30_000);
    assert!(
        w.count_events(
            BABY,
            0,
            |e| matches!(e, LinkEvent::Failed { reason } if reason == "signal_oversized")
        ) >= 1
    );
    assert!(w.sent_of(BABY, "sdp").is_empty());
    assert_ne!(w.state(BABY), LinkState::Connected);
    assert_ne!(w.state(PARENT), LinkState::Connected);
}

// ---- DTLS fingerprint binding --------------------------------------------------------------------

fn connected_within(w: &mut World, ms: u64) -> bool {
    w.start(BABY, 1);
    w.start(PARENT, 2);
    w.advance(ms);
    w.state(BABY) == LinkState::Connected && w.state(PARENT) == LinkState::Connected
}

#[test]
fn a_dtls_fingerprint_mismatch_never_verifies_even_with_a_valid_proof() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.set_dtls(PARENT, Dtls::Mismatch);
    assert!(!connected_within(&mut w, 3_000));
    assert_ne!(w.state(PARENT), LinkState::Connected);
    assert!(
        w.count_events(
            PARENT,
            0,
            |e| matches!(e, LinkEvent::Failed { reason } if reason == "dtls_fingerprint_mismatch")
        ) >= 1
    );
    // Two in a row at first pairing are terminal, like a wrong key.
    w.advance(10_000);
    assert_eq!(w.state(PARENT), LinkState::FailedNeedsUser);
    assert_eq!(
        w.status(PARENT).key.as_deref(),
        Some("pairing_key_mismatch")
    );
}

#[test]
fn a_dtls_fingerprint_mismatch_after_pairing_is_a_security_warning_and_not_terminal() {
    let mut w = World::empty();
    let mut baby = cfg(Role::Baby, &PSK_A);
    baby.paired_before = true;
    let mut parent = cfg(Role::Parent, &PSK_A);
    parent.paired_before = true;
    w.add_node(baby);
    w.add_node(parent);
    w.set_dtls(PARENT, Dtls::Mismatch);
    assert!(!connected_within(&mut w, 30_000));
    assert_ne!(w.state(PARENT), LinkState::FailedNeedsUser);
    assert!(
        w.count_events(
            PARENT,
            0,
            |e| matches!(e, LinkEvent::SecurityWarning { key } if key == "wrong_key_attempt")
        ) >= 1
    );
}

#[test]
fn a_missing_dtls_report_times_the_attempt_out() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.set_dtls(PARENT, Dtls::Silent);
    assert!(!connected_within(&mut w, 5_000));
    assert_ne!(w.state(PARENT), LinkState::Connected);
    w.advance(20_000);
    assert!(
        w.count_events(
            PARENT,
            0,
            |e| matches!(e, LinkEvent::Failed { reason } if reason == "attempt_timeout")
        ) >= 1
    );
}

#[test]
fn the_dtls_requirement_can_be_switched_off_for_platforms_without_stats() {
    let mut w = World::empty();
    for role in [Role::Baby, Role::Parent] {
        let mut c = cfg(role, &PSK_A);
        c.require_dtls_fingerprint = false;
        w.add_node(c);
        w.set_dtls(w.nodes.len() - 1, Dtls::Silent);
    }
    assert!(connected_within(&mut w, 5_000));
}

/// Hand-driven parent that receives `offer_sdp` from the baby and answers; returns the actions of
/// the DataChannel open.
fn parent_with_offer(offer_sdp: &str, answer_sdp: &str) -> (Link, Vec<LinkAction>) {
    let mut link = Link::new(cfg(Role::Parent, &PSK_A)).unwrap();
    let room = link.room_id().to_owned();
    link.handle(0, LinkInput::Start { entropy: 5 });
    link.handle(1, LinkInput::MailboxOpened { id: 0 });
    let text = |v: Value| LinkInput::MailboxFrame {
        id: 0,
        bytes: v.to_string(),
    };
    link.handle(
        2,
        text(json!({"type":"welcome","clientId":"p","roomId":room})),
    );
    link.handle(
        3,
        text(json!({"type":"members","peers":[
            {"clientId":"b","role":"baby"},{"clientId":"p","role":"parent"}]})),
    );
    // The baby confirms its session first, as a real one would; then it offers.
    let mut baby = SealedPeer::new(Role::Baby, &PSK_A, 21);
    handshake(&mut link, 3, &mut baby, "b");
    let offer = baby.say(
        MessageClass::Description,
        &json!({"type":"sdp","aid":"1f2e3d4c5b6a79880918273645546372",
            "sdp":{"type":"offer","sdp":offer_sdp}}),
    );
    link.handle(4, signal_frame("b", &offer));
    let attempt = link.live_attempt().expect("parent created an attempt");
    link.handle(
        5,
        LinkInput::AttemptLocalDescription {
            attempt,
            sdp_type: "answer".into(),
            sdp: answer_sdp.into(),
        },
    );
    let acts = link.handle(6, LinkInput::AttemptDataOpen { attempt });
    (link, acts)
}

fn sdp_with_lines(lines: &[String]) -> String {
    format!(
        "v=0\r\n{}\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=sendonly\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=recvonly\r\n",
        lines.join("\r\n")
    )
}

fn fp_line(seed: u8) -> String {
    let fp: String = (0..32u8)
        .map(|i| format!("{:02x}", seed.wrapping_add(i)))
        .collect();
    format!("a=fingerprint:sha-256 {}", colon_fp(&fp))
}

fn failed_reason(acts: &[LinkAction]) -> Option<String> {
    acts.iter().find_map(|a| match a {
        LinkAction::Emit {
            event: LinkEvent::Failed { reason },
        } => Some(reason.clone()),
        _ => None,
    })
}

#[test]
fn ambiguous_or_hidden_sdp_fingerprints_fail_the_attempt_as_fingerprint_invalid() {
    let good_answer = format!(
        "v=0\r\nm=audio 9 X 1\r\na=recvonly\r\nm=audio 9 X 1\r\na=sendonly\r\n{}\r\n",
        fp_line(0x40)
    );
    let hidden = format!(
        "v=0\r\ns={}\r\nm=audio 9 X 1\r\na=sendonly\r\nm=audio 9 X 1\r\na=recvonly\r\n{}\r\n",
        fp_line(0x10),
        fp_line(0x20)
    );
    let hidden_only = format!(
        "v=0\r\ns={}\r\nm=audio 9 X 1\r\na=sendonly\r\nm=audio 9 X 1\r\na=recvonly\r\n",
        fp_line(0x10)
    );
    let two = sdp_with_lines(&[fp_line(0x10), fp_line(0x20)]);
    let sha1_only = sdp_with_lines(&["a=fingerprint:sha-1 AA:BB:CC:DD".to_owned()]);
    for (name, offer) in [
        ("hidden in s=", hidden_only),
        ("two values", two),
        ("sha-1", sha1_only),
    ] {
        let (_, acts) = parent_with_offer(&offer, &good_answer);
        assert_eq!(
            failed_reason(&acts).as_deref(),
            Some("fingerprint_invalid"),
            "{name}"
        );
    }
    // A fingerprint hidden in s= next to the real line is not a failure by itself: the real line
    // is the one that counts (and it is what libwebrtc authenticates).
    let (_, acts) = parent_with_offer(&hidden, &good_answer);
    assert_eq!(failed_reason(&acts), None);
    // A bad local SDP is held to the same rule.
    let (_, acts) = parent_with_offer(
        &sdp_with_lines(&[fp_line(0x10)]),
        "v=0\r\nm=audio 9 X 1\r\na=recvonly\r\nm=audio 9 X 1\r\na=sendonly\r\n",
    );
    assert_eq!(failed_reason(&acts).as_deref(), Some("fingerprint_invalid"));
}

#[test]
fn a_matching_proof_with_a_dtls_fingerprint_that_differs_from_the_sdp_does_not_verify() {
    // The parent sees a valid-looking offer; the platform reports another certificate.
    let offer = sdp_with_lines(&[fp_line(0x10)]);
    let answer = format!(
        "v=0\r\nm=audio 9 X 1\r\na=recvonly\r\nm=audio 9 X 1\r\na=sendonly\r\n{}\r\n",
        fp_line(0x40)
    );
    let (mut link, _) = parent_with_offer(&offer, &answer);
    let attempt = link.live_attempt().unwrap();
    let acts = link.handle(
        7,
        LinkInput::AttemptDtlsFingerprint {
            attempt,
            fingerprint: colon_fp(&"cd".repeat(32)),
        },
    );
    assert_eq!(
        failed_reason(&acts).as_deref(),
        Some("dtls_fingerprint_mismatch")
    );
    assert!(
        acts.iter()
            .any(|a| matches!(a, LinkAction::CloseAttempt { .. }))
    );
    assert_ne!(link.status().unwrap().state, LinkState::Connected);
}

// ---- last contact --------------------------------------------------------------------------------

#[test]
fn status_carries_the_monotonic_time_of_the_last_contact_only_while_disconnected() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    assert_eq!(w.status(PARENT).last_alive_ms, None);
    w.advance(12_000);
    w.set_net(BABY, false);
    w.advance(40_000);
    let s = w.status(PARENT);
    assert_eq!(s.state, LinkState::Reconnecting);
    let last = s.last_alive_ms.expect("last contact is known");
    // Heartbeats every 10 s: the last one arrived within one interval before the outage.
    assert!(
        last <= 12_000 + 5_000 && last + 10_000 + 500 >= 17_000,
        "{last}"
    );
    w.advance(20_000);
    assert_eq!(
        w.status(PARENT).last_alive_ms,
        Some(last),
        "stable during the outage"
    );
    w.set_net(BABY, true);
    w.advance(30_000);
    assert_eq!(w.state(PARENT), LinkState::Connected);
    assert_eq!(w.status(PARENT).last_alive_ms, None);
}

// ---- acknowledge ---------------------------------------------------------------------------------

#[test]
fn acknowledge_mutes_for_five_minutes_then_jumps_to_level_3() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.set_net(BABY, false);
    w.advance(10_000);
    // Below level 2 the acknowledgement is ignored.
    w.input(PARENT, LinkInput::AcknowledgeAlarm);
    assert!(
        !w.nodes[PARENT]
            .events
            .iter()
            .any(|e| matches!(e, LinkEvent::AlarmAcknowledged { .. }))
    );
    w.advance(70_000);
    let s = w.status(PARENT);
    assert_eq!(s.level, 2);
    assert!(s.acknowledged_until.is_none());
    let t_ack = w.now;
    w.input(PARENT, LinkInput::AcknowledgeAlarm);
    let until = match w.nodes[PARENT].events.iter().rev().find_map(|e| match e {
        LinkEvent::AlarmAcknowledged { until_ms } => Some(*until_ms),
        _ => None,
    }) {
        Some(u) => u,
        None => panic!("no ack event"),
    };
    assert_eq!(until, t_ack + 300_000);
    assert_eq!(w.status(PARENT).acknowledged_until, Some(until));
    assert_eq!(w.status(PARENT).level, 2);
    // Natural level 3 would arrive at 180 s after last alive; the acknowledgement holds it at 2.
    w.advance(200_000);
    let s = w.status(PARENT);
    assert_eq!(s.level, 2, "still acknowledged");
    assert_eq!(s.state, LinkState::Reconnecting);
    // A second acknowledgement inside the window changes nothing.
    w.input(PARENT, LinkInput::AcknowledgeAlarm);
    assert_eq!(w.status(PARENT).acknowledged_until, Some(until));
    // At ack + 5 min the level becomes 3 immediately.
    while w.now < until - 500 {
        w.advance(500);
    }
    assert_eq!(w.status(PARENT).level, 2);
    w.advance(1_000);
    let s = w.status(PARENT);
    assert_eq!(s.level, 3);
    assert!(s.acknowledged_until.is_none());
}

#[test]
fn acknowledgement_is_cleared_on_recovery() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.set_net(BABY, false);
    w.advance(75_000);
    w.input(PARENT, LinkInput::AcknowledgeAlarm);
    assert!(w.status(PARENT).acknowledged_until.is_some());
    w.set_net(BABY, true);
    w.advance(30_000);
    let s = w.status(PARENT);
    assert_eq!(s.state, LinkState::Connected);
    assert!(s.acknowledged_until.is_none());
    // A new outage starts without an acknowledgement.
    w.set_net(BABY, false);
    w.advance(80_000);
    assert!(w.status(PARENT).acknowledged_until.is_none());
    assert_eq!(w.status(PARENT).level, 2);
}

#[test]
fn only_the_parent_can_acknowledge() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.set_net(PARENT, false);
    w.advance(80_000);
    w.input(BABY, LinkInput::AcknowledgeAlarm);
    assert!(w.status(BABY).acknowledged_until.is_none());
}

// ---- misc ----------------------------------------------------------------------------------------

#[test]
fn rooms_are_derived_from_the_psk() {
    let w = World::pair(&PSK_A, &PSK_B);
    assert_ne!(w.nodes[BABY].link.room_id(), w.nodes[PARENT].link.room_id());
    assert_eq!(w.nodes[BABY].link.room_id().len(), 22);
}

#[test]
fn entropy_makes_attempt_ids_deterministic() {
    let run = |entropy: u64| {
        let mut w = World::pair(&PSK_A, &PSK_A);
        w.start(BABY, entropy);
        w.start(PARENT, 2);
        w.advance(3_000);
        // The ids travel inside sealed envelopes; read them from the decrypted messages.
        w.nodes
            .iter()
            .flat_map(|n| n.sent.iter())
            .filter_map(|(m, _)| m["aid"].as_str().map(str::to_owned))
            .collect::<Vec<_>>()
    };
    assert_eq!(run(5), run(5));
    assert_ne!(run(5), run(6));
    for aid in run(5) {
        assert_eq!(aid.len(), 32);
        assert!(aid.bytes().all(|b| b.is_ascii_hexdigit()));
    }
}

#[test]
fn a_started_link_ignores_a_second_start_and_inputs_for_unknown_handles() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let handle = w.nodes[BABY].link.live_attempt().unwrap();
    let a = w.nodes[BABY]
        .link
        .handle(w.now, LinkInput::Start { entropy: 1 });
    assert!(a.is_empty());
    for bogus in [handle + 7, 0] {
        let a = w.nodes[BABY].link.handle(
            w.now,
            LinkInput::AttemptDataMessage {
                attempt: bogus,
                text: hb(1),
            },
        );
        assert!(a.is_empty(), "{a:?}");
    }
    assert_eq!(w.nodes[BABY].link.live_attempt(), Some(handle));
    let _ = encode_psk;
}

#[test]
fn late_callbacks_for_a_closed_attempt_are_dropped() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.set_net(BABY, false);
    w.advance(60_000);
    let old = w.nodes[BABY].created[0];
    let acts = w.nodes[BABY].link.handle(
        w.now,
        LinkInput::AttemptDataMessage {
            attempt: old,
            text: hb(9),
        },
    );
    assert!(
        !acts
            .iter()
            .any(|a| matches!(a, LinkAction::SendData { attempt, .. } if *attempt == old))
    );
    let acts = w.nodes[BABY].link.handle(
        w.now,
        LinkInput::AttemptPcState {
            attempt: old,
            state: "connected".into(),
        },
    );
    assert!(acts.iter().all(|a| !matches!(a, LinkAction::Emit { .. })));
}

#[test]
fn a_late_data_open_for_a_closed_attempt_keeps_the_pending_resend() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    // Media never opens, so the offer/answer (or request_offer) stays unanswered and armed.
    w.block_media = true;
    w.start(BABY, 1);
    w.start(PARENT, 2);
    w.advance(3_000);
    let live: Vec<(usize, u32)> = [BABY, PARENT]
        .into_iter()
        .filter_map(|n| w.nodes[n].link.live_attempt().map(|h| (n, h)))
        .collect();
    assert!(!live.is_empty());
    for (n, h) in live {
        // A handle the link no longer knows: a callback that raced the close.
        w.input(n, LinkInput::AttemptDataOpen { attempt: h + 1000 });
    }
    w.advance(8_000);
    let mut seen = std::collections::HashSet::new();
    let resent = w
        .server_signal_log
        .iter()
        .any(|(from, p)| !seen.insert((from.clone(), p.to_string())));
    assert!(
        resent,
        "the unanswered message must still be sent once more"
    );
}

// ---- live noise gate ---------------------------------------------------------------------------

fn peer_threshold(w: &World) -> Option<f64> {
    w.nodes[PARENT].events.iter().rev().find_map(|e| match e {
        LinkEvent::PeerStatus { status } => status.noise_threshold,
        _ => None,
    })
}

fn noise_alerts(w: &World) -> usize {
    w.nodes[PARENT]
        .events
        .iter()
        .filter(|e| matches!(e, LinkEvent::NoiseAlert { probe: false, .. }))
        .count()
}

#[test]
fn noise_that_started_before_trust_is_announced_when_the_link_comes_up() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.start(BABY, 1);
    // The gate turns loud while no parent is connected: the transition alert has nowhere to go.
    loud(&mut w, 3_000);
    assert_eq!(noise_alerts(&w), 0);
    w.start(PARENT, 2);
    w.advance(5_000);
    assert_eq!(w.state(PARENT), LinkState::Connected);
    assert_eq!(
        noise_alerts(&w),
        1,
        "the parent must hear about noise that is still going on"
    );
}

#[test]
fn noise_during_an_outage_is_announced_when_the_link_recovers() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.set_net(BABY, false);
    w.advance(45_000);
    loud(&mut w, 3_000);
    let before = noise_alerts(&w);
    w.set_net(BABY, true);
    w.advance(60_000);
    assert_eq!(w.state(PARENT), LinkState::Connected);
    assert!(
        noise_alerts(&w) > before,
        "noise that went on through the outage must be announced on recovery"
    );
}

#[test]
fn baby_status_carries_the_live_noise_threshold_to_the_parent() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    assert_eq!(peer_threshold(&w), Some(0.2));
    w.input(
        BABY,
        LinkInput::SetNoiseGate {
            threshold: 0.4,
            hold_ms: 2_000,
        },
    );
    w.advance(1_000);
    assert_eq!(peer_threshold(&w), Some(0.4));
    assert!(w.nodes[BABY].events.iter().any(|e| matches!(
        e,
        LinkEvent::NoiseGateChanged { threshold, hold_ms: 2_000 } if *threshold == 0.4
    )));
}

#[test]
fn invalid_noise_gate_settings_are_ignored() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    for (threshold, hold_ms) in [
        (0.0, 2_000),
        (0.009, 2_000),
        (1.01, 2_000),
        (f64::NAN, 2_000),
        (f64::INFINITY, 2_000),
        (0.3, 999),
        (0.3, 15_001),
    ] {
        w.input(BABY, LinkInput::SetNoiseGate { threshold, hold_ms });
    }
    w.advance(1_000);
    assert_eq!(peer_threshold(&w), Some(0.2));
    assert!(
        !w.nodes[BABY]
            .events
            .iter()
            .any(|e| matches!(e, LinkEvent::NoiseGateChanged { .. }))
    );
}

#[test]
fn raising_the_threshold_live_stops_alerts_for_the_same_sound() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    quiet(&mut w, 1_000);
    loud(&mut w, 1_500);
    assert_eq!(noise_alerts(&w), 1);
    w.input(
        BABY,
        LinkInput::SetNoiseGate {
            threshold: 0.9,
            hold_ms: 1_000,
        },
    );
    quiet(&mut w, 500);
    loud(&mut w, 3_000);
    assert_eq!(noise_alerts(&w), 1, "0.5 is below the new threshold");
}

#[test]
fn a_gate_change_during_a_loud_state_does_not_alert_twice() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    loud(&mut w, 1_500);
    assert_eq!(noise_alerts(&w), 1);
    w.input(
        BABY,
        LinkInput::SetNoiseGate {
            threshold: 0.2,
            hold_ms: 1_000,
        },
    );
    loud(&mut w, 3_000);
    assert_eq!(noise_alerts(&w), 1);
}
