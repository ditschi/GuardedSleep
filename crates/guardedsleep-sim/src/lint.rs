//! `gs-sim lint-trace`: the shareable-artefact lint of test-schemas section 1.4. It rejects what a
//! trace must never contain (IP literals, real-looking fingerprints, ICE candidates and
//! credentials, URLs, absolute timestamps, key-like strings) and unknown fields.

use guardedsleep_core::pairing::decode_psk;
use serde_json::Value;

use crate::trace::TRACE_SCHEMA;
use crate::world::{SYNTH_FP_PREFIX_HEX, SYNTH_PSK_PREFIX, SYNTH_SEED_PREFIX};

const TOP_LEVEL: &[&str] = &[
    "schema", "id", "class", "privacy", "origin", "seed", "config", "steps", "expect", "final",
];
const BANNED: &[&str] = &[
    "a=candidate",
    "ice-ufrag",
    "ice-pwd",
    "credential",
    "username",
    "://",
    ".local",
];
/// Anything at or above this is more likely an epoch timestamp than a simulated duration.
const ABSOLUTE_MS: f64 = 1.0e11;

pub fn lint_trace(text: &str) -> Vec<String> {
    let mut errs = Vec::new();
    let doc: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => return vec![format!("not JSON: {e}")],
    };
    let Some(obj) = doc.as_object() else {
        return vec!["top level is not an object".into()];
    };
    for k in obj.keys() {
        if !TOP_LEVEL.contains(&k.as_str()) {
            errs.push(format!("unknown top-level field `{k}`"));
        }
    }
    if doc["schema"] != TRACE_SCHEMA {
        errs.push(format!("schema must be {TRACE_SCHEMA}"));
    }
    if doc["privacy"] != "shareable" {
        errs.push("privacy must be `shareable`".into());
    }
    if !matches!(doc["class"].as_str(), Some("golden" | "simulated")) {
        errs.push("class must be golden or simulated for a committed trace".into());
    }
    let psk = doc["config"]["psk_b64url"].as_str().unwrap_or("");
    let psk_bytes = decode_psk(psk).unwrap_or_default();
    if !psk_bytes.starts_with(SYNTH_PSK_PREFIX) {
        errs.push("config.psk_b64url is not a declared synthetic PSK".into());
    }
    // The seed predicts every nonce and secret of a run: a real one must never be committed.
    let seed_hex: String = SYNTH_SEED_PREFIX
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if !doc["config"]["seed"]
        .as_str()
        .is_some_and(|s| s.starts_with(&seed_hex))
    {
        errs.push("config.seed is not a declared synthetic seed".into());
    }
    match (doc["steps"].as_array(), doc["expect"].as_array()) {
        (Some(steps), Some(expect)) => {
            if steps.len() != expect.len() {
                errs.push("steps and expect differ in length".into());
            }
            let mut last = 0u64;
            for (i, s) in steps.iter().enumerate() {
                if s["n"].as_u64() != Some(i as u64) {
                    errs.push(format!("step {i}: `n` is not the step index"));
                }
                let now = s["now"].as_u64().unwrap_or(0);
                if now < last {
                    errs.push(format!("step {i}: `now` runs backwards"));
                }
                last = now;
            }
        }
        _ => errs.push("steps/expect missing".into()),
    }
    walk(&doc, psk, &mut errs);
    errs.sort();
    errs.dedup();
    errs
}

fn walk(v: &Value, psk: &str, errs: &mut Vec<String>) {
    match v {
        Value::String(s) => {
            if s != psk {
                lint_string(s, psk, errs);
            }
        }
        Value::Number(n) => {
            if n.as_f64().is_some_and(|f| f.abs() >= ABSOLUTE_MS) {
                errs.push(format!("number {n} looks like an absolute timestamp"));
            }
        }
        Value::Array(a) => a.iter().for_each(|x| walk(x, psk, errs)),
        Value::Object(o) => o.values().for_each(|x| walk(x, psk, errs)),
        _ => {}
    }
}

fn lint_string(s: &str, psk: &str, errs: &mut Vec<String>) {
    for b in BANNED {
        if s.contains(b) {
            errs.push(format!("contains `{b}`"));
        }
    }
    let masked = mask_fingerprints(s, errs);
    if has_ipv4(&masked) {
        errs.push("contains an IPv4 literal".into());
    }
    if has_ipv6(&masked) {
        errs.push("contains an IPv6 literal".into());
    }
    if has_iso_date(&masked) {
        errs.push("contains an absolute date".into());
    }
    for token in masked.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-')) {
        let hex = token
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if token.len() >= 40 && !hex && token != psk {
            errs.push("contains a long base64url string that is not the declared PSK".into());
        }
        if hex && token.len() > 64 {
            errs.push("contains an unexpectedly long hex string".into());
        }
        if token.len() >= 12 && token.bytes().all(|b| b.is_ascii_digit()) {
            errs.push("contains a long number (absolute timestamp?)".into());
        }
    }
}

/// Replaces every `a=fingerprint:sha-256 <value>` by `FP` after checking the value is synthetic.
fn mask_fingerprints(s: &str, errs: &mut Vec<String>) -> String {
    const KEY: &str = "a=fingerprint:sha-256 ";
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find(KEY) {
        out.push_str(&rest[..i]);
        let after = &rest[i + KEY.len()..];
        let end = after
            .find(|c: char| !(c.is_ascii_hexdigit() || c == ':'))
            .unwrap_or(after.len());
        let value: String = after[..end].replace(':', "").to_ascii_lowercase();
        if !value.starts_with(SYNTH_FP_PREFIX_HEX) {
            errs.push("non-synthetic DTLS fingerprint".into());
        }
        out.push_str("FP");
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

fn has_ipv4(s: &str) -> bool {
    s.split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .any(|t| {
            let parts: Vec<&str> = t.split('.').collect();
            parts.len() == 4
                && parts.iter().all(|p| {
                    !p.is_empty() && p.len() <= 3 && p.parse::<u16>().is_ok_and(|n| n <= 255)
                })
        })
}

fn has_ipv6(s: &str) -> bool {
    s.split(|c: char| !(c.is_ascii_hexdigit() || c == ':'))
        .any(|t| {
            t.matches(':').count() >= 2
                && t.split(':')
                    .all(|g| g.len() <= 4 && g.bytes().all(|b| b.is_ascii_hexdigit()))
        })
}

fn has_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.windows(10).any(|w| {
        w[..4].iter().all(u8::is_ascii_digit)
            && w[4] == b'-'
            && w[5..7].iter().all(u8::is_ascii_digit)
            && w[7] == b'-'
            && w[8..10].iter().all(u8::is_ascii_digit)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{colon_upper, synthetic_psk};

    fn trace(extra_step_input: &str) -> String {
        let psk = guardedsleep_core::pairing::encode_psk(&synthetic_psk("A"));
        let seed: String = SYNTH_SEED_PREFIX
            .iter()
            .chain(&[0u8; 16])
            .map(|b| format!("{b:02x}"))
            .collect();
        format!(
            r#"{{"schema":"gs-trace/1","id":"t","class":"simulated","privacy":"shareable",
            "config":{{"role":"parent","psk_b64url":"{psk}","seed":"{seed}"}},
            "steps":[{{"n":0,"now":0,"input":{{"type":"signaling_message","text":"{extra_step_input}"}}}}],
            "expect":[[]]}}"#
        )
    }

    #[test]
    fn clean_trace_passes() {
        let fp = colon_upper(&"53594e54".repeat(8));
        let t = trace(&format!("a=fingerprint:sha-256 {fp}\\r\\n"));
        assert_eq!(lint_trace(&t), Vec::<String>::new());
    }

    #[test]
    fn rejects_the_forbidden_material() {
        for (bad, why) in [
            ("host 192.168.1.20 here", "IPv4"),
            ("fe80::1", "IPv6"),
            ("2001:db8:0:0:0:0:0:1", "IPv6"),
            ("a=fingerprint:sha-256 AA:BB:CC", "fingerprint"),
            ("a=candidate:1 1 udp", "candidate"),
            ("ice-pwd:xyz", "ice"),
            ("wss://example.org/ws", "URL"),
            ("phone.local", "mDNS"),
            ("2026-10-19T10:00:00Z", "date"),
            ("1790000000000", "timestamp"),
            ("QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVowMTIzNDU2Nzg5", "base64"),
        ] {
            assert!(!lint_trace(&trace(bad)).is_empty(), "{why} slipped through");
        }
    }

    #[test]
    fn rejects_unknown_fields_and_real_psks() {
        let t = trace("x").replace("\"id\":\"t\"", "\"id\":\"t\",\"device\":\"pixel\"");
        assert!(
            lint_trace(&t)
                .iter()
                .any(|e| e.contains("unknown top-level"))
        );
        let t = trace("x").replace(
            &guardedsleep_core::pairing::encode_psk(&synthetic_psk("A")),
            "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8",
        );
        assert!(lint_trace(&t).iter().any(|e| e.contains("synthetic PSK")));
    }

    #[test]
    fn rejects_a_seed_that_is_not_synthetic() {
        let t = trace("x").replace(
            "47532d53594e54482d534545442d7631",
            "00112233445566778899aabbccddeeff",
        );
        assert!(lint_trace(&t).iter().any(|e| e.contains("synthetic seed")));
    }
}
