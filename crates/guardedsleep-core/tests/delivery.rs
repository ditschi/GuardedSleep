//! Delivery rules that hold for every mailbox kind (`alpha-connection-model` task 3.5): ICE
//! candidates are bundled, an unanswered offer, answer or `request_offer` is sent once more with
//! the same envelope id. Driven through the relay mailbox of `common`.

mod common;

use common::*;
use guardedsleep_core::envelope::MessageClass;
use guardedsleep_core::link::LinkInput;
use guardedsleep_core::protocol::{IceCandidate, Role};

fn candidate(n: u32) -> IceCandidate {
    IceCandidate {
        candidate: format!("candidate:{n} 1 udp 2122260223 192.168.1.{n} 5000{n} typ host"),
        sdp_mid: Some("0".into()),
        sdp_m_line_index: Some(0),
    }
}

#[test]
fn candidates_leave_as_one_bundle() {
    // A parent that confirms but never answers the offer keeps the attempt negotiating.
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.start(BABY, 1);
    w.advance(1_000);
    let parent = w.add_impostor("c-parent", Role::Parent, &PSK_A, 34);
    w.impostor_greets(parent, BABY);
    for _ in 0..600 {
        if !w.nodes[BABY].created.is_empty() {
            break;
        }
        w.advance_by(10, 10);
    }
    let attempt = *w.nodes[BABY]
        .created
        .last()
        .expect("the baby made an attempt");
    for n in 1..=3 {
        w.input(
            BABY,
            LinkInput::AttemptLocalIce {
                attempt,
                candidate: candidate(n),
            },
        );
        w.advance_by(30, 10);
    }
    w.advance(500);
    let ice = w.sent_of(BABY, "ice");
    assert_eq!(ice.len(), 1, "one message for three candidates: {ice:?}");
    let list = ice[0].0["candidates"].as_array().expect("a bundle");
    assert_eq!(list.len(), 3);
    assert_eq!(list[0]["candidate"], candidate(1).candidate);
    // A bundle is one envelope of the candidates class, whatever it carries.
    assert!(ice[0].1["env"].as_str().is_some());
}

#[test]
fn a_request_nobody_answers_is_sent_once_more_with_the_same_envelope() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.start(PARENT, 1);
    w.advance(1_000);
    let baby = w.add_impostor("c-baby", Role::Baby, &PSK_A, 33);
    w.impostor_greets(baby, PARENT);
    w.advance(500);
    let first = w.sent_of(PARENT, "request_offer");
    assert_eq!(first.len(), 1);
    // Six seconds without an offer: the same envelope again, once.
    w.advance(6_000);
    let again = w.sent_of(PARENT, "request_offer");
    assert_eq!(again.len(), 2, "{again:?}");
    assert_eq!(again[1].1["env"], again[0].1["env"], "same envelope id");
    w.advance(10_000);
    assert_eq!(
        w.sent_of(PARENT, "request_offer").len(),
        2,
        "never a third copy"
    );
}

#[test]
fn an_answered_request_is_not_repeated() {
    let mut w = World::pair(&PSK_A, &PSK_A);
    w.connect_pair();
    let asks = w.sent_of(PARENT, "request_offer");
    assert!(asks.len() <= 1, "{asks:?}");
    let envs: Vec<_> = asks.iter().map(|(_, p)| p["env"].clone()).collect();
    let mut unique = envs.clone();
    unique.dedup();
    assert_eq!(envs, unique, "no copy of an answered request");
}

#[test]
fn a_message_class_is_not_changed_by_bundling() {
    // The bundle goes in the candidates envelope, the class that always held ICE messages.
    assert_eq!(MessageClass::Candidates.bucket(), 8 * 1024);
}
