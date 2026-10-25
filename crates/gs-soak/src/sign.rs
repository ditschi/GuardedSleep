//! `gs-soak sign`: BIP-340-signed Nostr events of the core's kind for the Node measurement
//! scripts, so that no second Schnorr implementation exists. Throwaway key, random content,
//! nothing derived from a pairing. One JSON line per event on stdout.

use std::time::{SystemTime, UNIX_EPOCH};

use guardedsleep_core::nostr::{build_event, scalar_key};
use serde_json::{Value, json};

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub fn random_bytes<const N: usize>() -> Result<[u8; N], String> {
    let mut out = [0u8; N];
    getrandom::fill(&mut out).map_err(|e| format!("randomness: {e}"))?;
    Ok(out)
}

/// `len` random base64url characters (the only alphabet the core's events allow).
pub fn random_content(len: usize) -> Result<String, String> {
    let mut raw = vec![0u8; len];
    getrandom::fill(&mut raw).map_err(|e| format!("randomness: {e}"))?;
    Ok(raw
        .iter()
        .map(|b| char::from(ALPHABET[usize::from(b & 63)]))
        .collect())
}

pub fn now_unix_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// A key signing several events, as a client of a relay would.
pub struct Signer {
    key: k256::schnorr::SigningKey,
}

impl Signer {
    pub fn new() -> Result<Self, String> {
        loop {
            // Rejection of the (practically impossible) zero scalar: draw again.
            if let Some(key) = scalar_key(&random_bytes::<32>()?) {
                return Ok(Self { key });
            }
        }
    }

    /// An event with `bytes` characters of random content at `created_at`.
    pub fn event(
        &self,
        tag: &str,
        bytes: usize,
        created_at: u64,
    ) -> Result<guardedsleep_core::nostr::Event, String> {
        let content = random_content(bytes.max(1))?;
        build_event(&self.key, &random_bytes::<32>()?, created_at, tag, &content)
            .ok_or_else(|| "event could not be built".to_owned())
    }
}

/// `BYTES:OFFSET_S` (the offset is optional and may be negative).
pub fn parse_spec(spec: &str) -> Option<(usize, i64)> {
    let (bytes, offset) = match spec.split_once(':') {
        Some((b, o)) => (b, o),
        None => (spec, "0"),
    };
    Some((bytes.parse().ok()?, offset.parse().ok()?))
}

pub fn shifted(now: u64, offset_s: i64) -> u64 {
    now.saturating_add_signed(offset_s)
}

pub fn run(tag: Option<String>, specs: &[String]) -> Result<(), String> {
    let signer = Signer::new()?;
    let tag = match tag {
        Some(t) => t,
        None => random_content(22)?,
    };
    let now = now_unix_s();
    for spec in specs {
        let (bytes, offset) = parse_spec(spec).ok_or_else(|| format!("bad spec `{spec}`"))?;
        let event = signer.event(&tag, bytes, shifted(now, offset))?;
        let parsed: Value = serde_json::from_str(&event.to_json()).map_err(|e| e.to_string())?;
        let line = json!({
            "bytes": bytes,
            "offset_s": offset,
            "kind": event.kind,
            "tag": tag,
            "pubkey": event.pubkey,
            "id": event.id,
            "frame": format!("[\"EVENT\",{}]", event.to_json()),
            "event": parsed,
        });
        println!("{line}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use guardedsleep_core::nostr::verify_event;

    #[test]
    fn specs_parse() {
        assert_eq!(parse_spec("2048"), Some((2048, 0)));
        assert_eq!(parse_spec("512:-300"), Some((512, -300)));
        assert_eq!(parse_spec("x"), None);
    }

    #[test]
    fn signed_events_verify() {
        let signer = Signer::new().unwrap_or_else(|e| panic!("{e}"));
        let event = signer.event("tagtagtag", 100, 1_700_000_000);
        let event = event.unwrap_or_else(|e| panic!("{e}"));
        assert!(verify_event(&event));
        assert_eq!(event.content.len(), 100);
        assert_eq!(event.created_at, 1_700_000_000);
    }

    #[test]
    fn time_shift_saturates() {
        assert_eq!(shifted(100, -300), 0);
        assert_eq!(shifted(100, 50), 150);
    }
}
