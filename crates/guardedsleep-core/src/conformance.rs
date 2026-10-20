//! Evaluates one conformance case (JSON in, JSON out). Every binding (Rust, Kotlin via UniFFI,
//! WASM) runs `vectors/conformance.json` through this entry point and compares against `expect`,
//! so all platforms provably share one behaviour.

use serde_json::{Value, json};

use crate::link::{Link, LinkConfig, LinkInput};
use crate::pairing::{
    decode_psk, derive_room_id, pair_proof_mac_v2_legacy, pair_proof_mac_v3, stop_notice_mac,
};
use crate::protocol::{
    Role, parse_data_message, parse_data_message_v2_legacy, parse_server_message,
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
