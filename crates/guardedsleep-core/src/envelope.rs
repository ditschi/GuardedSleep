//! The sealed signaling envelope (`alpha-connection-model`, design section 2): every signaling
//! message on every mailbox is sealed with XChaCha20-Poly1305 under a key derived from the PSK, so
//! a mailbox operator sees no SDP, candidate, fingerprint or address. The module is standalone:
//! `Link` does not use it yet (task 3.2).
//!
//! Wire format (all lengths fixed per class, so every message of a class is equally long):
//!
//! ```text
//! version (1) | sender role (1) | class (1) | nonce (24) | ciphertext (bucket) | tag (16)
//! ```
//!
//! The nonce is the envelope id. The associated data is `"gs-env/1" | role | class`. The plaintext
//! fills the class's bucket exactly: `recipient_session (16) | sender_session (16) | seq (u32 BE) |
//! message length (u16 BE) | message | zero padding`. The length field is not in the design text;
//! it is what lets the receiver strip the padding.
//!
//! Freshness is session-bound (see [`SessionState`]): a message names the receiver's current
//! session, the sender's session and a sequence number, so a relay replaying stored traffic is
//! ignored. Envelope ids are recorded for de-duplication only **after** an envelope
//! authenticated and passed every session check, so a corrupted copy from one relay cannot shadow
//! the genuine copy from another.

// Untrusted bytes are parsed here: return errors instead of panicking (AGENTS.md section 5.3).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

use std::collections::{HashSet, VecDeque};

use chacha20::rand_core::Rng;
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{Key, KeyInit, XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::{Zeroize, Zeroizing};

use crate::protocol::Role;

/// Envelope format version, the first byte on the wire.
pub const VERSION: u8 = 1;
/// The associated data starts with this string (the format version in text form).
const AD_PREFIX: &[u8] = b"gs-env/1";
/// Bytes before the ciphertext: version, role, class.
const HEADER_LEN: usize = 3;
/// Length of the nonce, which is also the envelope id.
pub const NONCE_LEN: usize = 24;
const TAG_LEN: usize = 16;
/// `recipient_session | sender_session | seq | message length`.
const PLAINTEXT_HEAD_LEN: usize = 16 + 16 + 4 + 2;

/// Size buckets of the padding ladder (design section 2).
pub const BUCKET_1K: usize = 1024;
pub const BUCKET_4K: usize = 4 * 1024;
pub const BUCKET_8K: usize = 8 * 1024;

/// Envelope ids kept at least this many (together with the time bound below, whichever keeps more).
pub const REPLAY_IDS: usize = 4096;
/// ... and at least this long, by the monotonic clock.
pub const REPLAY_WINDOW_MS: u64 = 30 * 60 * 1000;
/// Memory bound against a peer that holds the PSK and floods; an honest sender stays far below it
/// (at most 2 events per second per relay).
const REPLAY_HARD_CAP: usize = 65_536;
/// Sequence numbers remembered per sender session.
pub const SEQ_WINDOW: u32 = 256;
/// Last retired peer sessions remembered.
pub const RETIRED_KEEP: usize = 16;
/// Unconfirmed peer sessions remembered.
const CANDIDATES_KEEP: usize = 8;
/// Replies to new candidate sessions: at most one per this long overall.
pub const REPLY_MIN_INTERVAL_MS: u64 = 5_000;

// ---- keys ------------------------------------------------------------------------------------

/// What a key derived from the PSK is for. One HKDF info string per purpose; the room id and the
/// relay-admission key keep their own derivations in `pairing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// The envelope seal key.
    Seal,
    /// HMAC key of the daily Nostr tag.
    NostrTag,
    /// Seed of the daily per-role Nostr signing keys.
    NostrKey,
    /// Seed of the deterministic relay choice.
    RelayPick,
}

impl Purpose {
    fn info(self) -> &'static [u8] {
        match self {
            Purpose::Seal => b"guardedsleep seal v1",
            Purpose::NostrTag => b"guardedsleep nostr tag v1",
            Purpose::NostrKey => b"guardedsleep nostr key v1",
            Purpose::RelayPick => b"guardedsleep relay pick v1",
        }
    }
}

/// 32 secret bytes; wiped on drop and never printed.
pub struct SecretKey([u8; 32]);

impl SecretKey {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Drop for SecretKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretKey(<redacted>)")
    }
}

/// HKDF-SHA256: one extract over the PSK with an empty salt, then an expand with the purpose's
/// info string. Returns `None` for an empty PSK.
pub fn derive_key(psk: &[u8], purpose: Purpose) -> Option<SecretKey> {
    if psk.is_empty() {
        return None;
    }
    // `None` is the empty salt (RFC 5869: it stands for HashLen zero bytes, which HMAC treats
    // exactly like an empty key).
    let hk = Hkdf::<Sha256>::new(None, psk);
    let mut out = [0u8; 32];
    hk.expand(purpose.info(), &mut out).ok()?;
    let key = SecretKey(out);
    out.zeroize();
    Some(key)
}

// ---- message classes -------------------------------------------------------------------------

/// What an envelope carries. Authenticated in the clear header; fixes the padded size, so all
/// messages of a class have the same length. The class set and its buckets are the first cut of
/// task 2.2 and are settled with the sealing of the real messages in task 3.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageClass {
    /// Presence: role, session nonce, capability flags.
    Here,
    /// Small control messages: `request_offer`, `stop`.
    Control,
    /// An SDP offer or answer.
    Description,
    /// A bundle of ICE candidates.
    Candidates,
}

impl MessageClass {
    pub fn code(self) -> u8 {
        match self {
            MessageClass::Here => 1,
            MessageClass::Control => 2,
            MessageClass::Description => 3,
            MessageClass::Candidates => 4,
        }
    }

    pub fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            1 => MessageClass::Here,
            2 => MessageClass::Control,
            3 => MessageClass::Description,
            4 => MessageClass::Candidates,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            MessageClass::Here => "here",
            MessageClass::Control => "control",
            MessageClass::Description => "description",
            MessageClass::Candidates => "candidates",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        [
            MessageClass::Here,
            MessageClass::Control,
            MessageClass::Description,
            MessageClass::Candidates,
        ]
        .into_iter()
        .find(|c| c.name() == name)
    }

    /// The padded plaintext size of this class.
    pub fn bucket(self) -> usize {
        match self {
            MessageClass::Here | MessageClass::Control => BUCKET_1K,
            MessageClass::Description => BUCKET_4K,
            MessageClass::Candidates => BUCKET_8K,
        }
    }

    /// The longest message this class can carry.
    pub fn max_message(self) -> usize {
        self.bucket() - PLAINTEXT_HEAD_LEN
    }

    /// Total envelope length on the wire.
    pub fn wire_len(self) -> usize {
        HEADER_LEN + NONCE_LEN + self.bucket() + TAG_LEN
    }
}

fn role_code(role: Role) -> u8 {
    match role {
        Role::Baby => 0,
        Role::Parent => 1,
    }
}

fn role_from_code(code: u8) -> Option<Role> {
    match code {
        0 => Some(Role::Baby),
        1 => Some(Role::Parent),
        _ => None,
    }
}

// ---- sessions --------------------------------------------------------------------------------

/// A device's identity for one `Link` run: 16 random bytes, announced in `here`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId([u8; 16]);

impl SessionId {
    /// `recipient_session` of a first `here`, which names no session yet.
    pub const NONE: SessionId = SessionId([0; 16]);

    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn random<R: Rng>(rng: &mut R) -> Self {
        let mut b = [0u8; 16];
        rng.fill_bytes(&mut b);
        Self(b)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn is_none(&self) -> bool {
        self.0 == [0; 16]
    }
}

// A session id travels inside the ciphertext only; keep it out of logs all the same.
impl std::fmt::Debug for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionId(..)")
    }
}

// ---- errors ----------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealError {
    /// The message does not fit the class's bucket: a sender bug, never truncated.
    Oversized,
    /// The sequence counter of this session is used up (2^32 messages).
    SeqExhausted,
    /// `seal_to_peer` without a confirmed peer session.
    NoPeerSession,
    Crypto,
}

/// Why an envelope was dropped. None of them changes any state, apart from candidate bookkeeping
/// for `here`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenError {
    /// Shorter or longer than any envelope, or an unknown class or role.
    Malformed,
    UnsupportedVersion,
    /// Carries our own role: a reflected envelope.
    OwnRole,
    /// The AEAD tag did not verify: wrong key, a corrupted copy, or a changed header.
    BadTag,
    /// An envelope id already processed in this run.
    Duplicate,
    /// `recipient_session` is not our current session.
    WrongRecipient,
    /// `sender_session` is not the peer's current confirmed session.
    WrongSender,
    /// The sender's session was retired (it stopped, or the peer started a new one).
    RetiredSender,
    /// The sequence number was already seen or is older than the window.
    ReplayedSeq,
}

impl OpenError {
    pub fn as_str(self) -> &'static str {
        match self {
            OpenError::Malformed => "malformed",
            OpenError::UnsupportedVersion => "unsupported_version",
            OpenError::OwnRole => "own_role",
            OpenError::BadTag => "bad_tag",
            OpenError::Duplicate => "duplicate",
            OpenError::WrongRecipient => "wrong_recipient",
            OpenError::WrongSender => "wrong_sender",
            OpenError::RetiredSender => "retired_sender",
            OpenError::ReplayedSeq => "replayed_seq",
        }
    }
}

// ---- sealing ---------------------------------------------------------------------------------

/// A sealed envelope and its id (the nonce), which a resend reuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sealed {
    pub id: [u8; NONCE_LEN],
    pub bytes: Vec<u8>,
}

fn associated_data(role: Role, class: MessageClass) -> [u8; AD_PREFIX.len() + 2] {
    let mut ad = [0u8; AD_PREFIX.len() + 2];
    let (prefix, tail) = ad.split_at_mut(AD_PREFIX.len());
    prefix.copy_from_slice(AD_PREFIX);
    if let [r, c] = tail {
        *r = role_code(role);
        *c = class.code();
    }
    ad
}

fn cipher(key: &SecretKey) -> XChaCha20Poly1305 {
    XChaCha20Poly1305::new(&Key::from(key.0))
}

/// The authenticated fields around a message.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub role: Role,
    pub class: MessageClass,
    pub recipient: SessionId,
    pub sender: SessionId,
    pub seq: u32,
}

/// Seals one message with an explicit nonce. [`SessionState::seal`] draws the nonce from the
/// CSPRNG; fixed nonces exist for the conformance vectors only.
pub fn seal_with_nonce(
    key: &SecretKey,
    frame: &Frame,
    message: &[u8],
    nonce: [u8; NONCE_LEN],
) -> Result<Sealed, SealError> {
    let Frame {
        role,
        class,
        recipient,
        sender,
        seq,
    } = *frame;
    if message.len() > class.max_message() {
        return Err(SealError::Oversized);
    }
    let len = u16::try_from(message.len()).map_err(|_| SealError::Oversized)?;
    let mut plain = Zeroizing::new(Vec::with_capacity(class.bucket()));
    plain.extend_from_slice(recipient.as_bytes());
    plain.extend_from_slice(sender.as_bytes());
    plain.extend_from_slice(&seq.to_be_bytes());
    plain.extend_from_slice(&len.to_be_bytes());
    plain.extend_from_slice(message);
    plain.resize(class.bucket(), 0);
    let ad = associated_data(role, class);
    let ct = cipher(key)
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: plain.as_slice(),
                aad: &ad,
            },
        )
        .map_err(|_| SealError::Crypto)?;
    let mut bytes = Vec::with_capacity(class.wire_len());
    bytes.push(VERSION);
    bytes.push(role_code(role));
    bytes.push(class.code());
    bytes.extend_from_slice(&nonce);
    bytes.extend_from_slice(&ct);
    Ok(Sealed { id: nonce, bytes })
}

// ---- opening and the per-run state -----------------------------------------------------------

/// An envelope that authenticated and passed the session checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    pub id: [u8; NONCE_LEN],
    pub role: Role,
    pub class: MessageClass,
    pub sender_session: SessionId,
    pub recipient_session: SessionId,
    pub seq: u32,
    pub message: Vec<u8>,
    /// The sender's session became the peer's confirmed current session with this envelope.
    pub confirmed: bool,
    /// The core should send one `here` whose `recipient_session` is this candidate session. Set
    /// at most once per candidate and at most once per [`REPLY_MIN_INTERVAL_MS`].
    pub reply_to: Option<SessionId>,
}

/// Which of the last [`SEQ_WINDOW`] sequence numbers of one sender session were seen.
struct SeqWindow {
    highest: Option<u32>,
    /// Bit `i` set: `highest - i` was seen.
    bits: [u64; (SEQ_WINDOW / 64) as usize],
}

impl SeqWindow {
    fn new() -> Self {
        Self {
            highest: None,
            bits: [0; (SEQ_WINDOW / 64) as usize],
        }
    }

    fn is_new(&self, seq: u32) -> bool {
        let Some(high) = self.highest else {
            return true;
        };
        if seq > high {
            return true;
        }
        let back = high - seq;
        if back >= SEQ_WINDOW {
            return false;
        }
        let word = (back / 64) as usize;
        let bit = back % 64;
        self.bits.get(word).is_some_and(|w| (w >> bit) & 1 == 0)
    }

    fn mark(&mut self, seq: u32) {
        match self.highest {
            Some(high) if seq <= high => {
                let back = high - seq;
                if let Some(w) = self.bits.get_mut((back / 64) as usize) {
                    *w |= 1 << (back % 64);
                }
            }
            _ => {
                let shift = self.highest.map_or(0, |h| seq - h);
                self.shift_up(shift);
                self.highest = Some(seq);
                if let Some(w) = self.bits.first_mut() {
                    *w |= 1;
                }
            }
        }
    }

    fn shift_up(&mut self, shift: u32) {
        if shift >= SEQ_WINDOW {
            self.bits = [0; (SEQ_WINDOW / 64) as usize];
            return;
        }
        let words = (shift / 64) as usize;
        let bits = shift % 64;
        let old = self.bits;
        for (i, slot) in self.bits.iter_mut().enumerate() {
            let lo = i.checked_sub(words).and_then(|j| old.get(j)).copied();
            let hi = i
                .checked_sub(words + 1)
                .and_then(|j| old.get(j))
                .copied()
                .unwrap_or(0);
            *slot = match (lo, bits) {
                (None, _) => 0,
                (Some(l), 0) => l,
                (Some(l), b) => (l << b) | (hi >> (64 - b)),
            };
        }
    }
}

/// Envelope ids processed in this run, across all mailboxes: at least [`REPLAY_IDS`] ids and at
/// least [`REPLAY_WINDOW_MS`] of history.
struct ReplayWindow {
    order: VecDeque<([u8; NONCE_LEN], u64)>,
    seen: HashSet<[u8; NONCE_LEN]>,
}

impl ReplayWindow {
    fn new() -> Self {
        Self {
            order: VecDeque::new(),
            seen: HashSet::new(),
        }
    }

    fn contains(&self, id: &[u8; NONCE_LEN]) -> bool {
        self.seen.contains(id)
    }

    fn record(&mut self, id: [u8; NONCE_LEN], now_ms: u64) {
        if self.seen.insert(id) {
            self.order.push_back((id, now_ms));
        }
        while let Some((front_id, t)) = self.order.front().copied() {
            let aged = now_ms.saturating_sub(t) >= REPLAY_WINDOW_MS;
            let over = self.order.len() > REPLAY_IDS;
            if (aged && over) || self.order.len() > REPLAY_HARD_CAP {
                self.order.pop_front();
                self.seen.remove(&front_id);
            } else {
                break;
            }
        }
    }
}

struct PeerSession {
    id: SessionId,
    seqs: SeqWindow,
}

/// Everything one `Link` run knows about envelopes: its own session, the peer's candidate,
/// confirmed and retired sessions, the sequence windows and the run-wide replay window. There is
/// deliberately no way to clear any of it: reopening a mailbox must not forget what was seen.
pub struct SessionState {
    role: Role,
    own: SessionId,
    next_seq: Option<u32>,
    peer: Option<PeerSession>,
    candidates: VecDeque<SessionId>,
    retired: VecDeque<SessionId>,
    replied: VecDeque<SessionId>,
    last_reply_ms: Option<u64>,
    replay: ReplayWindow,
}

impl SessionState {
    /// `own` is this run's fresh session id ([`SessionId::random`]).
    pub fn new(role: Role, own: SessionId) -> Self {
        Self {
            role,
            own,
            next_seq: Some(0),
            peer: None,
            candidates: VecDeque::new(),
            retired: VecDeque::new(),
            replied: VecDeque::new(),
            last_reply_ms: None,
            replay: ReplayWindow::new(),
        }
    }

    /// A state that already confirmed `peer` and retired the given sessions (conformance vectors).
    pub fn with_peer(
        role: Role,
        own: SessionId,
        peer: Option<SessionId>,
        retired: &[SessionId],
    ) -> Self {
        let mut state = Self::new(role, own);
        for r in retired {
            state.push_retired(*r);
        }
        if let Some(p) = peer {
            state.promote(p);
        }
        state
    }

    pub fn own_session(&self) -> SessionId {
        self.own
    }

    /// The peer's confirmed current session, if any.
    pub fn peer_session(&self) -> Option<SessionId> {
        self.peer.as_ref().map(|p| p.id)
    }

    pub fn is_retired(&self, s: &SessionId) -> bool {
        self.retired.contains(s)
    }

    /// Retires a peer session for good; call after a `stop` from it was processed. If it is the
    /// confirmed session, the peer has none until a new one confirms.
    pub fn retire(&mut self, session: SessionId) {
        if self.peer.as_ref().is_some_and(|p| p.id == session) {
            self.peer = None;
        }
        self.candidates.retain(|c| *c != session);
        self.push_retired(session);
    }

    fn push_retired(&mut self, session: SessionId) {
        if !self.retired.contains(&session) {
            self.retired.push_back(session);
            while self.retired.len() > RETIRED_KEEP {
                self.retired.pop_front();
            }
        }
    }

    /// Seals a message to `recipient` with a nonce from the CSPRNG and the next sequence number.
    pub fn seal<R: Rng>(
        &mut self,
        key: &SecretKey,
        rng: &mut R,
        class: MessageClass,
        recipient: SessionId,
        message: &[u8],
    ) -> Result<Sealed, SealError> {
        if message.len() > class.max_message() {
            return Err(SealError::Oversized);
        }
        let seq = self.next_seq.ok_or(SealError::SeqExhausted)?;
        let mut nonce = [0u8; NONCE_LEN];
        rng.fill_bytes(&mut nonce);
        let frame = Frame {
            role: self.role,
            class,
            recipient,
            sender: self.own,
            seq,
        };
        let sealed = seal_with_nonce(key, &frame, message, nonce)?;
        self.next_seq = seq.checked_add(1);
        Ok(sealed)
    }

    /// Seals a message to the peer's confirmed session.
    pub fn seal_to_peer<R: Rng>(
        &mut self,
        key: &SecretKey,
        rng: &mut R,
        class: MessageClass,
        message: &[u8],
    ) -> Result<Sealed, SealError> {
        let peer = self.peer_session().ok_or(SealError::NoPeerSession)?;
        self.seal(key, rng, class, peer, message)
    }

    /// Opens one envelope received on any mailbox. `now_ms` is the monotonic clock.
    ///
    /// The checks run in this order: format, own role, tag, duplicate id, session rules. Nothing
    /// is recorded unless all of them pass.
    pub fn open(
        &mut self,
        key: &SecretKey,
        now_ms: u64,
        bytes: &[u8],
    ) -> Result<Opened, OpenError> {
        let (head, rest) = bytes
            .split_first_chunk::<HEADER_LEN>()
            .ok_or(OpenError::Malformed)?;
        let [version, role_b, class_b] = *head;
        if version != VERSION {
            return Err(OpenError::UnsupportedVersion);
        }
        let role = role_from_code(role_b).ok_or(OpenError::Malformed)?;
        let class = MessageClass::from_code(class_b).ok_or(OpenError::Malformed)?;
        if bytes.len() != class.wire_len() {
            return Err(OpenError::Malformed);
        }
        if role == self.role {
            return Err(OpenError::OwnRole);
        }
        let (nonce, ct) = rest
            .split_first_chunk::<NONCE_LEN>()
            .ok_or(OpenError::Malformed)?;
        let ad = associated_data(role, class);
        let plain = Zeroizing::new(
            cipher(key)
                .decrypt(&XNonce::from(*nonce), Payload { msg: ct, aad: &ad })
                .map_err(|_| OpenError::BadTag)?,
        );
        if self.replay.contains(nonce) {
            return Err(OpenError::Duplicate);
        }
        let parsed = parse_plaintext(&plain, class).ok_or(OpenError::Malformed)?;
        let decision = self.check_sessions(class, &parsed, now_ms)?;

        // Everything passed: now the state changes.
        self.replay.record(*nonce, now_ms);
        let sender = parsed.sender;
        let mut confirmed = false;
        if decision.promote {
            self.promote(sender);
            confirmed = true;
        }
        if let (false, Some(p)) = (class == MessageClass::Here, self.peer.as_mut()) {
            p.seqs.mark(parsed.seq);
        }
        if decision.new_candidate {
            self.mark_candidate(sender);
        }
        let mut reply_to = None;
        if decision.reply {
            self.replied.push_back(sender);
            while self.replied.len() > CANDIDATES_KEEP * 2 {
                self.replied.pop_front();
            }
            self.last_reply_ms = Some(now_ms);
            reply_to = Some(sender);
        }
        Ok(Opened {
            id: *nonce,
            role,
            class,
            sender_session: sender,
            recipient_session: parsed.recipient,
            seq: parsed.seq,
            message: parsed.message,
            confirmed,
            reply_to,
        })
    }

    fn check_sessions(
        &self,
        class: MessageClass,
        p: &Parsed,
        now_ms: u64,
    ) -> Result<Decision, OpenError> {
        if self.retired.contains(&p.sender) {
            return Err(OpenError::RetiredSender);
        }
        let addressed_to_us = p.recipient == self.own;
        let is_current = self.peer.as_ref().is_some_and(|c| c.id == p.sender);
        let known = is_current || self.candidates.contains(&p.sender);
        if class == MessageClass::Here {
            // A first `here` names nobody; a reply names the session it answers.
            if !p.recipient.is_none() && !addressed_to_us {
                return Err(OpenError::WrongRecipient);
            }
            // An answer to our own `here` proves the sender is live now.
            let promote = addressed_to_us && !is_current;
            let rate_ok = self
                .last_reply_ms
                .is_none_or(|t| now_ms.saturating_sub(t) >= REPLY_MIN_INTERVAL_MS);
            // One `here` per peer session answers it (also when this envelope already confirmed
            // the session, because the peer has not confirmed ours yet).
            let reply = !is_current && !self.replied.contains(&p.sender) && rate_ok;
            return Ok(Decision {
                promote,
                reply,
                new_candidate: !known && !promote,
            });
        }
        if !addressed_to_us {
            return Err(OpenError::WrongRecipient);
        }
        if is_current {
            let fresh = self.peer.as_ref().is_some_and(|c| c.seqs.is_new(p.seq));
            if !fresh {
                return Err(OpenError::ReplayedSeq);
            }
            return Ok(Decision::default());
        }
        if self.candidates.contains(&p.sender) {
            // A message that names our current session proves the sender is live now.
            return Ok(Decision {
                promote: true,
                ..Decision::default()
            });
        }
        Err(OpenError::WrongSender)
    }

    fn mark_candidate(&mut self, s: SessionId) {
        if self.peer.as_ref().is_some_and(|p| p.id == s) || self.candidates.contains(&s) {
            return;
        }
        self.candidates.push_back(s);
        while self.candidates.len() > CANDIDATES_KEEP {
            self.candidates.pop_front();
        }
    }

    /// Makes `s` the confirmed session; the session it replaces belongs to a run that is over.
    fn promote(&mut self, s: SessionId) {
        self.candidates.retain(|c| *c != s);
        if let Some(old) = self.peer.take() {
            self.push_retired(old.id);
        }
        self.peer = Some(PeerSession {
            id: s,
            seqs: SeqWindow::new(),
        });
    }
}

#[derive(Default)]
struct Decision {
    promote: bool,
    reply: bool,
    new_candidate: bool,
}

struct Parsed {
    recipient: SessionId,
    sender: SessionId,
    seq: u32,
    message: Vec<u8>,
}

fn parse_plaintext(plain: &[u8], class: MessageClass) -> Option<Parsed> {
    if plain.len() != class.bucket() {
        return None;
    }
    let (recipient, rest) = plain.split_first_chunk::<16>()?;
    let (sender, rest) = rest.split_first_chunk::<16>()?;
    let (seq, rest) = rest.split_first_chunk::<4>()?;
    let (len, rest) = rest.split_first_chunk::<2>()?;
    let len = usize::from(u16::from_be_bytes(*len));
    let message = rest.get(..len)?.to_vec();
    Some(Parsed {
        recipient: SessionId::new(*recipient),
        sender: SessionId::new(*sender),
        seq: u32::from_be_bytes(*seq),
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chacha20::ChaCha20Rng;
    use chacha20::rand_core::SeedableRng;
    use proptest::prelude::*;

    const PSK: [u8; 32] = [3; 32];

    /// Builds an envelope with chosen fields, as a relay holding stored traffic could not but a
    /// peer with the key (or a test) can.
    #[allow(clippy::too_many_arguments)]
    fn forge(
        k: &SecretKey,
        role: Role,
        class: MessageClass,
        recipient: SessionId,
        sender: SessionId,
        seq: u32,
        message: &[u8],
        nonce: [u8; NONCE_LEN],
    ) -> Result<Sealed, SealError> {
        let frame = Frame {
            role,
            class,
            recipient,
            sender,
            seq,
        };
        seal_with_nonce(k, &frame, message, nonce)
    }

    fn key() -> SecretKey {
        derive_key(&PSK, Purpose::Seal).unwrap()
    }

    fn rng(n: u8) -> ChaCha20Rng {
        ChaCha20Rng::from_seed([n; 32])
    }

    /// A baby and a parent that already confirmed each other.
    fn confirmed_pair() -> (
        SessionState,
        SessionState,
        ChaCha20Rng,
        ChaCha20Rng,
        SecretKey,
    ) {
        let (mut rb, mut rp) = (rng(1), rng(2));
        let k = key();
        let mut baby = SessionState::new(Role::Baby, SessionId::random(&mut rb));
        let mut parent = SessionState::new(Role::Parent, SessionId::random(&mut rp));
        let h1 = baby
            .seal(&k, &mut rb, MessageClass::Here, SessionId::NONE, b"here")
            .unwrap();
        let o = parent.open(&k, 0, &h1.bytes).unwrap();
        let h2 = parent
            .seal(
                &k,
                &mut rp,
                MessageClass::Here,
                o.reply_to.unwrap(),
                b"here",
            )
            .unwrap();
        let o = baby.open(&k, 1, &h2.bytes).unwrap();
        assert!(o.confirmed);
        let h3 = baby
            .seal(
                &k,
                &mut rb,
                MessageClass::Here,
                o.reply_to.unwrap(),
                b"here",
            )
            .unwrap();
        assert!(parent.open(&k, 2, &h3.bytes).unwrap().confirmed);
        assert_eq!(baby.peer_session(), Some(parent.own_session()));
        assert_eq!(parent.peer_session(), Some(baby.own_session()));
        (baby, parent, rb, rp, k)
    }

    #[test]
    fn hkdf_info_strings_give_four_independent_keys() {
        let keys: Vec<_> = [
            Purpose::Seal,
            Purpose::NostrTag,
            Purpose::NostrKey,
            Purpose::RelayPick,
        ]
        .into_iter()
        .map(|p| *derive_key(&PSK, p).unwrap().as_bytes())
        .collect();
        for (i, a) in keys.iter().enumerate() {
            for b in keys.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }
        assert!(derive_key(&[], Purpose::Seal).is_none());
    }

    #[test]
    fn the_seal_key_matches_an_independent_hkdf_computation() {
        // HKDF-SHA256(ikm = 32 x 0x03, salt = empty, info = "guardedsleep seal v1"), computed with
        // HMAC directly (extract, then one expand block) so the check does not rest on `hkdf`.
        use hmac::{Hmac, KeyInit, Mac};
        let mut ext = <Hmac<Sha256> as KeyInit>::new_from_slice(&[0u8; 32]).unwrap();
        ext.update(&PSK);
        let prk = ext.finalize().into_bytes();
        let mut exp = <Hmac<Sha256> as KeyInit>::new_from_slice(&prk).unwrap();
        exp.update(b"guardedsleep seal v1");
        exp.update(&[1]);
        let t1 = exp.finalize().into_bytes();
        assert_eq!(key().as_bytes().as_slice(), t1.as_slice());
    }

    #[test]
    fn debug_never_prints_key_material() {
        let k = key();
        let text = format!("{k:?} {:?}", SessionId::new([0xAB; 16]));
        assert!(!text.contains("ab") && !text.contains("171"), "{text}");
    }

    #[test]
    fn every_class_has_one_wire_length() {
        let k = key();
        let mut r = rng(4);
        let mut s = SessionState::new(Role::Baby, SessionId::random(&mut r));
        for class in [
            MessageClass::Here,
            MessageClass::Control,
            MessageClass::Description,
            MessageClass::Candidates,
        ] {
            for len in [0, 1, class.max_message()] {
                let m = vec![b'x'; len];
                let sealed = s.seal(&k, &mut r, class, SessionId::NONE, &m).unwrap();
                assert_eq!(sealed.bytes.len(), class.wire_len());
            }
            let too_big = vec![0u8; class.max_message() + 1];
            assert_eq!(
                s.seal(&k, &mut r, class, SessionId::NONE, &too_big),
                Err(SealError::Oversized)
            );
        }
        assert_eq!(MessageClass::Here.bucket(), 1024);
        assert_eq!(MessageClass::Description.bucket(), 4096);
        assert_eq!(MessageClass::Candidates.bucket(), 8192);
    }

    #[test]
    fn handshake_confirms_both_sessions_with_one_reply_each() {
        // confirmed_pair asserts the whole exchange.
        let _ = confirmed_pair();
    }

    #[test]
    fn a_message_round_trips() {
        let (mut baby, mut parent, mut rb, _, k) = confirmed_pair();
        let sealed = baby
            .seal_to_peer(&k, &mut rb, MessageClass::Description, b"{\"sdp\":1}")
            .unwrap();
        let o = parent.open(&k, 10, &sealed.bytes).unwrap();
        assert_eq!(o.message, b"{\"sdp\":1}");
        assert_eq!(o.class, MessageClass::Description);
        assert_eq!(o.role, Role::Baby);
        assert_eq!(o.id, sealed.id);
    }

    #[test]
    fn a_wrong_key_is_a_bad_tag() {
        let (mut baby, mut parent, mut rb, _, k) = confirmed_pair();
        let sealed = baby
            .seal_to_peer(&k, &mut rb, MessageClass::Control, b"x")
            .unwrap();
        let other = derive_key(&[4; 32], Purpose::Seal).unwrap();
        assert_eq!(
            parent.open(&other, 3, &sealed.bytes),
            Err(OpenError::BadTag)
        );
        // The envelope key is separate from the other derived keys.
        let tag_key = derive_key(&PSK, Purpose::NostrTag).unwrap();
        assert_eq!(
            parent.open(&tag_key, 3, &sealed.bytes),
            Err(OpenError::BadTag)
        );
    }

    #[test]
    fn truncated_and_malformed_input_is_rejected() {
        let (mut baby, mut parent, mut rb, _, k) = confirmed_pair();
        let sealed = baby
            .seal_to_peer(&k, &mut rb, MessageClass::Control, b"x")
            .unwrap();
        for cut in [0, 1, 2, 3, 20, 27, sealed.bytes.len() - 1] {
            assert_eq!(
                parent.open(&k, 3, sealed.bytes.get(..cut).unwrap()),
                Err(OpenError::Malformed),
                "cut at {cut}"
            );
        }
        let mut long = sealed.bytes.clone();
        long.push(0);
        assert_eq!(parent.open(&k, 3, &long), Err(OpenError::Malformed));
        let mut bad_version = sealed.bytes.clone();
        bad_version[0] = 2;
        assert_eq!(
            parent.open(&k, 3, &bad_version),
            Err(OpenError::UnsupportedVersion)
        );
        let mut bad_class = sealed.bytes.clone();
        bad_class[2] = 9;
        assert_eq!(parent.open(&k, 3, &bad_class), Err(OpenError::Malformed));
    }

    #[test]
    fn a_changed_header_fails_the_tag() {
        let (mut baby, mut parent, mut rb, _, k) = confirmed_pair();
        let sealed = baby
            .seal_to_peer(&k, &mut rb, MessageClass::Control, b"x")
            .unwrap();
        // Same length class (1 KiB): only the authenticated class byte differs.
        let mut as_here = sealed.bytes.clone();
        as_here[2] = MessageClass::Here.code();
        assert_eq!(parent.open(&k, 3, &as_here), Err(OpenError::BadTag));
        // The role byte, flipped to the receiver's own, is refused as reflection first.
        let mut as_parent = sealed.bytes.clone();
        as_parent[1] = 1;
        assert_eq!(parent.open(&k, 3, &as_parent), Err(OpenError::OwnRole));
    }

    #[test]
    fn a_reflected_envelope_is_rejected_by_role() {
        let (mut baby, _, mut rb, _, k) = confirmed_pair();
        let sealed = baby
            .seal_to_peer(&k, &mut rb, MessageClass::Control, b"x")
            .unwrap();
        // The attacker sends the baby's envelope back to the baby.
        assert_eq!(baby.open(&k, 3, &sealed.bytes), Err(OpenError::OwnRole));
    }

    #[test]
    fn a_replayed_envelope_is_rejected_and_changes_nothing() {
        let (mut baby, mut parent, mut rb, _, k) = confirmed_pair();
        let sealed = baby
            .seal_to_peer(&k, &mut rb, MessageClass::Control, b"x")
            .unwrap();
        assert!(parent.open(&k, 3, &sealed.bytes).is_ok());
        assert_eq!(parent.open(&k, 4, &sealed.bytes), Err(OpenError::Duplicate));
        // A later message still works, so the replay did not disturb the windows.
        let next = baby
            .seal_to_peer(&k, &mut rb, MessageClass::Control, b"y")
            .unwrap();
        assert_eq!(parent.open(&k, 5, &next.bytes).unwrap().message, b"y");
    }

    #[test]
    fn a_resealed_copy_with_a_new_id_but_an_old_seq_is_rejected() {
        let (mut baby, mut parent, mut rb, _, k) = confirmed_pair();
        let a = baby
            .seal_to_peer(&k, &mut rb, MessageClass::Control, b"x")
            .unwrap();
        let seq = parent.open(&k, 3, &a.bytes).unwrap().seq;
        // Same session and seq under a fresh nonce: only a holder of the key could build this.
        let again = forge(
            &k,
            Role::Baby,
            MessageClass::Control,
            parent.own_session(),
            baby.own_session(),
            seq,
            b"x",
            [9; NONCE_LEN],
        )
        .unwrap();
        assert_eq!(
            parent.open(&k, 5, &again.bytes),
            Err(OpenError::ReplayedSeq)
        );
    }

    #[test]
    fn the_wrong_recipient_session_is_rejected() {
        let (mut baby, mut parent, mut rb, _, k) = confirmed_pair();
        // Sealed for an earlier parent session.
        let old = SessionId::new([0x55; 16]);
        let sealed = baby
            .seal(&k, &mut rb, MessageClass::Control, old, b"x")
            .unwrap();
        assert_eq!(
            parent.open(&k, 3, &sealed.bytes),
            Err(OpenError::WrongRecipient)
        );
        // A rejected envelope is not remembered: the id stays usable for the genuine copy.
        assert!(!parent.replay.contains(&sealed.id));
    }

    #[test]
    fn the_wrong_sender_session_is_rejected() {
        let (_, mut parent, _, _, k) = confirmed_pair();
        let stranger = SessionId::new([0x66; 16]);
        let sealed = forge(
            &k,
            Role::Baby,
            MessageClass::Control,
            parent.own_session(),
            stranger,
            0,
            b"x",
            [1; NONCE_LEN],
        )
        .unwrap();
        assert_eq!(
            parent.open(&k, 3, &sealed.bytes),
            Err(OpenError::WrongSender)
        );
    }

    #[test]
    fn a_stored_stop_from_a_retired_sender_session_is_rejected() {
        let (mut baby, mut parent, mut rb, mut rp, k) = confirmed_pair();
        let stop = baby
            .seal_to_peer(&k, &mut rb, MessageClass::Control, b"{\"type\":\"stop\"}")
            .unwrap();
        let old_baby = baby.own_session();
        assert!(parent.open(&k, 3, &stop.bytes).is_ok());
        parent.retire(old_baby);
        assert!(parent.is_retired(&old_baby));
        assert_eq!(parent.peer_session(), None);

        // The baby restarts with a new session and the pair confirms again.
        let mut baby2 = SessionState::new(Role::Baby, SessionId::random(&mut rb));
        let h = baby2
            .seal(&k, &mut rb, MessageClass::Here, SessionId::NONE, b"here")
            .unwrap();
        let o = parent.open(&k, 10_000, &h.bytes).unwrap();
        let reply = parent
            .seal(
                &k,
                &mut rp,
                MessageClass::Here,
                o.reply_to.unwrap(),
                b"here",
            )
            .unwrap();
        assert!(baby2.open(&k, 10_001, &reply.bytes).unwrap().confirmed);
        let h2 = baby2
            .seal(
                &k,
                &mut rb,
                MessageClass::Here,
                parent.own_session(),
                b"here",
            )
            .unwrap();
        assert!(parent.open(&k, 10_002, &h2.bytes).unwrap().confirmed);

        // A relay now injects the old stop, with a fresh copy that was never delivered (new id).
        let stored = forge(
            &k,
            Role::Baby,
            MessageClass::Control,
            parent.own_session(),
            old_baby,
            7,
            b"{\"type\":\"stop\"}",
            [3; NONCE_LEN],
        )
        .unwrap();
        assert_eq!(
            parent.open(&k, 20_000, &stored.bytes),
            Err(OpenError::RetiredSender)
        );
        // An old `here` from the retired session creates no candidate and no reply either.
        let old_here = forge(
            &k,
            Role::Baby,
            MessageClass::Here,
            SessionId::NONE,
            old_baby,
            0,
            b"here",
            [4; NONCE_LEN],
        )
        .unwrap();
        assert_eq!(
            parent.open(&k, 20_001, &old_here.bytes),
            Err(OpenError::RetiredSender)
        );
    }

    #[test]
    fn a_confirmed_peer_restart_retires_the_previous_session() {
        let (baby, mut parent, mut rb, mut rp, k) = confirmed_pair();
        let old = baby.own_session();
        let mut baby2 = SessionState::new(Role::Baby, SessionId::random(&mut rb));
        let h = baby2
            .seal(&k, &mut rb, MessageClass::Here, SessionId::NONE, b"here")
            .unwrap();
        let o = parent.open(&k, 10_000, &h.bytes).unwrap();
        // Still the old session until the new one proves it is live.
        assert_eq!(parent.peer_session(), Some(old));
        let reply = parent
            .seal(
                &k,
                &mut rp,
                MessageClass::Here,
                o.reply_to.unwrap(),
                b"here",
            )
            .unwrap();
        baby2.open(&k, 10_001, &reply.bytes).unwrap();
        let h2 = baby2
            .seal(
                &k,
                &mut rb,
                MessageClass::Here,
                parent.own_session(),
                b"here",
            )
            .unwrap();
        parent.open(&k, 10_002, &h2.bytes).unwrap();
        assert_eq!(parent.peer_session(), Some(baby2.own_session()));
        assert!(parent.is_retired(&old));
    }

    #[test]
    fn an_old_here_makes_one_candidate_reply_and_never_confirms() {
        let (mut baby, _, _, _, k) = confirmed_pair();
        let old_parent = SessionId::new([0x77; 16]);
        let stale = forge(
            &k,
            Role::Parent,
            MessageClass::Here,
            SessionId::NONE,
            old_parent,
            0,
            b"here",
            [5; NONCE_LEN],
        )
        .unwrap();
        let o = baby.open(&k, 10_000, &stale.bytes).unwrap();
        assert_eq!(o.reply_to, Some(old_parent));
        assert!(!o.confirmed);
        assert_ne!(baby.peer_session(), Some(old_parent));
        // The same session again (a new copy): no second reply.
        let again = forge(
            &k,
            Role::Parent,
            MessageClass::Here,
            SessionId::NONE,
            old_parent,
            1,
            b"here",
            [6; NONCE_LEN],
        )
        .unwrap();
        assert_eq!(baby.open(&k, 20_000, &again.bytes).unwrap().reply_to, None);
    }

    #[test]
    fn candidate_replies_are_limited_to_one_per_five_seconds() {
        let (mut baby, _, _, _, k) = confirmed_pair();
        let mk = |n: u8| {
            forge(
                &k,
                Role::Parent,
                MessageClass::Here,
                SessionId::NONE,
                SessionId::new([n; 16]),
                0,
                b"here",
                [n; NONCE_LEN],
            )
            .unwrap()
        };
        assert!(
            baby.open(&k, 10_000, &mk(0x20).bytes)
                .unwrap()
                .reply_to
                .is_some()
        );
        assert!(
            baby.open(&k, 11_000, &mk(0x21).bytes)
                .unwrap()
                .reply_to
                .is_none()
        );
        assert!(
            baby.open(&k, 15_000, &mk(0x22).bytes)
                .unwrap()
                .reply_to
                .is_some()
        );
    }

    #[test]
    fn a_corrupted_copy_first_does_not_block_the_genuine_copy() {
        let (mut baby, mut parent, mut rb, _, k) = confirmed_pair();
        let genuine = baby
            .seal_to_peer(
                &k,
                &mut rb,
                MessageClass::Description,
                b"{\"sdp\":\"offer\"}",
            )
            .unwrap();
        let mut corrupted = genuine.bytes.clone();
        let last = corrupted.len() - 1;
        corrupted[last] ^= 1;
        // The corrupted copy keeps the genuine id (it is the nonce in clear).
        assert_eq!(parent.open(&k, 5, &corrupted), Err(OpenError::BadTag));
        let o = parent.open(&k, 6, &genuine.bytes).unwrap();
        assert_eq!(o.message, b"{\"sdp\":\"offer\"}");
        assert_eq!(
            parent.open(&k, 7, &genuine.bytes),
            Err(OpenError::Duplicate)
        );
    }

    #[test]
    fn a_duplicate_through_a_second_mailbox_is_processed_once() {
        let (mut baby, mut parent, mut rb, _, k) = confirmed_pair();
        let sealed = baby
            .seal_to_peer(&k, &mut rb, MessageClass::Control, b"x")
            .unwrap();
        assert!(parent.open(&k, 5, &sealed.bytes).is_ok());
        assert_eq!(parent.open(&k, 5, &sealed.bytes), Err(OpenError::Duplicate));
    }

    #[test]
    fn seq_window_accepts_reordering_and_rejects_old_and_seen() {
        let mut w = SeqWindow::new();
        for s in [5u32, 3, 4, 300, 100] {
            // 100 is more than 256 behind 300? 300-100=200: inside the window.
            assert!(w.is_new(s), "{s}");
            w.mark(s);
            assert!(!w.is_new(s), "{s} twice");
        }
        // 300 - 44 = 256: just outside the window.
        assert!(!w.is_new(44));
        assert!(w.is_new(45));
        // A big jump clears the old bits but keeps nothing wrong.
        w.mark(10_000);
        assert!(!w.is_new(300));
        assert!(w.is_new(9_999));
    }

    #[test]
    fn the_replay_window_keeps_ids_for_thirty_minutes_and_4096_ids() {
        let mut w = ReplayWindow::new();
        for i in 0..10u32 {
            let mut id = [0u8; NONCE_LEN];
            id[..4].copy_from_slice(&i.to_be_bytes());
            w.record(id, 0);
        }
        let mut id0 = [0u8; NONCE_LEN];
        id0[..4].copy_from_slice(&0u32.to_be_bytes());
        // Old but under 4096 ids: kept.
        w.record([0xEE; NONCE_LEN], REPLAY_WINDOW_MS * 5);
        assert!(w.contains(&id0));
        // Over 4096 ids and older than 30 minutes: trimmed. Young ids stay however many.
        for i in 0..5000u32 {
            let mut id = [1u8; NONCE_LEN];
            id[..4].copy_from_slice(&i.to_be_bytes());
            w.record(id, REPLAY_WINDOW_MS * 5);
        }
        assert!(!w.contains(&id0));
        let mut recent = [1u8; NONCE_LEN];
        recent[..4].copy_from_slice(&4999u32.to_be_bytes());
        assert!(w.contains(&recent));
        let mut first_young = [1u8; NONCE_LEN];
        first_young[..4].copy_from_slice(&0u32.to_be_bytes());
        assert!(w.contains(&first_young), "young ids are kept past 4096");
    }

    #[test]
    fn the_sequence_counter_runs_out_instead_of_wrapping() {
        let k = key();
        let mut r = rng(8);
        let mut s = SessionState::new(Role::Baby, SessionId::random(&mut r));
        s.next_seq = Some(u32::MAX);
        assert!(
            s.seal(&k, &mut r, MessageClass::Here, SessionId::NONE, b"a")
                .is_ok()
        );
        assert_eq!(
            s.seal(&k, &mut r, MessageClass::Here, SessionId::NONE, b"a"),
            Err(SealError::SeqExhausted)
        );
    }

    #[test]
    fn nonces_differ_across_messages() {
        let k = key();
        let mut r = rng(9);
        let mut s = SessionState::new(Role::Baby, SessionId::random(&mut r));
        let a = s
            .seal(&k, &mut r, MessageClass::Here, SessionId::NONE, b"a")
            .unwrap();
        let b = s
            .seal(&k, &mut r, MessageClass::Here, SessionId::NONE, b"a")
            .unwrap();
        assert_ne!(a.id, b.id);
        assert_ne!(a.bytes, b.bytes);
    }

    proptest! {
        #[test]
        fn any_message_round_trips(msg in proptest::collection::vec(any::<u8>(), 0..=1024 - PLAINTEXT_HEAD_LEN)) {
            let (mut baby, mut parent, mut rb, _, k) = confirmed_pair();
            let sealed = baby.seal_to_peer(&k, &mut rb, MessageClass::Control, &msg).unwrap();
            let o = parent.open(&k, 9, &sealed.bytes).unwrap();
            prop_assert_eq!(o.message, msg);
        }

        #[test]
        fn no_envelope_is_ever_opened_twice(msgs in proptest::collection::vec(any::<u8>(), 1..8)) {
            let (mut baby, mut parent, mut rb, _, k) = confirmed_pair();
            let mut sent = Vec::new();
            for m in &msgs {
                sent.push(baby.seal_to_peer(&k, &mut rb, MessageClass::Control, &[*m]).unwrap());
            }
            for s in &sent {
                prop_assert!(parent.open(&k, 9, &s.bytes).is_ok());
            }
            for s in &sent {
                prop_assert_eq!(parent.open(&k, 10, &s.bytes), Err(OpenError::Duplicate));
            }
        }

        #[test]
        fn a_reflected_envelope_is_never_accepted(msg in proptest::collection::vec(any::<u8>(), 0..100)) {
            let (mut baby, _, mut rb, _, k) = confirmed_pair();
            let sealed = baby.seal_to_peer(&k, &mut rb, MessageClass::Control, &msg).unwrap();
            prop_assert_eq!(baby.open(&k, 9, &sealed.bytes), Err(OpenError::OwnRole));
        }

        #[test]
        fn any_bit_flip_is_rejected(idx in 0usize..(1024 + 3 + NONCE_LEN + TAG_LEN), bit in 0u8..8) {
            let (mut baby, mut parent, mut rb, _, k) = confirmed_pair();
            let sealed = baby.seal_to_peer(&k, &mut rb, MessageClass::Control, b"payload").unwrap();
            let mut bytes = sealed.bytes.clone();
            bytes[idx] ^= 1 << bit;
            prop_assert!(parent.open(&k, 9, &bytes).is_err());
            // The genuine copy is unaffected by the rejected one.
            prop_assert!(parent.open(&k, 10, &sealed.bytes).is_ok());
        }

        #[test]
        fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
            let (_, mut parent, _, _, k) = confirmed_pair();
            let _ = parent.open(&k, 9, &bytes);
        }
    }
}
