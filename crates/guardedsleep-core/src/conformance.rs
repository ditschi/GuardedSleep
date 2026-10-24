//! Evaluates one conformance case (JSON in, JSON out). Every binding (Rust, Kotlin via UniFFI,
//! WASM) runs `vectors/conformance.json` through this entry point and compares against `expect`,
//! so all platforms provably share one behaviour.

use serde_json::{Value, json};

use crate::envelope::{
    Frame, MessageClass, NONCE_LEN, Purpose, SessionId, SessionState, derive_key, seal_with_nonce,
};
use crate::invite::{build_invite, parse_invite};
use crate::link::{Link, LinkConfig, LinkInput};
use crate::pairing::{
    AdmissionKey, admission_mac, decode_psk, derive_admission_key, derive_room_id, encode_psk,
    pair_proof_mac_v2_legacy, pair_proof_mac_v3, stop_notice_mac, verify_admission,
};
use crate::protocol::{
    Role, parse_challenge, parse_client_message, parse_data_message, parse_data_message_v2_legacy,
    parse_server_message,
};
use crate::session::{Input, Session, SessionConfig};

pub fn evaluate(case_json: &str) -> String {
    match evaluate_value(case_json) {
        Ok(v) => v.to_string(),
        Err(e) => json!({ "error": e }).to_string(),
    }
}

fn evaluate_value(case_json: &str) -> Result<Value, String> {
    let case: Value = serde_json::from_str(case_json).map_err(|e| e.to_string())?;
    if let Some(schema) = case.get("schema") {
        return eval_trace(&case, schema);
    }
    let kind = case
        .get("kind")
        .and_then(Value::as_str)
        .ok_or("missing kind")?;
    let s = |k: &str| {
        case.get(k)
            .and_then(Value::as_str)
            .ok_or(format!("missing {k}"))
    };
    match kind {
        "parse_server" => to_value(parse_server_message(s("raw")?)),
        "parse_client" => to_value(parse_client_message(s("raw")?)),
        "parse_challenge" => to_value(parse_challenge(s("raw")?)),
        "admission_key" => {
            let psk = decode_psk(s("psk_b64url")?).ok_or("bad psk")?;
            to_value(derive_admission_key(&psk).to_b64url())
        }
        "admission_mac" | "admission_verify" => {
            let key = AdmissionKey::from_b64url(s("key_b64url")?).ok_or("bad key")?;
            let role = match s("role")? {
                "baby" => Role::Baby,
                "parent" => Role::Parent,
                other => return Err(format!("bad role {other}")),
            };
            if kind == "admission_mac" {
                to_value(admission_mac(&key, s("nonce")?, role, s("room")?))
            } else {
                to_value(verify_admission(
                    &key,
                    s("nonce")?,
                    role,
                    s("room")?,
                    s("mac")?,
                ))
            }
        }
        "parse_data" => to_value(parse_data_message_v2_legacy(s("raw")?)),
        "parse_data_v3" => to_value(parse_data_message(s("raw")?)),
        "derive_room" => {
            let psk = decode_psk(s("psk_b64url")?).ok_or("bad psk")?;
            to_value(derive_room_id(&psk))
        }
        "pair_mac_v3" => {
            let psk = decode_psk(s("psk_b64url")?).ok_or("bad psk")?;
            let role = match s("sender_role")? {
                "baby" => Role::Baby,
                "parent" => Role::Parent,
                other => return Err(format!("bad sender_role {other}")),
            };
            to_value(pair_proof_mac_v3(
                &psk,
                role,
                s("room")?,
                s("fp_sender")?,
                s("fp_receiver")?,
            ))
        }
        "stop_mac" => {
            let psk = decode_psk(s("psk_b64url")?).ok_or("bad psk")?;
            let role = match s("sender_role")? {
                "baby" => Role::Baby,
                "parent" => Role::Parent,
                other => return Err(format!("bad sender_role {other}")),
            };
            to_value(stop_notice_mac(
                &psk,
                role,
                s("room")?,
                s("epoch")?,
                s("receiver_client_id")?,
                s("sender_aid")?,
            ))
        }
        "pair_mac" => {
            let psk = decode_psk(s("psk_b64url")?).ok_or("bad psk")?;
            to_value(pair_proof_mac_v2_legacy(
                &psk,
                s("room")?,
                s("local_sdp")?,
                s("remote_sdp")?,
            ))
        }
        "parse_invite" => {
            let now = case
                .get("now_epoch_s")
                .and_then(Value::as_u64)
                .ok_or("missing now_epoch_s")?;
            Ok(match parse_invite(s("text")?, now) {
                Ok(inv) => json!({ "ok": {
                    "psk_b64url": encode_psk(inv.psk()),
                    "urls": inv.urls,
                    "expires_at_epoch_s": inv.expires_at_epoch_s,
                } }),
                Err(e) => json!({ "error": e.as_str() }),
            })
        }
        "build_invite" => {
            let psk = decode_psk(s("psk_b64url")?).ok_or("bad psk")?;
            let urls: Vec<String> =
                serde_json::from_value(case.get("urls").cloned().ok_or("missing urls")?)
                    .map_err(|e| e.to_string())?;
            let expires = case
                .get("expires_at_epoch_s")
                .and_then(Value::as_u64)
                .ok_or("missing expires_at_epoch_s")?;
            Ok(match build_invite(&psk, &urls, expires) {
                Ok(text) => json!({ "ok": text }),
                Err(e) => json!({ "error": e.as_str() }),
            })
        }
        "envelope_key" => {
            let psk = decode_psk(s("psk_b64url")?).ok_or("bad psk")?;
            let purpose = match s("purpose")? {
                "seal" => Purpose::Seal,
                "nostr_tag" => Purpose::NostrTag,
                "nostr_key" => Purpose::NostrKey,
                "relay_pick" => Purpose::RelayPick,
                other => return Err(format!("bad purpose {other}")),
            };
            let key = derive_key(&psk, purpose).ok_or("empty psk")?;
            to_value(to_hex(key.as_bytes()))
        }
        "envelope_seal" => envelope_seal(&case),
        "envelope_open" => envelope_open(&case),
        "session" => run_session(&case),
        "link" => run_link(&case),
        other => Err(format!("unknown kind {other}")),
    }
}

/// A `gs-trace/1` document (docs/technical/test-schemas.md): replays `steps` and returns one
/// action list per step. A config with `alarm_after_ms` is a `Link`, otherwise a `Session`.
fn eval_trace(trace: &Value, schema: &Value) -> Result<Value, String> {
    if schema != "gs-trace/1" {
        return Err(format!("unsupported trace schema {schema}"));
    }
    let is_link = trace
        .get("config")
        .is_some_and(|c| c.get("alarm_after_ms").is_some());
    if is_link {
        run_link(trace)
    } else {
        run_session(trace)
    }
}

fn run_session(case: &Value) -> Result<Value, String> {
    let cfg: SessionConfig =
        serde_json::from_value(case.get("config").cloned().ok_or("missing config")?)
            .map_err(|e| e.to_string())?;
    let mut session = Session::new(cfg).map_err(|e| e.to_string())?;
    let steps = case
        .get("steps")
        .and_then(Value::as_array)
        .ok_or("missing steps")?;
    let mut results = Vec::with_capacity(steps.len());
    for step in steps {
        let now = step.get("now").and_then(Value::as_u64).ok_or("step.now")?;
        let input: Input = serde_json::from_value(step.get("input").cloned().ok_or("step.input")?)
            .map_err(|e| e.to_string())?;
        results.push(to_value(session.handle(now, input))?);
    }
    Ok(Value::Array(results))
}

fn run_link(case: &Value) -> Result<Value, String> {
    let cfg: LinkConfig =
        serde_json::from_value(case.get("config").cloned().ok_or("missing config")?)
            .map_err(|e| e.to_string())?;
    let mut link = Link::new(cfg).map_err(|e| e.to_string())?;
    let steps = case
        .get("steps")
        .and_then(Value::as_array)
        .ok_or("missing steps")?;
    let mut results = Vec::with_capacity(steps.len());
    for step in steps {
        let now = step.get("now").and_then(Value::as_u64).ok_or("step.now")?;
        let input: LinkInput =
            serde_json::from_value(step.get("input").cloned().ok_or("step.input")?)
                .map_err(|e| e.to_string())?;
        results.push(to_value(link.handle(now, input))?);
    }
    Ok(Value::Array(results))
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

fn envelope_key(case: &Value) -> Result<crate::envelope::SecretKey, String> {
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
    let envelopes = case
        .get("envelopes")
        .and_then(Value::as_array)
        .ok_or("missing envelopes")?;
    let mut results = Vec::with_capacity(envelopes.len());
    for (i, e) in envelopes.iter().enumerate() {
        let bytes = from_hex(e.as_str().unwrap_or_default()).ok_or("bad envelope hex")?;
        let now = u64::try_from(i).map_err(|e| e.to_string())? * 10_000;
        results.push(match state.open(&key, now, &bytes) {
            Ok(o) => json!({ "ok": {
                "class": o.class.name(),
                "sender_session": to_hex(o.sender_session.as_bytes()),
                "recipient_session": to_hex(o.recipient_session.as_bytes()),
                "seq": o.seq,
                "message": String::from_utf8_lossy(&o.message),
                "confirmed": o.confirmed,
                "reply_to": o.reply_to.map(|s| to_hex(s.as_bytes())),
            } }),
            Err(e) => json!({ "error": e.as_str() }),
        });
    }
    Ok(Value::Array(results))
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

fn to_value<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_vectors_match() {
        let raw = include_str!("../vectors/conformance.json");
        let cases: Vec<Value> = serde_json::from_str(raw).unwrap();
        assert!(!cases.is_empty());
        for case in cases {
            let name = case["name"].as_str().unwrap_or("?").to_owned();
            let got: Value = serde_json::from_str(&evaluate(&case.to_string())).unwrap();
            assert_eq!(got, case["expect"], "vector `{name}`");
        }
    }

    #[test]
    fn unknown_trace_schema_is_rejected() {
        let got: Value = serde_json::from_str(&evaluate(
            r#"{"schema":"gs-trace/2","config":{},"steps":[]}"#,
        ))
        .unwrap();
        assert!(got["error"].is_string());
    }
}
