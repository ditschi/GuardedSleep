//! `gs-soak soak`: two real `Link`s (baby and parent) in one process, talking over real public
//! Nostr relays for hours. The peer connection is faked (it carries only what the core sends on
//! the DataChannel once both descriptions and a remote candidate arrived), everything else is the
//! real path: presence, the challenge, `request_offer`, offer, answer, candidates, the sealed
//! envelopes, the per-relay pacing and parking of the core. Every M minutes the link is cut to
//! measure how long a recovery takes.
//!
//! The harness plays the platform: it owns the sockets and the clock, forwards frames in both
//! directions and never interprets a sealed envelope. It logs counters only, never frames, keys or
//! the pairing key.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

use guardedsleep_core::invite::InviteMailbox;
use guardedsleep_core::link::{
    Link, LinkAction, LinkConfig, LinkEvent, LinkInput, LinkState, SEED_LEN,
};
use guardedsleep_core::mailbox::{MailboxId, MailboxKind, RELAY_MAILBOX};
use guardedsleep_core::nostr::{
    RelayFrame, Verdict, classify_closed, classify_notice, classify_ok, parse_relay_frame,
};
use guardedsleep_core::pairing::{encode_psk, extract_sha256_fingerprint};
use guardedsleep_core::protocol::{IceCandidate, Role};
use serde_json::{Value, json};

use crate::report::{self, SoakReport};
use crate::sign::{now_unix_s, random_bytes};
use crate::stats::{RelayStats, percentile, verdict};
use crate::ws::{self, Ev, Msg, Out};

const TICK_MS: u64 = 250;
const WALL_CLOCK_EVERY_MS: u64 = 60_000;
/// How long a published event may take to reach the peer before it counts as lost.
const DELIVERY_WINDOW_MS: u64 = 20_000;
/// A recovery that has run this long counts as failed (with this value) in the percentiles.
const RECOVERY_LIMIT_MS: u64 = 120_000;
const SNAPSHOT_EVERY_MS: u64 = 300_000;
const PROGRESS_EVERY_MS: u64 = 60_000;
/// Local candidates a fake peer connection reports after its description.
const LOCAL_CANDIDATES: u32 = 6;
const PAIRING_WINDOW_S: u64 = 2 * 3_600;

pub struct SoakOptions {
    pub relays: Vec<String>,
    pub duration: Duration,
    /// Time between a verified link and the next forced loss; zero turns forced losses off.
    pub loss_every: Duration,
    /// Subscribe by tag only (the `authors` member of every `REQ` is dropped).
    pub tag_only: bool,
    /// The parent's wall clock is this far off (seconds), to see the clock refusals of a relay.
    pub parent_wall_offset_s: i64,
    /// Time the fake ICE needs after the descriptions and a remote candidate arrived.
    pub ice_ms: u64,
    pub out: Option<PathBuf>,
}

enum Timer {
    /// A mailbox the harness cannot or will not open (the own relay, LAN).
    MailboxClosed { actor: usize, id: MailboxId },
    LocalDesc { actor: usize, attempt: u32, kind: &'static str },
    LocalIce { actor: usize, attempt: u32, n: u32 },
    Connect { a: usize, ha: u32, b: usize, hb: u32 },
    Data { to: usize, handle: u32, text: String },
    DataClosed { to: usize, handle: u32 },
}

#[derive(Default)]
struct Pc {
    fp: String,
    local: bool,
    remote: bool,
    remote_candidates: u32,
    peer: Option<(usize, u32)>,
    dtls_fp: Option<String>,
    closed: bool,
    failed: bool,
    connecting: bool,
    connected: bool,
}

impl Pc {
    fn alive(&self) -> bool {
        !self.closed && !self.failed
    }
}

struct Actor {
    role: Role,
    link: Link,
    pcs: BTreeMap<u32, Pc>,
    state: Option<LinkState>,
    connected_at: Option<u64>,
    wall_offset_s: i64,
}

struct Sock {
    url: String,
    gen_no: u64,
    tx: Sender<Out>,
    spawned_ms: u64,
    opened: bool,
    /// The relay answered our `REQ` with `EOSE`: events that reach the relay now are delivered.
    ready: bool,
}

struct Pending {
    from: usize,
    sent_ms: u64,
    eligible: bool,
    peer_gen: u64,
    ok: Option<bool>,
    delivered: bool,
}

pub struct Soak {
    opts: SoakOptions,
    start: Instant,
    started_unix_s: u64,
    events: Sender<Msg>,
    inbox: Receiver<Msg>,
    actors: Vec<Actor>,
    socks: HashMap<(usize, MailboxId), Sock>,
    next_gen: u64,
    relays: BTreeMap<String, RelayStats>,
    pending: HashMap<(String, String), Pending>,
    timers: BTreeMap<(u64, u64), Timer>,
    seq: u64,
    initial_ms: Option<u64>,
    cut_at: Option<u64>,
    cut_counted_failed: bool,
    next_cut_at: Option<u64>,
    cuts: u64,
    recoveries: Vec<u64>,
    recovery_failures: u64,
    failures: BTreeMap<String, u64>,
    hints: BTreeMap<String, u64>,
    security_warnings: u64,
    link_lost_events: u64,
}

fn mailbox_set(relays: &[String]) -> Vec<InviteMailbox> {
    relays
        .iter()
        .map(|url| InviteMailbox {
            kind: MailboxKind::Nostr,
            url: url.clone(),
        })
        .collect()
}

impl Soak {
    pub fn new(opts: SoakOptions) -> Result<Self, String> {
        // A throwaway pairing key for this run; it never leaves the process and is never printed.
        let psk = random_bytes::<32>()?;
        let (events, inbox) = channel();
        let unix = now_unix_s();
        let mut actors = Vec::new();
        for (role, offset) in [(Role::Baby, 0), (Role::Parent, opts.parent_wall_offset_s)] {
            let seed = random_bytes::<SEED_LEN>()?;
            let mut cfg = LinkConfig::with_defaults(role, encode_psk(&psk), &seed);
            cfg.mailboxes = mailbox_set(&opts.relays);
            cfg.pin_window_until_s = Some(unix + PAIRING_WINDOW_S);
            if role == Role::Parent {
                cfg.device_name = "soak-parent".to_owned();
            }
            let link = Link::new(cfg).map_err(|e| format!("link config: {e:?}"))?;
            actors.push(Actor {
                role,
                link,
                pcs: BTreeMap::new(),
                state: None,
                connected_at: None,
                wall_offset_s: offset,
            });
        }
        let relays = opts
            .relays
            .iter()
            .map(|u| (u.clone(), RelayStats::new(u)))
            .collect();
        Ok(Self {
            opts,
            start: Instant::now(),
            started_unix_s: unix,
            events,
            inbox,
            actors,
            socks: HashMap::new(),
            next_gen: 1,
            relays,
            pending: HashMap::new(),
            timers: BTreeMap::new(),
            seq: 0,
            initial_ms: None,
            cut_at: None,
            cut_counted_failed: false,
            next_cut_at: None,
            cuts: 0,
            recoveries: Vec::new(),
            recovery_failures: 0,
            failures: BTreeMap::new(),
            hints: BTreeMap::new(),
            security_warnings: 0,
            link_lost_events: 0,
        })
    }

    fn now(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    fn after(&mut self, delay_ms: u64, timer: Timer) {
        self.seq += 1;
        let at = self.now() + delay_ms;
        self.timers.insert((at, self.seq), timer);
    }

    /// Runs until the planned duration is over; returns the final report.
    pub fn run(&mut self) -> Result<SoakReport, String> {
        let end_ms = self.opts.duration.as_millis() as u64;
        for a in 0..self.actors.len() {
            let entropy = u64::from(u32::from_le_bytes(random_bytes::<4>()?));
            self.deliver(a, LinkInput::Start { entropy });
            self.report_wall_clock(a);
        }
        let mut next_tick = TICK_MS;
        let mut next_wall = WALL_CLOCK_EVERY_MS;
        let mut next_settle = 1_000;
        let mut next_snapshot = SNAPSHOT_EVERY_MS;
        let mut next_progress = PROGRESS_EVERY_MS;
        loop {
            let now = self.now();
            if now >= end_ms {
                break;
            }
            let mut wake = next_tick.min(next_settle).min(end_ms);
            if let Some((at, _)) = self.timers.keys().next() {
                wake = wake.min(*at);
            }
            let wait = Duration::from_millis(wake.saturating_sub(now).max(1));
            match self.inbox.recv_timeout(wait) {
                Ok(msg) => self.on_ws(msg),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Err("socket channel closed".into()),
            }
            self.run_timers();
            let now = self.now();
            if now >= next_tick {
                next_tick = now + TICK_MS;
                for a in 0..self.actors.len() {
                    self.deliver(a, LinkInput::Tick);
                }
                // The baby's microphone level, so that the core does not report a silent mic.
                self.deliver(0, LinkInput::AudioLevel { rms: 0.02 });
            }
            if now >= next_wall {
                next_wall = now + WALL_CLOCK_EVERY_MS;
                for a in 0..self.actors.len() {
                    self.report_wall_clock(a);
                }
            }
            if now >= next_settle {
                next_settle = now + 1_000;
                self.settle(false);
                self.housekeeping(now);
            }
            if now >= next_snapshot {
                next_snapshot = now + SNAPSHOT_EVERY_MS;
                self.snapshot(false);
            }
            if now >= next_progress {
                next_progress = now + PROGRESS_EVERY_MS;
                self.progress();
            }
        }
        // Stop politely, let the last events settle, close every socket.
        for a in 0..self.actors.len() {
            self.deliver(a, LinkInput::Stop);
        }
        std::thread::sleep(Duration::from_millis(500));
        while let Ok(msg) = self.inbox.try_recv() {
            self.on_ws(msg);
        }
        self.settle(true);
        for sock in self.socks.values() {
            let _ = sock.tx.send(Out::Close);
        }
        self.socks.clear();
        let report = self.build_report(true);
        if let Some(path) = &self.opts.out {
            report::write_json(path, &report.json)?;
        }
        Ok(report)
    }

    fn report_wall_clock(&mut self, a: usize) {
        let unix = now_unix_s().saturating_add_signed(self.actors[a].wall_offset_s);
        self.deliver(a, LinkInput::WallClock { unix_s: unix });
    }

    fn deliver(&mut self, a: usize, input: LinkInput) {
        let now = self.now();
        let actions = self.actors[a].link.handle(now, input);
        self.exec(a, actions);
    }

    // ---- actions of the core -------------------------------------------------------------

    fn exec(&mut self, a: usize, actions: Vec<LinkAction>) {
        for action in actions {
            match action {
                LinkAction::OpenMailbox { id, kind, url } => match kind {
                    MailboxKind::Nostr => self.open_socket(a, id, &url),
                    // Our own relay and the LAN are not part of this test: a refused open, like a
                    // server that is down.
                    MailboxKind::Relay | MailboxKind::Lan => {
                        self.after(100, Timer::MailboxClosed { actor: a, id });
                    }
                },
                LinkAction::CloseMailbox { id } => self.close_socket(a, id),
                LinkAction::MailboxSend { id, frame } => self.send_frame(a, id, frame),
                LinkAction::CreateAttempt { attempt, .. } => {
                    self.actors[a].pcs.insert(
                        attempt,
                        Pc {
                            fp: hex(&random_bytes::<32>().unwrap_or([7; 32])),
                            ..Pc::default()
                        },
                    );
                }
                LinkAction::CloseAttempt { attempt } => self.close_pc(a, attempt),
                LinkAction::CreateOffer { attempt } => self.after(
                    15,
                    Timer::LocalDesc {
                        actor: a,
                        attempt,
                        kind: "offer",
                    },
                ),
                LinkAction::CreateAnswer { attempt } => self.after(
                    15,
                    Timer::LocalDesc {
                        actor: a,
                        attempt,
                        kind: "answer",
                    },
                ),
                LinkAction::SetRemoteDescription { attempt, sdp, .. } => {
                    self.set_remote(a, attempt, &sdp);
                }
                LinkAction::AddIceCandidate { attempt, .. } => {
                    if let Some(pc) = self.actors[a].pcs.get_mut(&attempt) {
                        pc.remote_candidates += 1;
                    }
                    self.maybe_connect(a, attempt);
                }
                LinkAction::SendData { attempt, text } => self.send_data(a, attempt, text),
                LinkAction::Emit { event } => self.on_event(a, event),
                LinkAction::SetMicEnabled { .. }
                | LinkAction::ApplyQuiet
                | LinkAction::SendProbeTone { .. } => {}
            }
        }
    }

    fn on_event(&mut self, a: usize, event: LinkEvent) {
        match event {
            LinkEvent::Status { status } => {
                self.actors[a].state = Some(status.state);
                if status.state == LinkState::Connected {
                    self.actors[a].connected_at = Some(self.now());
                }
                self.check_recovery();
            }
            LinkEvent::Failed { reason } => *self.failures.entry(reason).or_default() += 1,
            LinkEvent::Hint { key, active: true } => *self.hints.entry(key).or_default() += 1,
            LinkEvent::SecurityWarning { .. } => self.security_warnings += 1,
            LinkEvent::LinkLost { .. } => self.link_lost_events += 1,
            // ParentPinned carries a secret: never looked at, never logged.
            _ => {}
        }
    }

    // ---- sockets ---------------------------------------------------------------------------

    fn open_socket(&mut self, a: usize, id: MailboxId, url: &str) {
        if id == RELAY_MAILBOX {
            self.after(100, Timer::MailboxClosed { actor: a, id });
            return;
        }
        self.close_socket_quiet(a, id);
        let gen_no = self.next_gen;
        self.next_gen += 1;
        let tx = ws::spawn(url.to_owned(), a, id, gen_no, self.events.clone());
        let spawned_ms = self.now();
        self.socks.insert(
            (a, id),
            Sock {
                url: url.to_owned(),
                gen_no,
                tx,
                spawned_ms,
                opened: false,
                ready: false,
            },
        );
        self.stats(url).opens += 1;
    }

    fn close_socket_quiet(&mut self, a: usize, id: MailboxId) {
        if let Some(sock) = self.socks.remove(&(a, id)) {
            let _ = sock.tx.send(Out::Close);
        }
    }

    fn close_socket(&mut self, a: usize, id: MailboxId) {
        if let Some(sock) = self.socks.remove(&(a, id)) {
            let _ = sock.tx.send(Out::Close);
            self.stats(&sock.url).core_closes += 1;
        }
    }

    fn stats(&mut self, url: &str) -> &mut RelayStats {
        self.relays
            .entry(url.to_owned())
            .or_insert_with(|| RelayStats::new(url))
    }

    fn send_frame(&mut self, a: usize, id: MailboxId, frame: String) {
        if id == RELAY_MAILBOX {
            return;
        }
        let now = self.now();
        let Some(sock) = self.socks.get(&(a, id)) else {
            return;
        };
        let url = sock.url.clone();
        let mut frame = frame;
        if frame.starts_with("[\"EVENT\"") {
            if let Some(event_id) = event_id_of(&frame) {
                // Deliverable only if the peer listens on this relay right now.
                let peer_gen = self
                    .socks
                    .iter()
                    .find(|((who, _), s)| *who == 1 - a && s.url == url && s.ready)
                    .map(|(_, s)| s.gen_no);
                self.pending.insert(
                    (event_id, url.clone()),
                    Pending {
                        from: a,
                        sent_ms: now,
                        eligible: peer_gen.is_some(),
                        peer_gen: peer_gen.unwrap_or(0),
                        ok: None,
                        delivered: false,
                    },
                );
                let st = self.stats(&url);
                st.events_sent += 1;
                if peer_gen.is_some() {
                    st.eligible += 1;
                }
            }
        } else if self.opts.tag_only && frame.starts_with("[\"REQ\"") {
            frame = drop_authors(&frame).unwrap_or(frame);
        }
        if let Some(sock) = self.socks.get(&(a, id)) {
            let _ = sock.tx.send(Out::Text(frame));
        }
    }

    fn on_ws(&mut self, msg: Msg) {
        let key = (msg.actor, msg.id);
        let now = self.now();
        let (url, spawned_ms, opened) = match self.socks.get(&key) {
            Some(s) if s.gen_no == msg.gen_no => (s.url.clone(), s.spawned_ms, s.opened),
            _ => return,
        };
        match msg.ev {
            Ev::Opened => {
                if let Some(s) = self.socks.get_mut(&key) {
                    s.opened = true;
                }
                self.stats(&url).connect_ms.push(now.saturating_sub(spawned_ms));
                self.deliver(msg.actor, LinkInput::MailboxOpened { id: msg.id });
            }
            Ev::Text(text) => {
                self.on_frame(msg.actor, msg.id, &url, &text, now);
                self.deliver(
                    msg.actor,
                    LinkInput::MailboxFrame {
                        id: msg.id,
                        bytes: text,
                    },
                );
            }
            Ev::Closed(reason) => {
                self.socks.remove(&key);
                let class = reason.split(':').next().unwrap_or("closed").to_owned();
                if opened {
                    self.stats(&url).unexpected_closes += 1;
                } else {
                    self.stats(&url).open_failure(&class);
                }
                self.deliver(msg.actor, LinkInput::MailboxClosed { id: msg.id, reason });
            }
        }
    }

    /// Counts what a relay said; the frame itself goes on to the core untouched.
    fn on_frame(&mut self, actor: usize, id: MailboxId, url: &str, text: &str, now: u64) {
        let Some(frame) = parse_relay_frame(text) else {
            self.stats(url).unparsed_frames += 1;
            return;
        };
        match frame {
            RelayFrame::Event { event, .. } => {
                let mut latency = None;
                if let Some(p) = self.pending.get_mut(&(event.id.clone(), url.to_owned())) {
                    if p.from == actor {
                        self.stats(url).echoes += 1;
                    } else if !p.delivered {
                        p.delivered = true;
                        latency = Some(now.saturating_sub(p.sent_ms));
                    }
                }
                let st = self.stats(url);
                st.received_events += 1;
                if let Some(ms) = latency {
                    st.delivered += 1;
                    st.latency_ms.push(ms);
                }
            }
            RelayFrame::Ok {
                id: event_id,
                accepted,
                message,
            } => {
                if let Some(p) = self.pending.get_mut(&(event_id, url.to_owned())) {
                    p.ok = Some(accepted);
                }
                let st = self.stats(url);
                if accepted {
                    st.ok_true += 1;
                } else {
                    st.refused += 1;
                    st.refusal_sample(&message);
                    match classify_ok(false, &message) {
                        Verdict::Clock => st.refused_clock += 1,
                        Verdict::Park => st.refused_park += 1,
                        Verdict::Down => st.refused_down += 1,
                        Verdict::Fine => st.refused_other += 1,
                    }
                }
            }
            RelayFrame::Eose { .. } => {
                if let Some(s) = self.socks.get_mut(&(actor, id)) {
                    s.ready = true;
                }
            }
            RelayFrame::Notice { message } => {
                let st = self.stats(url);
                st.notices += 1;
                if classify_notice(&message) == Verdict::Park {
                    st.notice_refusals += 1;
                    st.refusal_sample(&message);
                }
            }
            RelayFrame::Closed { message, .. } => {
                if let Some(s) = self.socks.get_mut(&(actor, id)) {
                    s.ready = false;
                }
                let st = self.stats(url);
                st.closed_frames += 1;
                if classify_closed(&message) == Verdict::Park {
                    st.closed_refusals += 1;
                    st.refusal_sample(&message);
                }
            }
            RelayFrame::Auth => self.stats(url).auth_requests += 1,
        }
    }

    /// Turns the events older than the delivery window into delivered, lost or inconclusive.
    fn settle(&mut self, final_pass: bool) {
        let now = self.now();
        let due: Vec<(String, String)> = self
            .pending
            .iter()
            .filter(|(_, p)| final_pass || now >= p.sent_ms + DELIVERY_WINDOW_MS)
            .map(|(k, _)| k.clone())
            .collect();
        for key in due {
            let Some(p) = self.pending.remove(&key) else {
                continue;
            };
            let url = key.1;
            if p.delivered {
                continue;
            }
            if p.ok.is_none() {
                self.stats(&url).no_ok += 1;
            }
            if !p.eligible || p.ok == Some(false) {
                continue;
            }
            let window_over = now >= p.sent_ms + DELIVERY_WINDOW_MS;
            let peer_socket_same = self
                .socks
                .iter()
                .any(|((who, _), s)| *who == 1 - p.from && s.url == url && s.gen_no == p.peer_gen && s.ready);
            let st = self.stats(&url);
            if window_over && peer_socket_same {
                st.lost += 1;
            } else {
                st.inconclusive += 1;
            }
        }
    }

    // ---- recovery measurement ------------------------------------------------------------------

    fn check_recovery(&mut self) {
        if !self.actors.iter().all(|a| a.state == Some(LinkState::Connected)) {
            return;
        }
        let now = self.now();
        if self.initial_ms.is_none() {
            self.initial_ms = Some(now);
            self.schedule_cut(now);
        }
        if let Some(cut) = self.cut_at
            && self
                .actors
                .iter()
                .all(|a| a.connected_at.is_some_and(|t| t >= cut))
        {
            if !self.cut_counted_failed {
                self.recoveries.push(now.saturating_sub(cut));
            }
            self.cut_at = None;
            self.schedule_cut(now);
        }
    }

    fn schedule_cut(&mut self, from: u64) {
        let every = self.opts.loss_every.as_millis() as u64;
        self.next_cut_at = (every > 0).then_some(from + every);
    }

    fn housekeeping(&mut self, now: u64) {
        if let Some(cut) = self.cut_at
            && !self.cut_counted_failed
            && now.saturating_sub(cut) >= RECOVERY_LIMIT_MS
        {
            self.cut_counted_failed = true;
            self.recovery_failures += 1;
            self.recoveries.push(RECOVERY_LIMIT_MS);
        }
        let connected = self.actors.iter().all(|a| a.state == Some(LinkState::Connected));
        if self.cut_at.is_none()
            && connected
            && self.next_cut_at.is_some_and(|t| now >= t)
        {
            self.cut_link(now);
        }
    }

    /// The path fails: both peer connections report `failed`, as ICE does after a network loss.
    fn cut_link(&mut self, now: u64) {
        self.cuts += 1;
        self.cut_at = Some(now);
        self.cut_counted_failed = false;
        self.next_cut_at = None;
        let mut live = Vec::new();
        for (a, actor) in self.actors.iter_mut().enumerate() {
            for (h, pc) in &mut actor.pcs {
                if pc.connected && pc.alive() {
                    pc.failed = true;
                    live.push((a, *h));
                }
            }
        }
        for (a, h) in live {
            self.deliver(
                a,
                LinkInput::AttemptPcState {
                    attempt: h,
                    state: "failed".into(),
                },
            );
        }
    }

    // ---- the fake peer connection -------------------------------------------------------------

    fn sdp(&self, a: usize, fp: &str) -> String {
        let dir = match self.actors[a].role {
            Role::Baby => "sendonly",
            Role::Parent => "recvonly",
        };
        let colon: Vec<String> = (0..fp.len())
            .step_by(2)
            .filter_map(|i| fp.get(i..i + 2))
            .map(str::to_uppercase)
            .collect();
        // The fingerprint line stays last: the core reads hex until the first other character.
        format!(
            "v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na={dir}\r\na=fingerprint:sha-256 {}\r\n",
            colon.join(":")
        )
    }

    fn set_remote(&mut self, a: usize, attempt: u32, sdp: &str) {
        let fp = extract_sha256_fingerprint(sdp).map(|f| f.replace(':', "").to_lowercase());
        let mut peer = None;
        if let Some(fp) = &fp {
            for (m, other) in self.actors.iter().enumerate() {
                if m == a {
                    continue;
                }
                for (h, pc) in &other.pcs {
                    if &pc.fp == fp && !pc.closed {
                        peer = Some((m, *h));
                    }
                }
            }
        }
        if let Some(pc) = self.actors[a].pcs.get_mut(&attempt) {
            pc.remote = true;
            pc.peer = peer;
            pc.dtls_fp = extract_sha256_fingerprint(sdp);
        }
        self.maybe_connect(a, attempt);
    }

    fn ready(pc: &Pc) -> bool {
        pc.local
            && pc.remote
            && pc.remote_candidates > 0
            && pc.alive()
            && !pc.connecting
            && !pc.connected
    }

    fn maybe_connect(&mut self, a: usize, h: u32) {
        let Some(pc) = self.actors[a].pcs.get(&h) else {
            return;
        };
        let Some((b, hb)) = pc.peer.filter(|_| Self::ready(pc)) else {
            return;
        };
        let Some(other) = self.actors[b].pcs.get(&hb) else {
            return;
        };
        if !Self::ready(other) || other.peer != Some((a, h)) {
            return;
        }
        for (x, hx) in [(a, h), (b, hb)] {
            if let Some(pc) = self.actors[x].pcs.get_mut(&hx) {
                pc.connecting = true;
            }
        }
        let ice = self.opts.ice_ms;
        self.after(ice, Timer::Connect { a, ha: h, b, hb });
    }

    fn close_pc(&mut self, a: usize, h: u32) {
        let Some(pc) = self.actors[a].pcs.get_mut(&h) else {
            return;
        };
        if pc.closed {
            return;
        }
        let notify = pc.connected && pc.alive();
        let peer = pc.peer;
        pc.closed = true;
        if let (true, Some((b, hb))) = (notify, peer) {
            self.after(30, Timer::DataClosed { to: b, handle: hb });
        }
    }

    fn send_data(&mut self, a: usize, h: u32, text: String) {
        let Some(pc) = self.actors[a].pcs.get(&h) else {
            return;
        };
        if !(pc.connected && pc.alive()) {
            return;
        }
        let Some((b, hb)) = pc.peer else {
            return;
        };
        self.after(20, Timer::Data { to: b, handle: hb, text });
    }

    fn run_timers(&mut self) {
        loop {
            let now = self.now();
            let Some((&key, _)) = self.timers.iter().next() else {
                return;
            };
            if key.0 > now {
                return;
            }
            let Some(timer) = self.timers.remove(&key) else {
                return;
            };
            match timer {
                Timer::MailboxClosed { actor, id } => self.deliver(
                    actor,
                    LinkInput::MailboxClosed {
                        id,
                        reason: "not available in this test".into(),
                    },
                ),
                Timer::LocalDesc {
                    actor,
                    attempt,
                    kind,
                } => {
                    let Some(pc) = self.actors[actor].pcs.get_mut(&attempt) else {
                        continue;
                    };
                    if pc.closed {
                        continue;
                    }
                    pc.local = true;
                    let fp = pc.fp.clone();
                    let sdp = self.sdp(actor, &fp);
                    self.deliver(
                        actor,
                        LinkInput::AttemptLocalDescription {
                            attempt,
                            sdp_type: kind.into(),
                            sdp,
                        },
                    );
                    for n in 0..LOCAL_CANDIDATES {
                        self.after(
                            80 * u64::from(n + 1),
                            Timer::LocalIce { actor, attempt, n },
                        );
                    }
                    self.maybe_connect(actor, attempt);
                }
                Timer::LocalIce { actor, attempt, n } => {
                    if self.actors[actor].pcs.get(&attempt).is_some_and(|p| !p.closed) {
                        // A documentation address (RFC 5737): the candidate is only text.
                        let candidate = IceCandidate {
                            candidate: format!(
                                "candidate:{n} 1 udp 2122260223 192.0.2.{} {} typ host",
                                10 + n,
                                40_000 + n
                            ),
                            sdp_mid: Some("0".into()),
                            sdp_m_line_index: Some(0),
                        };
                        self.deliver(actor, LinkInput::AttemptLocalIce { attempt, candidate });
                    }
                }
                Timer::Connect { a, ha, b, hb } => {
                    let alive = |s: &Self, x: usize, h: u32| {
                        s.actors[x].pcs.get(&h).is_some_and(Pc::alive)
                    };
                    if !alive(self, a, ha) || !alive(self, b, hb) {
                        continue;
                    }
                    for (x, hx) in [(a, ha), (b, hb)] {
                        if let Some(pc) = self.actors[x].pcs.get_mut(&hx) {
                            pc.connected = true;
                        }
                    }
                    for (x, hx) in [(a, ha), (b, hb)] {
                        self.deliver(
                            x,
                            LinkInput::AttemptPcState {
                                attempt: hx,
                                state: "connected".into(),
                            },
                        );
                        self.deliver(x, LinkInput::AttemptDataOpen { attempt: hx });
                        if let Some(fingerprint) =
                            self.actors[x].pcs.get(&hx).and_then(|p| p.dtls_fp.clone())
                        {
                            self.deliver(
                                x,
                                LinkInput::AttemptDtlsFingerprint {
                                    attempt: hx,
                                    fingerprint,
                                },
                            );
                        }
                    }
                }
                Timer::Data { to, handle, text } => {
                    if self.actors[to]
                        .pcs
                        .get(&handle)
                        .is_some_and(|p| p.connected && p.alive())
                    {
                        self.deliver(to, LinkInput::AttemptDataMessage { attempt: handle, text });
                    }
                }
                Timer::DataClosed { to, handle } => {
                    if self.actors[to].pcs.get(&handle).is_some_and(Pc::alive) {
                        self.deliver(to, LinkInput::AttemptDataClosed { attempt: handle });
                    }
                }
            }
        }
    }

    // ---- reporting -----------------------------------------------------------------------------

    fn progress(&self) {
        let states: Vec<String> = self
            .actors
            .iter()
            .map(|a| a.state.map_or("-".to_owned(), |s| format!("{s:?}")))
            .collect();
        let lost: u64 = self.relays.values().map(|r| r.lost).sum();
        let sent: u64 = self.relays.values().map(|r| r.events_sent).sum();
        eprintln!(
            "[soak] t={}s link={} cuts={} recovered={} lost={} sent={}",
            self.now() / 1_000,
            states.join("/"),
            self.cuts,
            self.recoveries.len(),
            lost,
            sent
        );
    }

    fn snapshot(&self, complete: bool) {
        if let Some(path) = &self.opts.out {
            let report = self.build_report(complete);
            if let Err(e) = report::write_json(path, &report.json) {
                eprintln!("[soak] snapshot failed: {e}");
            }
        }
    }

    fn build_report(&self, complete: bool) -> SoakReport {
        let relays: Vec<RelayStats> = self.relays.values().cloned().collect();
        let verdict = verdict(&relays, &self.recoveries);
        let rec = &self.recoveries;
        let json = json!({
            "tool": "gs-soak",
            "kind": "soak",
            "started_unix_s": self.started_unix_s,
            "planned_s": self.opts.duration.as_secs(),
            "elapsed_s": self.now() / 1_000,
            "complete": complete,
            "options": {
                "relays": self.opts.relays,
                "loss_every_s": self.opts.loss_every.as_secs(),
                "tag_only_filter": self.opts.tag_only,
                "parent_wall_offset_s": self.opts.parent_wall_offset_s,
                "fake_ice_ms": self.opts.ice_ms,
            },
            "link": {
                "initial_setup_ms": self.initial_ms,
                "cuts": self.cuts,
                "recoveries": {
                    "n": rec.len(),
                    "failed": self.recovery_failures,
                    "p50": percentile(rec, 50),
                    "p95": percentile(rec, 95),
                    "max": rec.iter().max(),
                    "samples_ms": rec,
                },
                "states": self.actors.iter()
                    .map(|a| a.state.map(|s| format!("{s:?}"))).collect::<Vec<_>>(),
                "failed_attempts": self.failures,
                "hints": self.hints,
                "security_warnings": self.security_warnings,
                "link_lost_events": self.link_lost_events,
            },
            "relays": relays.iter().map(RelayStats::to_json).collect::<Vec<Value>>(),
            "verdict": {
                "result": verdict.outcome.as_str(),
                "reasons": verdict.reasons,
            },
        });
        SoakReport {
            json,
            relays,
            verdict,
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The id of an `["EVENT", {...}]` frame.
fn event_id_of(frame: &str) -> Option<String> {
    let value: Value = serde_json::from_str(frame).ok()?;
    value
        .get(1)?
        .get("id")?
        .as_str()
        .map(str::to_owned)
}

/// The same `REQ` without the `authors` member of its filters (the tag-only filter test).
fn drop_authors(frame: &str) -> Option<String> {
    let mut value: Value = serde_json::from_str(frame).ok()?;
    for filter in value.as_array_mut()?.iter_mut().skip(2) {
        filter.as_object_mut()?.remove("authors");
    }
    serde_json::to_string(&value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_only_drops_authors_and_keeps_the_rest() {
        let req = r##"["REQ","s1",{"kinds":[24861],"authors":["ab"],"#t":["tag"],"since":5}]"##;
        let out = drop_authors(req).unwrap_or_default();
        assert!(!out.contains("authors"));
        assert!(out.contains("\"#t\":[\"tag\"]") && out.contains("\"since\":5"));
    }

    #[test]
    fn event_id_is_read_from_the_frame() {
        let frame = r#"["EVENT",{"id":"abc","kind":1}]"#;
        assert_eq!(event_id_of(frame).as_deref(), Some("abc"));
        assert_eq!(event_id_of("[\"REQ\",\"x\"]"), None);
    }
}
