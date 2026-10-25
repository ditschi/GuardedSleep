//! Conformance cases for the sealed envelope (`envelope_key`, `envelope_seal`, `envelope_open`).
//! Compiled only for tests and the `conformance` cargo feature: it needs fixed-nonce sealing and
//! raw key export, which a release build must not contain.

use chacha20::ChaCha20Rng;
use chacha20::rand_core::SeedableRng;
use serde_json::{Value, json};

use crate::envelope::{
    CHALLENGE_LEN, Frame, MessageClass, NONCE_LEN, Purpose, SecretKey, SessionId, SessionState,
    derive_key, seal_with_nonce,
};
use crate::pairing::decode_psk;
use crate::protocol::Role;

pub(crate) fn evaluate(kind: &str, case: &Value) -> Result<Value, String> {
    match kind {
        "envelope_key" => {
            let psk = decode_psk(str_field(case, "psk_b64url")?).ok_or("bad psk")?;
            let purpose = Purpose::from_name(str_field(case, "purpose")?).ok_or("bad purpose")?;
            let key = derive_key(&psk, purpose).ok_or("empty psk")?;
            Ok(Value::String(to_hex(key.as_bytes())))
        }
        "envelope_seal" => envelope_seal(case),
        _ => envelope_open(case),
    }
}

fn str_field<'a>(case: &'a Value, k: &str) -> Result<&'a str, String> {
    case.get(k)
        .and_then(Value::as_str)
        .ok_or(format!("missing {k}"))
}

fn role_field(case: &Value, k: &str) -> Result<Role, String> {
    match str_field(case, k)? {
        "baby" => Ok(Role::Baby),
        "parent" => Ok(Role::Parent),
        other => Err(format!("bad {k} {other}")),
    }
}

fn session_field(case: &Value, k: &str) -> Result<SessionId, String> {
    session_from_hex(str_field(case, k)?)
}

fn session_from_hex(text: &str) -> Result<SessionId, String> {
    let bytes = from_hex(text).ok_or("bad hex")?;
    let arr: [u8; 16] = bytes.try_into().map_err(|_| "session must be 16 bytes")?;
    Ok(SessionId::new(arr))
}

/// An optional 16-byte hex field; absent means all zeros.
fn challenge_field(case: &Value, k: &str) -> Result<[u8; CHALLENGE_LEN], String> {
    match case.get(k).and_then(Value::as_str) {
        None => Ok([0; CHALLENGE_LEN]),
        Some(text) => from_hex(text)
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| format!("{k} must be 16 bytes")),
    }
}

fn envelope_key(case: &Value) -> Result<SecretKey, String> {
    let psk = decode_psk(str_field(case, "psk_b64url")?).ok_or("bad psk")?;
    derive_key(&psk, Purpose::Seal).ok_or_else(|| "empty psk".to_owned())
}

/// Seals with a fixed nonce, so the output is a reproducible golden value.
fn envelope_seal(case: &Value) -> Result<Value, String> {
    let key = envelope_key(case)?;
    let class = MessageClass::from_name(str_field(case, "class")?).ok_or("bad class")?;
    let nonce: [u8; NONCE_LEN] = from_hex(str_field(case, "nonce")?)
        .and_then(|b| b.try_into().ok())
        .ok_or("nonce must be 24 bytes")?;
    let frame = Frame {
        role: role_field(case, "role")?,
        class,
        recipient: session_field(case, "recipient_session")?,
        sender: session_field(case, "sender_session")?,
        seq: case
            .get("seq")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or("missing seq")?,
        challenge: challenge_field(case, "challenge")?,
        echo: challenge_field(case, "echo")?,
        parent_tag: challenge_field(case, "parent_tag")?,
    };
    // "message_repeat" builds large messages without a huge vector file.
    let mut message = str_field(case, "message")?.as_bytes().to_vec();
    if let Some(n) = case.get("message_repeat").and_then(Value::as_u64) {
        message = message.repeat(usize::try_from(n).map_err(|e| e.to_string())?);
    }
    Ok(match seal_with_nonce(&key, &frame, &message, nonce) {
        Ok(sealed) => json!({ "ok": to_hex(&sealed.bytes) }),
        Err(e) => json!({ "error": format!("{e:?}").to_ascii_lowercase() }),
    })
}

/// Opens a sequence of envelopes in one receiver state; the result has one entry per envelope.
/// The challenges of replies come from a generator seeded with `rng_seed` (default 32 bytes of
/// 0x42), so the golden output is reproducible. `times` (monotonic ms per envelope) defaults to
/// 10 s apart.
fn envelope_open(case: &Value) -> Result<Value, String> {
    let key = envelope_key(case)?;
    let peer = case
        .get("peer_session")
        .and_then(Value::as_str)
        .map(session_from_hex)
        .transpose()?;
    let retired: Vec<SessionId> = case
        .get("retired_sessions")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|v| session_from_hex(v.as_str().unwrap_or_default()))
                .collect::<Result<_, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    let mut state = SessionState::with_peer(
        role_field(case, "role")?,
        session_field(case, "own_session")?,
        peer,
        &retired,
    );
    let mut rng = ChaCha20Rng::from_seed(
        challenge_seed(case.get("rng_seed").and_then(Value::as_str)).ok_or("bad rng_seed")?,
    );
    let envelopes = case
        .get("envelopes")
        .and_then(Value::as_array)
        .ok_or("missing envelopes")?;
    let times: Option<Vec<u64>> = case
        .get("times")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_u64).collect());
    let mut results = Vec::with_capacity(envelopes.len());
    for (i, e) in envelopes.iter().enumerate() {
        let bytes = from_hex(e.as_str().unwrap_or_default()).ok_or("bad envelope hex")?;
        let now = match &times {
            Some(t) => *t.get(i).ok_or("times shorter than envelopes")?,
            None => u64::try_from(i).map_err(|e| e.to_string())? * 10_000,
        };
        results.push(match state.open(&key, &mut rng, now, &bytes) {
            Ok(o) => json!({ "ok": {
                "class": o.class.name(),
                "sender_session": to_hex(o.sender_session.as_bytes()),
                "recipient_session": to_hex(o.recipient_session.as_bytes()),
                "seq": o.seq,
                "message": String::from_utf8_lossy(&o.message),
                "confirmed": o.confirmed,
                "reply": o.reply.map(|r| json!({
                    "to": to_hex(r.to.as_bytes()),
                    "challenge": to_hex(&r.challenge),
                    "echo": to_hex(&r.echo),
                })),
            } }),
            Err(e) => json!({ "error": e.as_str() }),
        });
    }
    Ok(Value::Array(results))
}

fn challenge_seed(text: Option<&str>) -> Option<[u8; 32]> {
    match text {
        None => Some([0x42; 32]),
        Some(t) => from_hex(t)?.try_into().ok(),
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    text.as_bytes()
        .chunks(2)
        .map(|pair| {
            let digit = |c: &u8| char::from(*c).to_digit(16).map(|v| v as u8);
            match pair {
                [h, l] => Some((digit(h)? << 4) | digit(l)?),
                _ => None,
            }
        })
        .collect()
}
