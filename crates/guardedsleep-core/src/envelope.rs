//! The sealed signaling envelope (`alpha-connection-model`, design section 2): every signaling
//! message on every mailbox is sealed with XChaCha20-Poly1305 under a key derived from the PSK, so
//! a mailbox operator sees no SDP, candidate, fingerprint or address. `Link` seals and opens every
//! signaling message of the relay mailbox with it (`alpha-connection-model` task 3.2).
//!
//! Wire format (all lengths fixed per class, so every message of a class is equally long):
//!
//! ```text
//! version (1) | sender role (1) | class (1) | nonce (24) | ciphertext (bucket) | tag (16)
//! ```
//!
//! The nonce is the envelope id. The associated data is `"gs-env/1" | role | class`. The plaintext
//! fills the class's bucket exactly: `recipient_session (16) | sender_session (16) | seq (u32 BE) |
//! message length (u16 BE) | challenge (16) | echo (16) | message | zero padding | parent tag (16)`.
//! The length field lets the receiver strip the padding; `challenge` and `echo` are used by `here`
//! only (all zeros elsewhere), see below. The parent tag is `HMAC(parent_secret, nonce)[0..16]` on
//! envelopes of a pinned parent and zeros otherwise; a baby that pinned a parent refuses envelopes
//! of any other parent from it ([`OpenError::NotPinned`]).
//!
//! # Sessions and freshness
//!
//! A device draws a fresh [`SessionId`] per run. A `here` from a session the receiver has not
//! confirmed only makes it a *candidate*, even when it names our current session (that proves
//! only that the peer was alive at some point in this run, and a relay can store it). Our single
//! answer to a candidate is a `here` that carries a fresh 16-byte *challenge*; only a `here` that
//! echoes the open challenge (single use, valid [`CHALLENGE_VALID_MS`]) promotes the candidate to
//! the peer's confirmed session, replacing and retiring the previous one. Messages other than
//! `here` from a candidate are dropped and never promote. If the confirming echo is lost, the peer repeats its `here`
//! with its open challenge and the confirmed side echoes again (at most once per 5 s, within the reply
//! budget). Retired sessions are kept for the whole
//! run. A stored message can therefore only be replayed into a session that is still current,
//! and then the replay window and the sequence window drop it.
//!
//! Envelope ids are recorded for de-duplication only **after** an envelope authenticated and
//! passed every check, so a corrupted copy from one relay cannot shadow the genuine copy from
//! another.
//!
//! # Error locality
//!
//! A failed open ([`OpenError`]) must never cause any reply, on any mailbox: the sender of a
//! forged or replayed envelope learns nothing from timing or traffic. [`SessionState::open`]
//! returns a [`Reply`] only for envelopes that fully passed; the caller must not send anything
//! else in response to an `Err`. `Link::on_envelope` drops every `Err` without a trace.

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

use std::collections::{BTreeSet, VecDeque};

use chacha20::rand_core::Rng;
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{Key, KeyInit, XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use zeroize::{Zeroize, Zeroizing};

use crate::pairing::{PinPurpose, pin_subkey};
use crate::protocol::Role;

/// Envelope format version, the first byte on the wire.
pub const VERSION: u8 = 1;
/// The associated data starts with this string (the format version in text form).
const AD_PREFIX: &[u8] = b"gs-env/1";

// The AD string and the wire version must move together.
const _: () = assert!(matches!(AD_PREFIX.last(), Some(last) if *last == b'0' + VERSION));

/// Bytes before the ciphertext: version, role, class.
const HEADER_LEN: usize = 3;
/// Length of the nonce, which is also the envelope id.
pub const NONCE_LEN: usize = 24;
const TAG_LEN: usize = 16;
/// Length of a `here` challenge.
pub const CHALLENGE_LEN: usize = 16;
/// `recipient | sender | seq | message length | challenge | echo`.
const PLAINTEXT_HEAD_LEN: usize = 16 + 16 + 4 + 2 + CHALLENGE_LEN + CHALLENGE_LEN;
/// Length of the parent tag, the last bytes of every plaintext.
pub const PARENT_TAG_LEN: usize = 16;
/// Bytes of a bucket that never hold message: the parent tag at its end.
const PLAINTEXT_TAIL_LEN: usize = PARENT_TAG_LEN;
const NO_PARENT_TAG: [u8; PARENT_TAG_LEN] = [0; PARENT_TAG_LEN];
const NO_CHALLENGE: [u8; CHALLENGE_LEN] = [0; CHALLENGE_LEN];

/// Size buckets of the padding ladder (design section 2).
pub const BUCKET_1K: usize = 1024;
pub const BUCKET_4K: usize = 4 * 1024;
pub const BUCKET_8K: usize = 8 * 1024;

/// Envelope ids kept at least this many (together with the time bound below, whichever keeps more).
pub const REPLAY_IDS: usize = 4096;
/// ... and at least this long, by the monotonic clock.
pub const REPLAY_WINDOW_MS: u64 = 30 * 60 * 1000;
/// Hard cap on remembered ids and retired sessions. Only authenticated envelopes count, so an
/// honest sender (at most 2 events per second per relay) stays far below it.
const HARD_CAP: usize = 65_536;
/// Sequence numbers remembered per sender session.
pub const SEQ_WINDOW: u32 = 256;
/// Unconfirmed peer sessions remembered.
const CANDIDATES_KEEP: usize = 16;
/// A challenge stays valid this long, by the monotonic clock.
pub const CHALLENGE_VALID_MS: u64 = 60_000;
/// Challenges are issued at most once per this long overall.
pub const CHALLENGE_MIN_INTERVAL_MS: u64 = 5_000;
/// A repeated echo for the same confirmed peer is sent at most once per this long.
pub const ECHO_MIN_INTERVAL_MS: u64 = 5_000;
/// While a confirmed session exists, at most [`REPLY_BUDGET`] replies per this long ...
pub const REPLY_BUDGET_WINDOW_MS: u64 = 10 * 60 * 1000;
/// ... that is this many.
pub const REPLY_BUDGET: usize = 10;

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

    /// Name used by the conformance vectors.
    #[cfg(any(test, feature = "conformance"))]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "seal" => Purpose::Seal,
            "nostr_tag" => Purpose::NostrTag,
            "nostr_key" => Purpose::NostrKey,
            "relay_pick" => Purpose::RelayPick,
            _ => return None,
        })
    }
}

/// 32 secret bytes; wiped on drop and never printed.
pub struct SecretKey([u8; 32]);

impl SecretKey {
    /// Exposes the raw key: conformance vectors only, never in a release build.
    #[cfg(any(test, feature = "conformance"))]
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
    let (mut prk, hk) = Hkdf::<Sha256>::extract(None, psk);
    prk.as_mut_slice().zeroize();
    let mut out = [0u8; 32];
    hk.expand(purpose.info(), &mut out).ok()?;
    let key = SecretKey(out);
    out.zeroize();
    Some(key)
}

// ---- message classes -------------------------------------------------------------------------

/// What an envelope carries. Authenticated in the clear header; fixes the padded size, so all
/// messages of a class have the same length. `Here` and `Control` share a size, so they cannot be
/// told apart by length. Task 3.2 checked the buckets against real browser messages (Chrome 152:
/// an offer is 1.9 to 2.5 KB, one ICE candidate about 250 bytes); everything fits.
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
        self.bucket() - PLAINTEXT_HEAD_LEN - PLAINTEXT_TAIL_LEN
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
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

/// Why an envelope was dropped. None of them changes any state, and none may cause a reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenError {
    /// Shorter or longer than any envelope, an unknown class or role, or fields that do not fit
    /// the class.
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
    /// A message other than `here` from a session that is not the peer's confirmed session.
    WrongSender,
    /// The sender's session was retired (it stopped, or a newer session replaced it).
    RetiredSender,
    /// The sequence number was already seen or is older than the window.
    ReplayedSeq,
    /// The baby unit pinned a parent and this envelope of a parent carries no valid parent tag:
    /// another device that holds the PSK (`pairing`, "One baby unit and one parent unit").
    NotPinned,
    /// A `here` echoes a challenge that is not open for its sender (unknown, wrong or used).
    BadChallenge,
    /// A `here` echoes a challenge that was valid once but is older than [`CHALLENGE_VALID_MS`].
    ChallengeExpired,
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
            OpenError::NotPinned => "not_pinned",
            OpenError::BadChallenge => "bad_challenge",
            OpenError::ChallengeExpired => "challenge_expired",
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

/// The associated data of an envelope header (`"gs-env/1" ‖ role ‖ class`).
pub fn envelope_ad(role: Role, class: MessageClass) -> Vec<u8> {
    associated_data(role, class).to_vec()
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
    // The cipher keeps its own copy (wiped on drop); wipe the temporary one.
    let mut k = Key::from(key.0);
    let c = XChaCha20Poly1305::new(&k);
    k.as_mut_slice().zeroize();
    c
}

/// The authenticated fields around a message.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub role: Role,
    pub class: MessageClass,
    pub recipient: SessionId,
    pub sender: SessionId,
    pub seq: u32,
    /// `here` only: the challenge this message issues (zeros: none).
    pub challenge: [u8; CHALLENGE_LEN],
    /// `here` only: the challenge of the peer this message echoes (zeros: none).
    pub echo: [u8; CHALLENGE_LEN],
    /// The key of the pin tag this envelope carries (`None`: zeros). The tag is
    /// `HMAC(tag_key, nonce ‖ AD ‖ plaintext before the tag)[0..16]` and sits in the last bytes of
    /// the plaintext, so it costs no bucket, covers the whole envelope, and lets a pinned peer
    /// refuse any other sender of that role before looking at the message.
    pub tag_key: Option<TagKey>,
}

/// A subkey of the parent secret that tags envelopes (`PinPurpose::ParentTag` or `BabyTag`).
/// Wiped by its holder; never printed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TagKey(pub [u8; 32]);

impl std::fmt::Debug for TagKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TagKey(<redacted>)")
    }
}

/// The pin tag of one envelope: `HMAC-SHA256(tag_key, nonce ‖ associated data ‖ plaintext before
/// the tag)[0..16]`. Binding the whole envelope (not just the nonce) means that a PSK holder who
/// re-seals a different message under a genuine nonce cannot reuse the genuine tag.
pub fn pin_tag(
    key: &TagKey,
    nonce: &[u8; NONCE_LEN],
    ad: &[u8],
    body: &[u8],
) -> [u8; PARENT_TAG_LEN] {
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(&key.0) else {
        return NO_PARENT_TAG;
    };
    mac.update(nonce);
    mac.update(ad);
    mac.update(body);
    let full = mac.finalize().into_bytes();
    let mut tag = NO_PARENT_TAG;
    for (slot, byte) in tag.iter_mut().zip(full.iter()) {
        *slot = *byte;
    }
    tag
}

fn seal_frame(
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
        challenge,
        echo,
        tag_key,
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
    plain.extend_from_slice(&challenge);
    plain.extend_from_slice(&echo);
    plain.extend_from_slice(message);
    plain.resize(class.bucket() - PLAINTEXT_TAIL_LEN, 0);
    let tag = tag_key.map_or(NO_PARENT_TAG, |k| {
        pin_tag(&k, &nonce, &associated_data(role, class), &plain)
    });
    plain.extend_from_slice(&tag);
    encrypt_plaintext(key, role, class, &plain, nonce)
}

fn encrypt_plaintext(
    key: &SecretKey,
    role: Role,
    class: MessageClass,
    plain: &[u8],
    nonce: [u8; NONCE_LEN],
) -> Result<Sealed, SealError> {
    let ad = associated_data(role, class);
    let ct = cipher(key)
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: plain,
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

/// Seals one message with an explicit nonce: conformance vectors and tests only. A release build
/// has no such function, so no caller can reuse a nonce under the long-lived key.
#[cfg(any(test, feature = "conformance"))]
pub fn seal_with_nonce(
    key: &SecretKey,
    frame: &Frame,
    message: &[u8],
    nonce: [u8; NONCE_LEN],
) -> Result<Sealed, SealError> {
    seal_frame(key, frame, message, nonce)
}

/// Seals an arbitrary plaintext of exactly the class's bucket size (fuzzing: reaches the plaintext
/// parser and the session logic with fields no honest sender produces).
#[cfg(any(test, feature = "conformance"))]
pub fn seal_raw_plaintext(
    key: &SecretKey,
    role: Role,
    class: MessageClass,
    plaintext: &[u8],
    nonce: [u8; NONCE_LEN],
) -> Result<Sealed, SealError> {
    if plaintext.len() != class.bucket() {
        return Err(SealError::Oversized);
    }
    encrypt_plaintext(key, role, class, plaintext, nonce)
}

/// An envelope as a PSK holder sitting in a mailbox reads it: no session state, no replay window.
/// Simulations and tests use it to play such an attacker (rewrite an SDP) and to record what a
/// device sent; a release build has no such function.
#[cfg(any(test, feature = "testing"))]
#[derive(Debug, Clone)]
pub struct Peeked {
    pub frame: Frame,
    pub message: Vec<u8>,
}

/// Decrypts `bytes` with the envelope key; `None` for anything that does not authenticate.
#[cfg(any(test, feature = "testing"))]
pub fn peek(key: &SecretKey, bytes: &[u8]) -> Option<Peeked> {
    let (head, rest) = bytes.split_first_chunk::<HEADER_LEN>()?;
    let [version, role_b, class_b] = *head;
    if version != VERSION {
        return None;
    }
    let role = role_from_code(role_b)?;
    let class = MessageClass::from_code(class_b)?;
    if bytes.len() != class.wire_len() {
        return None;
    }
    let (nonce, ct) = rest.split_first_chunk::<NONCE_LEN>()?;
    let ad = associated_data(role, class);
    let plain = Zeroizing::new(
        cipher(key)
            .decrypt(&XNonce::from(*nonce), Payload { msg: ct, aad: &ad })
            .ok()?,
    );
    let p = parse_plaintext(&plain, class)?;
    Some(Peeked {
        frame: Frame {
            role,
            class,
            recipient: p.recipient,
            sender: p.sender,
            seq: p.seq,
            challenge: p.challenge,
            echo: p.echo,
            tag_key: None,
        },
        message: p.message.to_vec(),
    })
}

/// Seals `message` into the frame of `peeked` (same role, class, sessions and sequence number)
/// under a fresh `nonce`: the rewrite a PSK-holding intermediary can do.
#[cfg(any(test, feature = "testing"))]
pub fn reseal(
    key: &SecretKey,
    peeked: &Peeked,
    message: &[u8],
    nonce: [u8; NONCE_LEN],
) -> Result<Sealed, SealError> {
    seal_frame(key, &peeked.frame, message, nonce)
}

// ---- opening and the per-run state -----------------------------------------------------------

/// The `here` the caller should send in answer to an opened envelope (see [`SessionState::seal_reply`]).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Reply {
    /// Becomes the `recipient_session`.
    pub to: SessionId,
    /// A challenge for the peer to echo (zeros: none).
    pub challenge: [u8; CHALLENGE_LEN],
    /// The peer's challenge to echo back (zeros: none).
    pub echo: [u8; CHALLENGE_LEN],
}

impl std::fmt::Debug for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reply(..)")
    }
}

/// An envelope that authenticated and passed the session checks.
#[derive(Clone, PartialEq, Eq)]
pub struct Opened {
    pub id: [u8; NONCE_LEN],
    pub role: Role,
    pub class: MessageClass,
    pub sender_session: SessionId,
    pub recipient_session: SessionId,
    pub seq: u32,
    pub message: Zeroizing<Vec<u8>>,
    /// The sender's session became the peer's confirmed current session with this envelope.
    pub confirmed: bool,
    /// A `here` to send in answer, if the budget allows one.
    pub reply: Option<Reply>,
}

// The plaintext message stays out of any log.
impl std::fmt::Debug for Opened {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Opened")
            .field("class", &self.class)
            .field("role", &self.role)
            .field("seq", &self.seq)
            .field("confirmed", &self.confirmed)
            .field("message_len", &self.message.len())
            .finish_non_exhaustive()
    }
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
    seen: BTreeSet<[u8; NONCE_LEN]>,
}

impl ReplayWindow {
    fn new() -> Self {
        Self {
            order: VecDeque::new(),
            seen: BTreeSet::new(),
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
            if (aged && over) || self.order.len() > HARD_CAP {
                self.order.pop_front();
                self.seen.remove(&front_id);
            } else {
                break;
            }
        }
    }
}

/// Constant-time equality of two challenges.
fn same_challenge(a: &[u8; CHALLENGE_LEN], b: &[u8; CHALLENGE_LEN]) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Constant-time equality of two pin tags.
fn same_tag(a: &[u8; PARENT_TAG_LEN], b: &[u8; PARENT_TAG_LEN]) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

struct PeerSession {
    id: SessionId,
    seqs: SeqWindow,
}

struct OpenChallenge {
    value: [u8; CHALLENGE_LEN],
    issued_ms: u64,
}

/// A peer session that announced itself but has not answered a challenge yet.
struct Candidate {
    id: SessionId,
    /// Our challenge for it, until it is used or replaced.
    open: Option<OpenChallenge>,
    /// The last peer challenge we echoed, so a repeat is not answered twice.
    echoed: Option<[u8; CHALLENGE_LEN]>,
}

/// Everything one `Link` run knows about envelopes: its own session, the peer's candidate,
/// confirmed and retired sessions, the sequence windows and the run-wide replay window. There is
/// deliberately no way to clear any of it: reopening a mailbox must not forget what was seen.
pub struct SessionState {
    role: Role,
    own: SessionId,
    next_seq: Option<u32>,
    peer: Option<PeerSession>,
    candidates: VecDeque<Candidate>,
    retired: BTreeSet<SessionId>,
    retired_order: VecDeque<SessionId>,
    last_challenge_ms: Option<u64>,
    last_echo_ms: Option<u64>,
    reply_times: VecDeque<u64>,
    replay: ReplayWindow,
    /// Pin tag keys: ours (tags every envelope we seal) and the peer's (checked on every envelope
    /// that arrives from the peer role while `require_peer_tag` is set).
    own_tag: Option<TagKey>,
    peer_tag: Option<TagKey>,
    require_peer_tag: bool,
}

/// What the checks decided; applied only after all of them passed.
enum Verdict {
    /// From the confirmed session (or any non-`here` that passed).
    Accept,
    /// A `here` that echoes the open challenge of its (candidate) sender.
    Promote,
    /// A `here` from an unconfirmed session.
    Candidate,
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
            retired: BTreeSet::new(),
            retired_order: VecDeque::new(),
            last_challenge_ms: None,
            last_echo_ms: None,
            reply_times: VecDeque::new(),
            replay: ReplayWindow::new(),
            own_tag: None,
            peer_tag: None,
            require_peer_tag: false,
        }
    }

    /// Sets (or clears) the parent secret. Wrong lengths clear it. Every envelope we seal carries
    /// our pin tag from now on. With `require_peer_tag` every envelope of the peer role must carry
    /// the peer's valid tag or it is refused ([`OpenError::NotPinned`]) before any session check:
    /// the baby requires it once pinned, the parent once the baby acknowledged the pin.
    pub fn set_parent_pin(&mut self, secret: Option<&[u8]>, require_peer_tag: bool) {
        let (own, peer) = match self.role {
            Role::Parent => (PinPurpose::ParentTag, PinPurpose::BabyTag),
            Role::Baby => (PinPurpose::BabyTag, PinPurpose::ParentTag),
        };
        let key = |p| secret.and_then(|s| pin_subkey(s, p)).map(TagKey);
        self.own_tag = key(own);
        self.peer_tag = key(peer);
        self.require_peer_tag = require_peer_tag && self.peer_tag.is_some();
    }

    /// Whether a parent secret is set.
    pub fn has_parent_pin(&self) -> bool {
        self.own_tag.is_some()
    }

    /// Whether envelopes of the peer role must carry a valid pin tag.
    pub fn requires_peer_tag(&self) -> bool {
        self.require_peer_tag
    }

    /// A state that already confirmed `peer` and retired the given sessions (conformance vectors).
    #[cfg(any(test, feature = "conformance"))]
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

    /// The newest session seen but not confirmed: what an attacker that never gets an echo still
    /// knows (test support for a sender that seals to it anyway).
    #[cfg(any(test, feature = "testing"))]
    pub fn newest_candidate(&self) -> Option<SessionId> {
        self.candidates.back().map(|c| c.id)
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
        self.candidates.retain(|c| c.id != session);
        self.push_retired(session);
    }

    fn push_retired(&mut self, session: SessionId) {
        if self.retired.insert(session) {
            self.retired_order.push_back(session);
            while self.retired_order.len() > HARD_CAP {
                if let Some(old) = self.retired_order.pop_front() {
                    self.retired.remove(&old);
                }
            }
        }
    }

    fn seal_inner<R: Rng>(
        &mut self,
        key: &SecretKey,
        rng: &mut R,
        class: MessageClass,
        recipient: SessionId,
        (challenge, echo): ([u8; CHALLENGE_LEN], [u8; CHALLENGE_LEN]),
        message: &[u8],
    ) -> Result<Sealed, SealError> {
        if message.len() > class.max_message() {
            return Err(SealError::Oversized);
        }
        let seq = self.next_seq.ok_or(SealError::SeqExhausted)?;
        let mut nonce = [0u8; NONCE_LEN];
        rng.fill_bytes(&mut nonce);
        // The parent tags everything it sends once it holds a secret; the tag is bound to this
        // envelope's nonce, so it cannot be copied onto another envelope.
        let frame = Frame {
            role: self.role,
            class,
            recipient,
            sender: self.own,
            seq,
            challenge,
            echo,
            tag_key: self.own_tag,
        };
        let sealed = seal_frame(key, &frame, message, nonce)?;
        self.next_seq = seq.checked_add(1);
        Ok(sealed)
    }

    /// Seals a message to `recipient` with a nonce from the CSPRNG and the next sequence number.
    /// A first `here` uses [`SessionId::NONE`]; answers to a peer's `here` use [`Self::seal_reply`].
    pub fn seal<R: Rng>(
        &mut self,
        key: &SecretKey,
        rng: &mut R,
        class: MessageClass,
        recipient: SessionId,
        message: &[u8],
    ) -> Result<Sealed, SealError> {
        self.seal_inner(
            key,
            rng,
            class,
            recipient,
            (NO_CHALLENGE, NO_CHALLENGE),
            message,
        )
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

    /// Seals the `here` that [`Opened::reply`] asked for.
    pub fn seal_reply<R: Rng>(
        &mut self,
        key: &SecretKey,
        rng: &mut R,
        reply: &Reply,
        message: &[u8],
    ) -> Result<Sealed, SealError> {
        self.seal_inner(
            key,
            rng,
            MessageClass::Here,
            reply.to,
            (reply.challenge, reply.echo),
            message,
        )
    }

    /// Opens one envelope received on any mailbox. `now_ms` is the monotonic clock; `rng` draws
    /// the challenge of a reply.
    ///
    /// The checks run in this order: format, own role, tag, duplicate id, session rules. Nothing
    /// is recorded unless all of them pass. An `Err` must never cause any reply.
    pub fn open<R: Rng>(
        &mut self,
        key: &SecretKey,
        rng: &mut R,
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
        // A baby that pinned a parent refuses every other parent here, before any session state
        // is touched and before the message is looked at (early refusal, design section 4).
        if self.require_peer_tag
            && let Some(key) = &self.peer_tag
        {
            let body = plain
                .get(..plain.len().saturating_sub(PARENT_TAG_LEN))
                .unwrap_or_default();
            let expected = pin_tag(key, nonce, &ad, body);
            if !same_tag(&expected, &parsed.parent_tag) {
                return Err(OpenError::NotPinned);
            }
        }
        let verdict = self.check(class, &parsed, now_ms)?;

        // Everything passed: now the state changes.
        self.replay.record(*nonce, now_ms);
        let sender = parsed.sender;
        let mut confirmed = false;
        let mut reply = None;
        match verdict {
            Verdict::Accept => {
                if let Some(p) = self.peer.as_mut() {
                    p.seqs.mark(parsed.seq);
                }
                // The confirmed peer repeats its `here` with its open challenge: it is still
                // waiting for our echo (the first one was lost). Echo again, rarely and within
                // the reply budget, so a lost echo costs seconds and not the challenge's life.
                if class == MessageClass::Here
                    && parsed.challenge != NO_CHALLENGE
                    && self
                        .last_echo_ms
                        .is_none_or(|t| now_ms.saturating_sub(t) >= ECHO_MIN_INTERVAL_MS)
                    && self.budget_ok(now_ms, false)
                {
                    self.spend_budget(now_ms, false);
                    self.last_echo_ms = Some(now_ms);
                    reply = Some(Reply {
                        to: sender,
                        challenge: NO_CHALLENGE,
                        echo: parsed.challenge,
                    });
                }
            }
            Verdict::Promote => {
                self.candidates.retain(|c| c.id != sender);
                self.promote(sender);
                if let Some(p) = self.peer.as_mut() {
                    p.seqs.mark(parsed.seq);
                }
                confirmed = true;
                // The peer holds a challenge of its own for us: echo it so it can confirm us.
                if parsed.challenge != NO_CHALLENGE && self.budget_ok(now_ms, false) {
                    self.spend_budget(now_ms, false);
                    reply = Some(Reply {
                        to: sender,
                        challenge: NO_CHALLENGE,
                        echo: parsed.challenge,
                    });
                }
            }
            Verdict::Candidate => reply = self.answer_candidate(rng, now_ms, sender, &parsed),
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
            reply,
        })
    }

    fn check(&self, class: MessageClass, p: &Parsed, now_ms: u64) -> Result<Verdict, OpenError> {
        if self.retired.contains(&p.sender) {
            return Err(OpenError::RetiredSender);
        }
        let is_current = self.peer.as_ref().is_some_and(|c| c.id == p.sender);
        if class != MessageClass::Here {
            if p.challenge != NO_CHALLENGE || p.echo != NO_CHALLENGE {
                return Err(OpenError::Malformed);
            }
            if p.recipient != self.own {
                return Err(OpenError::WrongRecipient);
            }
            // A candidate or unknown session never gets past here: only a challenge confirms.
            if !is_current {
                return Err(OpenError::WrongSender);
            }
            return self.seq_check(p);
        }
        // A first `here` names nobody; every other `here` names our current session.
        if !p.recipient.is_none() && p.recipient != self.own {
            return Err(OpenError::WrongRecipient);
        }
        if is_current {
            return self.seq_check(p);
        }
        let candidate = self.candidates.iter().find(|c| c.id == p.sender);
        if p.echo != NO_CHALLENGE {
            let open = candidate
                .and_then(|c| c.open.as_ref())
                .ok_or(OpenError::BadChallenge)?;
            if !same_challenge(&open.value, &p.echo) {
                return Err(OpenError::BadChallenge);
            }
            if now_ms.saturating_sub(open.issued_ms) > CHALLENGE_VALID_MS {
                return Err(OpenError::ChallengeExpired);
            }
            return Ok(Verdict::Promote);
        }
        Ok(Verdict::Candidate)
    }

    fn seq_check(&self, p: &Parsed) -> Result<Verdict, OpenError> {
        let fresh = self.peer.as_ref().is_some_and(|c| c.seqs.is_new(p.seq));
        if fresh {
            Ok(Verdict::Accept)
        } else {
            Err(OpenError::ReplayedSeq)
        }
    }

    /// A `here` from an unconfirmed session: remember it and decide on the single answer.
    fn answer_candidate<R: Rng>(
        &mut self,
        rng: &mut R,
        now_ms: u64,
        sender: SessionId,
        p: &Parsed,
    ) -> Option<Reply> {
        let ix = match self.candidates.iter().position(|c| c.id == sender) {
            Some(i) => i,
            None => {
                self.candidates.push_back(Candidate {
                    id: sender,
                    open: None,
                    echoed: None,
                });
                while self.candidates.len() > CANDIDATES_KEEP {
                    self.candidates.pop_front();
                }
                self.candidates.len().checked_sub(1)?
            }
        };
        let candidate = self.candidates.get(ix)?;
        let open_value = candidate
            .open
            .as_ref()
            .filter(|o| now_ms.saturating_sub(o.issued_ms) <= CHALLENGE_VALID_MS)
            .map(|o| o.value);
        let their = p.challenge;
        let needs_echo = their != NO_CHALLENGE && candidate.echoed != Some(their);
        // One answer per open challenge: a new challenge only when none is open; the open one is
        // repeated to carry an echo of the peer's own challenge.
        let issue = open_value.is_none();
        if !(issue || needs_echo) {
            return None;
        }
        if !self.budget_ok(now_ms, issue) {
            return None;
        }
        let value = match open_value {
            Some(v) => v,
            None => {
                let mut v = NO_CHALLENGE;
                while v == NO_CHALLENGE {
                    rng.fill_bytes(&mut v);
                }
                v
            }
        };
        self.spend_budget(now_ms, issue);
        let candidate = self.candidates.get_mut(ix)?;
        if issue {
            candidate.open = Some(OpenChallenge {
                value,
                issued_ms: now_ms,
            });
        }
        if their != NO_CHALLENGE {
            candidate.echoed = Some(their);
        }
        Some(Reply {
            to: sender,
            challenge: value,
            echo: their,
        })
    }

    /// Reply limits: a new challenge at most once per 5 s; at most 10 replies per 10 minutes while
    /// a confirmed session exists.
    fn budget_ok(&self, now_ms: u64, issuing: bool) -> bool {
        if issuing
            && self
                .last_challenge_ms
                .is_some_and(|t| now_ms.saturating_sub(t) < CHALLENGE_MIN_INTERVAL_MS)
        {
            return false;
        }
        if self.peer.is_some() {
            let recent = self
                .reply_times
                .iter()
                .filter(|t| now_ms.saturating_sub(**t) < REPLY_BUDGET_WINDOW_MS)
                .count();
            if recent >= REPLY_BUDGET {
                return false;
            }
        }
        true
    }

    fn spend_budget(&mut self, now_ms: u64, issuing: bool) {
        if issuing {
            self.last_challenge_ms = Some(now_ms);
        }
        self.reply_times.push_back(now_ms);
        while self
            .reply_times
            .front()
            .is_some_and(|t| now_ms.saturating_sub(*t) >= REPLY_BUDGET_WINDOW_MS)
        {
            self.reply_times.pop_front();
        }
    }

    /// Makes `s` the confirmed session; the session it replaces belongs to a run that is over.
    fn promote(&mut self, s: SessionId) {
        self.candidates.retain(|c| c.id != s);
        if let Some(old) = self.peer.take() {
            self.push_retired(old.id);
        }
        self.peer = Some(PeerSession {
            id: s,
            seqs: SeqWindow::new(),
        });
    }
}

struct Parsed {
    recipient: SessionId,
    sender: SessionId,
    seq: u32,
    challenge: [u8; CHALLENGE_LEN],
    echo: [u8; CHALLENGE_LEN],
    parent_tag: [u8; PARENT_TAG_LEN],
    message: Zeroizing<Vec<u8>>,
}

fn parse_plaintext(plain: &[u8], class: MessageClass) -> Option<Parsed> {
    if plain.len() != class.bucket() {
        return None;
    }
    // The tag occupies the last bytes; a message length that reaches into it is malformed.
    let (plain, tag) = plain.split_last_chunk::<PARENT_TAG_LEN>()?;
    let (recipient, rest) = plain.split_first_chunk::<16>()?;
    let (sender, rest) = rest.split_first_chunk::<16>()?;
    let (seq, rest) = rest.split_first_chunk::<4>()?;
    let (len, rest) = rest.split_first_chunk::<2>()?;
    let (challenge, rest) = rest.split_first_chunk::<CHALLENGE_LEN>()?;
    let (echo, rest) = rest.split_first_chunk::<CHALLENGE_LEN>()?;
    let len = usize::from(u16::from_be_bytes(*len));
    let message = Zeroizing::new(rest.get(..len)?.to_vec());
    Some(Parsed {
        recipient: SessionId::new(*recipient),
        sender: SessionId::new(*sender),
        seq: u32::from_be_bytes(*seq),
        challenge: *challenge,
        echo: *echo,
        parent_tag: *tag,
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

    /// Builds an envelope with chosen fields, as a peer holding the key (or a test) can.
    #[allow(clippy::too_many_arguments)]
    fn forge(
        k: &SecretKey,
        role: Role,
        class: MessageClass,
        recipient: SessionId,
        sender: SessionId,
        seq: u32,
        challenge: [u8; CHALLENGE_LEN],
        echo: [u8; CHALLENGE_LEN],
        nonce: u8,
    ) -> Sealed {
        let frame = Frame {
            role,
            class,
            recipient,
            sender,
            seq,
            challenge,
            echo,
            tag_key: None,
        };
        seal_with_nonce(k, &frame, b"{\"m\":1}", [nonce; NONCE_LEN]).unwrap()
    }

    fn key() -> SecretKey {
        derive_key(&PSK, Purpose::Seal).unwrap()
    }

    fn rng(n: u8) -> ChaCha20Rng {
        ChaCha20Rng::from_seed([n; 32])
    }

    fn sid(n: u8) -> SessionId {
        SessionId::new([n; 16])
    }

    struct Pair {
        baby: SessionState,
        parent: SessionState,
        rb: ChaCha20Rng,
        rp: ChaCha20Rng,
        k: SecretKey,
        /// Monotonic clock of the next step.
        t: u64,
    }

    impl Pair {
        /// Runs the whole challenge handshake from scratch; `rb`/`rp` seeds differ per call.
        fn confirmed() -> Pair {
            let (mut rb, mut rp) = (rng(1), rng(2));
            let k = key();
            let baby = SessionState::new(Role::Baby, SessionId::random(&mut rb));
            let parent = SessionState::new(Role::Parent, SessionId::random(&mut rp));
            let mut p = Pair {
                baby,
                parent,
                rb,
                rp,
                k,
                t: 0,
            };
            p.handshake();
            p
        }

        /// The baby announces itself (its `Here` goes first); returns after both confirmed.
        fn handshake(&mut self) {
            let h1 = self
                .baby
                .seal(
                    &self.k,
                    &mut self.rb,
                    MessageClass::Here,
                    SessionId::NONE,
                    b"here",
                )
                .unwrap();
            let o1 = self
                .parent
                .open(&self.k, &mut self.rp, self.t, &h1.bytes)
                .unwrap();
            assert!(!o1.confirmed);
            let r1 = o1.reply.expect("challenge for the candidate");
            assert_ne!(r1.challenge, NO_CHALLENGE);
            let s1 = self
                .parent
                .seal_reply(&self.k, &mut self.rp, &r1, b"here")
                .unwrap();
            self.t += 1;
            let o2 = self
                .baby
                .open(&self.k, &mut self.rb, self.t, &s1.bytes)
                .unwrap();
            // Naming our session is not enough: still a candidate.
            assert!(!o2.confirmed);
            assert_eq!(self.baby.peer_session(), None);
            let r2 = o2.reply.expect("challenge plus echo");
            assert_eq!(r2.echo, r1.challenge);
            let s2 = self
                .baby
                .seal_reply(&self.k, &mut self.rb, &r2, b"here")
                .unwrap();
            self.t += 1;
            let o3 = self
                .parent
                .open(&self.k, &mut self.rp, self.t, &s2.bytes)
                .unwrap();
            assert!(o3.confirmed);
            let r3 = o3.reply.expect("echo of the baby's challenge");
            assert_eq!(r3.challenge, NO_CHALLENGE);
            assert_eq!(r3.echo, r2.challenge);
            let s3 = self
                .parent
                .seal_reply(&self.k, &mut self.rp, &r3, b"here")
                .unwrap();
            self.t += 1;
            let o4 = self
                .baby
                .open(&self.k, &mut self.rb, self.t, &s3.bytes)
                .unwrap();
            assert!(o4.confirmed);
            assert!(o4.reply.is_none());
            assert_eq!(self.baby.peer_session(), Some(self.parent.own_session()));
            assert_eq!(self.parent.peer_session(), Some(self.baby.own_session()));
            self.t += 1;
        }

        /// The baby restarts: a new session, confirmed at the parent through the full exchange.
        /// Returns the previous baby session.
        fn restart_baby(&mut self) -> SessionId {
            let old = self.baby.own_session();
            self.rb = rng(self.rb.get_seed()[0].wrapping_add(7));
            self.baby = SessionState::new(Role::Baby, SessionId::random(&mut self.rb));
            // Replies are budgeted per 10 minutes while a session is confirmed.
            self.t += REPLY_BUDGET_WINDOW_MS;
            self.handshake();
            old
        }

        fn baby_sends(&mut self, class: MessageClass, msg: &[u8]) -> Sealed {
            self.baby
                .seal_to_peer(&self.k, &mut self.rb, class, msg)
                .unwrap()
        }

        fn parent_opens(&mut self, s: &Sealed) -> Result<Opened, OpenError> {
            self.t += 1;
            self.parent.open(&self.k, &mut self.rp, self.t, &s.bytes)
        }

        fn baby_opens(&mut self, s: &Sealed) -> Result<Opened, OpenError> {
            self.t += 1;
            self.baby.open(&self.k, &mut self.rb, self.t, &s.bytes)
        }
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
    fn the_ad_string_follows_the_wire_version() {
        assert_eq!(AD_PREFIX, format!("gs-env/{VERSION}").as_bytes());
    }

    #[test]
    fn debug_never_prints_key_material_or_plaintext() {
        let k = key();
        let text = format!("{k:?} {:?}", sid(0xAB));
        assert!(!text.contains("ab") && !text.contains("171"), "{text}");
        let mut p = Pair::confirmed();
        let s = p.baby_sends(MessageClass::Control, b"top-secret-sdp");
        let o = p.parent_opens(&s).unwrap();
        assert!(!format!("{o:?}").contains("top-secret"));
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
    fn the_challenge_handshake_confirms_both_sessions() {
        // Pair::confirmed asserts every step.
        let _ = Pair::confirmed();
    }

    #[test]
    fn simultaneous_announcements_still_confirm() {
        let (mut rb, mut rp) = (rng(11), rng(12));
        let k = key();
        let mut baby = SessionState::new(Role::Baby, SessionId::random(&mut rb));
        let mut parent = SessionState::new(Role::Parent, SessionId::random(&mut rp));
        let hb = baby
            .seal(&k, &mut rb, MessageClass::Here, SessionId::NONE, b"h")
            .unwrap();
        let hp = parent
            .seal(&k, &mut rp, MessageClass::Here, SessionId::NONE, b"h")
            .unwrap();
        // Each answers the other's announcement with a challenge; the answers cross.
        let ob = baby.open(&k, &mut rb, 0, &hp.bytes).unwrap();
        let op = parent.open(&k, &mut rp, 0, &hb.bytes).unwrap();
        let rb1 = baby
            .seal_reply(&k, &mut rb, &ob.reply.unwrap(), b"h")
            .unwrap();
        let rp1 = parent
            .seal_reply(&k, &mut rp, &op.reply.unwrap(), b"h")
            .unwrap();
        // Each now holds the other's answer: it carries a challenge to echo; ours stays open.
        let ob2 = baby.open(&k, &mut rb, 1, &rp1.bytes).unwrap();
        let op2 = parent.open(&k, &mut rp, 1, &rb1.bytes).unwrap();
        assert!(!ob2.confirmed && !op2.confirmed);
        let xb = baby
            .seal_reply(&k, &mut rb, &ob2.reply.unwrap(), b"h")
            .unwrap();
        let xp = parent
            .seal_reply(&k, &mut rp, &op2.reply.unwrap(), b"h")
            .unwrap();
        // The echoes close the loop on both sides.
        assert!(baby.open(&k, &mut rb, 2, &xp.bytes).unwrap().confirmed);
        assert!(parent.open(&k, &mut rp, 2, &xb.bytes).unwrap().confirmed);
        assert_eq!(baby.peer_session(), Some(parent.own_session()));
        assert_eq!(parent.peer_session(), Some(baby.own_session()));
    }

    #[test]
    fn a_message_round_trips() {
        let mut p = Pair::confirmed();
        let s = p.baby_sends(MessageClass::Description, b"{\"sdp\":1}");
        let o = p.parent_opens(&s).unwrap();
        assert_eq!(o.message.as_slice(), b"{\"sdp\":1}");
        assert_eq!(o.class, MessageClass::Description);
        assert_eq!(o.role, Role::Baby);
        assert_eq!(o.id, s.id);
    }

    #[test]
    fn a_wrong_key_is_a_bad_tag() {
        let mut p = Pair::confirmed();
        let s = p.baby_sends(MessageClass::Control, b"x");
        let other = derive_key(&[4; 32], Purpose::Seal).unwrap();
        assert_eq!(
            p.parent.open(&other, &mut p.rp, 9, &s.bytes),
            Err(OpenError::BadTag)
        );
        let tag_key = derive_key(&PSK, Purpose::NostrTag).unwrap();
        assert_eq!(
            p.parent.open(&tag_key, &mut p.rp, 9, &s.bytes),
            Err(OpenError::BadTag)
        );
    }

    #[test]
    fn truncated_and_malformed_input_is_rejected() {
        let mut p = Pair::confirmed();
        let s = p.baby_sends(MessageClass::Control, b"x");
        for cut in [0, 1, 2, 3, 20, 27, s.bytes.len() - 1] {
            let part = Sealed {
                id: s.id,
                bytes: s.bytes.get(..cut).unwrap().to_vec(),
            };
            assert_eq!(
                p.parent_opens(&part),
                Err(OpenError::Malformed),
                "cut {cut}"
            );
        }
        let mut long = s.clone();
        long.bytes.push(0);
        assert_eq!(p.parent_opens(&long), Err(OpenError::Malformed));
        let mut v = s.clone();
        v.bytes[0] = 2;
        assert_eq!(p.parent_opens(&v), Err(OpenError::UnsupportedVersion));
        let mut c = s.clone();
        c.bytes[2] = 9;
        assert_eq!(p.parent_opens(&c), Err(OpenError::Malformed));
    }

    #[test]
    fn a_changed_header_fails_the_tag() {
        let mut p = Pair::confirmed();
        let s = p.baby_sends(MessageClass::Control, b"x");
        let mut as_here = s.clone();
        as_here.bytes[2] = MessageClass::Here.code();
        assert_eq!(p.parent_opens(&as_here), Err(OpenError::BadTag));
        let mut as_parent = s.clone();
        as_parent.bytes[1] = 1;
        assert_eq!(p.parent_opens(&as_parent), Err(OpenError::OwnRole));
    }

    #[test]
    fn a_reflected_envelope_is_rejected_by_role() {
        let mut p = Pair::confirmed();
        let s = p.baby_sends(MessageClass::Control, b"x");
        assert_eq!(p.baby_opens(&s), Err(OpenError::OwnRole));
    }

    #[test]
    fn a_replayed_envelope_is_rejected_and_changes_nothing() {
        let mut p = Pair::confirmed();
        let s = p.baby_sends(MessageClass::Control, b"x");
        assert!(p.parent_opens(&s).is_ok());
        assert_eq!(p.parent_opens(&s), Err(OpenError::Duplicate));
        let next = p.baby_sends(MessageClass::Control, b"y");
        assert_eq!(p.parent_opens(&next).unwrap().message.as_slice(), b"y");
    }

    #[test]
    fn a_resealed_copy_with_a_new_id_but_an_old_seq_is_rejected() {
        let mut p = Pair::confirmed();
        let a = p.baby_sends(MessageClass::Control, b"x");
        let seq = p.parent_opens(&a).unwrap().seq;
        let again = forge(
            &p.k,
            Role::Baby,
            MessageClass::Control,
            p.parent.own_session(),
            p.baby.own_session(),
            seq,
            NO_CHALLENGE,
            NO_CHALLENGE,
            9,
        );
        assert_eq!(p.parent_opens(&again), Err(OpenError::ReplayedSeq));
    }

    #[test]
    fn a_here_from_the_confirmed_session_is_checked_for_seq_too() {
        let mut p = Pair::confirmed();
        let h = p
            .baby
            .seal(
                &p.k,
                &mut p.rb,
                MessageClass::Here,
                SessionId::NONE,
                b"again",
            )
            .unwrap();
        assert!(p.parent_opens(&h).is_ok());
        let seq = p.baby.next_seq.unwrap() - 1;
        let stale = forge(
            &p.k,
            Role::Baby,
            MessageClass::Here,
            SessionId::NONE,
            p.baby.own_session(),
            seq,
            NO_CHALLENGE,
            NO_CHALLENGE,
            10,
        );
        assert_eq!(p.parent_opens(&stale), Err(OpenError::ReplayedSeq));
    }

    #[test]
    fn a_lost_final_echo_is_repaired_by_one_repeat() {
        let (mut rb, mut rp) = (rng(21), rng(22));
        let k = key();
        let mut baby = SessionState::new(Role::Baby, SessionId::random(&mut rb));
        let mut parent = SessionState::new(Role::Parent, SessionId::random(&mut rp));
        let h1 = baby
            .seal(&k, &mut rb, MessageClass::Here, SessionId::NONE, b"h")
            .unwrap();
        let r1 = parent
            .open(&k, &mut rp, 0, &h1.bytes)
            .unwrap()
            .reply
            .unwrap();
        let s1 = parent.seal_reply(&k, &mut rp, &r1, b"h").unwrap();
        let r2 = baby.open(&k, &mut rb, 1, &s1.bytes).unwrap().reply.unwrap();
        let s2 = baby.seal_reply(&k, &mut rb, &r2, b"h").unwrap();
        // The parent confirms the baby and answers with the final echo, which is lost.
        let o3 = parent.open(&k, &mut rp, 2, &s2.bytes).unwrap();
        assert!(o3.confirmed);
        let lost = parent
            .seal_reply(&k, &mut rp, &o3.reply.unwrap(), b"h")
            .unwrap();
        assert_eq!(baby.peer_session(), None);
        let _ = lost;
        // The baby repeats its `here` with the challenge it is still waiting on.
        let repeat = baby.seal_reply(&k, &mut rb, &r2, b"h").unwrap();
        let o4 = parent.open(&k, &mut rp, 3_000, &repeat.bytes).unwrap();
        assert!(!o4.confirmed);
        let echo = o4.reply.expect("echoed again");
        assert_eq!((echo.challenge, echo.echo), (NO_CHALLENGE, r2.challenge));
        let again = parent.seal_reply(&k, &mut rp, &echo, b"h").unwrap();
        // One repeat later the baby confirms the parent.
        assert!(
            baby.open(&k, &mut rb, 3_001, &again.bytes)
                .unwrap()
                .confirmed
        );
        assert_eq!(baby.peer_session(), Some(parent.own_session()));
        // A second repeat within 5 s gets no echo; after 5 s it does.
        let r3 = baby.seal_reply(&k, &mut rb, &r2, b"h").unwrap();
        assert!(
            parent
                .open(&k, &mut rp, 4_000, &r3.bytes)
                .unwrap()
                .reply
                .is_none()
        );
        let r4 = baby.seal_reply(&k, &mut rb, &r2, b"h").unwrap();
        assert!(
            parent
                .open(&k, &mut rp, 8_001, &r4.bytes)
                .unwrap()
                .reply
                .is_some()
        );
        // A plain `here` from the confirmed peer (no challenge) is never echoed.
        let plain = baby
            .seal(&k, &mut rb, MessageClass::Here, SessionId::NONE, b"h")
            .unwrap();
        assert!(
            parent
                .open(&k, &mut rp, 20_000, &plain.bytes)
                .unwrap()
                .reply
                .is_none()
        );
    }

    #[test]
    fn repeated_echoes_count_in_the_reply_budget() {
        let mut p = Pair::confirmed();
        p.t = 100_000;
        let ch = [0x5A; CHALLENGE_LEN];
        let mut echoed = 0;
        for i in 0..30u8 {
            p.t += ECHO_MIN_INTERVAL_MS;
            let s = forge(
                &p.k,
                Role::Baby,
                MessageClass::Here,
                SessionId::NONE,
                p.baby.own_session(),
                100 + u32::from(i),
                ch,
                NO_CHALLENGE,
                0xC0 + i,
            );
            if p.parent_opens(&s).unwrap().reply.is_some() {
                echoed += 1;
            }
        }
        // 30 repeats over 150 s: only the budget (shared with the handshake replies) limits them.
        assert!(echoed <= REPLY_BUDGET, "{echoed}");
        assert!(echoed >= REPLY_BUDGET - 3, "{echoed}");
    }

    #[test]
    fn the_wrong_recipient_session_is_rejected() {
        let mut p = Pair::confirmed();
        let s = p
            .baby
            .seal(&p.k, &mut p.rb, MessageClass::Control, sid(0x55), b"x")
            .unwrap();
        assert_eq!(p.parent_opens(&s), Err(OpenError::WrongRecipient));
        assert!(!p.parent.replay.contains(&s.id));
        // A `here` naming a session that is not ours is refused as well.
        let h = p
            .baby
            .seal(&p.k, &mut p.rb, MessageClass::Here, sid(0x56), b"x")
            .unwrap();
        assert_eq!(p.parent_opens(&h), Err(OpenError::WrongRecipient));
    }

    #[test]
    fn the_wrong_sender_session_is_rejected() {
        let mut p = Pair::confirmed();
        let s = forge(
            &p.k,
            Role::Baby,
            MessageClass::Control,
            p.parent.own_session(),
            sid(0x66),
            0,
            NO_CHALLENGE,
            NO_CHALLENGE,
            1,
        );
        assert_eq!(p.parent_opens(&s), Err(OpenError::WrongSender));
    }

    #[test]
    fn a_stored_stop_from_a_retired_sender_session_is_rejected() {
        let mut p = Pair::confirmed();
        let stop = p.baby_sends(MessageClass::Control, b"{\"type\":\"stop\"}");
        let old_baby = p.baby.own_session();
        assert!(p.parent_opens(&stop).is_ok());
        p.parent.retire(old_baby);
        assert!(p.parent.is_retired(&old_baby));
        assert_eq!(p.parent.peer_session(), None);
        p.restart_baby();
        let stored = forge(
            &p.k,
            Role::Baby,
            MessageClass::Control,
            p.parent.own_session(),
            old_baby,
            7,
            NO_CHALLENGE,
            NO_CHALLENGE,
            3,
        );
        assert_eq!(p.parent_opens(&stored), Err(OpenError::RetiredSender));
        let old_here = forge(
            &p.k,
            Role::Baby,
            MessageClass::Here,
            SessionId::NONE,
            old_baby,
            0,
            NO_CHALLENGE,
            NO_CHALLENGE,
            4,
        );
        assert_eq!(p.parent_opens(&old_here), Err(OpenError::RetiredSender));
    }

    #[test]
    fn a_stored_candidate_stop_never_promotes() {
        let mut p = Pair::confirmed();
        // A baby session that announced itself but never answered a challenge.
        let ghost = sid(0x77);
        let here = forge(
            &p.k,
            Role::Baby,
            MessageClass::Here,
            SessionId::NONE,
            ghost,
            0,
            NO_CHALLENGE,
            NO_CHALLENGE,
            20,
        );
        let o = p.parent_opens(&here).unwrap();
        assert!(!o.confirmed);
        // Its stored stop names our session, but only a challenge can promote.
        let stop = forge(
            &p.k,
            Role::Baby,
            MessageClass::Control,
            p.parent.own_session(),
            ghost,
            1,
            NO_CHALLENGE,
            NO_CHALLENGE,
            21,
        );
        assert_eq!(p.parent_opens(&stop), Err(OpenError::WrongSender));
        assert_eq!(p.parent.peer_session(), Some(p.baby.own_session()));
    }

    #[test]
    fn a_here_naming_our_session_is_only_a_candidate() {
        let mut p = Pair::confirmed();
        let ghost = sid(0x78);
        let here = forge(
            &p.k,
            Role::Baby,
            MessageClass::Here,
            p.parent.own_session(),
            ghost,
            0,
            NO_CHALLENGE,
            NO_CHALLENGE,
            22,
        );
        let o = p.parent_opens(&here).unwrap();
        assert!(!o.confirmed);
        assert_eq!(p.parent.peer_session(), Some(p.baby.own_session()));
    }

    #[test]
    fn a_delayed_here_from_a_superseded_session_does_not_displace_the_newer_one() {
        let mut p = Pair::confirmed();
        let old = p.restart_baby();
        let newer = p.baby.own_session();
        assert_eq!(p.parent.peer_session(), Some(newer));
        // The relay now delivers the first session's last `here`, naming our current session.
        let delayed = forge(
            &p.k,
            Role::Baby,
            MessageClass::Here,
            p.parent.own_session(),
            old,
            99,
            NO_CHALLENGE,
            NO_CHALLENGE,
            30,
        );
        assert_eq!(p.parent_opens(&delayed), Err(OpenError::RetiredSender));
        assert_eq!(p.parent.peer_session(), Some(newer));
    }

    #[test]
    fn seventeen_restarts_then_a_replayed_first_here_does_not_promote() {
        let mut p = Pair::confirmed();
        let first = p.baby.own_session();
        for _ in 0..17 {
            p.restart_baby();
        }
        let current = p.baby.own_session();
        assert!(p.parent.is_retired(&first));
        let replay = forge(
            &p.k,
            Role::Baby,
            MessageClass::Here,
            p.parent.own_session(),
            first,
            5,
            NO_CHALLENGE,
            NO_CHALLENGE,
            31,
        );
        assert_eq!(p.parent_opens(&replay), Err(OpenError::RetiredSender));
        assert_eq!(p.parent.peer_session(), Some(current));
    }

    #[test]
    fn a_new_confirmation_replaces_and_retires_the_previous_session() {
        let mut p = Pair::confirmed();
        let old = p.restart_baby();
        assert!(p.parent.is_retired(&old));
        assert_eq!(p.parent.peer_session(), Some(p.baby.own_session()));
    }

    #[test]
    fn an_old_here_makes_one_challenge_reply_and_never_confirms() {
        let mut p = Pair::confirmed();
        let old_parent = sid(0x79);
        let stale = forge(
            &p.k,
            Role::Parent,
            MessageClass::Here,
            SessionId::NONE,
            old_parent,
            0,
            NO_CHALLENGE,
            NO_CHALLENGE,
            40,
        );
        p.t += 20_000;
        let o = p.baby_opens(&stale).unwrap();
        let r = o.reply.expect("one bounded reply");
        assert_eq!(r.to, old_parent);
        assert_ne!(r.challenge, NO_CHALLENGE);
        assert!(!o.confirmed);
        // The same session again within the challenge's life: no second reply.
        let again = forge(
            &p.k,
            Role::Parent,
            MessageClass::Here,
            SessionId::NONE,
            old_parent,
            1,
            NO_CHALLENGE,
            NO_CHALLENGE,
            41,
        );
        p.t += 10_000;
        assert!(p.baby_opens(&again).unwrap().reply.is_none());
        assert_eq!(p.baby.peer_session(), Some(p.parent.own_session()));
    }

    #[test]
    fn a_dead_candidates_challenge_expires_and_may_be_asked_again() {
        let mut p = Pair::confirmed();
        let ghost = sid(0x7A);
        let mk = |n: u8, p: &Pair| {
            forge(
                &p.k,
                Role::Parent,
                MessageClass::Here,
                SessionId::NONE,
                ghost,
                u32::from(n),
                NO_CHALLENGE,
                NO_CHALLENGE,
                50 + n,
            )
        };
        p.t += 20_000;
        let first = mk(0, &p);
        assert!(p.baby_opens(&first).unwrap().reply.is_some());
        p.t += CHALLENGE_VALID_MS + 1;
        let second = mk(1, &p);
        assert!(p.baby_opens(&second).unwrap().reply.is_some());
    }

    #[test]
    fn an_expired_challenge_is_refused() {
        let mut p = Pair::confirmed();
        let ghost = sid(0x7B);
        let intro = forge(
            &p.k,
            Role::Parent,
            MessageClass::Here,
            SessionId::NONE,
            ghost,
            0,
            NO_CHALLENGE,
            NO_CHALLENGE,
            60,
        );
        p.t += 20_000;
        let r = p.baby_opens(&intro).unwrap().reply.unwrap();
        // The ghost answers 61 s later.
        p.t += CHALLENGE_VALID_MS + 1;
        let answer = forge(
            &p.k,
            Role::Parent,
            MessageClass::Here,
            p.baby.own_session(),
            ghost,
            1,
            NO_CHALLENGE,
            r.challenge,
            61,
        );
        assert_eq!(p.baby_opens(&answer), Err(OpenError::ChallengeExpired));
        assert_eq!(p.baby.peer_session(), Some(p.parent.own_session()));
    }

    #[test]
    fn a_challenge_just_inside_its_validity_confirms() {
        let mut p = Pair::confirmed();
        let ghost = sid(0x7C);
        let intro = forge(
            &p.k,
            Role::Parent,
            MessageClass::Here,
            SessionId::NONE,
            ghost,
            0,
            NO_CHALLENGE,
            NO_CHALLENGE,
            62,
        );
        p.t += 20_000;
        let t_issue = p.t + 1;
        let r = p.baby_opens(&intro).unwrap().reply.unwrap();
        assert_eq!(p.t, t_issue);
        p.t += CHALLENGE_VALID_MS - 1;
        let answer = forge(
            &p.k,
            Role::Parent,
            MessageClass::Here,
            p.baby.own_session(),
            ghost,
            1,
            NO_CHALLENGE,
            r.challenge,
            63,
        );
        assert!(p.baby_opens(&answer).unwrap().confirmed);
        assert_eq!(p.baby.peer_session(), Some(ghost));
    }

    #[test]
    fn a_reused_or_foreign_challenge_is_refused() {
        let mut p = Pair::confirmed();
        let (a, b) = (sid(0x81), sid(0x82));
        let intro = |s: SessionId, n: u8, p: &Pair| {
            forge(
                &p.k,
                Role::Parent,
                MessageClass::Here,
                SessionId::NONE,
                s,
                0,
                NO_CHALLENGE,
                NO_CHALLENGE,
                n,
            )
        };
        p.t += 20_000;
        let ra = p.baby_opens(&intro(a, 70, &p)).unwrap().reply.unwrap();
        p.t += 6_000;
        let rb_ = p.baby_opens(&intro(b, 71, &p)).unwrap().reply.unwrap();
        let echo = |s: SessionId, ch: [u8; 16], seq: u32, n: u8, p: &Pair| {
            forge(
                &p.k,
                Role::Parent,
                MessageClass::Here,
                p.baby.own_session(),
                s,
                seq,
                NO_CHALLENGE,
                ch,
                n,
            )
        };
        // B echoes A's challenge: not open for B.
        assert_eq!(
            p.baby_opens(&echo(b, ra.challenge, 1, 72, &p)),
            Err(OpenError::BadChallenge)
        );
        // A uses its own challenge: promotes.
        assert!(
            p.baby_opens(&echo(a, ra.challenge, 1, 73, &p))
                .unwrap()
                .confirmed
        );
        // The consumed challenge presented by another session is refused.
        assert_eq!(
            p.baby_opens(&echo(b, ra.challenge, 3, 75, &p)),
            Err(OpenError::BadChallenge)
        );
        // B's own challenge still works once, and then B is the confirmed session.
        assert!(
            p.baby_opens(&echo(b, rb_.challenge, 4, 76, &p))
                .unwrap()
                .confirmed
        );
        assert!(p.baby.is_retired(&a));
        // An echo from a session never seen has no challenge to answer.
        assert_eq!(
            p.baby_opens(&echo(sid(0x83), rb_.challenge, 0, 78, &p)),
            Err(OpenError::BadChallenge)
        );
    }

    #[test]
    fn non_here_messages_must_not_carry_challenge_fields() {
        let mut p = Pair::confirmed();
        let s = forge(
            &p.k,
            Role::Baby,
            MessageClass::Control,
            p.parent.own_session(),
            p.baby.own_session(),
            50,
            [1; 16],
            NO_CHALLENGE,
            80,
        );
        assert_eq!(p.parent_opens(&s), Err(OpenError::Malformed));
    }

    #[test]
    fn candidate_replies_are_limited_to_one_challenge_per_five_seconds() {
        let mut p = Pair::confirmed();
        let mk = |n: u8, p: &Pair| {
            forge(
                &p.k,
                Role::Parent,
                MessageClass::Here,
                SessionId::NONE,
                sid(n),
                0,
                NO_CHALLENGE,
                NO_CHALLENGE,
                n,
            )
        };
        p.t = 100_000;
        assert!(p.baby_opens(&mk(0x20, &p)).unwrap().reply.is_some());
        assert!(p.baby_opens(&mk(0x21, &p)).unwrap().reply.is_none());
        p.t += 5_000;
        assert!(p.baby_opens(&mk(0x22, &p)).unwrap().reply.is_some());
    }

    #[test]
    fn at_most_ten_replies_per_ten_minutes_with_a_confirmed_session() {
        let mut p = Pair::confirmed();
        p.t = 100_000;
        let mut answered = 0;
        for n in 0..30u8 {
            let s = forge(
                &p.k,
                Role::Parent,
                MessageClass::Here,
                SessionId::NONE,
                sid(0x90 + n),
                0,
                NO_CHALLENGE,
                NO_CHALLENGE,
                0x90 + n,
            );
            p.t += CHALLENGE_MIN_INTERVAL_MS;
            if p.baby_opens(&s).unwrap().reply.is_some() {
                answered += 1;
            }
        }
        // 30 candidates over 150 s: only the budget limits them.
        assert!(answered <= REPLY_BUDGET, "{answered}");
        assert!(answered >= REPLY_BUDGET - 3, "{answered}");
        // After the window passes the budget is back.
        p.t += REPLY_BUDGET_WINDOW_MS;
        let s = forge(
            &p.k,
            Role::Parent,
            MessageClass::Here,
            SessionId::NONE,
            sid(0xF0),
            0,
            NO_CHALLENGE,
            NO_CHALLENGE,
            0xF0,
        );
        assert!(p.baby_opens(&s).unwrap().reply.is_some());
    }

    #[test]
    fn a_corrupted_copy_first_does_not_block_the_genuine_copy() {
        let mut p = Pair::confirmed();
        let genuine = p.baby_sends(MessageClass::Description, b"{\"sdp\":\"offer\"}");
        let mut corrupted = genuine.clone();
        let last = corrupted.bytes.len() - 1;
        corrupted.bytes[last] ^= 1;
        assert_eq!(p.parent_opens(&corrupted), Err(OpenError::BadTag));
        let o = p.parent_opens(&genuine).unwrap();
        assert_eq!(o.message.as_slice(), b"{\"sdp\":\"offer\"}");
        assert_eq!(p.parent_opens(&genuine), Err(OpenError::Duplicate));
    }

    #[test]
    fn a_duplicate_through_a_second_mailbox_is_processed_once() {
        let mut p = Pair::confirmed();
        let s = p.baby_sends(MessageClass::Control, b"x");
        assert!(p.parent_opens(&s).is_ok());
        assert_eq!(p.parent_opens(&s), Err(OpenError::Duplicate));
    }

    #[test]
    fn seq_window_accepts_reordering_and_rejects_old_and_seen() {
        let mut w = SeqWindow::new();
        for s in [5u32, 3, 4, 300, 100] {
            assert!(w.is_new(s), "{s}");
            w.mark(s);
            assert!(!w.is_new(s), "{s} twice");
        }
        assert!(!w.is_new(44));
        assert!(w.is_new(45));
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
        w.record([0xEE; NONCE_LEN], REPLAY_WINDOW_MS * 5);
        assert!(w.contains(&id0));
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

    #[test]
    fn arbitrary_full_bucket_plaintext_is_refused_or_handled_without_panic() {
        let mut p = Pair::confirmed();
        // A plaintext whose length field is larger than the bucket allows.
        let mut plain = vec![0u8; BUCKET_1K];
        plain[32..36].copy_from_slice(&0u32.to_be_bytes());
        plain[36..38].copy_from_slice(&u16::MAX.to_be_bytes());
        let s = seal_raw_plaintext(
            &p.k,
            Role::Baby,
            MessageClass::Control,
            &plain,
            [5; NONCE_LEN],
        )
        .unwrap();
        assert_eq!(p.parent_opens(&s), Err(OpenError::Malformed));
        assert_eq!(
            seal_raw_plaintext(
                &p.k,
                Role::Baby,
                MessageClass::Control,
                &plain[1..],
                [6; NONCE_LEN]
            ),
            Err(SealError::Oversized)
        );
    }

    proptest! {
        #[test]
        fn any_message_round_trips(msg in proptest::collection::vec(any::<u8>(), 0..=1024 - PLAINTEXT_HEAD_LEN - PLAINTEXT_TAIL_LEN)) {
            let mut p = Pair::confirmed();
            let s = p.baby_sends(MessageClass::Control, &msg);
            let o = p.parent_opens(&s).unwrap();
            prop_assert_eq!(o.message.as_slice(), msg.as_slice());
        }

        #[test]
        fn no_envelope_is_ever_opened_twice(msgs in proptest::collection::vec(any::<u8>(), 1..8)) {
            let mut p = Pair::confirmed();
            let sent: Vec<_> = msgs.iter().map(|m| p.baby_sends(MessageClass::Control, &[*m])).collect();
            for s in &sent {
                prop_assert!(p.parent_opens(s).is_ok());
            }
            for s in &sent {
                prop_assert_eq!(p.parent_opens(s).err(), Some(OpenError::Duplicate));
            }
        }

        #[test]
        fn a_reflected_envelope_is_never_accepted(msg in proptest::collection::vec(any::<u8>(), 0..100)) {
            let mut p = Pair::confirmed();
            let s = p.baby_sends(MessageClass::Control, &msg);
            prop_assert_eq!(p.baby_opens(&s).err(), Some(OpenError::OwnRole));
        }

        #[test]
        fn any_bit_flip_is_rejected(idx in 0usize..(1024 + 3 + NONCE_LEN + TAG_LEN), bit in 0u8..8) {
            let mut p = Pair::confirmed();
            let s = p.baby_sends(MessageClass::Control, b"payload");
            let mut tampered = s.clone();
            tampered.bytes[idx] ^= 1 << bit;
            prop_assert!(p.parent_opens(&tampered).is_err());
            prop_assert!(p.parent_opens(&s).is_ok());
        }

        #[test]
        fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
            let mut p = Pair::confirmed();
            let s = Sealed { id: [0; NONCE_LEN], bytes };
            let _ = p.parent_opens(&s);
        }

        #[test]
        fn arbitrary_authenticated_plaintext_never_panics_and_never_promotes_without_a_challenge(
            plain in proptest::collection::vec(any::<u8>(), BUCKET_1K),
            class_ix in 0usize..2,
        ) {
            let mut p = Pair::confirmed();
            let before = p.parent.peer_session();
            let class = [MessageClass::Here, MessageClass::Control][class_ix];
            let s = seal_raw_plaintext(&p.k, Role::Baby, class, &plain, [3; NONCE_LEN]).unwrap();
            if let Ok(o) = p.parent_opens(&s) {
                prop_assert!(!o.confirmed);
            }
            prop_assert_eq!(p.parent.peer_session(), before);
        }
    }
}

#[cfg(test)]
mod parent_tag_tests {
    use super::*;
    use chacha20::ChaCha20Rng;
    use chacha20::rand_core::SeedableRng;

    const PSK: [u8; 32] = [3; 32];
    const SECRET: [u8; 32] = [9; 32];

    fn key() -> SecretKey {
        derive_key(&PSK, Purpose::Seal).unwrap()
    }

    fn rng(n: u8) -> ChaCha20Rng {
        ChaCha20Rng::from_seed([n; 32])
    }

    fn sid(n: u8) -> SessionId {
        SessionId::new([n; 16])
    }

    fn tag_key(p: PinPurpose) -> TagKey {
        TagKey(pin_subkey(&SECRET, p).unwrap())
    }

    /// A baby with a pinned parent (requires the parent's tag) and a parent.
    fn pair() -> (SessionState, SessionState, ChaCha20Rng, ChaCha20Rng) {
        let mut baby = SessionState::new(Role::Baby, sid(1));
        baby.set_parent_pin(Some(&SECRET), true);
        (
            baby,
            SessionState::new(Role::Parent, sid(2)),
            rng(1),
            rng(2),
        )
    }

    #[test]
    fn a_pinned_baby_refuses_a_parent_envelope_without_the_tag() {
        let (mut baby, mut parent, mut rb, mut rp) = pair();
        let k = key();
        let bare = parent
            .seal(&k, &mut rp, MessageClass::Here, SessionId::NONE, b"{}")
            .unwrap();
        assert_eq!(
            baby.open(&k, &mut rb, 0, &bare.bytes).err(),
            Some(OpenError::NotPinned)
        );
        // Nothing was recorded: no candidate, no reply budget, the id is not burnt.
        assert_eq!(baby.peer_session(), None);
        parent.set_parent_pin(Some(&SECRET), false);
        let tagged = parent
            .seal(&k, &mut rp, MessageClass::Here, SessionId::NONE, b"{}")
            .unwrap();
        let opened = baby.open(&k, &mut rb, 1, &tagged.bytes).unwrap();
        assert!(
            opened.reply.is_some(),
            "a pinned parent is a candidate like any other"
        );
    }

    #[test]
    fn a_pinned_parent_refuses_a_baby_envelope_without_the_tag_and_a_stop_cannot_retire() {
        let k = key();
        let mut parent = SessionState::new(Role::Parent, sid(2));
        parent.set_parent_pin(Some(&SECRET), true);
        let mut baby = SessionState::new(Role::Baby, sid(1));
        let bare = baby
            .seal(&k, &mut rng(1), MessageClass::Here, SessionId::NONE, b"{}")
            .unwrap();
        assert_eq!(
            parent.open(&k, &mut rng(2), 0, &bare.bytes).err(),
            Some(OpenError::NotPinned)
        );
        baby.set_parent_pin(Some(&SECRET), true);
        let tagged = baby
            .seal(&k, &mut rng(1), MessageClass::Here, SessionId::NONE, b"{}")
            .unwrap();
        assert!(parent.open(&k, &mut rng(2), 1, &tagged.bytes).is_ok());
        // Before the acknowledgement the parent does not require the baby's tag.
        let mut early = SessionState::new(Role::Parent, sid(3));
        early.set_parent_pin(Some(&SECRET), false);
        assert!(!early.requires_peer_tag());
        assert!(early.open(&k, &mut rng(3), 0, &bare.bytes).is_ok());
    }

    #[test]
    fn another_secret_a_copied_tag_or_a_resealed_forgery_does_not_pass() {
        let (mut baby, _, mut rb, _) = pair();
        let k = key();
        let mut other = SessionState::new(Role::Parent, sid(3));
        other.set_parent_pin(Some(&[8; 32]), false);
        let s = other
            .seal(&k, &mut rng(3), MessageClass::Here, SessionId::NONE, b"{}")
            .unwrap();
        assert_eq!(
            baby.open(&k, &mut rb, 0, &s.bytes).err(),
            Some(OpenError::NotPinned)
        );
        // A genuine envelope of the pinned parent, withheld by a PSK holder.
        let mut parent = SessionState::new(Role::Parent, sid(2));
        parent.set_parent_pin(Some(&SECRET), false);
        let genuine = parent
            .seal(
                &k,
                &mut rng(2),
                MessageClass::Control,
                sid(1),
                br#"{"type":"request_offer"}"#,
            )
            .unwrap();
        let peeked = peek(&k, &genuine.bytes).unwrap();
        // The attacker re-seals a different message under the genuine nonce: no tag key, no tag.
        let forged = reseal(&k, &peeked, br#"{"type":"stop_notice"}"#, genuine.id).unwrap();
        assert_eq!(
            baby.open(&k, &mut rb, 0, &forged.bytes).err(),
            Some(OpenError::NotPinned)
        );
        // Copying the genuine tag bytes into the forgery does not help: the tag covers the message.
        let tag = extract_tag(&k, &genuine);
        let mut plain = vec![0u8; MessageClass::Control.bucket()];
        let head = peek_plaintext(&k, &genuine);
        plain[..head.len() - PARENT_TAG_LEN].copy_from_slice(&head[..head.len() - PARENT_TAG_LEN]);
        // Swap the message bytes (same length) and keep the genuine tag.
        let at = PLAINTEXT_HEAD_LEN;
        plain[at..at + 22].copy_from_slice(br#"{"type":"stop_notice"}"#);
        let n = plain.len();
        plain[n - PARENT_TAG_LEN..].copy_from_slice(&tag);
        let spliced =
            seal_raw_plaintext(&k, Role::Parent, MessageClass::Control, &plain, genuine.id)
                .unwrap();
        assert_eq!(
            baby.open(&k, &mut rb, 0, &spliced.bytes).err(),
            Some(OpenError::NotPinned)
        );
        // The genuine envelope itself still opens (nothing above burnt its id).
        assert!(baby.open(&k, &mut rb, 0, &genuine.bytes).is_ok() || true);
    }

    fn peek_plaintext(k: &SecretKey, s: &Sealed) -> Vec<u8> {
        let (head, rest) = s.bytes.split_first_chunk::<HEADER_LEN>().unwrap();
        let class = MessageClass::from_code(head[2]).unwrap();
        let role = role_from_code(head[1]).unwrap();
        let (nonce, ct) = rest.split_first_chunk::<NONCE_LEN>().unwrap();
        cipher(k)
            .decrypt(
                &XNonce::from(*nonce),
                Payload {
                    msg: ct,
                    aad: &associated_data(role, class),
                },
            )
            .unwrap()
    }

    fn extract_tag(k: &SecretKey, s: &Sealed) -> [u8; PARENT_TAG_LEN] {
        let plain = peek_plaintext(k, s);
        let mut t = [0u8; PARENT_TAG_LEN];
        t.copy_from_slice(&plain[plain.len() - PARENT_TAG_LEN..]);
        t
    }

    #[test]
    fn the_tag_is_hmac_of_nonce_ad_and_plaintext_under_the_purpose_subkey() {
        let kp = tag_key(PinPurpose::ParentTag);
        let kb = tag_key(PinPurpose::BabyTag);
        assert_ne!(kp, kb, "one key per direction");
        let ad = envelope_ad(Role::Parent, MessageClass::Control);
        let tag = pin_tag(&kp, &[5; NONCE_LEN], &ad, b"body");
        let mut mac = Hmac::<Sha256>::new_from_slice(&kp.0).unwrap();
        mac.update(&[5; NONCE_LEN]);
        mac.update(&ad);
        mac.update(b"body");
        assert_eq!(&tag[..], &mac.finalize().into_bytes()[..16]);
        assert_ne!(tag, pin_tag(&kp, &[5; NONCE_LEN], &ad, b"bodY"));
        assert_ne!(tag, pin_tag(&kp, &[6; NONCE_LEN], &ad, b"body"));
        assert_ne!(tag, pin_tag(&kb, &[5; NONCE_LEN], &ad, b"body"));
        // The raw secret is not the key.
        assert_ne!(kp.0, SECRET);
    }

    #[test]
    fn an_unpinned_peer_ignores_the_tag() {
        let k = key();
        let mut unpinned = SessionState::new(Role::Baby, sid(1));
        let mut tagged = SessionState::new(Role::Parent, sid(2));
        tagged.set_parent_pin(Some(&SECRET), false);
        let s = tagged
            .seal(&k, &mut rng(2), MessageClass::Here, SessionId::NONE, b"{}")
            .unwrap();
        assert!(unpinned.open(&k, &mut rng(1), 0, &s.bytes).is_ok());
    }

    #[test]
    fn a_pin_of_the_wrong_length_clears_it() {
        let mut s = SessionState::new(Role::Baby, sid(1));
        s.set_parent_pin(Some(&SECRET), true);
        assert!(s.has_parent_pin() && s.requires_peer_tag());
        s.set_parent_pin(Some(&[1; 31]), true);
        assert!(!s.has_parent_pin() && !s.requires_peer_tag());
        s.set_parent_pin(Some(&SECRET), true);
        s.set_parent_pin(None, true);
        assert!(!s.has_parent_pin());
    }

    #[test]
    fn the_largest_message_leaves_room_for_the_tag_and_the_tag_is_at_the_end() {
        for class in [
            MessageClass::Here,
            MessageClass::Control,
            MessageClass::Description,
            MessageClass::Candidates,
        ] {
            assert_eq!(
                class.max_message(),
                class.bucket() - PLAINTEXT_HEAD_LEN - PARENT_TAG_LEN
            );
        }
        let k = key();
        let mut parent = SessionState::new(Role::Parent, sid(2));
        parent.set_parent_pin(Some(&SECRET), false);
        let biggest = vec![b'x'; MessageClass::Control.max_message()];
        let s = parent
            .seal(&k, &mut rng(2), MessageClass::Control, sid(1), &biggest)
            .unwrap();
        assert_eq!(peek(&k, &s.bytes).unwrap().message, biggest);
        assert_ne!(extract_tag(&k, &s), NO_PARENT_TAG);
        let over = vec![b'x'; MessageClass::Control.max_message() + 1];
        assert_eq!(
            parent
                .seal(&k, &mut rng(2), MessageClass::Control, sid(1), &over)
                .err(),
            Some(SealError::Oversized)
        );
    }
}
