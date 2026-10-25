//! `gs-soak burst`: the reconnect burst of spike unknown 2 and the tag-only filter of unknown 3,
//! against real relays and without the Link. Two clients behind this machine's one IP each
//! publish an offer or answer plus candidates within about a second, each subscribed to the other
//! one's events. Per relay: how many `OK`s, which refusals, how many events arrived and how fast.
//! Throwaway keys, random content and a random tag; nothing derived from a pairing.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

use guardedsleep_core::nostr::{KIND, RelayFrame, parse_relay_frame};
use serde_json::{Value, json};

use crate::sign::{Signer, now_unix_s, random_content};
use crate::stats::percentile;
use crate::ws::{self, Ev, Msg, Out};

/// Offer or answer, as a sealed envelope (base64url characters).
const SDP_BYTES: usize = 2_600;
/// One bundle of candidates.
const CANDIDATE_BYTES: usize = 420;
const CONNECT_WAIT: Duration = Duration::from_secs(12);
const EOSE_WAIT: Duration = Duration::from_secs(8);
const COLLECT: Duration = Duration::from_secs(8);

pub struct BurstOptions {
    pub relays: Vec<String>,
    /// Candidate bundles per client on top of the offer or answer.
    pub candidates: usize,
    /// Time between two events of one client; 0 sends them back to back.
    pub spacing_ms: u64,
    /// Subscribe by tag only (no `authors`).
    pub tag_only: bool,
}

struct Sent {
    at: Instant,
    from: usize,
    ok: Option<(bool, String, Duration)>,
    delivered: Option<Duration>,
}

struct Client {
    tx: Sender<Out>,
    pubkey: String,
    signer: Signer,
}

pub fn run(opts: &BurstOptions) -> Result<Value, String> {
    let mut reports = Vec::new();
    for url in &opts.relays {
        reports.push(one_relay(url, opts)?);
    }
    Ok(json!({
        "mode": "burst",
        "candidates_per_client": opts.candidates,
        "spacing_ms": opts.spacing_ms,
        "tag_only_filter": opts.tag_only,
        "relays": reports,
    }))
}

fn wait_for<F: FnMut(&Msg) -> bool>(
    rx: &Receiver<Msg>,
    budget: Duration,
    mut done: F,
) -> Result<(), ()> {
    let end = Instant::now() + budget;
    loop {
        let left = end.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(msg) => {
                if done(&msg) {
                    return Ok(());
                }
            }
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return Err(()),
        }
    }
}

fn one_relay(url: &str, opts: &BurstOptions) -> Result<Value, String> {
    let (events, rx) = channel::<Msg>();
    let tag = random_content(22)?;
    let mut clients = Vec::new();
    for actor in 0..2 {
        let signer = Signer::new()?;
        // The pubkey of a throwaway key: the first signed event names it.
        let probe = signer.event(&tag, 1, now_unix_s())?;
        clients.push(Client {
            tx: ws::spawn(url.to_owned(), actor, 0, 1, events.clone()),
            pubkey: probe.pubkey,
            signer,
        });
    }
    let started = Instant::now();
    let mut opened = [false; 2];
    let connected = wait_for(&rx, CONNECT_WAIT, |m| {
        match &m.ev {
            Ev::Opened => opened[m.actor.min(1)] = true,
            Ev::Closed(_) => return true,
            Ev::Text(_) => {}
        }
        opened.iter().all(|o| *o)
    });
    if connected.is_err() || !opened.iter().all(|o| *o) {
        close(&clients);
        return Ok(json!({"url": url, "result": "no_connection", "opened": opened}));
    }
    let connect_ms = started.elapsed().as_millis() as u64;
    // Each client listens for the other one.
    for (i, c) in clients.iter().enumerate() {
        let other = &clients[1 - i].pubkey;
        let authors = if opts.tag_only {
            String::new()
        } else {
            format!(",\"authors\":[\"{other}\"]")
        };
        let req = format!(
            "[\"REQ\",\"burst\",{{\"kinds\":[{KIND}],\"#t\":[\"{tag}\"],\"since\":{}{authors}}}]",
            now_unix_s().saturating_sub(5)
        );
        let _ = c.tx.send(Out::Text(req));
    }
    let mut eose = [false; 2];
    let _ = wait_for(&rx, EOSE_WAIT, |m| {
        if let Ev::Text(t) = &m.ev
            && matches!(parse_relay_frame(t), Some(RelayFrame::Eose { .. }))
        {
            eose[m.actor.min(1)] = true;
        }
        eose.iter().all(|e| *e)
    });
    let subscribed = eose.iter().all(|e| *e);

    // The burst: client 0 sends an offer and bundles, client 1 an answer and bundles.
    let mut sent: HashMap<String, Sent> = HashMap::new();
    let t0 = Instant::now();
    let per_client = 1 + opts.candidates;
    let mut order = Vec::new();
    for n in 0..per_client {
        for who in 0..2 {
            order.push((who, n));
        }
    }
    for (who, n) in order {
        let bytes = if n == 0 { SDP_BYTES } else { CANDIDATE_BYTES };
        let c = &clients[who];
        let event = c.signer.event(&tag, bytes, now_unix_s())?;
        sent.insert(
            event.id.clone(),
            Sent {
                at: Instant::now(),
                from: who,
                ok: None,
                delivered: None,
            },
        );
        let _ =
            c.tx.send(Out::Text(format!("[\"EVENT\",{}]", event.to_json())));
        if opts.spacing_ms > 0 && who == 1 {
            std::thread::sleep(Duration::from_millis(opts.spacing_ms));
        }
    }
    let burst_ms = t0.elapsed().as_millis() as u64;
    let mut closed_early = false;
    let mut notices = Vec::new();
    let end = Instant::now() + COLLECT;
    let total = sent.len();
    while Instant::now() < end {
        let left = end.saturating_duration_since(Instant::now());
        let Ok(m) = rx.recv_timeout(left) else { break };
        match m.ev {
            Ev::Closed(_) => {
                closed_early = true;
                break;
            }
            Ev::Opened => {}
            Ev::Text(t) => match parse_relay_frame(&t) {
                Some(RelayFrame::Ok {
                    id,
                    accepted,
                    message,
                }) => {
                    if let Some(s) = sent.get_mut(&id) {
                        s.ok = Some((accepted, message, s.at.elapsed()));
                    }
                }
                Some(RelayFrame::Event { event, .. }) => {
                    if let Some(s) = sent.get_mut(&event.id)
                        && s.from != m.actor
                        && s.delivered.is_none()
                    {
                        s.delivered = Some(s.at.elapsed());
                    }
                }
                Some(RelayFrame::Notice { message }) => notices.push(clip(&message)),
                _ => {}
            },
        }
        if sent
            .values()
            .all(|s| s.ok.is_some() && s.delivered.is_some())
        {
            break;
        }
    }
    close(&clients);

    let accepted = sent
        .values()
        .filter(|s| matches!(&s.ok, Some((true, ..))))
        .count();
    let mut refusals: Vec<String> = sent
        .values()
        .filter_map(|s| match &s.ok {
            Some((false, msg, _)) => Some(clip(msg)),
            _ => None,
        })
        .collect();
    refusals.sort();
    refusals.dedup();
    let no_ok = sent.values().filter(|s| s.ok.is_none()).count();
    let delivered = sent.values().filter(|s| s.delivered.is_some()).count();
    let latencies: Vec<u64> = sent
        .values()
        .filter_map(|s| s.delivered.map(|d| d.as_millis() as u64))
        .collect();
    Ok(json!({
        "url": url,
        "result": "done",
        "connect_ms": connect_ms,
        "subscribed": subscribed,
        "events": total,
        "burst_ms": burst_ms,
        "accepted": accepted,
        "refused": total - accepted - no_ok,
        "no_ok": no_ok,
        "refusal_messages": refusals,
        "notices": notices,
        "delivered": delivered,
        "delivery_ms": {"p50": percentile(&latencies, 50), "p95": percentile(&latencies, 95)},
        "socket_closed_during_burst": closed_early,
    }))
}

fn close(clients: &[Client]) {
    for c in clients {
        let _ = c.tx.send(Out::Close);
    }
}

fn clip(text: &str) -> String {
    text.chars().take(100).collect()
}
