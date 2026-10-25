//! The simulated public Nostr relays (`alpha-connection-model` task 3.5). A relay here is a small
//! NIP-01 server in virtual time: it verifies events, answers `OK`, serves matching subscriptions,
//! and can misbehave the way real ones do (refuse an event for its time, rate-limit, keep ephemeral
//! events for a few minutes like strfry, accept events without ever delivering them, go away).
//!
//! The platform side is thin on purpose, as in the real clients: one socket per `open_mailbox`, text
//! frames both ways, and no retry of its own. A socket whose path is dead (network down, new
//! address) drops frames silently, like a half-open connection.

use std::collections::BTreeMap;

use guardedsleep_core::link::{LinkInput, LinkState};
use guardedsleep_core::mailbox::MailboxId;
use guardedsleep_core::nostr::{KIND, verify_event};
use guardedsleep_core::testing::event_of_frame;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Ev, Time, ToClient, World};
use crate::scenario::Step;

/// One simulated relay, as a scenario declares it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NostrRelaySpec {
    /// `wss://host` as in the invite.
    pub url: String,
    /// The relay keeps ephemeral events this long and serves them to a new subscription
    /// (strfry's `ephemeralEventsLifetimeSeconds` is 300). 0: it does not.
    #[serde(default)]
    pub retention_s: u64,
    /// The relay accepts events with `OK true` but never forwards or serves them.
    #[serde(default)]
    pub drops: bool,
    /// Events further than this many seconds from the relay's clock are refused with the
    /// `invalid:` message strfry uses. Absent: no check.
    #[serde(default)]
    pub created_at_window_s: Option<u64>,
    /// More than `events` EVENTs within `per_ms` are refused with `rate-limited:`.
    #[serde(default)]
    pub rate_limit: Option<RateLimit>,
    /// The relay is down from the start.
    #[serde(default)]
    pub down: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateLimit {
    pub events: usize,
    pub per_ms: u64,
}

#[derive(Debug, Clone, Default)]
struct Filter {
    kinds: Option<Vec<u64>>,
    authors: Option<Vec<String>>,
    tags: Option<Vec<String>>,
    since: Option<u64>,
    limit: Option<usize>,
}

impl Filter {
    fn parse(v: &Value) -> Option<Filter> {
        let o = v.as_object()?;
        let strings = |k: &str| {
            o.get(k).and_then(Value::as_array).map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_owned))
                    .collect::<Vec<_>>()
            })
        };
        Some(Filter {
            kinds: o
                .get("kinds")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_u64).collect()),
            authors: strings("authors"),
            tags: strings("#t"),
            since: o.get("since").and_then(Value::as_u64),
            limit: o
                .get("limit")
                .and_then(Value::as_u64)
                .and_then(|n| usize::try_from(n).ok()),
        })
    }

    fn matches(&self, e: &guardedsleep_core::nostr::Event) -> bool {
        self.kinds
            .as_ref()
            .is_none_or(|k| k.contains(&u64::from(e.kind)))
            && self
                .authors
                .as_ref()
                .is_none_or(|a| a.iter().any(|x| *x == e.pubkey))
            && self
                .tags
                .as_ref()
                .is_none_or(|t| e.tag_value().is_some_and(|v| t.iter().any(|x| x == v)))
            && self.since.is_none_or(|s| e.created_at >= s)
    }
}

struct Stored {
    at: Time,
    event: guardedsleep_core::nostr::Event,
}

struct RelaySim {
    spec: NostrRelaySpec,
    up: bool,
    drops: bool,
    stored: Vec<Stored>,
    recent: Vec<Time>,
}

struct NSock {
    actor: usize,
    id: MailboxId,
    url: String,
    epoch: u32,
    accepted: bool,
    subs: BTreeMap<String, Vec<Filter>>,
}

/// A text frame an actor sent to a Nostr relay.
pub struct NostrSent {
    pub actor: usize,
    pub t: Time,
    pub url: String,
    pub text: String,
    pub probe: bool,
}

#[derive(Default)]
pub struct NostrWorld {
    relays: BTreeMap<String, RelaySim>,
    socks: BTreeMap<usize, NSock>,
    by_actor: BTreeMap<(usize, MailboxId), usize>,
    next: usize,
    pub sent: Vec<NostrSent>,
}

impl NostrWorld {
    pub fn new(specs: &[NostrRelaySpec]) -> NostrWorld {
        NostrWorld {
            relays: specs
                .iter()
                .map(|s| {
                    (
                        s.url.clone(),
                        RelaySim {
                            up: !s.down,
                            drops: s.drops,
                            spec: s.clone(),
                            stored: Vec::new(),
                            recent: Vec::new(),
                        },
                    )
                })
                .collect(),
            ..Default::default()
        }
    }
}

impl World {
    fn nlat(&mut self) -> u64 {
        self.rng.range(5, 40)
    }

    fn to_client(&mut self, nsock: usize, msg: ToClient) {
        let Some(s) = self.nostr.socks.get(&nsock) else {
            return;
        };
        let (actor, epoch) = (s.actor, s.epoch);
        // A dead path drops silently; the core finds out by the answer timeout.
        if !self.reach(actor, epoch) && !matches!(msg, ToClient::Closed) {
            return;
        }
        let lat = self.nlat();
        let t = self.fifo_time(6, nsock, 0, self.now + lat, false);
        self.push(t, 2, Ev::NToClient { actor, nsock, msg });
    }

    fn to_client_text(&mut self, nsock: usize, v: Value) {
        self.to_client(nsock, ToClient::Text(v.to_string()));
    }

    // ---- platform side -----------------------------------------------------------------

    pub(super) fn nostr_open(&mut self, a: usize, id: MailboxId, url: &str) {
        self.nostr_close(a, id);
        let nsock = self.nostr.next;
        self.nostr.next += 1;
        let epoch = self.actors[a].path_epoch;
        self.nostr.socks.insert(
            nsock,
            NSock {
                actor: a,
                id,
                url: url.to_owned(),
                epoch,
                accepted: false,
                subs: BTreeMap::new(),
            },
        );
        self.nostr.by_actor.insert((a, id), nsock);
        let (relay_up, reach) = (
            self.nostr.relays.get(url).is_some_and(|r| r.up),
            self.reach(a, epoch),
        );
        if !reach {
            let d = 3_000 + self.rng.range(0, 500);
            self.push(
                self.now + d,
                2,
                Ev::NToClient {
                    actor: a,
                    nsock,
                    msg: ToClient::Closed,
                },
            );
        } else if !relay_up {
            let d = self.rng.range(20, 80);
            self.push(
                self.now + d,
                2,
                Ev::NToClient {
                    actor: a,
                    nsock,
                    msg: ToClient::Closed,
                },
            );
        } else {
            let d = self.rng.range(5, 40);
            self.push(self.now + d, 2, Ev::NAccept { nsock });
        }
    }

    pub(super) fn nostr_close(&mut self, a: usize, id: MailboxId) {
        if let Some(nsock) = self.nostr.by_actor.remove(&(a, id)) {
            self.nostr.socks.remove(&nsock);
        }
    }

    pub(super) fn nostr_kill(&mut self, a: usize) {
        let gone: Vec<usize> = self
            .nostr
            .socks
            .iter()
            .filter(|(_, s)| s.actor == a)
            .map(|(k, _)| *k)
            .collect();
        for k in gone {
            self.nostr.socks.remove(&k);
        }
        self.nostr.by_actor.retain(|(actor, _), _| *actor != a);
    }

    pub(super) fn nostr_send(&mut self, a: usize, id: MailboxId, text: String) {
        let Some(&nsock) = self.nostr.by_actor.get(&(a, id)) else {
            return;
        };
        let Some(s) = self.nostr.socks.get(&nsock) else {
            return;
        };
        let (epoch, url) = (s.epoch, s.url.clone());
        let now = self.now;
        // A frame sent while the link has been connected for a while belongs to a reachability
        // check: nothing else opens a Nostr socket then.
        let probe = self.actors[a]
            .statuses
            .last()
            .is_some_and(|(since, st)| st.state == LinkState::Connected && now > since + 1_000);
        self.nostr.sent.push(NostrSent {
            actor: a,
            t: now,
            url,
            text: text.clone(),
            probe,
        });
        if !self.reach(a, epoch) {
            return;
        }
        let lat = self.nlat();
        let t = self.fifo_time(5, nsock, 0, self.now + lat, false);
        self.push(t, 2, Ev::NToRelay { nsock, text });
    }

    pub(super) fn on_nostr_client_rx(&mut self, a: usize, nsock: usize, msg: ToClient) {
        let Some(s) = self.nostr.socks.get(&nsock) else {
            return;
        };
        let id = s.id;
        if self.nostr.by_actor.get(&(a, id)) != Some(&nsock) || !self.actors[a].alive {
            return;
        }
        if matches!(msg, ToClient::Closed) {
            self.nostr.by_actor.remove(&(a, id));
            self.nostr.socks.remove(&nsock);
        }
        let input = match msg {
            ToClient::Opened => LinkInput::MailboxOpened { id },
            ToClient::Text(bytes) => LinkInput::MailboxFrame { id, bytes },
            ToClient::Closed => LinkInput::MailboxClosed {
                id,
                reason: "closed".into(),
            },
        };
        self.deliver(a, input);
    }

    // ---- relay side --------------------------------------------------------------------

    pub(super) fn on_nostr_accept(&mut self, nsock: usize) {
        let Some(s) = self.nostr.socks.get_mut(&nsock) else {
            return;
        };
        let (actor, epoch) = (s.actor, s.epoch);
        let url = s.url.clone();
        let up = self.nostr.relays.get(&url).is_some_and(|r| r.up);
        if !up || !self.reach(actor, epoch) {
            self.to_client(nsock, ToClient::Closed);
            return;
        }
        if let Some(s) = self.nostr.socks.get_mut(&nsock) {
            s.accepted = true;
        }
        self.to_client(nsock, ToClient::Opened);
    }

    pub(super) fn on_nostr_relay_rx(&mut self, nsock: usize, text: &str) {
        let Some(s) = self.nostr.socks.get(&nsock) else {
            return;
        };
        if !s.accepted {
            return;
        }
        let url = s.url.clone();
        if !self.nostr.relays.get(&url).is_some_and(|r| r.up) {
            return;
        }
        let Ok(Value::Array(items)) = serde_json::from_str::<Value>(text) else {
            return;
        };
        match items.first().and_then(Value::as_str) {
            Some("REQ") => self.relay_req(nsock, &url, &items),
            Some("CLOSE") => {
                if let (Some(sub), Some(s)) = (
                    items.get(1).and_then(Value::as_str),
                    self.nostr.socks.get_mut(&nsock),
                ) {
                    s.subs.remove(sub);
                }
            }
            Some("EVENT") => self.relay_event(nsock, &url, text),
            _ => {}
        }
    }

    fn relay_req(&mut self, nsock: usize, url: &str, items: &[Value]) {
        let Some(sub) = items.get(1).and_then(Value::as_str) else {
            return;
        };
        let filters: Vec<Filter> = items.iter().skip(2).filter_map(Filter::parse).collect();
        let now = self.now;
        let mut serve: Vec<guardedsleep_core::nostr::Event> = Vec::new();
        if let Some(r) = self.nostr.relays.get_mut(url) {
            let keep = r.spec.retention_s * 1_000;
            r.stored.retain(|s| now.saturating_sub(s.at) <= keep);
            if !r.drops {
                for st in &r.stored {
                    if filters.iter().any(|f| f.matches(&st.event)) {
                        serve.push(st.event.clone());
                    }
                }
            }
        }
        if let Some(s) = self.nostr.socks.get_mut(&nsock) {
            s.subs.insert(sub.to_owned(), filters.clone());
        }
        // `limit` only matters for the throwaway filter of a reachability check.
        let limit = filters.iter().filter_map(|f| f.limit).min();
        if let Some(n) = limit {
            serve.truncate(n);
        }
        for e in serve {
            self.to_client(
                nsock,
                ToClient::Text(guardedsleep_core::testing::relay_event_frame(sub, &e)),
            );
        }
        self.to_client_text(nsock, json!(["EOSE", sub]));
    }

    fn relay_event(&mut self, nsock: usize, url: &str, text: &str) {
        let Some(event) = event_of_frame(text) else {
            return;
        };
        let now = self.now;
        let true_wall = self.wall_base + now / 1_000;
        let (verdict, drops, retention_ms) = {
            let Some(r) = self.nostr.relays.get_mut(url) else {
                return;
            };
            let mut verdict: Option<&str> = None;
            if !verify_event(&event) || event.kind != KIND {
                verdict = Some("invalid: bad signature");
            } else if r
                .spec
                .created_at_window_s
                .is_some_and(|w| event.created_at.abs_diff(true_wall) > w)
            {
                verdict = Some("invalid: event creation date is too far off from the current time");
            } else if let Some(limit) = &r.spec.rate_limit {
                r.recent.retain(|t| now.saturating_sub(*t) < limit.per_ms);
                if r.recent.len() >= limit.events {
                    verdict = Some("rate-limited: slow down");
                } else {
                    r.recent.push(now);
                }
            }
            (verdict, r.drops, r.spec.retention_s * 1_000)
        };
        if let Some(message) = verdict {
            self.to_client_text(nsock, json!(["OK", event.id, false, message]));
            return;
        }
        self.to_client_text(nsock, json!(["OK", event.id, true, ""]));
        if drops {
            return;
        }
        if retention_ms > 0
            && let Some(r) = self.nostr.relays.get_mut(url)
        {
            r.stored.push(Stored {
                at: now,
                event: event.clone(),
            });
        }
        let targets: Vec<(usize, String)> = self
            .nostr
            .socks
            .iter()
            .filter(|(k, s)| **k != nsock && s.url == url && s.accepted)
            .flat_map(|(k, s)| {
                s.subs
                    .iter()
                    .filter(|(_, fs)| fs.iter().any(|f| f.matches(&event)))
                    .map(move |(sub, _)| (*k, sub.clone()))
            })
            .collect();
        for (k, sub) in targets {
            let frame = guardedsleep_core::testing::relay_event_frame(&sub, &event);
            self.to_client(k, ToClient::Text(frame));
        }
    }

    // ---- timeline verbs ----------------------------------------------------------------

    pub(super) fn nostr_verb(&mut self, st: &Step) {
        let Some(url) = st.id.clone() else {
            self.violations
                .push(format!("`{}` needs `id` (the relay url)", st.verb));
            return;
        };
        if !self.nostr.relays.contains_key(&url) {
            self.violations
                .push(format!("`{}`: unknown nostr relay {url}", st.verb));
            return;
        }
        match st.verb.as_str() {
            "nostr_down" => {
                if let Some(r) = self.nostr.relays.get_mut(&url) {
                    r.up = false;
                    r.stored.clear();
                }
                let gone: Vec<usize> = self
                    .nostr
                    .socks
                    .iter()
                    .filter(|(_, s)| s.url == url)
                    .map(|(k, _)| *k)
                    .collect();
                for k in gone {
                    self.to_client(k, ToClient::Closed);
                }
            }
            "nostr_up" => {
                if let Some(r) = self.nostr.relays.get_mut(&url) {
                    r.up = true;
                }
            }
            // `mode`: `on` accepts without delivering, anything else is honest again.
            _ => {
                if let Some(r) = self.nostr.relays.get_mut(&url) {
                    r.drops = st.mode.as_deref() == Some("on");
                }
            }
        }
    }
}
