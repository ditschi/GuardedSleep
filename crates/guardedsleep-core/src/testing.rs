//! Test support for sealed signaling, shared by the integration tests and the simulation: a peer
//! that holds the PSK and speaks sealed envelopes (so it can run the confirmation handshake with a
//! `Link` and say sealed messages), and readers for what a device sent. Only in test and
//! `testing` builds: a release build has no way to read or forge an envelope without a session.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chacha20::ChaCha20Rng;
use chacha20::rand_core::SeedableRng;
use serde_json::{Value, json};

use crate::envelope::{
    MessageClass, NONCE_LEN, OpenError, Opened, Purpose, Sealed, SecretKey, SessionId,
    SessionState, derive_key, peek, reseal,
};
use crate::link::{Link, LinkAction, LinkInput};
use crate::protocol::Role;

/// The presence message every `here` carries.
pub const HERE: &[u8] = br#"{"type":"here"}"#;

fn wrap(sealed: &Sealed) -> Value {
    json!({ "env": URL_SAFE_NO_PAD.encode(&sealed.bytes) })
}

/// A device or attacker that holds the PSK and keeps its own session state.
pub struct SealedPeer {
    pub role: Role,
    key: SecretKey,
    rng: ChaCha20Rng,
    pub sess: SessionState,
}

impl SealedPeer {
    /// `seed` makes the peer's nonces and session reproducible.
    pub fn new(role: Role, psk: &[u8], seed: u8) -> Self {
        let key = derive_key(psk, Purpose::Seal).expect("psk is not empty");
        let mut rng = ChaCha20Rng::from_seed([seed; 32]);
        let sess = SessionState::new(role, SessionId::random(&mut rng));
        SealedPeer {
            role,
            key,
            rng,
            sess,
        }
    }

    /// An unsolicited presence message, as a relay `signal` payload.
    pub fn here(&mut self) -> Value {
        let sealed = self
            .sess
            .seal(
                &self.key,
                &mut self.rng,
                MessageClass::Here,
                SessionId::NONE,
                HERE,
            )
            .expect("a here fits its class");
        wrap(&sealed)
    }

    /// A message sealed to the confirmed session of the other side; `None` without one.
    pub fn try_say(&mut self, class: MessageClass, message: &Value) -> Option<Value> {
        let sealed = self
            .sess
            .seal_to_peer(
                &self.key,
                &mut self.rng,
                class,
                message.to_string().as_bytes(),
            )
            .ok()?;
        Some(wrap(&sealed))
    }

    /// Like [`Self::try_say`]; panics when the other side never confirmed (a test bug).
    pub fn say(&mut self, class: MessageClass, message: &Value) -> Value {
        self.try_say(class, message)
            .expect("the other side's session is confirmed")
    }

    /// A message sealed to the newest session seen from the other side even though it never
    /// echoed a challenge (an attacker does not wait for a reply that will not come).
    pub fn say_unconfirmed(&mut self, class: MessageClass, message: &Value) -> Option<Value> {
        let to = self
            .sess
            .peer_session()
            .or_else(|| self.sess.newest_candidate())?;
        Some(self.say_to(class, to, message))
    }

    /// A message sealed to an explicit recipient session.
    pub fn say_to(&mut self, class: MessageClass, to: SessionId, message: &Value) -> Value {
        let sealed = self
            .sess
            .seal(
                &self.key,
                &mut self.rng,
                class,
                to,
                message.to_string().as_bytes(),
            )
            .expect("the message fits its class");
        wrap(&sealed)
    }

    /// Opens a payload; the `here` it answers with (challenge or echo) comes back as a payload.
    pub fn accept(
        &mut self,
        now: u64,
        payload: &Value,
    ) -> Result<(Opened, Option<Value>), OpenError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(payload["env"].as_str().ok_or(OpenError::Malformed)?)
            .map_err(|_| OpenError::Malformed)?;
        let opened = self.sess.open(&self.key, &mut self.rng, now, &bytes)?;
        let reply = opened.reply.map(|r| {
            let sealed = self
                .sess
                .seal_reply(&self.key, &mut self.rng, &r, HERE)
                .expect("a here fits its class");
            wrap(&sealed)
        });
        Ok((opened, reply))
    }
}

/// The message inside a wire payload under `psk`, if it is an envelope of that key. This is what a
/// PSK holder in a mailbox reads; the tests use it to see what a device sent.
pub fn decode_message(psk: &[u8], payload: &Value) -> Option<Value> {
    let key = derive_key(psk, Purpose::Seal)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload["env"].as_str()?).ok()?;
    serde_json::from_slice(&peek(&key, &bytes)?.message).ok()
}

/// The nonce (envelope id) of a wire payload, read from the clear header: what a relay that wants to
/// rewrite an envelope without invalidating a parent tag (which depends on the nonce alone) reuses.
pub fn nonce_of(payload: &Value) -> Option<[u8; NONCE_LEN]> {
    let bytes = URL_SAFE_NO_PAD.decode(payload["env"].as_str()?).ok()?;
    // version, role, class, then the nonce
    <[u8; NONCE_LEN]>::try_from(bytes.get(3..3 + NONCE_LEN)?).ok()
}

/// A PSK-holding intermediary: decrypts `payload`, lets `edit` change the message and seals it
/// again into the same frame (same role, class, sessions and sequence number) with `nonce`.
pub fn rewrite_message(
    psk: &[u8],
    payload: &Value,
    nonce: [u8; NONCE_LEN],
    edit: impl FnOnce(&mut Value),
) -> Option<Value> {
    let key = derive_key(psk, Purpose::Seal)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload["env"].as_str()?).ok()?;
    let peeked = peek(&key, &bytes)?;
    let mut message: Value = serde_json::from_slice(&peeked.message).ok()?;
    edit(&mut message);
    let sealed = reseal(&key, &peeked, message.to_string().as_bytes(), nonce).ok()?;
    Some(wrap(&sealed))
}

/// The `(to, payload)` of every relay `signal` frame in `actions`.
pub fn signals_in(actions: &[LinkAction]) -> Vec<(String, Value)> {
    actions
        .iter()
        .filter_map(|a| match a {
            LinkAction::MailboxSend { frame, .. } => {
                let v: Value = serde_json::from_str(frame).ok()?;
                (v["type"] == "signal").then(|| {
                    (
                        v["to"].as_str().unwrap_or("").to_owned(),
                        v["payload"].clone(),
                    )
                })
            }
            _ => None,
        })
        .collect()
}

/// The input that delivers a relay `signal` from `from`.
pub fn signal_frame(from: &str, payload: &Value) -> LinkInput {
    LinkInput::MailboxFrame {
        id: 0,
        bytes: json!({ "type": "signal", "from": from, "payload": payload }).to_string(),
    }
}

/// Runs the confirmation handshake between a hand-driven `link` and `peer` (client id `peer_id` on
/// the relay) until both confirmed. Returns every action the link took.
pub fn handshake(
    link: &mut Link,
    now: u64,
    peer: &mut SealedPeer,
    peer_id: &str,
) -> Vec<LinkAction> {
    let mut all = Vec::new();
    let mut pending = vec![peer.here()];
    for _ in 0..8 {
        let mut next = Vec::new();
        for p in pending.drain(..) {
            let acts = link.handle(now, signal_frame(peer_id, &p));
            for (to, payload) in signals_in(&acts) {
                if to == peer_id
                    && let Ok((_, Some(reply))) = peer.accept(now, &payload)
                {
                    next.push(reply);
                }
            }
            all.extend(acts);
        }
        if next.is_empty() {
            break;
        }
        pending = next;
    }
    all
}
