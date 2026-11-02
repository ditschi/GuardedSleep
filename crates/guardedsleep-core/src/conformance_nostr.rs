//! Conformance cases for the Nostr mailbox (`nostr_epoch`, `nostr_tag`, `nostr_pubkey`,
//! `nostr_event`, `nostr_verify`, `nostr_frame`). The expected values come from an independent
//! implementation (pure-Python BIP-340 and HKDF/HMAC from its standard library, see
//! `openspec/changes/alpha-connection-model/tasks.md` 3.5). Compiled only for tests and the
//! `conformance` cargo feature, because `nostr_event` signs with a caller-chosen `aux_rand`.

use serde_json::{Value, json};

use crate::nostr::{
    Event, NostrKeys, RelayFrame, build_event, epoch_of, hex_decode, parse_relay_frame,
    verify_event,
};
use crate::pairing::decode_psk;
use crate::protocol::Role;

pub(crate) fn evaluate(kind: &str, case: &Value) -> Result<Value, String> {
    match kind {
        "nostr_epoch" => {
            let unix_s = u64_field(case, "unix_s")?;
            Ok(json!(epoch_of(unix_s)))
        }
        "nostr_tag" => {
            let keys = keys(case)?;
            Ok(json!(keys.tag(epoch(case)?)))
        }
        "nostr_pubkey" => {
            let keys = keys(case)?;
            Ok(json!(keys.public_key_hex(role(case)?, epoch(case)?)))
        }
        "nostr_event" => nostr_event(case),
        "nostr_verify" => {
            let text = str_field(case, "event")?;
            let wrapped = format!("[\"EVENT\",\"s\",{text}]");
            Ok(json!(match parse_relay_frame(&wrapped) {
                Some(RelayFrame::Event { event, .. }) => verify_event(&event),
                _ => false,
            }))
        }
        _ => Ok(match parse_relay_frame(str_field(case, "raw")?) {
            Some(frame) => frame_json(&frame),
            None => Value::Null,
        }),
    }
}

fn nostr_event(case: &Value) -> Result<Value, String> {
    let keys = keys(case)?;
    let (role, epoch) = (role(case)?, epoch(case)?);
    let key = keys.signing_key(role, epoch).ok_or("no key that epoch")?;
    let aux: [u8; 32] = hex_decode(str_field(case, "aux")?)
        .and_then(|b| b.try_into().ok())
        .ok_or("aux must be 32 bytes")?;
    let created_at = u64_field(case, "created_at")?;
    let tag = keys.tag(epoch);
    let ev = build_event(&key, &aux, created_at, &tag, str_field(case, "content")?)
        .ok_or("content is not base64url")?;
    Ok(json!({
        "id": ev.id,
        "pubkey": ev.pubkey,
        "sig": ev.sig,
        "tag": tag,
        "json": ev.to_json(),
    }))
}

fn event_json(e: &Event) -> Value {
    json!({
        "id": e.id,
        "pubkey": e.pubkey,
        "created_at": e.created_at,
        "kind": e.kind,
        "tags": e.tags,
        "content": e.content,
        "sig": e.sig,
    })
}

fn frame_json(frame: &RelayFrame) -> Value {
    match frame {
        RelayFrame::Event { sub, event } => {
            json!({ "type": "event", "sub": sub, "event": event_json(event) })
        }
        RelayFrame::Ok {
            id,
            accepted,
            message,
        } => json!({ "type": "ok", "id": id, "accepted": accepted, "message": message }),
        RelayFrame::Eose { sub } => json!({ "type": "eose", "sub": sub }),
        RelayFrame::Notice { message } => json!({ "type": "notice", "message": message }),
        RelayFrame::Closed { sub, message } => {
            json!({ "type": "closed", "sub": sub, "message": message })
        }
        RelayFrame::Auth => json!({ "type": "auth" }),
    }
}

fn str_field<'a>(case: &'a Value, k: &str) -> Result<&'a str, String> {
    case.get(k)
        .and_then(Value::as_str)
        .ok_or(format!("missing {k}"))
}

fn u64_field(case: &Value, k: &str) -> Result<u64, String> {
    case.get(k)
        .and_then(Value::as_u64)
        .ok_or(format!("missing {k}"))
}

fn epoch(case: &Value) -> Result<u32, String> {
    u32::try_from(u64_field(case, "epoch")?).map_err(|_| "epoch out of range".to_owned())
}

fn role(case: &Value) -> Result<Role, String> {
    match str_field(case, "role")? {
        "baby" => Ok(Role::Baby),
        "parent" => Ok(Role::Parent),
        other => Err(format!("bad role {other}")),
    }
}

fn keys(case: &Value) -> Result<NostrKeys, String> {
    let psk = decode_psk(str_field(case, "psk_b64url")?).ok_or("bad psk")?;
    NostrKeys::derive(&psk).ok_or_else(|| "empty psk".to_owned())
}
