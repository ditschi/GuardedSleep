//! The parent pin, the closed mailboxes and the calm hints (`alpha-connection-model` task 3.4),
//! driven through two real `Link`s against the fake server of `common`.

mod common;

use common::*;
use guardedsleep_core::invite::InviteMailbox;
use guardedsleep_core::link::{LinkConfig, LinkEvent, LinkInput, LinkState};
use guardedsleep_core::mailbox::MailboxKind;
use guardedsleep_core::pairing::ParentSecret;
use guardedsleep_core::protocol::Role;

/// A baby and a parent whose device name is `name`.
fn named_pair(name: &str) -> World {
    let mut w = World::empty();
    w.add_node(cfg(Role::Baby, &PSK_A));
    let mut parent: LinkConfig = cfg(Role::Parent, &PSK_A);
    parent.device_name = name.to_owned();
    w.add_node(parent);
    w
}

fn pinned(w: &World, n: usize) -> Vec<ParentSecret> {
    w.nodes[n]
        .events
        .iter()
        .filter_map(|e| match e {
            LinkEvent::ParentPinned { secret } => Some(secret.clone()),
            _ => None,
        })
        .collect()
}

fn names(w: &World, n: usize, mark: usize) -> Vec<String> {
    w.events_since(n, mark)
        .iter()
        .filter_map(|e| match e {
            LinkEvent::ParentConnected { name } => Some(name.clone()),
            _ => None,
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn is_unknown_parent_warning(e: &LinkEvent) -> bool {
    matches!(e, LinkEvent::SecurityWarning { key } if key == "unknown_parent_attempt")
}

fn hint(key: &str, active: bool) -> impl Fn(&LinkEvent) -> bool + '_ {
    move |e| matches!(e, LinkEvent::Hint { key: k, active: a } if k == key && *a == active)
}

fn loud(w: &mut World, ms: u64) {
    for _ in 0..(ms / 100) {
        w.advance_by(100, 100);
        w.input(BABY, LinkInput::AudioLevel { rms: 0.5 });
    }
}

// ---- pinning ---------------------------------------------------------------------------------

#[test]
fn the_first_verification_pins_the_secret_and_shows_the_name() {
    let mut w = named_pair("Pixel 8");
    w.connect_pair();
    let baby = pinned(&w, BABY);
    let parent = pinned(&w, PARENT);
    assert_eq!(baby.len(), 1);
    assert_eq!(
        parent.len(),
        1,
        "the parent keeps it after the acknowledgement"
    );
    assert_eq!(baby[0], parent[0]);
    assert_eq!(baby[0].bytes.len(), 32);
    assert_eq!(names(&w, BABY, 0), vec!["Pixel 8".to_owned()]);
    // Persisted like the platforms do; a pinned config is the next run's input.
    assert_eq!(w.nodes[BABY].cfg.pinned_parent.as_ref(), Some(&baby[0]));
    assert_eq!(w.nodes[PARENT].cfg.pinned_parent.as_ref(), Some(&parent[0]));
}

#[test]
fn the_secret_never_goes_through_a_mailbox_or_a_log_line() {
    let mut w = named_pair("Pixel 8");
    w.connect_pair();
    w.advance(60_000);
    let secret = hex(&pinned(&w, BABY)[0].bytes);
    for (_, frame) in &w.frames {
        assert!(
            !frame.contains(&secret),
            "a mailbox frame carries the secret"
        );
    }
    for n in [BABY, PARENT] {
        for (message, _) in &w.nodes[n].sent {
            assert!(!message.to_string().contains(&secret), "a sealed message");
        }
        // `Debug` of events, config and actions is redacted (the one carrier is `ParentPinned`).
        assert!(!format!("{:?}", w.nodes[n].events).contains(&secret));
        assert!(!format!("{:?}", w.nodes[n].cfg).contains(&secret));
    }
    // The envelope tag is a MAC of the nonce: it never reveals the secret either.
    for (_, payload) in &w.server_signal_log {
        assert!(!payload.to_string().contains(&secret));
    }
}

#[test]
fn later_attempts_prove_the_secret_and_the_baby_shows_the_name_again() {
    let mut w = named_pair("Pixel 8");
    w.connect_pair();
    let m = w.mark(BABY);
    w.restart(BABY, 11);
    w.restart(PARENT, 12);
    w.advance(60_000);
    assert_eq!(w.state(BABY), LinkState::Connected);
    assert_eq!(w.state(PARENT), LinkState::Connected);
    assert_eq!(names(&w, BABY, m), vec!["Pixel 8".to_owned()]);
    assert_eq!(
        w.count_events(BABY, m, |e| matches!(e, LinkEvent::SecurityWarning { .. })),
        0
    );
    // Pinning happened once: the later runs only prove.
    assert_eq!(pinned(&w, BABY).len(), 1);
    assert_eq!(pinned(&w, PARENT).len(), 1);
}

#[test]
fn a_parent_that_lost_the_secret_is_refused_before_any_attempt_and_warned_about() {
    let mut w = named_pair("Pixel 8");
    w.connect_pair();
    w.nodes[PARENT].cfg.pinned_parent = None;
    let baby_attempts = w.nodes[BABY].created.len();
    let m = w.mark(BABY);
    w.restart(PARENT, 12);
    w.advance(120_000);
    assert_eq!(
        w.nodes[BABY].created.len(),
        baby_attempts,
        "the baby never offers to a parent it did not pin"
    );
    assert_eq!(w.count_events(BABY, m, is_unknown_parent_warning), 1);
    assert_ne!(w.state(PARENT), LinkState::Connected);
    assert!(names(&w, BABY, m).is_empty());
    // A warning is not a reply: the refused parent learns nothing from the baby.
    assert!(
        w.nodes[BABY]
            .sent_signal_types
            .iter()
            .all(|t| t != "request_offer")
    );
}

#[test]
fn warnings_about_unknown_parents_are_rate_limited() {
    let mut w = named_pair("Pixel 8");
    w.connect_pair();
    w.nodes[PARENT].cfg.pinned_parent = None;
    let m = w.mark(BABY);
    w.restart(PARENT, 12);
    w.advance(250_000);
    assert_eq!(w.count_events(BABY, m, is_unknown_parent_warning), 1);
    w.advance(200_000);
    assert!(w.count_events(BABY, m, is_unknown_parent_warning) >= 2);
}

// ---- the proof -------------------------------------------------------------------------------

/// A pairing that pinned, then both devices restart; the parent's proof is faulty on the restart.
fn restart_with_faulty_proof(fault: ProofFault) -> (World, usize) {
    let mut w = named_pair("Pixel 8");
    w.connect_pair();
    w.nodes[PARENT].parent_proof = fault;
    let m = w.mark(BABY);
    w.restart(BABY, 11);
    w.restart(PARENT, 12);
    (w, m)
}

#[test]
fn a_tampered_proof_gets_no_microphone_path_and_no_data() {
    let (mut w, m) = restart_with_faulty_proof(ProofFault::Corrupt);
    let pm = w.mark(PARENT);
    // The baby hears something loud the whole time.
    loud(&mut w, 40_000);
    assert!(
        w.count_events(
            BABY,
            m,
            |e| matches!(e, LinkEvent::Failed { reason } if reason == "parent_proof_mismatch")
        ) >= 1
    );
    assert!(w.count_events(BABY, m, is_unknown_parent_warning) >= 1);
    assert_eq!(
        w.count_events(BABY, m, |e| matches!(
            e,
            LinkEvent::Status { status } if status.state == LinkState::Connected
        )),
        0,
        "an attempt that did not prove its parent is never a link"
    );
    assert!(names(&w, BABY, m).is_empty());
    assert!(
        w.nodes[BABY].mic_calls.iter().all(|on| !on),
        "the microphone path never opened: {:?}",
        w.nodes[BABY].mic_calls
    );
    // What the parent got from that attempt: no sound level, no alerts, no phone state.
    for e in w.events_since(PARENT, pm) {
        match e {
            LinkEvent::PeerHeartbeat { rms, .. } => assert_eq!(*rms, None),
            LinkEvent::NoiseAlert { .. } | LinkEvent::PeerStatus { .. } => {
                panic!("sent to an unproven parent: {e:?}")
            }
            _ => {}
        }
    }
}

#[test]
fn a_missing_proof_is_refused_after_ten_seconds() {
    let (mut w, m) = restart_with_faulty_proof(ProofFault::Withhold);
    w.advance(40_000);
    let refusals = w.count_events(
        BABY,
        m,
        |e| matches!(e, LinkEvent::Failed { reason } if reason == "parent_proof_missing"),
    );
    assert!(refusals >= 1, "no refusal");
    assert!(w.count_events(BABY, m, is_unknown_parent_warning) >= 1);
    assert_ne!(w.state(BABY), LinkState::Connected);
    assert!(w.nodes[BABY].mic_calls.iter().all(|on| !on));
}

#[test]
fn a_refused_attempt_does_not_reset_the_baby_outage() {
    let (mut w, m) = restart_with_faulty_proof(ProofFault::Corrupt);
    w.advance(200_000);
    // The refused attempts never counted as recovery, so the outage kept its start.
    assert_eq!(
        w.count_events(BABY, m, |e| matches!(e, LinkEvent::LinkRestored { .. })),
        0
    );
}

// ---- mailboxes while verified ---------------------------------------------------------------

#[test]
fn a_reachability_check_pings_a_socket_that_never_says_hello() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let start = w.frames.len();
    let opens = w.nodes[BABY].open_signaling_calls;
    w.advance(30 * 60 * 1000 + 5_000);
    // Both devices opened one socket for the check and said nothing but a ping on it: no hello
    // (so no room id), no signal (so no sealed envelope and no tag), no key.
    assert_eq!(w.nodes[BABY].open_signaling_calls, opens + 1);
    let frames: Vec<&str> = w.frames[start..].iter().map(|(_, f)| f.as_str()).collect();
    assert_eq!(frames.len(), 2, "{frames:?}");
    assert!(
        frames.iter().all(|f| *f == r#"{"type":"ping"}"#),
        "{frames:?}"
    );
    for n in [BABY, PARENT] {
        let room = w.nodes[n].link.room_id().to_owned();
        assert!(frames.iter().all(|f| !f.contains(&room)));
        assert_eq!(w.state(n), LinkState::Connected);
        assert!(w.client_id(n).is_none(), "the socket is closed again");
        assert_eq!(
            w.count_events(n, 0, |e| matches!(e, LinkEvent::Hint { .. })),
            0,
            "a reachable server raises no hint"
        );
    }
}

#[test]
fn an_unreachable_check_raises_the_calm_hint_and_the_next_one_clears_it() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.advance(30 * 60 * 1000 - 8_000);
    // The parent is offline during its check only.
    w.nodes[PARENT].net_up = false;
    w.advance(14_000);
    w.nodes[PARENT].net_up = true;
    w.advance(1_000);
    assert_eq!(
        w.count_events(PARENT, 0, hint("recovery_path_unavailable", true)),
        1
    );
    assert_eq!(
        w.count_events(BABY, 0, hint("recovery_path_unavailable", true)),
        0
    );
    assert_eq!(
        w.state(PARENT),
        LinkState::Connected,
        "a hint is not an alarm"
    );
    assert_eq!(w.status(PARENT).level, 0);
    // The next check finds the server again.
    w.advance(30 * 60 * 1000);
    assert_eq!(
        w.count_events(PARENT, 0, hint("recovery_path_unavailable", false)),
        1
    );
}

#[test]
fn losing_the_attempt_clears_the_hint_and_reopens_the_mailbox_at_once() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    w.advance(30 * 60 * 1000 - 8_000);
    w.nodes[PARENT].net_up = false;
    w.advance(14_000);
    w.nodes[PARENT].net_up = true;
    assert_eq!(
        w.count_events(PARENT, 0, hint("recovery_path_unavailable", true)),
        1
    );
    let opens = w.nodes[PARENT].open_signaling_calls;
    w.set_net(BABY, false);
    w.advance(30_000);
    assert_eq!(
        w.count_events(PARENT, 0, hint("recovery_path_unavailable", false)),
        1
    );
    assert!(w.nodes[PARENT].open_signaling_calls > opens);
}

// ---- the clock -------------------------------------------------------------------------------

fn clock_pair(baby_s: u64, parent_s: u64) -> World {
    let mut w = World::pair(&PSK_A, &PSK_A);
    // The parent joins the room only after both clocks were reported, so its first `here` has one.
    w.nodes[PARENT].net_up = false;
    w.start(BABY, 1);
    w.input(BABY, LinkInput::WallClock { unix_s: baby_s });
    w.start(PARENT, 2);
    w.input(PARENT, LinkInput::WallClock { unix_s: parent_s });
    w.set_net(PARENT, true);
    w.advance(10_000);
    w
}

#[test]
fn clocks_more_than_ten_minutes_apart_raise_the_hint_on_both_devices() {
    let w = clock_pair(1_000_000, 1_000_000 + 11 * 60);
    for n in [BABY, PARENT] {
        assert_eq!(
            w.count_events(n, 0, hint("clock_skew", true)),
            1,
            "node {n}"
        );
        assert_eq!(w.state(n), LinkState::Connected, "a skew is not an outage");
    }
}

#[test]
fn clocks_within_ten_minutes_raise_nothing() {
    let w = clock_pair(1_000_000, 1_000_000 + 9 * 60);
    for n in [BABY, PARENT] {
        assert_eq!(
            w.count_events(n, 0, |e| matches!(e, LinkEvent::Hint { .. })),
            0
        );
    }
}

// ---- the mailbox update ----------------------------------------------------------------------

fn set(urls: &[(MailboxKind, &str)]) -> Vec<InviteMailbox> {
    urls.iter()
        .map(|(kind, url)| InviteMailbox {
            kind: *kind,
            url: (*url).to_owned(),
        })
        .collect()
}

fn updates(w: &World, n: usize) -> Vec<Vec<InviteMailbox>> {
    w.nodes[n]
        .events
        .iter()
        .filter_map(|e| match e {
            LinkEvent::MailboxesUpdated { mailboxes } => Some(mailboxes.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_baby_sends_its_mailbox_set_after_verification_and_the_parent_stores_it() {
    let mut w = World::empty();
    let mut baby = cfg(Role::Baby, &PSK_A);
    let sent = set(&[
        (MailboxKind::Relay, "wss://relay.example"),
        (MailboxKind::Nostr, "wss://nostr.example"),
    ]);
    baby.mailboxes = sent.clone();
    w.add_node(baby);
    w.add_node(cfg(Role::Parent, &PSK_A));
    w.connect_pair();
    assert_eq!(updates(&w, PARENT), vec![sent.clone()]);
    // After a lost attempt the next verification sends it again.
    w.set_net(BABY, false);
    w.advance(40_000);
    w.set_net(BABY, true);
    w.advance(30_000);
    assert_eq!(updates(&w, PARENT), vec![sent.clone(), sent]);
    assert!(updates(&w, BABY).is_empty());
}

#[test]
fn an_invalid_mailbox_set_is_never_sent_or_stored() {
    let mut w = World::empty();
    let mut baby = cfg(Role::Baby, &PSK_A);
    // A LAN mailbox must be a private address; this one is public.
    baby.mailboxes = set(&[(MailboxKind::Lan, "ws://8.8.8.8:1234")]);
    w.add_node(baby);
    w.add_node(cfg(Role::Parent, &PSK_A));
    w.connect_pair();
    assert!(updates(&w, PARENT).is_empty());
}

#[test]
fn a_received_set_that_breaks_the_invite_rules_is_dropped() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let bad = serde_json::json!({
        "type": "mailboxes", "v": 3,
        "mailboxes": [{"kind": "lan", "url": "ws://8.8.8.8:1234"}]
    })
    .to_string();
    w.inject_data(PARENT, &bad);
    let too_many = serde_json::json!({
        "type": "mailboxes", "v": 3,
        "mailboxes": (0..7).map(|i| serde_json::json!({"kind":"nostr","url":format!("wss://n{i}.example")})).collect::<Vec<_>>()
    })
    .to_string();
    w.inject_data(PARENT, &too_many);
    assert!(updates(&w, PARENT).is_empty());
}

#[test]
fn only_the_parent_takes_a_mailbox_update_and_only_the_baby_a_secret() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let m = w.mark(BABY);
    let update = serde_json::json!({
        "type": "mailboxes", "v": 3,
        "mailboxes": [{"kind": "relay", "url": "wss://relay.example"}]
    })
    .to_string();
    w.inject_data(BABY, &update);
    assert!(updates(&w, BABY).is_empty());
    let pin = serde_json::json!({
        "type": "parent_pin", "v": 3, "secret": "00".repeat(32), "name": "x"
    })
    .to_string();
    // A different secret reaches a baby that pinned another one: refused, not re-pinned.
    w.inject_data(BABY, &pin);
    assert_eq!(pinned(&w, BABY).len(), 1);
    assert!(
        w.count_events(
            BABY,
            m,
            |e| matches!(e, LinkEvent::Failed { reason } if reason == "parent_secret_mismatch")
        ) >= 1
    );
}
