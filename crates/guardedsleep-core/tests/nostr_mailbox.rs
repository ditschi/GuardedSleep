//! The Nostr mailbox kind of a `Link` (`alpha-connection-model` task 3.5): opening and
//! subscribing, the daily address, the event budget, parking, the clock hint and the hand-over of
//! attempts. A hand-driven `Link` talks to a [`NostrPeer`] through fake relay frames.

use guardedsleep_core::invite::InviteMailbox;
use guardedsleep_core::link::{Link, LinkAction, LinkConfig, LinkEvent, LinkInput};
use guardedsleep_core::mailbox::{MailboxId, MailboxKind};
use guardedsleep_core::nostr::{self, Event, KIND, NostrKeys, verify_event};
use guardedsleep_core::pairing::encode_psk;
use guardedsleep_core::protocol::Role;
use guardedsleep_core::testing::{
    NostrPeer, decode_message, frame_input, nostr_events_in, nostr_frames, payload_of, requests_in,
};
use serde_json::{Value, json};

const PSK: [u8; 32] = [7; 32];
const EPOCH: u32 = 1000;
const URL_A: &str = "wss://relay-a.example";
const URL_B: &str = "wss://relay-b.example";
const URL_C: &str = "wss://relay-c.example";

fn nostr_mailbox(url: &str) -> InviteMailbox {
    InviteMailbox {
        kind: MailboxKind::Nostr,
        url: url.into(),
    }
}

struct Rig {
    link: Link,
    now: u64,
    /// The wall clock (Unix seconds) at `now == 0`.
    wall0: u64,
    peer: NostrPeer,
    sub: Vec<(MailboxId, String)>,
    /// Open every Nostr mailbox the link asks for and answer every subscription with EOSE.
    auto: bool,
    /// Everything the link did, with the time.
    log: Vec<(u64, LinkAction)>,
}

impl Rig {
    fn new(role: Role, urls: &[&str], auto: bool, edit: impl FnOnce(&mut LinkConfig)) -> Rig {
        let mut cfg = LinkConfig::with_defaults(role, encode_psk(&PSK), &[5; 32]);
        cfg.mailboxes = urls.iter().map(|u| nostr_mailbox(u)).collect();
        edit(&mut cfg);
        let other = match role {
            Role::Baby => Role::Parent,
            Role::Parent => Role::Baby,
        };
        let mut rig = Rig {
            link: Link::new(cfg).unwrap(),
            now: 0,
            wall0: nostr::epoch_start_s(EPOCH) + 3 * 3600,
            peer: NostrPeer::new(other, &PSK, 21),
            sub: Vec::new(),
            auto,
            log: Vec::new(),
        };
        rig.feed(LinkInput::Start { entropy: 1 });
        rig
    }

    fn parent(urls: &[&str]) -> Rig {
        Rig::new(Role::Parent, urls, true, |_| {})
    }

    fn wall(&self) -> u64 {
        self.wall0 + self.now / 1000
    }

    fn report_wall(&mut self) -> Vec<LinkAction> {
        let unix_s = self.wall();
        self.feed(LinkInput::WallClock { unix_s })
    }

    fn sub_of(&self, id: MailboxId) -> String {
        self.sub
            .iter()
            .rev()
            .find(|(i, _)| *i == id)
            .map(|(_, s)| s.clone())
            .expect("a subscription")
    }

    fn feed(&mut self, input: LinkInput) -> Vec<LinkAction> {
        let mut all = Vec::new();
        let mut queue = vec![input];
        while let Some(next) = queue.pop() {
            let actions = self.link.handle(self.now, next);
            for a in &actions {
                self.log.push((self.now, a.clone()));
                match a {
                    LinkAction::OpenMailbox { id: 0, .. } => {
                        queue.push(LinkInput::MailboxClosed {
                            id: 0,
                            reason: "unreachable".into(),
                        });
                    }
                    LinkAction::OpenMailbox { id, .. } if self.auto => {
                        queue.push(LinkInput::MailboxOpened { id: *id });
                    }
                    LinkAction::MailboxSend { id, frame } if *id != 0 => {
                        if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(frame)
                            && items.first().and_then(Value::as_str) == Some("REQ")
                        {
                            let sub = items[1].as_str().unwrap().to_owned();
                            self.sub.push((*id, sub.clone()));
                            if self.auto {
                                queue.push(frame_input(*id, &format!("[\"EOSE\",\"{sub}\"]")));
                            }
                        }
                    }
                    _ => {}
                }
            }
            all.extend(actions);
        }
        all
    }

    /// Ticks every 250 ms for `ms`, reporting the wall clock every minute like a driver.
    fn advance(&mut self, ms: u64) -> Vec<LinkAction> {
        let mut all = Vec::new();
        let end = self.now + ms;
        while self.now < end {
            self.now = (self.now + 250).min(end);
            if self.now.is_multiple_of(60_000) {
                all.extend(self.report_wall());
            }
            all.extend(self.feed(LinkInput::Tick));
        }
        all
    }

    /// Delivers a frame the peer's relay would deliver on mailbox `id`.
    fn deliver(&mut self, id: MailboxId, payload: &Value) -> Vec<LinkAction> {
        let sub = self.sub_of(id);
        let wall = self.wall();
        let frame = self.peer.frame(&sub, wall, payload);
        self.feed(frame_input(id, &frame))
    }

    /// The peer announces itself and the two sides run the confirmation handshake on mailbox 1.
    fn confirm(&mut self) -> Vec<LinkAction> {
        let mut all = Vec::new();
        let mut pending = vec![self.peer.peer.here()];
        for _ in 0..6 {
            let mut next = Vec::new();
            for p in pending.drain(..) {
                let mut acts = self.deliver(1, &p);
                acts.extend(self.advance(600));
                all.extend(acts.clone());
                for (_, ev) in nostr_events_in(&acts) {
                    if let Ok((_, Some(reply))) = self.peer.peer.accept(self.now, &payload_of(&ev))
                    {
                        next.push(reply);
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            pending = next;
        }
        assert!(
            self.link.peer_confirmed(),
            "the handshake confirmed the peer"
        );
        all
    }

    fn events(actions: &[LinkAction]) -> Vec<(MailboxId, Event)> {
        nostr_events_in(actions)
    }

    /// The sealed messages the link published, by their `type`.
    fn kinds(actions: &[LinkAction]) -> Vec<(MailboxId, String)> {
        Self::events(actions)
            .into_iter()
            .filter_map(|(id, e)| {
                decode_message(&PSK, &payload_of(&e))
                    .map(|m| (id, m["type"].as_str().unwrap_or("").to_owned()))
            })
            .collect()
    }
}

fn opens(actions: &[LinkAction]) -> Vec<(MailboxId, MailboxKind, String)> {
    actions
        .iter()
        .filter_map(|a| match a {
            LinkAction::OpenMailbox { id, kind, url } => Some((*id, *kind, url.clone())),
            _ => None,
        })
        .collect()
}

fn closes(actions: &[LinkAction]) -> Vec<MailboxId> {
    actions
        .iter()
        .filter_map(|a| match a {
            LinkAction::CloseMailbox { id } => Some(*id),
            _ => None,
        })
        .collect()
}

fn hint_events(actions: &[LinkAction], key: &str) -> Vec<bool> {
    actions
        .iter()
        .filter_map(|a| match a {
            LinkAction::Emit {
                event: LinkEvent::Hint { key: k, active },
            } if k == key => Some(*active),
            _ => None,
        })
        .collect()
}

fn keys() -> NostrKeys {
    NostrKeys::derive(&PSK).unwrap()
}

// ---- opening and subscribing -----------------------------------------------------------------

#[test]
fn no_nostr_mailbox_opens_without_a_wall_clock() {
    let mut r = Rig::new(Role::Parent, &[URL_A], false, |_| {});
    let acts = r.advance(4_000);
    assert!(opens(&acts).iter().all(|(id, _, _)| *id == 0));
    assert!(hint_events(&acts, "clock_skew").is_empty());
    // After the grace period the missing clock is a hint, not a silent failure.
    let acts = r.advance(2_000);
    assert_eq!(hint_events(&acts, "clock_skew"), vec![true]);
    // The clock arrives: the mailbox opens and the hint goes away.
    let mut acts = r.report_wall();
    acts.extend(r.advance(500));
    assert_eq!(
        opens(&acts)
            .into_iter()
            .filter(|(id, ..)| *id == 1)
            .collect::<Vec<_>>(),
        vec![(1, MailboxKind::Nostr, URL_A.to_owned())]
    );
    assert_eq!(hint_events(&acts, "clock_skew"), vec![false]);
}

#[test]
fn an_opened_mailbox_subscribes_to_the_peers_daily_address_and_announces_itself() {
    let mut r = Rig::new(Role::Parent, &[URL_A], false, |_| {});
    r.report_wall();
    r.advance(500);
    let acts = r.feed(LinkInput::MailboxOpened { id: 1 });
    let reqs = requests_in(&acts);
    assert_eq!(reqs.len(), 1);
    let (id, sub, filters) = &reqs[0];
    assert_eq!(*id, 1);
    assert!(!sub.is_empty());
    assert_eq!(filters.len(), 1);
    let k = keys();
    assert_eq!(
        filters[0],
        json!({
            "kinds": [KIND],
            "authors": [k.public_key_hex(Role::Baby, EPOCH).unwrap()],
            "#t": [k.tag(EPOCH)],
            "since": r.wall() - 15 * 60,
        })
    );
    // The first `here` goes out at once, signed by the parent's key of the day.
    let events = Rig::events(&acts);
    assert_eq!(events.len(), 1);
    let ev = &events[0].1;
    assert!(verify_event(ev));
    assert_eq!(ev.kind, KIND);
    assert_eq!(ev.pubkey, k.public_key_hex(Role::Parent, EPOCH).unwrap());
    assert_eq!(ev.tag_value(), Some(k.tag(EPOCH).as_str()));
    assert_eq!(ev.created_at, r.wall());
    let msg = decode_message(&PSK, &payload_of(ev)).unwrap();
    assert_eq!(msg["type"], "here");
    assert_eq!(msg["t"], r.wall());
}

#[test]
fn both_days_are_listened_to_around_the_epoch_change_and_the_subscription_follows() {
    let mut r = Rig::parent(&[URL_A]);
    // 40 minutes before the change: one day.
    r.wall0 = nostr::epoch_start_s(EPOCH + 1) - 40 * 60;
    r.report_wall();
    r.advance(1_000);
    let first: Vec<_> = requests_in(&r.log.iter().map(|(_, a)| a.clone()).collect::<Vec<_>>());
    assert_eq!(first.last().unwrap().2.len(), 1);
    // 29 minutes before the change both days are asked for, then the new day alone.
    let acts = r.advance(11 * 60 * 1000 + 30_000);
    let reqs = requests_in(&acts);
    assert_eq!(reqs.last().unwrap().2.len(), 2, "both days near the change");
    let k = keys();
    let authors: Vec<_> = reqs
        .last()
        .unwrap()
        .2
        .iter()
        .map(|f| f["authors"][0].clone())
        .collect();
    assert!(authors.contains(&json!(k.public_key_hex(Role::Baby, EPOCH).unwrap())));
    assert!(authors.contains(&json!(k.public_key_hex(Role::Baby, EPOCH + 1).unwrap())));
    let acts = r.advance(60 * 60 * 1000);
    let reqs = requests_in(&acts);
    let last = reqs.last().unwrap();
    assert_eq!(last.2.len(), 1);
    assert_eq!(last.2[0]["#t"][0], k.tag(EPOCH + 1));
}

#[test]
fn the_subscription_is_repeated_every_minute_and_an_unanswered_one_closes_the_socket() {
    let mut r = Rig::new(Role::Parent, &[URL_A], false, |_| {});
    r.report_wall();
    r.advance(500);
    let acts = r.feed(LinkInput::MailboxOpened { id: 1 });
    let sub = requests_in(&acts)[0].1.clone();
    r.feed(frame_input(1, &format!("[\"EOSE\",\"{sub}\"]")));
    // Answered: the same subscription is issued again after 60 s.
    let acts = r.advance(60_500);
    let reqs = requests_in(&acts);
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].1, sub);
    // Not answered within 10 s: the mailbox counts as closed and is reopened.
    let acts = r.advance(10_500);
    assert_eq!(closes(&acts), vec![1]);
    let acts = r.advance(3_000);
    assert_eq!(
        opens(&acts).into_iter().filter(|(id, ..)| *id == 1).count(),
        1
    );
}

#[test]
fn only_same_wifi_or_my_own_server_opens_no_public_relay() {
    let mut r = Rig::new(Role::Parent, &[URL_A, URL_B], true, |c| c.only_local = true);
    r.report_wall();
    let acts = r.advance(10_000);
    assert!(opens(&acts).iter().all(|(id, ..)| *id == 0));
    assert!(nostr_frames(&acts).is_empty());
}

#[test]
fn a_loopback_relay_is_only_accepted_in_development() {
    let local = |c: &mut LinkConfig| {
        c.mailboxes = vec![InviteMailbox {
            kind: MailboxKind::Nostr,
            url: "ws://127.0.0.1:7777".into(),
        }];
    };
    let mut r = Rig::new(Role::Parent, &[], true, local);
    r.report_wall();
    assert!(opens(&r.advance(2_000)).iter().all(|(id, ..)| *id == 0));
    let mut r = Rig::new(Role::Parent, &[], true, |c| {
        local(c);
        c.allow_dev_loopback = true;
    });
    r.report_wall();
    let acts = r.advance(2_000);
    assert!(opens(&acts).contains(&(1, MailboxKind::Nostr, "ws://127.0.0.1:7777".into())));
}

// ---- presence, confirmation and attempts ------------------------------------------------------

#[test]
fn presence_repeats_on_a_backoff_while_waiting() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    let mut times: Vec<u64> = Vec::new();
    for _ in 0..4_800 {
        let before = r.now;
        let acts = r.advance(250);
        let _ = before;
        if Rig::kinds(&acts).iter().any(|(_, t)| t == "here") {
            times.push(r.now);
        }
    }
    // At open, then after 5, 15 and 60 s, then every 5 minutes (20 minutes simulated here).
    let gaps: Vec<u64> = times.windows(2).map(|w| (w[1] - w[0]) / 1000).collect();
    assert_eq!(&gaps[..3], &[5, 15, 60]);
    assert!(gaps[3..].iter().all(|g| *g == 300), "{gaps:?}");
}

#[test]
fn a_parent_confirms_the_baby_over_nostr_and_asks_for_an_offer_only_then() {
    let mut r = Rig::parent(&[URL_A, URL_B]);
    r.report_wall();
    let acts = r.advance(30_000);
    // Nobody confirmed: presence only, no request for an offer.
    assert!(Rig::kinds(&acts).iter().all(|(_, t)| t == "here"));
    let mut acts = r.confirm();
    acts.extend(r.advance(3_000));
    let asks: Vec<_> = Rig::kinds(&acts)
        .into_iter()
        .filter(|(_, t)| t == "request_offer")
        .collect();
    assert_eq!(asks.len(), 2, "one per relay: {asks:?}");
    assert_eq!(
        asks.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        vec![1, 2]
    );
    // The same envelope goes to both relays.
    let events = Rig::events(&acts);
    let contents: Vec<_> = events
        .iter()
        .filter(|(_, e)| {
            decode_message(&PSK, &payload_of(e)).is_some_and(|m| m["type"] == "request_offer")
        })
        .map(|(_, e)| e.content.clone())
        .collect();
    assert_eq!(contents[0], contents[1]);
}

#[test]
fn a_peer_heard_long_ago_is_not_asked_again() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    r.confirm();
    // The baby falls silent: after 7 minutes without a sign the requests stop.
    r.advance(8 * 60 * 1000);
    let acts = r.advance(2 * 60 * 1000);
    assert!(
        Rig::kinds(&acts).iter().all(|(_, t)| t != "request_offer"),
        "no request to a silent peer"
    );
    // Its presence brings the requests back.
    let here = r.peer.peer.here();
    r.deliver(1, &here);
    let acts = r.advance(40_000);
    assert!(Rig::kinds(&acts).iter().any(|(_, t)| t == "request_offer"));
}

#[test]
fn the_unanswered_request_is_sent_once_more_in_a_new_event_with_the_same_envelope() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    let mark = r.log.len();
    r.confirm();
    r.advance(30_000);
    let acts: Vec<LinkAction> = r.log[mark..].iter().map(|(_, a)| a.clone()).collect();
    let asks: Vec<Event> = Rig::events(&acts)
        .into_iter()
        .map(|(_, e)| e)
        .filter(|e| {
            decode_message(&PSK, &payload_of(e)).is_some_and(|m| m["type"] == "request_offer")
        })
        .collect();
    // A request that nobody answers is sent once more with the same envelope id; every envelope
    // appears at most twice.
    let mut by_content: std::collections::BTreeMap<String, Vec<&Event>> = Default::default();
    for e in &asks {
        by_content.entry(e.content.clone()).or_default().push(e);
    }
    assert!(by_content.values().all(|v| v.len() <= 2));
    let twice: Vec<_> = by_content.values().filter(|v| v.len() == 2).collect();
    assert!(!twice.is_empty(), "a resend happened");
    for pair in twice {
        assert_ne!(pair[0].id, pair[1].id, "a new event");
        assert!(pair[1].created_at >= pair[0].created_at + 5);
        assert!(verify_event(pair[1]));
    }
}

// ---- the budget -------------------------------------------------------------------------------

#[test]
fn events_leave_at_most_twice_a_second_per_relay() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(500);
    // Three candidates ask for a reply each, spaced so every one is answered: the replies are
    // queued behind each other.
    let mut times: Vec<u64> = Vec::new();
    for i in 0..3u8 {
        let mut stranger = NostrPeer::new(Role::Baby, &PSK, 40 + i);
        let here = stranger.peer.here();
        let sub = r.sub_of(1);
        let wall = r.wall();
        let frame = stranger.frame(&sub, wall, &here);
        // The candidate is a different session of the same author key; it is fine for the test.
        let acts = r.feed(frame_input(1, &frame));
        for _ in Rig::events(&acts) {
            times.push(r.now);
        }
        for _ in 0..24 {
            let acts = r.advance(250);
            for _ in Rig::events(&acts) {
                times.push(r.now);
            }
        }
    }
    assert!(times.len() >= 3);
    for w in times.windows(2) {
        assert!(w[1] - w[0] >= 500, "two events {} ms apart", w[1] - w[0]);
    }
}

#[test]
fn an_hour_of_attempts_stays_within_120_events_and_presence_goes_on() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    r.confirm();
    let start = r.now;
    let mut setup = 0usize;
    let mut presence = 0usize;
    let mut since_here = 0u64;
    while r.now - start < 3_600_000 {
        // The baby keeps announcing itself every 4 minutes but never answers a request.
        since_here += 250;
        if since_here >= 240_000 {
            since_here = 0;
            let here = r.peer.peer.here();
            r.deliver(1, &here);
        }
        let acts = r.advance(250);
        for (_, t) in Rig::kinds(&acts) {
            if t == "here" {
                presence += 1;
            } else {
                setup += 1;
            }
        }
    }
    assert!(setup <= 120, "{setup} setup events in an hour");
    assert!(setup >= 100, "the budget is what limits it: {setup}");
    assert!(presence >= 10, "presence kept going: {presence}");
    // The next hour starts afresh.
    let acts = r.advance(10 * 60 * 1000);
    assert!(Rig::kinds(&acts).iter().any(|(_, t)| t == "request_offer"));
}

// ---- parking and the clock --------------------------------------------------------------------

fn refuse(r: &mut Rig, message: &str) -> Vec<LinkAction> {
    let id = "a".repeat(64);
    r.feed(frame_input(
        1,
        &format!("[\"OK\",\"{id}\",false,\"{message}\"]"),
    ))
}

#[test]
fn a_refusing_relay_is_parked_for_one_five_and_thirty_minutes() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    for (i, minutes) in [1u64, 5, 30, 30].into_iter().enumerate() {
        let acts = refuse(&mut r, "rate-limited: slow down");
        assert_eq!(closes(&acts), vec![1], "round {i}");
        // Left out until the backoff is over ...
        let acts = r.advance(minutes * 60_000 - 2_000);
        assert!(
            opens(&acts).iter().all(|(id, ..)| *id != 1),
            "round {i}: reopened early"
        );
        // ... then tried again.
        let acts = r.advance(4_000);
        assert!(
            opens(&acts).iter().any(|(id, ..)| *id == 1),
            "round {i}: not reopened"
        );
        r.advance(1_000);
    }
}

#[test]
fn every_kind_of_refusal_parks_and_other_relays_carry_on() {
    for message in [
        "blocked: not allowed",
        "restricted: members only",
        "auth-required: log in",
        "pow: 20 bits needed",
        "rate-limited: slow down",
    ] {
        let mut r = Rig::parent(&[URL_A, URL_B]);
        r.report_wall();
        r.advance(1_000);
        let acts = refuse(&mut r, message);
        assert_eq!(closes(&acts), vec![1], "{message}");
        // The second relay keeps working.
        let acts = r.advance(20_000);
        assert!(!Rig::events(&acts).iter().any(|(id, _)| *id == 1));
        assert!(Rig::events(&acts).iter().any(|(id, _)| *id == 2));
    }
}

#[test]
fn a_notice_about_a_rate_limit_parks_and_an_ordinary_one_does_not() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    let acts = r.feed(frame_input(1, "[\"NOTICE\",\"hello there\"]"));
    assert!(closes(&acts).is_empty());
    let acts = r.feed(frame_input(
        1,
        "[\"NOTICE\",\"ERROR: rate limit exceeded\"]",
    ));
    assert_eq!(closes(&acts), vec![1]);
}

fn refuse_on(r: &mut Rig, mailbox: MailboxId, message: &str) -> Vec<LinkAction> {
    let id = "a".repeat(64);
    r.feed(frame_input(
        mailbox,
        &format!("[\"OK\",\"{id}\",false,\"{message}\"]"),
    ))
}

fn accept_on(r: &mut Rig, mailbox: MailboxId) -> Vec<LinkAction> {
    let id = "b".repeat(64);
    r.feed(frame_input(
        mailbox,
        &format!("[\"OK\",\"{id}\",true,\"\"]"),
    ))
}

const TOO_FAR: &str = "invalid: event creation date is too far off from the current time";

#[test]
fn one_relay_refusing_the_creation_time_is_not_enough_for_the_clock_hint() {
    let mut r = Rig::parent(&[URL_A, URL_B]);
    r.report_wall();
    r.advance(1_000);
    let acts = refuse_on(&mut r, 1, TOO_FAR);
    assert!(closes(&acts).is_empty(), "not parked");
    assert!(hint_events(&acts, "clock_skew").is_empty());
    // A second relay that agrees raises it, and it goes away by itself after a while.
    let acts = refuse_on(&mut r, 2, TOO_FAR);
    assert_eq!(hint_events(&acts, "clock_skew"), vec![true]);
    assert!(closes(&acts).is_empty());
    let acts = r.advance(31 * 60 * 1000);
    assert_eq!(hint_events(&acts, "clock_skew"), vec![false]);
}

#[test]
fn a_relay_that_keeps_refusing_the_time_the_others_accept_is_parked() {
    let mut r = Rig::parent(&[URL_A, URL_B]);
    r.report_wall();
    r.advance(1_000);
    accept_on(&mut r, 2);
    assert!(closes(&refuse_on(&mut r, 1, TOO_FAR)).is_empty());
    assert!(closes(&refuse_on(&mut r, 1, TOO_FAR)).is_empty());
    // Without a relay that accepts, three refusals would not park (the clock may be the problem).
    let acts = refuse_on(&mut r, 1, TOO_FAR);
    assert_eq!(closes(&acts), vec![1]);
    assert!(
        hint_events(&acts, "clock_skew").is_empty(),
        "one relay only"
    );
}

#[test]
fn forged_events_park_the_relay_and_a_good_one_in_between_is_still_used() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    let sub = r.sub_of(1);
    let wall = r.wall();
    let here = r.peer.peer.here();
    // Two bad events do not park (a relay may be flaky) ...
    for _ in 0..2 {
        let mut ev = r.peer.event(wall, &here);
        ev.sig = "0".repeat(128);
        let acts = r.feed(frame_input(
            1,
            &guardedsleep_core::testing::relay_event_frame(&sub, &ev),
        ));
        assert!(closes(&acts).is_empty());
    }
    // ... a third within the minute does.
    let mut ev = r.peer.event(wall, &here);
    ev.sig = "1".repeat(128);
    let acts = r.feed(frame_input(
        1,
        &guardedsleep_core::testing::relay_event_frame(&sub, &ev),
    ));
    assert_eq!(closes(&acts), vec![1]);
}

#[test]
fn events_by_someone_else_or_of_another_tag_count_as_bad() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    let sub = r.sub_of(1);
    let wall = r.wall();
    // A stranger (other PSK) signed it correctly, but it is not the peer of the subscription.
    let mut stranger = NostrPeer::new(Role::Baby, &[9; 32], 3);
    let here = stranger.peer.here();
    let mut closed = false;
    for _ in 0..3 {
        let frame = stranger.frame(&sub, wall, &here);
        closed |= !closes(&r.feed(frame_input(1, &frame))).is_empty();
    }
    assert!(closed, "three out-of-scope events park the relay");
}

#[test]
fn our_own_events_echoed_by_the_relay_are_dropped_without_a_penalty() {
    // A subscription limited to the daily tag (or a relay that ignores the author filter) hands
    // our own events back. They are not peer traffic and not a forgery: ten of them in a minute
    // do not park the relay.
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    let sub = r.sub_of(1);
    let mut own = NostrPeer::new(Role::Parent, &PSK, 7);
    for _ in 0..10 {
        let wall = r.wall();
        let here = own.peer.here();
        let frame = own.frame(&sub, wall, &here);
        let acts = r.feed(frame_input(1, &frame));
        assert!(closes(&acts).is_empty(), "an echo parks the relay");
    }
    let acts = r.advance(2_000);
    assert!(closes(&acts).is_empty());
}

#[test]
fn a_flood_of_distinct_valid_events_is_dropped_not_punished() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    let sub = r.sub_of(1);
    for _ in 0..200 {
        let wall = r.wall();
        let here = r.peer.peer.here();
        let frame = r.peer.frame(&sub, wall, &here);
        assert!(closes(&r.feed(frame_input(1, &frame))).is_empty());
    }
    // The rest of the window is dropped; the relay is still in use afterwards.
    let acts = r.advance(70_000);
    assert!(closes(&acts).is_empty());
}

#[test]
fn a_replayed_event_is_dropped_before_it_is_counted() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    let sub = r.sub_of(1);
    let wall = r.wall();
    let here = r.peer.peer.here();
    let frame = r.peer.frame(&sub, wall, &here);
    for _ in 0..250 {
        assert!(closes(&r.feed(frame_input(1, &frame))).is_empty());
    }
    // Over 60 copies in a minute neither park nor starve a different, new event.
    let before = r.log.len();
    let again = r.peer.peer.here();
    let wall = r.wall();
    let fresh = r.peer.frame(&sub, wall, &again);
    r.feed(frame_input(1, &fresh));
    assert!(r.log.len() >= before);
}

#[test]
fn events_with_a_time_outside_the_window_are_dropped_without_a_penalty() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    let sub = r.sub_of(1);
    for age in [16 * 60, 60 * 60, 3 * 3600] {
        let here = r.peer.peer.here();
        let frame = r.peer.frame(&sub, r.wall() - age, &here);
        assert!(closes(&r.feed(frame_input(1, &frame))).is_empty());
    }
    for ahead in [11 * 60, 3600] {
        let here = r.peer.peer.here();
        let frame = r.peer.frame(&sub, r.wall() + ahead, &here);
        assert!(closes(&r.feed(frame_input(1, &frame))).is_empty());
    }
    // Five bad ones in a row never added up to a park.
    let acts = r.advance(1_000);
    assert!(closes(&acts).is_empty());
}

#[test]
fn a_resubscription_asks_from_just_before_the_newest_event_seen() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    let sub = r.sub_of(1);
    let here = r.peer.peer.here();
    let at = r.wall();
    let frame = r.peer.frame(&sub, at, &here);
    r.feed(frame_input(1, &frame));
    let acts = r.advance(61_000);
    let reqs = requests_in(&acts);
    let since = reqs.last().unwrap().2[0]["since"].as_u64().unwrap();
    assert_eq!(since, at - 30);
}

#[test]
fn a_signed_time_older_than_seven_minutes_is_no_sign_of_life() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    r.confirm();
    r.advance(8 * 60 * 1000);
    // The peer's presence arrives, but the event is ten minutes old (a relay's store): it proves
    // nothing about the peer being there now, so no request follows.
    let here = r.peer.peer.here();
    let sub = r.sub_of(1);
    let frame = r.peer.frame(&sub, r.wall() - 10 * 60, &here);
    r.feed(frame_input(1, &frame));
    let acts = r.advance(60_000);
    assert!(Rig::kinds(&acts).iter().all(|(_, t)| t != "request_offer"));
}

// ---- relay misbehaviour -----------------------------------------------------------------------

#[test]
fn an_unsolicited_auth_frame_changes_nothing_but_auth_required_answers_park() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    assert!(closes(&r.feed(frame_input(1, "[\"AUTH\",\"challenge\"]"))).is_empty());
    assert_eq!(closes(&refuse(&mut r, "auth-required: log in")), vec![1]);
}

#[test]
fn a_relay_that_talks_too_much_is_disconnected_and_retried_with_backoff() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    let mut closed = None;
    for i in 1..=400 {
        let acts = r.feed(frame_input(1, "[\"NOTICE\",\"hello\"]"));
        if !closes(&acts).is_empty() {
            closed = Some(i);
            break;
        }
    }
    assert!(
        closed.is_some_and(|n| (298..=301).contains(&n)),
        "{closed:?}"
    );
    // Not parked: it comes back after the short backoff.
    let acts = r.advance(3_000);
    assert!(opens(&acts).iter().any(|(id, ..)| *id == 1));
}

#[test]
fn an_error_answer_drops_the_socket_and_a_mute_answer_is_ignored() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    assert!(closes(&refuse(&mut r, "mute: you are muted")).is_empty());
    assert_eq!(closes(&refuse(&mut r, "error: shutting down")), vec![1]);
    let acts = r.advance(3_000);
    assert!(
        opens(&acts).iter().any(|(id, ..)| *id == 1),
        "retried, not parked"
    );
}

// ---- a flapping relay -------------------------------------------------------------------------

#[test]
fn a_relay_that_closes_right_after_eose_gets_a_slower_retry_every_time() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    let mut waits = Vec::new();
    r.advance(500);
    for _ in 0..4 {
        let closed_at = r.now;
        r.feed(LinkInput::MailboxClosed {
            id: 1,
            reason: "closed".into(),
        });
        let mut reopened = None;
        for _ in 0..4_800 {
            let acts = r.advance(250);
            if opens(&acts).iter().any(|(id, ..)| *id == 1) {
                reopened = Some(r.now - closed_at);
                break;
            }
        }
        waits.push(reopened.expect("it reopens"));
    }
    // 2, 5, 15 and 60 s: answering the subscription does not reset the backoff.
    assert!(waits[0] >= 2_000 && waits[0] < 3_000, "{waits:?}");
    assert!(waits[1] >= 5_000 && waits[1] < 6_000, "{waits:?}");
    assert!(waits[2] >= 15_000 && waits[2] < 16_000, "{waits:?}");
    assert!(waits[3] >= 60_000 && waits[3] < 61_000, "{waits:?}");
}

#[test]
fn the_opening_presence_goes_to_a_relay_at_most_every_three_minutes() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    let acts = r.advance(500);
    let heres = |acts: &[LinkAction]| {
        Rig::kinds(acts)
            .iter()
            .filter(|(id, t)| *id == 1 && t == "here")
            .count()
    };
    assert_eq!(heres(&acts), 1);
    r.feed(LinkInput::MailboxClosed {
        id: 1,
        reason: "closed".into(),
    });
    let acts = r.advance(2_500);
    assert!(opens(&acts).iter().any(|(id, ..)| *id == 1));
    assert_eq!(
        heres(&acts),
        0,
        "no second opening presence within 3 minutes"
    );
}

#[test]
fn a_socket_open_for_a_minute_resets_the_backoff() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(500);
    r.feed(LinkInput::MailboxClosed {
        id: 1,
        reason: "closed".into(),
    });
    r.advance(2_600);
    // Open again and healthy for more than a minute ...
    r.advance(70_000);
    let closed_at = r.now;
    r.feed(LinkInput::MailboxClosed {
        id: 1,
        reason: "closed".into(),
    });
    let mut wait = None;
    for _ in 0..40 {
        let acts = r.advance(250);
        if opens(&acts).iter().any(|(id, ..)| *id == 1) {
            wait = Some(r.now - closed_at);
            break;
        }
    }
    // ... so the next failure starts from the 2 s step again, not the 5 s one.
    assert!(
        wait.is_some_and(|w| (2_000..3_000).contains(&w)),
        "{wait:?}"
    );
}

#[test]
fn oversized_and_malformed_frames_are_ignored() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    let long = format!("[\"NOTICE\",\"{}\"]", "x".repeat(70_000));
    for text in [&long[..], "", "{}", "[\"EVENT\"]", "not json"] {
        assert!(closes(&r.feed(frame_input(1, text))).is_empty());
    }
    // Frames for a mailbox that does not exist change nothing.
    assert!(r.feed(frame_input(9, "[\"EOSE\",\"s\"]")).is_empty());
}

// ---- the mailbox set --------------------------------------------------------------------------

#[test]
fn duplicate_and_invalid_entries_make_one_mailbox_each() {
    let mut r = Rig::new(
        Role::Parent,
        &[URL_A, URL_A, "wss://bad host", URL_B],
        true,
        |_| {},
    );
    r.report_wall();
    let acts = r.advance(1_000);
    let urls: Vec<_> = opens(&acts)
        .into_iter()
        .filter(|(id, ..)| *id != 0)
        .map(|(id, _, url)| (id, url))
        .collect();
    assert_eq!(urls, vec![(1, URL_A.to_owned()), (2, URL_B.to_owned())]);
}

#[test]
fn the_third_relay_dies_and_the_others_carry_the_pairing() {
    let mut r = Rig::parent(&[URL_A, URL_B, URL_C]);
    r.report_wall();
    r.advance(1_000);
    let acts = r.feed(LinkInput::MailboxClosed {
        id: 2,
        reason: "gone".into(),
    });
    assert!(closes(&acts).is_empty(), "the platform already closed it");
    // The request leaves right after the handshake; it must reach the live relays only.
    let mut acts = r.confirm();
    acts.extend(r.advance(4_000));
    let asks = Rig::kinds(&acts)
        .into_iter()
        .filter(|(_, t)| t == "request_offer")
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    assert!(
        asks.contains(&1) && asks.contains(&3) && !asks.contains(&2),
        "{asks:?}"
    );
}

// ---- attempts over Nostr ---------------------------------------------------------------------

const AID: &str = "1f2e3d4c5b6a79880918273645546372";

fn offer() -> Value {
    json!({"type":"sdp","aid":AID,"sdp":{"type":"offer","sdp":"v=0\r\nm=audio 9 X 1\r\na=sendonly\r\na=fingerprint:sha-256 AA\r\n"}})
}

fn candidate(n: u32) -> guardedsleep_core::protocol::IceCandidate {
    guardedsleep_core::protocol::IceCandidate {
        candidate: format!("candidate:{n} 1 udp 2122260223 192.168.1.{n} 5000{n} typ host"),
        sdp_mid: Some("0".into()),
        sdp_m_line_index: Some(0),
    }
}

/// The attempt handle in a list of actions.
fn created(actions: &[LinkAction]) -> Option<u32> {
    actions.iter().find_map(|a| match a {
        LinkAction::CreateAttempt { attempt, .. } => Some(*attempt),
        _ => None,
    })
}

#[test]
fn an_offer_over_nostr_starts_an_attempt_and_candidates_leave_in_two_batches() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    r.confirm();
    let payload = r.peer.peer.say(
        guardedsleep_core::envelope::MessageClass::Description,
        &offer(),
    );
    let acts = r.deliver(1, &payload);
    let attempt = created(&acts).expect("the offer started an attempt");
    let mut all = r.feed(LinkInput::AttemptLocalDescription {
        attempt,
        sdp_type: "answer".into(),
        sdp: "v=0\r\nm=audio 9 X 1\r\na=recvonly\r\na=fingerprint:sha-256 BB\r\n".into(),
    });
    // Two candidates close together leave as one message, 400 ms after the first.
    all.extend(r.feed(LinkInput::AttemptLocalIce {
        attempt,
        candidate: candidate(1),
    }));
    all.extend(r.advance(100));
    all.extend(r.feed(LinkInput::AttemptLocalIce {
        attempt,
        candidate: candidate(2),
    }));
    all.extend(r.advance(1_000));
    // A second batch, then a third that the event budget keeps off the public relays.
    all.extend(r.feed(LinkInput::AttemptLocalIce {
        attempt,
        candidate: candidate(3),
    }));
    all.extend(r.advance(2_500));
    all.extend(r.feed(LinkInput::AttemptLocalIce {
        attempt,
        candidate: candidate(4),
    }));
    all.extend(r.advance(5_000));
    let messages: Vec<Value> = Rig::events(&all)
        .into_iter()
        .filter_map(|(_, e)| decode_message(&PSK, &payload_of(&e)))
        .filter(|m| m["type"] == "ice")
        .collect();
    assert_eq!(messages.len(), 2, "{messages:?}");
    assert_eq!(messages[0]["candidates"].as_array().unwrap().len(), 2);
    assert_eq!(messages[1]["candidates"].as_array().unwrap().len(), 1);
    // The answer and the two batches: three of the six setup events of this attempt.
    let kinds: Vec<String> = Rig::kinds(&all).into_iter().map(|(_, t)| t).collect();
    assert_eq!(
        kinds.iter().filter(|t| *t == "sdp").count(),
        1 + 1,
        "answer and its resend"
    );
}

#[test]
fn a_stop_leaves_over_nostr_before_the_sockets_close() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    r.confirm();
    let acts = r.feed(LinkInput::Stop);
    let kinds = Rig::kinds(&acts);
    assert!(kinds.iter().any(|(_, t)| t == "stop_notice"), "{kinds:?}");
    let pos_stop = acts
        .iter()
        .position(|a| matches!(a, LinkAction::MailboxSend { id: 1, frame } if frame.starts_with("[\"EVENT\"")))
        .unwrap();
    let pos_close = acts
        .iter()
        .position(|a| matches!(a, LinkAction::CloseMailbox { id: 1 }))
        .unwrap();
    assert!(pos_stop < pos_close);
}

#[test]
fn a_stop_of_the_peer_over_nostr_ends_the_outage_without_alarm() {
    let mut r = Rig::parent(&[URL_A]);
    r.report_wall();
    r.advance(1_000);
    r.confirm();
    let stop = r.peer.peer.say(
        guardedsleep_core::envelope::MessageClass::Control,
        &json!({"type":"stop_notice"}),
    );
    let acts = r.deliver(1, &stop);
    assert!(acts.iter().any(|a| matches!(
        a,
        LinkAction::Emit {
            event: LinkEvent::PeerStopped
        }
    )));
    // No request for an offer follows a stop, however long we wait.
    let acts = r.advance(60_000);
    assert!(Rig::kinds(&acts).iter().all(|(_, t)| t != "request_offer"));
}
