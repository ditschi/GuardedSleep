//! The Nostr mailbox kind (`alpha-connection-model`, design section 3): the daily address of a
//! pairing on public relays (epoch, tag, per-role BIP-340 keys) and the NIP-01 frames the core
//! speaks. Pure functions only: the sockets belong to the platform and the policy (pacing, budgets,
//! parking, subscriptions) to `Link`.
//!
//! What a relay sees of a sealed envelope is an event of the fixed ephemeral kind [`KIND`], signed
//! by the daily key of the sending role, with one `t` tag (the daily tag) and the base64url of the
//! envelope as content. Content and tags are ASCII base64url and nothing else, so the NIP-01 id
//! preimage has no escaping to diverge on, and a frame is at most [`MAX_FRAME_BYTES`].
//!
//! Key schedule (all from the PSK; see `envelope::Purpose`):
//!
//! - epoch: days since the Unix epoch counted from 12:00 UTC (u32, big endian when hashed);
//! - tag: `base64url(HMAC-SHA256(tag_key, epoch_be32))[0..22]`;
//! - signing scalar: `HKDF-Expand(sign_seed, "role|" ‖ role ‖ epoch_be32)` reduced modulo the curve
//!   order into a non-zero scalar (zero: no key for that epoch and role);
//! - BIP-340 signatures use `aux_rand` drawn from the Link CSPRNG.
//!
//! Signatures only prove that an event comes from a holder of the PSK for that day; peer
//! authentication stays with the sealed envelope and the pairing proof.

// Untrusted relay frames are parsed here: return errors instead of panicking (AGENTS.md section 5.3).
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

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hkdf::Hkdf;
use hmac::{Hmac, KeyInit, Mac};
use k256::elliptic_curve::ops::Reduce;
use k256::schnorr::{Signature, SigningKey, VerifyingKey};
use k256::{FieldBytes, NonZeroScalar, Scalar};
use serde_json::Value;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::envelope::{Purpose, SecretKey, derive_key};
use crate::protocol::Role;

/// The one ephemeral event kind of the Nostr mailbox (NIP-01 ephemeral range 20000..30000).
/// Chosen on 2026-10-25 against the kinds table of the NIPs repository: not listed there (the
/// neighbours are 24133 NIP-46 and 24242 Blossom). Recorded in `signaling-mailboxes`.
pub const KIND: u32 = 24861;

/// The longest frame the core accepts or builds (text bytes), on the platform and here.
pub const MAX_FRAME_BYTES: usize = 64 * 1024;

/// The epoch rolls over at this second of the UTC day (12:00).
const EPOCH_OFFSET_S: u64 = 12 * 3600;
const DAY_S: u64 = 86_400;

/// Length of the tag in characters (132 bits).
pub const TAG_LEN: usize = 22;

/// How far back a subscription asks (`since = now - SINCE_S`).
pub const SINCE_S: u64 = 15 * 60;

/// Around the rollover both neighbouring epochs are subscribed: this many seconds either side.
pub const DUAL_EPOCH_S: u64 = 30 * 60;

/// Longest relay message text kept for classification (the rest is cut off).
const MAX_MESSAGE_CHARS: usize = 200;
/// Most tags and the longest tag string an inbound event may carry.
const MAX_TAGS: usize = 4;
const MAX_TAG_STRING: usize = 64;
/// The longest subscription id a relay may use for us.
const MAX_SUB_ID: usize = 64;

/// Days since the Unix epoch counted from 12:00 UTC. A clock before the first 12:00 UTC is epoch 0.
pub fn epoch_of(unix_s: u64) -> u32 {
    let days = unix_s.saturating_sub(EPOCH_OFFSET_S) / DAY_S;
    u32::try_from(days).unwrap_or(u32::MAX)
}

/// First second (Unix) of `epoch`.
pub fn epoch_start_s(epoch: u32) -> u64 {
    u64::from(epoch) * DAY_S + EPOCH_OFFSET_S
}

/// The epochs to listen to at `unix_s`: the current one, plus the neighbour inside the half hour
/// before and after a rollover, so a clock that is a few minutes off still meets the peer.
pub fn epochs_to_listen(unix_s: u64) -> Vec<u32> {
    let current = epoch_of(unix_s);
    let start = epoch_start_s(current);
    let mut epochs = vec![current];
    let into_epoch = unix_s.saturating_sub(start);
    let to_next = start.saturating_add(DAY_S).saturating_sub(unix_s);
    if into_epoch < DUAL_EPOCH_S {
        if let Some(previous) = current.checked_sub(1) {
            epochs.push(previous);
        }
    } else if to_next <= DUAL_EPOCH_S
        && let Some(next) = current.checked_add(1)
    {
        epochs.push(next);
    }
    epochs
}

/// The keys and tags of a pairing on public relays, derived once from the PSK.
pub struct NostrKeys {
    tag_key: SecretKey,
    seed: SecretKey,
}

impl NostrKeys {
    pub fn derive(psk: &[u8]) -> Option<Self> {
        Some(Self {
            tag_key: derive_key(psk, Purpose::NostrTag)?,
            seed: derive_key(psk, Purpose::NostrKey)?,
        })
    }

    /// The daily tag, 22 base64url characters.
    pub fn tag(&self, epoch: u32) -> String {
        // HMAC accepts any key length, so this cannot fail; the `else` keeps the parser lint happy.
        let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(self.tag_key.expose()) else {
            return String::new();
        };
        mac.update(&epoch.to_be_bytes());
        let digest = mac.finalize().into_bytes();
        URL_SAFE_NO_PAD
            .encode(digest)
            .chars()
            .take(TAG_LEN)
            .collect()
    }

    /// The BIP-340 signing key of `role` for `epoch`, or `None` when the derived scalar is zero
    /// (the device then uses its other mailboxes for that epoch).
    pub fn signing_key(&self, role: Role, epoch: u32) -> Option<SigningKey> {
        let hk = Hkdf::<Sha256>::from_prk(self.seed.expose()).ok()?;
        let mut info = Vec::with_capacity(5 + 6 + 4);
        info.extend_from_slice(b"role|");
        info.extend_from_slice(role.as_str().as_bytes());
        info.extend_from_slice(&epoch.to_be_bytes());
        let mut okm = Zeroizing::new([0u8; 32]);
        hk.expand(&info, &mut *okm).ok()?;
        scalar_key(&okm)
    }

    /// The x-only public key of `role` for `epoch` as 64 lowercase hex characters.
    pub fn public_key_hex(&self, role: Role, epoch: u32) -> Option<String> {
        let sk = self.signing_key(role, epoch)?;
        Some(hex_encode(&sk.verifying_key().to_bytes()))
    }
}

/// Reduces 32 bytes modulo the curve order into a signing key; `None` for the zero scalar.
pub fn scalar_key(okm: &[u8; 32]) -> Option<SigningKey> {
    let bytes = Zeroizing::new(FieldBytes::from(*okm));
    let scalar = <Scalar as Reduce<FieldBytes>>::reduce(&bytes);
    let nonzero = NonZeroScalar::new(scalar).into_option()?;
    Some(SigningKey::from(nonzero))
}

// ---- events ----------------------------------------------------------------------------------

/// A NIP-01 event of this mailbox kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

/// Whether `text` is non-empty ASCII base64url (`A-Za-z0-9_-`), the only alphabet allowed in
/// content and tags.
pub fn is_base64url(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// The NIP-01 id: SHA-256 of the canonical `[0,pubkey,created_at,kind,tags,content]` text. Every
/// string is known to be plain base64url (or hex), so no escaping is needed.
fn event_id(
    pubkey: &str,
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> [u8; 32] {
    let mut text = String::with_capacity(content.len() + 160);
    text.push_str("[0,\"");
    text.push_str(pubkey);
    text.push_str("\",");
    text.push_str(&created_at.to_string());
    text.push(',');
    text.push_str(&kind.to_string());
    text.push_str(",[");
    for (i, tag) in tags.iter().enumerate() {
        if i > 0 {
            text.push(',');
        }
        text.push('[');
        for (j, item) in tag.iter().enumerate() {
            if j > 0 {
                text.push(',');
            }
            text.push('"');
            text.push_str(item);
            text.push('"');
        }
        text.push(']');
    }
    text.push_str("],\"");
    text.push_str(content);
    text.push_str("\"]");
    Sha256::digest(text.as_bytes()).into()
}

/// Builds and signs an event with one `t` tag. `content` and `tag` must be base64url.
pub fn build_event(
    key: &SigningKey,
    aux_rand: &[u8; 32],
    created_at: u64,
    tag: &str,
    content: &str,
) -> Option<Event> {
    if !is_base64url(content) || !is_base64url(tag) {
        return None;
    }
    let pubkey = hex_encode(&key.verifying_key().to_bytes());
    let tags = vec![vec!["t".to_owned(), tag.to_owned()]];
    let id = event_id(&pubkey, created_at, KIND, &tags, content);
    let sig = key.sign_raw(&id, aux_rand).ok()?;
    Some(Event {
        id: hex_encode(&id),
        pubkey,
        created_at,
        kind: KIND,
        tags,
        content: content.to_owned(),
        sig: hex_encode(&sig.to_bytes()),
    })
}

/// The id and the BIP-340 signature of `event` check out (the caller still checks author, tag and
/// kind against what it subscribed to).
pub fn verify_event(event: &Event) -> bool {
    let Some(id) = hex_decode(&event.id).filter(|b| b.len() == 32) else {
        return false;
    };
    if event_id(
        &event.pubkey,
        event.created_at,
        event.kind,
        &event.tags,
        &event.content,
    )
    .as_slice()
        != id.as_slice()
    {
        return false;
    }
    let Some(pk) = hex_decode(&event.pubkey).and_then(|b| VerifyingKey::from_slice(&b).ok()) else {
        return false;
    };
    let Some(sig) = hex_decode(&event.sig).and_then(|b| Signature::from_slice(&b).ok()) else {
        return false;
    };
    pk.verify_raw(&id, &sig).is_ok()
}

impl Event {
    /// The event as the JSON object of an `EVENT` frame.
    pub fn to_json(&self) -> String {
        let mut tags = String::from("[");
        for (i, tag) in self.tags.iter().enumerate() {
            if i > 0 {
                tags.push(',');
            }
            tags.push('[');
            for (j, item) in tag.iter().enumerate() {
                if j > 0 {
                    tags.push(',');
                }
                tags.push('"');
                tags.push_str(item);
                tags.push('"');
            }
            tags.push(']');
        }
        tags.push(']');
        format!(
            "{{\"id\":\"{}\",\"pubkey\":\"{}\",\"created_at\":{},\"kind\":{},\"tags\":{},\"content\":\"{}\",\"sig\":\"{}\"}}",
            self.id, self.pubkey, self.created_at, self.kind, tags, self.content, self.sig
        )
    }

    /// The value of the first `t` tag.
    pub fn tag_value(&self) -> Option<&str> {
        self.tags
            .iter()
            .find(|t| t.first().map(String::as_str) == Some("t"))
            .and_then(|t| t.get(1))
            .map(String::as_str)
    }
}

fn parse_event(v: &Value) -> Option<Event> {
    let o = v.as_object()?;
    let hex64 = |k: &str, len: usize| {
        let s = o.get(k)?.as_str()?;
        (s.len() == len && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
            .then(|| s.to_owned())
    };
    let id = hex64("id", 64)?;
    let pubkey = hex64("pubkey", 64)?;
    let sig = hex64("sig", 128)?;
    let created_at = o.get("created_at")?.as_u64()?;
    let kind = u32::try_from(o.get("kind")?.as_u64()?).ok()?;
    let content = o.get("content")?.as_str()?;
    if !is_base64url(content) || content.len() > MAX_FRAME_BYTES {
        return None;
    }
    let raw_tags = o.get("tags")?.as_array()?;
    if raw_tags.len() > MAX_TAGS {
        return None;
    }
    let mut tags = Vec::with_capacity(raw_tags.len());
    for raw in raw_tags {
        let items = raw.as_array()?;
        if items.is_empty() || items.len() > 2 {
            return None;
        }
        let mut tag = Vec::with_capacity(items.len());
        for item in items {
            let s = item.as_str()?;
            if s.len() > MAX_TAG_STRING || !is_base64url(s) {
                return None;
            }
            tag.push(s.to_owned());
        }
        tags.push(tag);
    }
    Some(Event {
        id,
        pubkey,
        created_at,
        kind,
        tags,
        content: content.to_owned(),
        sig,
    })
}

// ---- frames ----------------------------------------------------------------------------------

/// One filter of a `REQ`: our kind, one author, one tag, a lower time bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    pub author: String,
    pub tag: String,
    pub since: u64,
}

impl Filter {
    fn to_json(&self) -> String {
        format!(
            "{{\"kinds\":[{}],\"authors\":[\"{}\"],\"#t\":[\"{}\"],\"since\":{}}}",
            KIND, self.author, self.tag, self.since
        )
    }
}

/// `["REQ", sub, filter...]` for a subscription (a `REQ` with a known id replaces that
/// subscription, NIP-01).
pub fn req_frame(sub: &str, filters: &[Filter]) -> Option<String> {
    if !is_base64url(sub) || sub.len() > MAX_SUB_ID || filters.is_empty() {
        return None;
    }
    let mut text = format!("[\"REQ\",\"{sub}\"");
    for f in filters {
        if !is_hex64(&f.author) || !is_base64url(&f.tag) {
            return None;
        }
        text.push(',');
        text.push_str(&f.to_json());
    }
    text.push(']');
    Some(text)
}

/// The throwaway `REQ` of a reachability check: nothing derived from the pairing and not even the
/// event kind, only a random event id that cannot exist (`random_id_hex`: 64 hex characters).
pub fn probe_req_frame(sub: &str, random_id_hex: &str) -> Option<String> {
    if !is_base64url(sub) || sub.len() > MAX_SUB_ID || !is_hex64(random_id_hex) {
        return None;
    }
    Some(format!(
        "[\"REQ\",\"{sub}\",{{\"ids\":[\"{random_id_hex}\"],\"limit\":1}}]"
    ))
}

pub fn close_frame(sub: &str) -> Option<String> {
    (is_base64url(sub) && sub.len() <= MAX_SUB_ID).then(|| format!("[\"CLOSE\",\"{sub}\"]"))
}

pub fn event_frame(event: &Event) -> String {
    format!("[\"EVENT\",{}]", event.to_json())
}

fn is_hex64(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// What a relay can say to us.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelayFrame {
    Event {
        sub: String,
        event: Event,
    },
    Ok {
        id: String,
        accepted: bool,
        message: String,
    },
    Eose {
        sub: String,
    },
    Notice {
        message: String,
    },
    Closed {
        sub: String,
        message: String,
    },
    /// NIP-42 challenge: we never authenticate.
    Auth,
}

/// Parses one relay text frame; anything malformed, oversized or with characters outside the
/// allowed alphabets is `None`.
pub fn parse_relay_frame(text: &str) -> Option<RelayFrame> {
    if text.len() > MAX_FRAME_BYTES {
        return None;
    }
    let value: Value = serde_json::from_str(text).ok()?;
    let items = value.as_array()?;
    let verb = items.first()?.as_str()?;
    let sub_at = |i: usize| {
        let s = items.get(i)?.as_str()?;
        (is_base64url(s) && s.len() <= MAX_SUB_ID).then(|| s.to_owned())
    };
    let message_at = |i: usize| -> String {
        items
            .get(i)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .chars()
            .take(MAX_MESSAGE_CHARS)
            .collect()
    };
    match verb {
        "EVENT" if items.len() == 3 => Some(RelayFrame::Event {
            sub: sub_at(1)?,
            event: parse_event(items.get(2)?)?,
        }),
        "OK" if items.len() >= 3 => {
            let id = items.get(1)?.as_str()?;
            is_hex64(id).then_some(())?;
            Some(RelayFrame::Ok {
                id: id.to_owned(),
                accepted: items.get(2)?.as_bool()?,
                message: message_at(3),
            })
        }
        "EOSE" if items.len() == 2 => Some(RelayFrame::Eose { sub: sub_at(1)? }),
        "NOTICE" if items.len() >= 2 => Some(RelayFrame::Notice {
            message: message_at(1),
        }),
        "CLOSED" if items.len() >= 2 => Some(RelayFrame::Closed {
            sub: sub_at(1)?,
            message: message_at(2),
        }),
        "AUTH" => Some(RelayFrame::Auth),
        _ => None,
    }
}

// ---- relay answers ---------------------------------------------------------------------------

/// What a relay's refusal means for the mailbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Nothing to do (accepted, duplicate, or an unrelated notice).
    Fine,
    /// The relay objects to our clock: raise `clock_skew`, do not park.
    Clock,
    /// A refusal (blocked, restricted, authentication or proof of work, rate limit, any other
    /// rejection): park the relay.
    Park,
    /// The relay reports an internal error: treat the socket as down and reconnect with backoff.
    Down,
}

fn mentions_clock(lower: &str) -> bool {
    [
        "created_at",
        "creation",
        "too old",
        "too far",
        "too new",
        "future",
        "timestamp",
    ]
    .iter()
    .any(|w| lower.contains(w))
}

/// Classifies the `OK` answer to one of our events.
pub fn classify_ok(accepted: bool, message: &str) -> Verdict {
    if accepted {
        return Verdict::Fine;
    }
    let lower = message.to_ascii_lowercase();
    if lower.starts_with("duplicate") || lower.starts_with("mute:") {
        return Verdict::Fine;
    }
    if lower.starts_with("error:") {
        return Verdict::Down;
    }
    if mentions_clock(&lower) {
        return Verdict::Clock;
    }
    Verdict::Park
}

/// Classifies a `NOTICE`: only an obvious rate limit or refusal counts, other notices are noise.
pub fn classify_notice(message: &str) -> Verdict {
    let lower = message.to_ascii_lowercase();
    if mentions_clock(&lower) {
        return Verdict::Clock;
    }
    let refusal = [
        "rate-limit",
        "rate limit",
        "ratelimit",
        "too many",
        "blocked",
        "restricted",
        "auth-required",
        "pow:",
        "proof of work",
    ];
    if refusal.iter().any(|w| lower.contains(w)) {
        Verdict::Park
    } else {
        Verdict::Fine
    }
}

/// Classifies a `CLOSED` of our subscription (the relay ended it).
pub fn classify_closed(message: &str) -> Verdict {
    let lower = message.to_ascii_lowercase();
    let refusal = [
        "auth-required",
        "restricted",
        "blocked",
        "rate-limited",
        "rate limit",
        "pow:",
    ];
    if refusal.iter().any(|w| lower.contains(w)) {
        Verdict::Park
    } else {
        Verdict::Fine
    }
}

// ---- hex -------------------------------------------------------------------------------------

pub fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
        out.push(char::from_digit(u32::from(b & 0xf), 16).unwrap_or('0'));
    }
    out
}

/// Strict lowercase hex.
pub fn hex_decode(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    text.as_bytes()
        .chunks(2)
        .map(|pair| match pair {
            [h, l] => {
                let digit = |c: &u8| match c {
                    b'0'..=b'9' => Some(c - b'0'),
                    b'a'..=b'f' => Some(c - b'a' + 10),
                    _ => None,
                };
                Some((digit(h)? << 4) | digit(l)?)
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn psk() -> Vec<u8> {
        (0u8..32).collect()
    }

    fn keys() -> NostrKeys {
        NostrKeys::derive(&psk()).unwrap()
    }

    #[test]
    fn the_epoch_turns_over_at_noon_utc() {
        assert_eq!(epoch_of(0), 0);
        assert_eq!(epoch_of(43_199), 0);
        assert_eq!(epoch_of(43_200), 0);
        assert_eq!(epoch_of(43_200 + 86_399), 0);
        assert_eq!(epoch_of(43_200 + 86_400), 1);
        assert_eq!(epoch_start_s(1), 43_200 + 86_400);
        // 2026-10-25 12:00:00 UTC is exactly an epoch start.
        let noon = 1_792_929_600;
        assert_eq!(epoch_start_s(epoch_of(noon)), noon);
        assert_eq!(epoch_of(noon - 1) + 1, epoch_of(noon));
    }

    #[test]
    fn both_epochs_are_listened_to_for_the_hour_around_the_rollover() {
        let start = epoch_start_s(100);
        assert_eq!(epochs_to_listen(start + 3 * 3600), vec![100]);
        assert_eq!(epochs_to_listen(start + 29 * 60), vec![100, 99]);
        assert_eq!(epochs_to_listen(start + 30 * 60), vec![100]);
        assert_eq!(epochs_to_listen(start + DAY_S - 30 * 60 - 1), vec![100]);
        assert_eq!(epochs_to_listen(start + DAY_S - 30 * 60), vec![100, 101]);
        assert_eq!(epochs_to_listen(start + DAY_S - 1), vec![100, 101]);
        assert_eq!(epochs_to_listen(start + DAY_S), vec![101, 100]);
    }

    #[test]
    fn tags_and_keys_rotate_daily_and_differ_by_role() {
        let k = keys();
        let t1 = k.tag(7);
        assert_eq!(t1.len(), TAG_LEN);
        assert!(is_base64url(&t1));
        assert_ne!(t1, k.tag(8));
        assert_eq!(t1, k.tag(7));
        let baby = k.public_key_hex(Role::Baby, 7).unwrap();
        let parent = k.public_key_hex(Role::Parent, 7).unwrap();
        assert_ne!(baby, parent);
        assert_ne!(baby, k.public_key_hex(Role::Baby, 8).unwrap());
        assert!(is_hex64(&baby));
    }

    #[test]
    fn another_psk_has_other_tags_and_keys() {
        let other = NostrKeys::derive(&[9u8; 32]).unwrap();
        assert_ne!(keys().tag(7), other.tag(7));
        assert_ne!(
            keys().public_key_hex(Role::Baby, 7),
            other.public_key_hex(Role::Baby, 7)
        );
    }

    #[test]
    fn the_signature_matches_the_first_bip340_test_vector() {
        // BIP-340 test-vectors.csv, index 0: secret key 3, aux_rand and message all zero.
        let mut d = [0u8; 32];
        d[31] = 3;
        let key = scalar_key(&d).unwrap();
        assert_eq!(
            hex_encode(&key.verifying_key().to_bytes()),
            "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"
        );
        let sig = key.sign_raw(&[0u8; 32], &[0u8; 32]).unwrap();
        assert_eq!(
            hex_encode(&sig.to_bytes()),
            "e907831f80848d1069a5371b402410364bdf1c5f8307b0084c55f1ce2dca8215\
             25f66a4a85ea8b71e482a74f382d2ce5ebeee8fdb2172f477df4900d310536c0"
        );
    }

    #[test]
    fn a_zero_scalar_gives_no_key() {
        assert!(scalar_key(&[0u8; 32]).is_none());
        // The curve order reduces to zero as well.
        let n =
            hex_decode("fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141").unwrap();
        let n: [u8; 32] = n.try_into().unwrap();
        assert!(scalar_key(&n).is_none());
        let mut one = [0u8; 32];
        one[31] = 1;
        assert!(scalar_key(&one).is_some());
    }

    #[test]
    fn a_signed_event_verifies_and_a_changed_one_does_not() {
        let k = keys();
        let sk = k.signing_key(Role::Baby, 7).unwrap();
        let tag = k.tag(7);
        let ev = build_event(&sk, &[3u8; 32], 1_700_000_000, &tag, "QUJD-_x").unwrap();
        assert_eq!(ev.kind, KIND);
        assert_eq!(ev.tag_value(), Some(tag.as_str()));
        assert!(verify_event(&ev));
        for mutate in [
            |e: &mut Event| e.created_at += 1,
            |e: &mut Event| e.content.push('A'),
            |e: &mut Event| e.kind += 1,
            |e: &mut Event| e.id = "0".repeat(64),
            |e: &mut Event| e.sig = "0".repeat(128),
            |e: &mut Event| e.tags[0][1] = "other".into(),
        ] {
            let mut bad = ev.clone();
            mutate(&mut bad);
            assert!(!verify_event(&bad));
        }
    }

    #[test]
    fn the_event_round_trips_through_its_json() {
        let k = keys();
        let sk = k.signing_key(Role::Parent, 7).unwrap();
        let ev = build_event(&sk, &[1u8; 32], 5, &k.tag(7), "AAAA").unwrap();
        let frame = format!("[\"EVENT\",\"sub1\",{}]", ev.to_json());
        let Some(RelayFrame::Event { sub, event }) = parse_relay_frame(&frame) else {
            panic!("event frame");
        };
        assert_eq!(sub, "sub1");
        assert_eq!(event, ev);
        assert!(event_frame(&ev).starts_with("[\"EVENT\",{\"id\":\""));
    }

    #[test]
    fn content_and_tags_outside_base64url_are_refused() {
        let k = keys();
        let sk = k.signing_key(Role::Baby, 7).unwrap();
        assert!(build_event(&sk, &[1u8; 32], 5, &k.tag(7), "a b").is_none());
        assert!(build_event(&sk, &[1u8; 32], 5, &k.tag(7), "é").is_none());
        assert!(build_event(&sk, &[1u8; 32], 5, "t\"ag", "AAAA").is_none());
        let ev = build_event(&sk, &[1u8; 32], 5, &k.tag(7), "AAAA").unwrap();
        for (from, to) in [("\"AAAA\"", "\"AA AA\""), ("\"AAAA\"", "\"AA\\u00e9\"")] {
            let json = ev.to_json().replace(from, to);
            assert!(parse_relay_frame(&format!("[\"EVENT\",\"s\",{json}]")).is_none());
        }
        // A tag with a space or an extra, non-ASCII tag is refused too.
        let json = ev.to_json().replace("\"t\"", "\"t t\"");
        assert!(parse_relay_frame(&format!("[\"EVENT\",\"s\",{json}]")).is_none());
    }

    #[test]
    fn frames_have_the_nip01_shape() {
        let f = Filter {
            author: "ab".repeat(32),
            tag: "TAG".into(),
            since: 99,
        };
        let req = req_frame("sub-1", &[f.clone(), f]).unwrap();
        assert_eq!(
            req,
            format!(
                "[\"REQ\",\"sub-1\",{{\"kinds\":[{KIND}],\"authors\":[\"{a}\"],\"#t\":[\"TAG\"],\"since\":99}},{{\"kinds\":[{KIND}],\"authors\":[\"{a}\"],\"#t\":[\"TAG\"],\"since\":99}}]",
                a = "ab".repeat(32)
            )
        );
        assert_eq!(close_frame("sub-1").unwrap(), "[\"CLOSE\",\"sub-1\"]");
        assert!(req_frame("bad id", &[]).is_none());
        assert!(close_frame("a\"b").is_none());
        let probe = probe_req_frame("p1", &"ab".repeat(32)).unwrap();
        assert_eq!(
            probe,
            format!(
                "[\"REQ\",\"p1\",{{\"ids\":[\"{}\"],\"limit\":1}}]",
                "ab".repeat(32)
            )
        );
        assert!(!probe.contains("authors") && !probe.contains(&KIND.to_string()));
        assert!(probe_req_frame("p1", "RANDOM").is_none());
    }

    #[test]
    fn relay_frames_parse() {
        let id = "a".repeat(64);
        assert_eq!(
            parse_relay_frame(&format!("[\"OK\",\"{id}\",true,\"\"]")),
            Some(RelayFrame::Ok {
                id: id.clone(),
                accepted: true,
                message: String::new()
            })
        );
        assert_eq!(
            parse_relay_frame(&format!("[\"OK\",\"{id}\",false,\"blocked: no\"]")),
            Some(RelayFrame::Ok {
                id,
                accepted: false,
                message: "blocked: no".into()
            })
        );
        assert_eq!(
            parse_relay_frame("[\"EOSE\",\"s1\"]"),
            Some(RelayFrame::Eose { sub: "s1".into() })
        );
        assert_eq!(
            parse_relay_frame("[\"NOTICE\",\"hi\"]"),
            Some(RelayFrame::Notice {
                message: "hi".into()
            })
        );
        assert_eq!(
            parse_relay_frame("[\"CLOSED\",\"s1\",\"auth-required: x\"]"),
            Some(RelayFrame::Closed {
                sub: "s1".into(),
                message: "auth-required: x".into()
            })
        );
        assert_eq!(
            parse_relay_frame("[\"AUTH\",\"c\"]"),
            Some(RelayFrame::Auth)
        );
        for bad in [
            "",
            "{}",
            "[]",
            "[1]",
            "[\"EOSE\"]",
            "[\"EOSE\",\"has space\"]",
            "[\"OK\",\"short\",true,\"\"]",
            "[\"OK\",\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",1,\"\"]",
            "[\"WHAT\",1]",
            "[\"EVENT\",\"s\"]",
        ] {
            assert!(parse_relay_frame(bad).is_none(), "{bad}");
        }
        let long = format!("[\"NOTICE\",\"{}\"]", "x".repeat(MAX_FRAME_BYTES));
        assert!(parse_relay_frame(&long).is_none());
    }

    #[test]
    fn refusals_are_classified() {
        assert_eq!(classify_ok(true, ""), Verdict::Fine);
        assert_eq!(classify_ok(false, "duplicate: have it"), Verdict::Fine);
        assert_eq!(classify_ok(false, "blocked: not allowed"), Verdict::Park);
        assert_eq!(classify_ok(false, "rate-limited: slow down"), Verdict::Park);
        assert_eq!(classify_ok(false, "restricted: x"), Verdict::Park);
        assert_eq!(classify_ok(false, "auth-required: x"), Verdict::Park);
        assert_eq!(classify_ok(false, "pow: need 20 bits"), Verdict::Park);
        assert_eq!(classify_ok(false, ""), Verdict::Park);
        assert_eq!(classify_ok(false, "error: shutting down"), Verdict::Down);
        assert_eq!(classify_ok(false, "mute: you are muted"), Verdict::Fine);
        assert_eq!(
            classify_ok(false, "invalid: event creation date is too far off"),
            Verdict::Clock
        );
        assert_eq!(
            classify_ok(false, "invalid: created_at too old"),
            Verdict::Clock
        );
        assert_eq!(classify_notice("ERROR: rate limit exceeded"), Verdict::Park);
        assert_eq!(classify_notice("hello"), Verdict::Fine);
        assert_eq!(
            classify_notice("created_at is in the future"),
            Verdict::Clock
        );
        assert_eq!(classify_closed("auth-required: x"), Verdict::Park);
        assert_eq!(classify_closed("error: shutting down"), Verdict::Fine);
    }

    mod props {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// Whatever a relay sends, parsing never panics, and an event that parses stays inside
            /// the permitted alphabets and survives a round trip through our own serialisation.
            #[test]
            fn arbitrary_frames_never_panic(text in "\\[\"(EVENT|OK|EOSE|NOTICE|CLOSED|AUTH)\",.{0,300}") {
                if let Some(RelayFrame::Event { sub, event }) = parse_relay_frame(&text) {
                    prop_assert!(is_base64url(&event.content));
                    prop_assert!(is_base64url(&sub));
                    let _ = verify_event(&event);
                }
            }

            #[test]
            fn arbitrary_text_never_panics(text in ".{0,400}") {
                let _ = parse_relay_frame(&text);
            }

            #[test]
            fn a_built_event_always_verifies_and_round_trips(
                content in "[A-Za-z0-9_-]{1,200}",
                created_at in 0u64..4_000_000_000,
                aux in proptest::array::uniform32(any::<u8>()),
            ) {
                let k = keys();
                let epoch = epoch_of(created_at);
                let sk = k.signing_key(Role::Baby, epoch).unwrap();
                let ev = build_event(&sk, &aux, created_at, &k.tag(epoch), &content).unwrap();
                prop_assert!(verify_event(&ev));
                let frame = format!("[\"EVENT\",\"s\",{}]", ev.to_json());
                prop_assert_eq!(
                    parse_relay_frame(&frame),
                    Some(RelayFrame::Event { sub: "s".into(), event: ev })
                );
            }
        }
    }

    #[test]
    fn hex_is_strict_lowercase() {
        assert_eq!(hex_decode("00ff"), Some(vec![0, 255]));
        assert_eq!(hex_decode("00FF"), None);
        assert_eq!(hex_decode("0"), None);
        assert_eq!(hex_encode(&[1, 171]), "01ab");
    }
}
