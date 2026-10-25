//! The Nostr mailboxes of a `Link` (`alpha-connection-model` task 3.5, design section 3): opening
//! and subscribing, publishing under the event budget, parking, and the reachability check.
//!
//! The wire pieces (epoch, tag, keys, NIP-01 frames) live in `nostr`; the platform only opens one
//! WebSocket per mailbox id and passes text frames. Nothing here reads a clock or a socket: time
//! comes in with `now` (monotonic) and the wall clock the platform reported.
//!
//! Budget (per relay): at most 2 events per second (`PACE_MS`), at most 120 per hour (then only
//! presence and a stop go), at most 6 setup envelopes per attempt and side (counted once for the
//! whole set, since every envelope goes to every relay), and at most 60 inbound events per minute.
//! A relay that refuses, delivers forged events or floods is parked with backoff 1, 5 and 30
//! minutes. A relay's `OK` is not delivery: the resend rule lives in `link.rs`.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chacha20::rand_core::Rng;
use k256::schnorr::SigningKey;
use zeroize::Zeroize;

use super::{HINT_CLOCK_SKEW, Link, LinkAction, Mode};
use crate::envelope::{CHALLENGE_LEN, CHALLENGE_VALID_MS, MessageClass, Reply, Sealed, SessionId};
use crate::invite::{InviteMailbox, MAX_MAILBOXES, validate_mailboxes};
use crate::mailbox::{MailboxId, MailboxKind, MailboxState, Outgoing};
use crate::nostr::{
    Event, Filter, KIND, MAX_FRAME_BYTES, NostrKeys, RelayFrame, SINCE_S, Verdict, build_event,
    classify_closed, classify_notice, classify_ok, close_frame, epoch_of, epochs_to_listen,
    event_frame, parse_relay_frame, probe_req_frame, req_frame, verify_event,
};

/// The `from` of an envelope that came in over Nostr (a relay client id never starts with `~`).
pub(super) const NOSTR_ORIGIN: &str = "~nostr";

/// Spacing of events on one relay (2 per second).
const PACE_MS: u64 = 500;
const HOUR_MS: u64 = 3_600_000;
const HOUR_CAP: u32 = 120;
/// Setup envelopes (offer, answer, `request_offer`, candidates) per attempt and side.
const ATTEMPT_CAP: u32 = 6;
const RX_WINDOW_MS: u64 = 60_000;
const RX_CAP: u32 = 60;
/// Invalid or out-of-scope events within a minute before the relay is parked.
const BAD_WINDOW_MS: u64 = 60_000;
const BAD_CAP: u32 = 3;
const PARK_MS: [u64; 3] = [60_000, 300_000, 1_800_000];
/// A relay that stayed clean for this long starts again at the first backoff step.
const PARK_FORGET_MS: u64 = 2 * HOUR_MS;
const OUTBOX_CAP: usize = 16;
/// While not connected the subscription is re-issued this often (a browser cannot see a half-open
/// socket), and the relay must answer within [`EOSE_WAIT_MS`] or the mailbox counts as closed.
const RESUBSCRIBE_MS: u64 = 60_000;
const EOSE_WAIT_MS: u64 = 10_000;
const OPEN_TIMEOUT_MS: u64 = 15_000;
const RECONNECT_MS: [u64; 5] = [2_000, 5_000, 15_000, 60_000, 300_000];
/// A peer session heard over Nostr this recently counts as present (its `here` repeats at least
/// every 5 minutes while it waits).
pub(super) const PEER_FRESH_MS: u64 = 7 * 60 * 1000;
/// Without a wall clock after this long the clock hint is raised (the platforms report it at start).
const CLOCK_GRACE_MS: u64 = 5_000;
/// A relay's clock refusal keeps the hint up this long.
const CLOCK_REFUSAL_MS: u64 = 30 * 60 * 1000;
/// Unsolicited `here` repeats over Nostr: after these delays, then every 5 minutes.
const HERE_BACKOFF_MS: [u64; 3] = [5_000, 15_000, 60_000];
const HERE_LONG_MS: u64 = 300_000;

/// The material of one epoch: tag, our signing key and the peer's public key.
#[derive(Clone)]
pub(super) struct Day {
    pub epoch: u32,
    pub tag: String,
    pub own: Option<SigningKey>,
    pub peer_pub: Option<String>,
}

/// How a sealed envelope is treated on the Nostr mailboxes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum NostrClass {
    /// Not sent over Nostr (a late trickle batch).
    Skip,
    /// Presence or a stop: allowed over the hourly budget.
    Presence,
    /// Attempt setup: counts against the attempt and hour budgets.
    Setup,
}

fn random_text(rng: &mut impl Rng, bytes: usize) -> String {
    let mut raw = vec![0u8; bytes];
    rng.fill_bytes(&mut raw);
    URL_SAFE_NO_PAD.encode(raw)
}

impl Link {
    // ---- the set ---------------------------------------------------------------------------

    /// The Nostr URLs of a mailbox set that pass the invite grammar.
    fn nostr_urls_of(&self, set: &[InviteMailbox]) -> Vec<String> {
        let mut urls: Vec<String> = Vec::new();
        for m in set.iter().filter(|m| m.kind == MailboxKind::Nostr) {
            if let Some(ok) =
                validate_mailboxes(std::slice::from_ref(m), self.cfg.allow_dev_loopback)
                    .and_then(|v| v.into_iter().next())
                && !urls.contains(&ok.url)
                && urls.len() < MAX_MAILBOXES
            {
                urls.push(ok.url);
            }
        }
        urls
    }

    /// Builds the Nostr mailboxes from the configured set. "Only same Wi-Fi or my own server"
    /// ignores every public entry.
    pub(super) fn init_nostr(&mut self) {
        self.nostr_keys = NostrKeys::derive(&self.psk);
        if self.cfg.only_local {
            return;
        }
        for url in self.nostr_urls_of(&self.cfg.mailboxes) {
            self.mb.add_nostr(&url);
        }
    }

    /// The parent adopts the Nostr entries of a mailbox update from the baby. Relays that are no
    /// longer named are dropped (and closed); the switch "only same Wi-Fi or my own server" keeps
    /// the public entries out.
    pub(super) fn adopt_nostr_set(&mut self, set: &[InviteMailbox], out: &mut Vec<LinkAction>) {
        self.cfg.mailboxes = set.to_vec();
        if self.cfg.only_local {
            return;
        }
        let urls = self.nostr_urls_of(set);
        let mut dropped: Vec<MailboxId> = Vec::new();
        for b in &self.mb.nostr {
            if !urls.contains(&b.url) {
                if b.state != MailboxState::Closed {
                    out.push(LinkAction::CloseMailbox { id: b.id });
                }
                dropped.push(b.id);
            }
        }
        self.mb.nostr.retain(|b| !dropped.contains(&b.id));
        if let Some(p) = self.probe.as_mut() {
            p.pending.retain(|id| !dropped.contains(id));
        }
        for url in &urls {
            self.mb.add_nostr(url);
        }
    }

    pub(super) fn nostr_any_open(&self) -> bool {
        self.mb.nostr.iter().any(|b| b.is_open())
    }

    /// Whether a peer session confirmed in this run was heard over Nostr recently.
    pub(super) fn nostr_ready(&self) -> bool {
        self.sess.peer_session().is_some()
            && self
                .nostr_peer_heard_at
                .is_some_and(|t| self.now.saturating_sub(t) <= PEER_FRESH_MS)
            && self.nostr_any_open()
    }

    // ---- keys of the day -------------------------------------------------------------------

    fn day(&mut self, epoch: u32) -> Option<Day> {
        if let Some(d) = self.nostr_days.iter().find(|d| d.epoch == epoch) {
            return Some(d.clone());
        }
        let keys = self.nostr_keys.as_ref()?;
        let day = Day {
            epoch,
            tag: keys.tag(epoch),
            own: keys.signing_key(self.cfg.role, epoch),
            peer_pub: keys.public_key_hex(self.other_role(), epoch),
        };
        if self.nostr_days.len() >= 4 {
            self.nostr_days.remove(0);
        }
        self.nostr_days.push(day.clone());
        Some(day)
    }

    /// A signed event frame for a sealed envelope, dated now.
    fn nostr_event_frame(&mut self, wall: u64, sealed: &[u8]) -> Option<String> {
        let day = self.day(epoch_of(wall))?;
        let key = day.own?;
        let content = URL_SAFE_NO_PAD.encode(sealed);
        let mut aux = [0u8; 32];
        self.csprng.as_mut()?.fill_bytes(&mut aux);
        let event = build_event(&key, &aux, wall, &day.tag, &content);
        aux.zeroize();
        event.map(|e| event_frame(&e))
    }

    // ---- clock hint ------------------------------------------------------------------------

    /// `clock_skew` is up while the peer's clock differs, our own clock is unknown although Nostr
    /// relays are configured, or a relay recently refused an event for its time.
    pub(super) fn refresh_clock_hint(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        let unknown = !self.mb.nostr.is_empty()
            && self.wall_unix_s.is_none()
            && now.saturating_sub(self.started_at) >= CLOCK_GRACE_MS;
        let refused = self
            .clock_refused_at
            .is_some_and(|t| now.saturating_sub(t) <= CLOCK_REFUSAL_MS);
        self.set_hint(HINT_CLOCK_SKEW, self.peer_skew || unknown || refused, out);
    }

    // ---- housekeeping ----------------------------------------------------------------------

    pub(super) fn nostr_housekeeping(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        if self.mb.nostr.is_empty() || self.mode != Mode::Running {
            return;
        }
        self.refresh_clock_hint(now, out);
        if self.live_verified() {
            return;
        }
        let wall = self.wall_now(now);
        for i in 0..self.mb.nostr.len() {
            let Some(b) = self.mb.nostr.get(i) else {
                break;
            };
            match b.state {
                MailboxState::Closed => {
                    if wall.is_some() && now >= b.next_open_at && now >= b.parked_until {
                        self.nostr_open(i, now, out);
                    }
                }
                MailboxState::Opening => {
                    if now.saturating_sub(b.changed_at) >= OPEN_TIMEOUT_MS {
                        self.nostr_down(i, now, true, out);
                    }
                }
                MailboxState::Open | MailboxState::Joined => {
                    if b.probe_sub.is_none() {
                        self.nostr_open_housekeeping(i, now, wall, out);
                    }
                }
            }
        }
        self.nostr_here_housekeeping(now, out);
        self.nostr_drain(now, false, out);
    }

    fn nostr_open(&mut self, i: usize, now: u64, out: &mut Vec<LinkAction>) {
        let Some(b) = self.mb.nostr.get_mut(i) else {
            return;
        };
        b.state = MailboxState::Opening;
        b.changed_at = now;
        out.push(LinkAction::OpenMailbox {
            id: b.id,
            kind: MailboxKind::Nostr,
            url: b.url.clone(),
        });
    }

    /// The socket is gone or unusable: close it (unless the platform did) and retry with backoff.
    pub(super) fn nostr_down(
        &mut self,
        i: usize,
        now: u64,
        emit_close: bool,
        out: &mut Vec<LinkAction>,
    ) {
        let Some(b) = self.mb.nostr.get_mut(i) else {
            return;
        };
        let id = b.id;
        if emit_close && b.state != MailboxState::Closed {
            out.push(LinkAction::CloseMailbox { id });
        }
        b.reset_connection();
        let step = (b.failures as usize).min(RECONNECT_MS.len() - 1);
        b.next_open_at = now.saturating_add(RECONNECT_MS.get(step).copied().unwrap_or(300_000));
        b.failures = b.failures.saturating_add(1);
        self.probe_forget(id, now, out);
    }

    /// Leaves the relay out for 1, 5 or 30 minutes.
    pub(super) fn nostr_park(&mut self, i: usize, now: u64, out: &mut Vec<LinkAction>) {
        let Some(b) = self.mb.nostr.get_mut(i) else {
            return;
        };
        let id = b.id;
        if b.last_park_at
            .is_some_and(|t| now.saturating_sub(t) > PARK_FORGET_MS)
        {
            b.park_level = 0;
        }
        let step = (b.park_level as usize).min(PARK_MS.len() - 1);
        b.parked_until = now.saturating_add(PARK_MS.get(step).copied().unwrap_or(1_800_000));
        b.park_level = b.park_level.saturating_add(1);
        b.last_park_at = Some(now);
        if b.state != MailboxState::Closed {
            out.push(LinkAction::CloseMailbox { id });
        }
        b.reset_connection();
        self.probe_forget(id, now, out);
    }

    /// A mailbox that was part of the reachability check is gone (closed, parked, dropped).
    fn probe_forget(&mut self, id: MailboxId, now: u64, out: &mut Vec<LinkAction>) {
        let empty = match self.probe.as_mut() {
            Some(p) => {
                p.pending.retain(|x| *x != id);
                p.pending.is_empty()
            }
            None => false,
        };
        if empty && self.probing() {
            let ok = self.probe.as_ref().is_some_and(|p| p.ok);
            self.finish_probe(now, ok, out);
        }
    }

    // ---- socket events ---------------------------------------------------------------------

    pub(super) fn on_nostr_opened(&mut self, id: MailboxId, now: u64, out: &mut Vec<LinkAction>) {
        let Some(i) = self.mb.nostr.iter().position(|b| b.id == id) else {
            return;
        };
        let wall = self.wall_now(now);
        let sub_id = match self.csprng.as_mut() {
            Some(rng) => random_text(rng, 12),
            None => return,
        };
        let probing = {
            let Some(b) = self.mb.nostr.get_mut(i) else {
                return;
            };
            if b.state != MailboxState::Opening {
                return;
            }
            b.state = MailboxState::Open;
            b.changed_at = now;
            b.sub_id = sub_id.clone();
            b.last_send_at = None;
            b.probe_sub.is_some()
        };
        if probing {
            // A reachability check: a throwaway filter and nothing of the pairing.
            let tag = match self.csprng.as_mut() {
                Some(rng) => random_text(rng, 17),
                None => return,
            };
            if let Some(frame) = probe_req_frame(&sub_id, &tag)
                && let Some(b) = self.mb.nostr.get_mut(i)
            {
                b.probe_sub = Some(sub_id);
                b.eose_deadline = Some(now.saturating_add(EOSE_WAIT_MS));
                out.push(LinkAction::MailboxSend { id, frame });
            }
            return;
        }
        if let Some(w) = wall {
            let epochs = epochs_to_listen(w);
            self.nostr_subscribe(i, now, w, epochs, out);
        }
        // The relay is new to the peer: say who we are on it.
        self.nostr_announce(now, Some(id));
    }

    pub(super) fn on_nostr_closed(&mut self, id: MailboxId, now: u64, out: &mut Vec<LinkAction>) {
        let Some(i) = self.mb.nostr.iter().position(|b| b.id == id) else {
            return;
        };
        if self
            .mb
            .nostr
            .get(i)
            .is_some_and(|b| b.state != MailboxState::Closed)
        {
            self.nostr_down(i, now, false, out);
        }
    }

    fn nostr_subscribe(
        &mut self,
        i: usize,
        now: u64,
        wall: u64,
        epochs: Vec<u32>,
        out: &mut Vec<LinkAction>,
    ) {
        let mut filters = Vec::with_capacity(epochs.len());
        for epoch in &epochs {
            if let Some(day) = self.day(*epoch)
                && let Some(author) = day.peer_pub
            {
                filters.push(Filter {
                    author,
                    tag: day.tag,
                    since: wall.saturating_sub(SINCE_S),
                });
            }
        }
        let Some(b) = self.mb.nostr.get_mut(i) else {
            return;
        };
        b.sub_epochs = epochs;
        b.sub_sent_at = now;
        if let Some(frame) = req_frame(&b.sub_id, &filters) {
            let deadline = now.saturating_add(EOSE_WAIT_MS);
            b.eose_deadline = Some(b.eose_deadline.map_or(deadline, |d| d.min(deadline)));
            out.push(LinkAction::MailboxSend { id: b.id, frame });
        }
    }

    fn nostr_open_housekeeping(
        &mut self,
        i: usize,
        now: u64,
        wall: Option<u64>,
        out: &mut Vec<LinkAction>,
    ) {
        let Some(b) = self.mb.nostr.get(i) else {
            return;
        };
        if b.eose_deadline.is_some_and(|d| now >= d) {
            // The relay never answered the subscription: treat the socket as closed.
            self.nostr_down(i, now, true, out);
            return;
        }
        let Some(w) = wall else {
            return;
        };
        let epochs = epochs_to_listen(w);
        if b.sub_epochs != epochs || now.saturating_sub(b.sub_sent_at) >= RESUBSCRIBE_MS {
            self.nostr_subscribe(i, now, w, epochs, out);
        }
    }

    /// One inbound text frame of a Nostr mailbox.
    pub(super) fn on_nostr_frame(
        &mut self,
        id: MailboxId,
        now: u64,
        text: &str,
        out: &mut Vec<LinkAction>,
    ) {
        let Some(i) = self.mb.nostr.iter().position(|b| b.id == id) else {
            return;
        };
        if self
            .mb
            .nostr
            .get(i)
            .is_none_or(|b| b.state != MailboxState::Open)
            || text.len() > MAX_FRAME_BYTES
        {
            return;
        }
        let Some(frame) = parse_relay_frame(text) else {
            return;
        };
        match frame {
            RelayFrame::Event { sub, event } => self.on_nostr_event(i, now, &sub, event, out),
            RelayFrame::Eose { sub } => self.on_nostr_eose(i, now, &sub, out),
            RelayFrame::Ok {
                accepted, message, ..
            } => self.on_nostr_verdict(i, now, classify_ok(accepted, &message), out),
            RelayFrame::Notice { message } => {
                self.on_nostr_verdict(i, now, classify_notice(&message), out)
            }
            RelayFrame::Closed { sub, message } => {
                let ours = self.mb.nostr.get(i).is_some_and(|b| {
                    b.sub_id == sub || b.probe_sub.as_deref() == Some(sub.as_str())
                });
                if !ours {
                    return;
                }
                if classify_closed(&message) == Verdict::Park {
                    self.nostr_park(i, now, out);
                } else {
                    self.nostr_down(i, now, true, out);
                }
            }
            RelayFrame::Auth => self.nostr_park(i, now, out),
        }
    }

    fn on_nostr_verdict(
        &mut self,
        i: usize,
        now: u64,
        verdict: Verdict,
        out: &mut Vec<LinkAction>,
    ) {
        match verdict {
            Verdict::Fine => {}
            Verdict::Clock => self.clock_refused_at = Some(now),
            Verdict::Park => self.nostr_park(i, now, out),
        }
    }

    fn on_nostr_eose(&mut self, i: usize, now: u64, sub: &str, out: &mut Vec<LinkAction>) {
        let Some(b) = self.mb.nostr.get_mut(i) else {
            return;
        };
        if b.probe_sub.as_deref() == Some(sub) {
            // Reachable: it answered. Close the subscription and the socket (the check is over).
            b.eose_deadline = None;
            if let Some(p) = self.probe.as_mut() {
                p.ok = true;
            }
            self.finish_probe(now, true, out);
        } else if b.sub_id == sub {
            b.eose_deadline = None;
            b.failures = 0;
        }
    }

    fn on_nostr_event(
        &mut self,
        i: usize,
        now: u64,
        sub: &str,
        event: Event,
        out: &mut Vec<LinkAction>,
    ) {
        let Some(b) = self.mb.nostr.get_mut(i) else {
            return;
        };
        if sub != b.sub_id {
            return;
        }
        if now.saturating_sub(b.rx_start) >= RX_WINDOW_MS {
            b.rx_start = now;
            b.rx_count = 0;
        }
        b.rx_count = b.rx_count.saturating_add(1);
        if b.rx_count > RX_CAP {
            self.nostr_park(i, now, out);
            return;
        }
        let epochs = b.sub_epochs.clone();
        let mut in_scope = event.kind == KIND;
        if in_scope {
            in_scope = epochs.iter().any(|e| {
                self.day(*e).is_some_and(|d| {
                    d.peer_pub.as_deref() == Some(event.pubkey.as_str())
                        && event.tag_value() == Some(d.tag.as_str())
                })
            });
        }
        if !in_scope || !verify_event(&event) {
            let Some(b) = self.mb.nostr.get_mut(i) else {
                return;
            };
            if now.saturating_sub(b.bad_start) >= BAD_WINDOW_MS {
                b.bad_start = now;
                b.bad_count = 0;
            }
            b.bad_count = b.bad_count.saturating_add(1);
            if b.bad_count >= BAD_CAP {
                self.nostr_park(i, now, out);
            }
            return;
        }
        let Ok(bytes) = URL_SAFE_NO_PAD.decode(&event.content) else {
            return;
        };
        self.open_envelope(now, NOSTR_ORIGIN, &bytes, out);
    }

    // ---- publishing ------------------------------------------------------------------------

    /// Queues a sealed envelope on every open Nostr mailbox (`target`: only that one).
    pub(super) fn nostr_enqueue(
        &mut self,
        target: Option<MailboxId>,
        sealed: &[u8],
        presence: bool,
    ) {
        for b in self.mb.nostr.iter_mut().filter(|b| b.is_open()) {
            if target.is_some_and(|t| t != b.id) {
                continue;
            }
            if b.outbox.len() >= OUTBOX_CAP {
                // Presence is cheap to repeat; drop the oldest setup envelope first.
                let drop_at = b.outbox.iter().position(|o| !o.presence).unwrap_or(0);
                b.outbox.remove(drop_at);
            }
            b.outbox.push_back(Outgoing {
                sealed: sealed.to_vec(),
                presence,
            });
        }
    }

    /// Fans a sealed envelope out to the Nostr mailboxes according to its class.
    pub(super) fn nostr_publish(&mut self, sealed: &Sealed, class: NostrClass) {
        match class {
            NostrClass::Skip => {}
            NostrClass::Presence => self.nostr_enqueue(None, &sealed.bytes, true),
            NostrClass::Setup => {
                if self.nostr_attempt_events < ATTEMPT_CAP {
                    self.nostr_attempt_events += 1;
                    self.nostr_enqueue(None, &sealed.bytes, false);
                }
            }
        }
    }

    /// Sends what the pacing and the hourly budget allow. `all`: ignore the pacing (a stop that
    /// must leave before the socket closes).
    pub(super) fn nostr_drain(&mut self, now: u64, all: bool, out: &mut Vec<LinkAction>) {
        let Some(wall) = self.wall_now(now) else {
            return;
        };
        for i in 0..self.mb.nostr.len() {
            while let Some(b) = self.mb.nostr.get_mut(i) {
                if !b.is_open() || b.outbox.is_empty() {
                    break;
                }
                if !all
                    && b.last_send_at
                        .is_some_and(|t| now < t.saturating_add(PACE_MS))
                {
                    break;
                }
                if now.saturating_sub(b.hour_start) >= HOUR_MS {
                    b.hour_start = now;
                    b.hour_sent = 0;
                }
                let Some(item) = b.outbox.pop_front() else {
                    break;
                };
                if !item.presence && b.hour_sent >= HOUR_CAP {
                    continue;
                }
                let id = b.id;
                if let Some(frame) = self.nostr_event_frame(wall, &item.sealed)
                    && let Some(b) = self.mb.nostr.get_mut(i)
                {
                    b.hour_sent = b.hour_sent.saturating_add(1);
                    b.last_send_at = Some(now);
                    out.push(LinkAction::MailboxSend { id, frame });
                }
            }
        }
    }

    // ---- presence --------------------------------------------------------------------------

    /// Seals a `here` and queues it, together with a repeat of the challenge we still wait on. With
    /// `target` only that relay gets it (it just opened); otherwise this is an unsolicited repeat
    /// and the next one is scheduled.
    pub(super) fn nostr_announce(&mut self, now: u64, target: Option<MailboxId>) {
        let here = self.here_message(now);
        let sealed = match self.csprng.as_mut() {
            Some(rng) => self.sess.seal(
                &self.env_key,
                rng,
                MessageClass::Here,
                SessionId::NONE,
                &here,
            ),
            None => return,
        };
        if let Ok(sealed) = sealed {
            self.nostr_enqueue(target, &sealed.bytes, true);
        }
        let pending = match self.pending_reply {
            Some((reply, at)) if now.saturating_sub(at) <= CHALLENGE_VALID_MS => Some(reply),
            _ => None,
        };
        if let Some(reply) = pending {
            self.nostr_reply(now, target, &reply);
        }
        if target.is_some() && self.nostr_here_sent == 0 {
            // The first relay's opening announcement is the first presence of this stretch.
            self.nostr_here_sent = 1;
            self.nostr_here_next_at = now.saturating_add(HERE_BACKOFF_MS[0]);
        }
        if target.is_none() {
            self.nostr_here_sent = self.nostr_here_sent.saturating_add(1);
            let wait = HERE_BACKOFF_MS
                .get(self.nostr_here_sent.saturating_sub(1) as usize)
                .copied()
                .unwrap_or(HERE_LONG_MS);
            self.nostr_here_next_at = now.saturating_add(wait);
        }
    }

    /// The `here` that answers a candidate or echoes a challenge, over Nostr.
    pub(super) fn nostr_reply(&mut self, now: u64, target: Option<MailboxId>, reply: &Reply) {
        let here = self.here_message(now);
        let Some(rng) = self.csprng.as_mut() else {
            return;
        };
        if let Ok(sealed) = self.sess.seal_reply(&self.env_key, rng, reply, &here) {
            self.nostr_enqueue(target, &sealed.bytes, true);
        }
        if reply.challenge != [0; CHALLENGE_LEN] {
            let issued = match self.pending_reply {
                Some((old, at)) if old.challenge == reply.challenge => at,
                _ => now,
            };
            self.pending_reply = Some((*reply, issued));
        }
    }

    /// While not connected the `here` repeats (5, 15, 60 s, then every 5 minutes), so a peer that
    /// opens its relays later finds us and so the peer's freshness check has something to see.
    fn nostr_here_housekeeping(&mut self, now: u64, out: &mut Vec<LinkAction>) {
        let _ = out;
        if self.nostr_any_open() && now >= self.nostr_here_next_at {
            self.nostr_announce(now, None);
        }
    }

    // ---- the reachability check ------------------------------------------------------------

    /// Opens every Nostr relay that is not parked for the check; returns their ids.
    pub(super) fn nostr_probe_open(
        &mut self,
        now: u64,
        out: &mut Vec<LinkAction>,
    ) -> Vec<MailboxId> {
        let mut ids = Vec::new();
        for i in 0..self.mb.nostr.len() {
            let sub = match self.csprng.as_mut() {
                Some(rng) => random_text(rng, 12),
                None => break,
            };
            let Some(b) = self.mb.nostr.get(i) else {
                break;
            };
            if b.state != MailboxState::Closed || now < b.parked_until {
                continue;
            }
            if let Some(b) = self.mb.nostr.get_mut(i) {
                b.probe_sub = Some(sub);
            }
            self.nostr_open(i, now, out);
            if let Some(b) = self.mb.nostr.get(i) {
                ids.push(b.id);
            }
        }
        ids
    }

    /// Closes the Nostr sockets of a finished check.
    pub(super) fn nostr_probe_close(&mut self, out: &mut Vec<LinkAction>) {
        for b in &mut self.mb.nostr {
            if b.probe_sub.is_some() {
                if b.state != MailboxState::Closed {
                    if b.state == MailboxState::Open
                        && let Some(frame) = b.probe_sub.as_deref().and_then(close_frame)
                    {
                        out.push(LinkAction::MailboxSend { id: b.id, frame });
                    }
                    out.push(LinkAction::CloseMailbox { id: b.id });
                }
                b.reset_connection();
            }
        }
    }

    /// Closes every Nostr socket (the attempt became a link, or the run ends).
    pub(super) fn nostr_close_all(
        &mut self,
        now: u64,
        reset_backoff: bool,
        out: &mut Vec<LinkAction>,
    ) {
        for b in &mut self.mb.nostr {
            if b.state != MailboxState::Closed {
                out.push(LinkAction::CloseMailbox { id: b.id });
            }
            b.reset_connection();
            if reset_backoff {
                b.failures = 0;
                b.next_open_at = now;
            }
        }
        self.nostr_peer_heard_at = None;
    }
}
