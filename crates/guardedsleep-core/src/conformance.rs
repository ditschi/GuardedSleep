//! Evaluates one conformance case (JSON in, JSON out). Every binding (Rust, Kotlin via UniFFI,
//! WASM) runs `vectors/conformance.json` through this entry point and compares against `expect`,
//! so all platforms provably share one behaviour.

use serde_json::{Value, json};

use crate::envelope::{MessageClass, TagKey, envelope_ad, pin_tag};
use crate::invite::{InviteMailbox, build_invite, parse_invite};
use crate::link::{Link, LinkConfig, LinkInput};
use crate::pairing::{
    AdmissionKey, PinPurpose, admission_mac, baby_proof_mac, decode_psk, derive_admission_key,
    derive_room_id, encode_psk, pair_proof_mac_v2_legacy, pair_proof_mac_v3, parent_proof_mac,
    pin_subkey, verify_admission,
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
        "pair_mac" => {
            let psk = decode_psk(s("psk_b64url")?).ok_or("bad psk")?;
            to_value(pair_proof_mac_v2_legacy(
                &psk,
                s("room")?,
                s("local_sdp")?,
                s("remote_sdp")?,
            ))
        }
        "parent_proof" => {
            let secret = hex_bytes(s("secret")?).ok_or("bad secret")?;
            to_value(parent_proof_mac(&secret, s("fp_parent")?, s("fp_baby")?))
        }
        "baby_proof" => {
            let secret = hex_bytes(s("secret")?).ok_or("bad secret")?;
            to_value(baby_proof_mac(&secret, s("fp_baby")?, s("fp_parent")?))
        }
        "pin_subkey" => {
            let secret = hex_bytes(s("secret")?).ok_or("bad secret")?;
            let purpose = PinPurpose::from_name(s("purpose")?).ok_or("bad purpose")?;
            to_value(pin_subkey(&secret, purpose).map(|k| hex_text(&k)))
        }
        "pin_tag" => {
            let secret = hex_bytes(s("secret")?).ok_or("bad secret")?;
            let purpose = PinPurpose::from_name(s("purpose")?).ok_or("bad purpose")?;
            let key = pin_subkey(&secret, purpose).ok_or("bad secret")?;
            let nonce: [u8; 24] = hex_bytes(s("nonce")?)
                .and_then(|b| b.try_into().ok())
                .ok_or("bad nonce")?;
            let role = match s("role")? {
                "baby" => Role::Baby,
                "parent" => Role::Parent,
                other => return Err(format!("bad role {other}")),
            };
            let class = MessageClass::from_name(s("class")?).ok_or("bad class")?;
            let body = hex_bytes(s("body")?).ok_or("bad body")?;
            let tag = pin_tag(&TagKey(key), &nonce, &envelope_ad(role, class), &body);
            to_value(hex_text(&tag))
        }
        "parse_invite" => {
            let now = case
                .get("now_epoch_s")
                .and_then(Value::as_u64)
                .ok_or("missing now_epoch_s")?;
            Ok(match parse_invite(s("text")?, now, dev_loopback(&case)) {
                Ok(inv) => json!({ "ok": {
                    "psk_b64url": encode_psk(inv.psk()),
                    "mailboxes": inv.mailboxes,
                    "expires_at_epoch_s": inv.expires_at_epoch_s,
                } }),
                Err(e) => json!({ "error": e.as_str() }),
            })
        }
        "build_invite" => {
            let psk = decode_psk(s("psk_b64url")?).ok_or("bad psk")?;
            let mailboxes: Vec<InviteMailbox> =
                serde_json::from_value(case.get("mailboxes").cloned().ok_or("missing mailboxes")?)
                    .map_err(|e| e.to_string())?;
            let expires = case
                .get("expires_at_epoch_s")
                .and_then(Value::as_u64)
                .ok_or("missing expires_at_epoch_s")?;
            Ok(
                match build_invite(&psk, &mailboxes, expires, dev_loopback(&case)) {
                    Ok(text) => json!({ "ok": text }),
                    Err(e) => json!({ "error": e.as_str() }),
                },
            )
        }
        // Fixed-nonce sealing and key export exist only in test and conformance builds.
        #[cfg(any(test, feature = "conformance"))]
        "envelope_key" | "envelope_seal" | "envelope_open" => {
            crate::conformance_envelope::evaluate(kind, &case)
        }
        "session" => run_session(&case),
        "link" => run_link(&case),
        other => Err(format!("unknown kind {other}")),
    }
}

/// The development-only loopback switch of the invite codec; absent means off.
fn dev_loopback(case: &Value) -> bool {
    case.get("allow_dev_loopback")
        .and_then(Value::as_bool)
        .unwrap_or(false)
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

fn hex_bytes(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    text.as_bytes()
        .chunks(2)
        .map(|pair| {
            let digit = |c: u8| char::from(c).to_digit(16).map(|v| v as u8);
            match pair {
                [hi, lo] => Some((digit(*hi)? << 4) | digit(*lo)?),
                _ => None,
            }
        })
        .collect()
}

fn hex_text(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
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
