//! The v3 pairing invite: the text a baby device shows as a QR code (or copies) and a parent device
//! scans or pastes. One codec for every client (Android via UniFFI, the browser via WASM), so no
//! platform parses invite text itself.
//!
//! Wire form (UTF-8 JSON): `{"v":3,"k":<PSK, unpadded base64url of 32 bytes>,"s":[<ws:// or wss://
//! signaling URLs>],"x":<expiry, epoch seconds>}`. No room id, fingerprint, IP address or device
//! name is carried. The clock is a parameter (sans-IO) and nothing here emits prose.

// Untrusted input is parsed here: return errors instead of panicking (AGENTS.md §5.3).
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

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::{Value, json};
use zeroize::Zeroizing;

/// Length of the pairing key in bytes.
pub const PSK_LEN: usize = 32;
/// The only invite version this core accepts and emits.
pub const INVITE_VERSION: u64 = 3;

/// Why a text is not an acceptable invite. The platform maps each variant to its own copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteError {
    /// Not a v3 GuardedSleep invite (foreign text, wrong version, bad key, bad URL, no expiry).
    NotOurCode,
    /// Empty text, or JSON-looking text that is not complete JSON (most likely cut off in a copy).
    IncompletePaste,
    /// A valid invite whose expiry has passed.
    Expired,
}

impl InviteError {
    /// Stable snake_case key, used in conformance vectors and at the WASM boundary.
    pub fn as_str(self) -> &'static str {
        match self {
            InviteError::NotOurCode => "not_our_code",
            InviteError::IncompletePaste => "incomplete_paste",
            InviteError::Expired => "expired",
        }
    }
}

impl std::fmt::Display for InviteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for InviteError {}

/// A parsed invite. The PSK is wiped on drop and never printed (`Debug` is redacted).
#[derive(Clone)]
pub struct Invite {
    psk: Zeroizing<[u8; PSK_LEN]>,
    pub urls: Vec<String>,
    pub expires_at_epoch_s: u64,
}

impl Invite {
    /// The raw pairing key. Never log it or put it into an Intent, Bundle or URL.
    pub fn psk(&self) -> &[u8; PSK_LEN] {
        &self.psk
    }
}

impl PartialEq for Invite {
    fn eq(&self, other: &Self) -> bool {
        *self.psk == *other.psk
            && self.urls == other.urls
            && self.expires_at_epoch_s == other.expires_at_epoch_s
    }
}

impl Eq for Invite {}

impl std::fmt::Debug for Invite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Invite")
            .field("psk", &"..")
            .field("urls", &self.urls)
            .field("expires_at_epoch_s", &self.expires_at_epoch_s)
            .finish()
    }
}

/// Builds the invite text. Rejects a key that is not 32 bytes, an empty URL list, a URL that is not
/// a `ws://` / `wss://` URL with a host, and a zero expiry (`NotOurCode`), so the output always
/// parses back.
pub fn build_invite(
    psk: &[u8],
    urls: &[String],
    expires_at_epoch_s: u64,
) -> Result<String, InviteError> {
    if psk.len() != PSK_LEN
        || urls.is_empty()
        || !urls.iter().all(|u| is_signaling_url(u))
        || expires_at_epoch_s == 0
    {
        return Err(InviteError::NotOurCode);
    }
    Ok(json!({
        "v": INVITE_VERSION,
        "k": URL_SAFE_NO_PAD.encode(psk),
        "s": urls,
        "x": expires_at_epoch_s,
    })
    .to_string())
}

/// Parses a scanned QR text or a pasted code. Surrounding whitespace is ignored. `now_epoch_s` is
/// the platform's wall clock in epoch seconds; an invite is expired when `x <= now_epoch_s`.
pub fn parse_invite(text: &str, now_epoch_s: u64) -> Result<Invite, InviteError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(InviteError::IncompletePaste);
    }
    let value: Value = serde_json::from_str(trimmed).map_err(|_| {
        // JSON-looking text that does not parse was most likely cut off while copying.
        if trimmed.starts_with('{') {
            InviteError::IncompletePaste
        } else {
            InviteError::NotOurCode
        }
    })?;
    let obj = value.as_object().ok_or(InviteError::NotOurCode)?;
    if obj.get("v").and_then(Value::as_u64) != Some(INVITE_VERSION) {
        return Err(InviteError::NotOurCode);
    }
    let psk = obj
        .get("k")
        .and_then(Value::as_str)
        .and_then(|k| URL_SAFE_NO_PAD.decode(k).ok())
        .map(Zeroizing::new)
        .and_then(|bytes| <[u8; PSK_LEN]>::try_from(bytes.as_slice()).ok())
        .ok_or(InviteError::NotOurCode)?;
    let psk = Zeroizing::new(psk);
    let urls = obj
        .get("s")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .and_then(|a| {
            a.iter()
                .map(|u| {
                    u.as_str()
                        .filter(|u| is_signaling_url(u))
                        .map(str::to_owned)
                })
                .collect::<Option<Vec<_>>>()
        })
        .ok_or(InviteError::NotOurCode)?;
    let expires_at_epoch_s = obj
        .get("x")
        .and_then(Value::as_u64)
        .filter(|x| *x > 0)
        .ok_or(InviteError::NotOurCode)?;
    if expires_at_epoch_s <= now_epoch_s {
        return Err(InviteError::Expired);
    }
    Ok(Invite {
        psk,
        urls,
        expires_at_epoch_s,
    })
}

/// `ws://` or `wss://`, then a non-empty host (before any `/`, `?` or `#`), no whitespace.
fn is_signaling_url(url: &str) -> bool {
    let Some(rest) = url
        .strip_prefix("ws://")
        .or_else(|| url.strip_prefix("wss://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    !authority.is_empty() && !authority.starts_with(':') && !rest.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const NOW: u64 = 1_700_000_000;

    fn psk() -> Vec<u8> {
        (0..32).collect()
    }

    fn urls() -> Vec<String> {
        vec!["ws://10.0.2.2:8787".into(), "wss://a.example/sig".into()]
    }

    fn payload(v: &str, k: &str, s: &str, x: &str) -> String {
        format!(r#"{{"v":{v},"k":"{k}","s":{s},"x":{x}}}"#)
    }

    fn good_key() -> String {
        URL_SAFE_NO_PAD.encode(psk())
    }

    fn good(x: u64) -> String {
        payload(
            "3",
            &good_key(),
            r#"["ws://10.0.2.2:8787","wss://a.example/sig"]"#,
            &x.to_string(),
        )
    }

    #[test]
    fn round_trip() {
        let text = build_invite(&psk(), &urls(), NOW + 300).unwrap();
        let parsed = parse_invite(&text, NOW).unwrap();
        assert_eq!(parsed.psk().as_slice(), psk().as_slice());
        assert_eq!(parsed.urls, urls());
        assert_eq!(parsed.expires_at_epoch_s, NOW + 300);
    }

    #[test]
    fn build_emits_the_documented_shape() {
        let text = build_invite(&psk(), &["ws://h:1".into()], 42).unwrap();
        assert_eq!(
            text,
            format!(r#"{{"v":3,"k":"{}","s":["ws://h:1"],"x":42}}"#, good_key())
        );
    }

    #[test]
    fn build_rejects_what_parse_would_reject() {
        assert_eq!(
            build_invite(&psk()[..16], &urls(), NOW),
            Err(InviteError::NotOurCode)
        );
        assert_eq!(build_invite(&psk(), &[], NOW), Err(InviteError::NotOurCode));
        assert_eq!(
            build_invite(&psk(), &["http://h".into()], NOW),
            Err(InviteError::NotOurCode)
        );
        assert_eq!(
            build_invite(&psk(), &urls(), 0),
            Err(InviteError::NotOurCode)
        );
    }

    #[test]
    fn surrounding_whitespace_is_ignored() {
        assert!(parse_invite(&format!("  \n{}\n ", good(NOW + 5)), NOW).is_ok());
    }

    #[test]
    fn expiry_boundary() {
        assert!(parse_invite(&good(NOW + 1), NOW).is_ok());
        assert_eq!(parse_invite(&good(NOW), NOW), Err(InviteError::Expired));
        assert_eq!(parse_invite(&good(NOW - 1), NOW), Err(InviteError::Expired));
    }

    #[test]
    fn rejects_wrong_version() {
        let t = payload("2", &good_key(), r#"["ws://h"]"#, "4000000000");
        assert_eq!(parse_invite(&t, NOW), Err(InviteError::NotOurCode));
        let t = payload(r#""3""#, &good_key(), r#"["ws://h"]"#, "4000000000");
        assert_eq!(parse_invite(&t, NOW), Err(InviteError::NotOurCode));
    }

    #[test]
    fn rejects_bad_key() {
        let short = URL_SAFE_NO_PAD.encode([0u8; 16]);
        let long = URL_SAFE_NO_PAD.encode([0u8; 33]);
        let padded = format!("{}=", good_key());
        for k in [short, long, padded, "not base64!".into(), String::new()] {
            let t = payload("3", &k, r#"["ws://h"]"#, "4000000000");
            assert_eq!(parse_invite(&t, NOW), Err(InviteError::NotOurCode), "{k}");
        }
    }

    #[test]
    fn rejects_bad_urls() {
        for s in [
            "[]",
            r#"["http://h"]"#,
            r#"["ws://"]"#,
            r#"["ws:///path"]"#,
            r#"["ws://:80"]"#,
            r#"["ws://a b"]"#,
            r#"[" ws://h"]"#,
            r#"["ws://h", 5]"#,
            r#""ws://h""#,
        ] {
            let t = payload("3", &good_key(), s, "4000000000");
            assert_eq!(parse_invite(&t, NOW), Err(InviteError::NotOurCode), "{s}");
        }
    }

    #[test]
    fn rejects_missing_or_bad_expiry() {
        for x in ["0", "-5", "1.5", r#""4000000000""#, "null"] {
            let t = payload("3", &good_key(), r#"["ws://h"]"#, x);
            assert_eq!(parse_invite(&t, NOW), Err(InviteError::NotOurCode), "{x}");
        }
        let no_x = format!(r#"{{"v":3,"k":"{}","s":["ws://h"]}}"#, good_key());
        assert_eq!(parse_invite(&no_x, NOW), Err(InviteError::NotOurCode));
    }

    #[test]
    fn cut_off_json_is_incomplete() {
        let t = good(NOW + 5);
        assert_eq!(
            parse_invite(&t[..t.len() - 10], NOW),
            Err(InviteError::IncompletePaste)
        );
        assert_eq!(parse_invite("   ", NOW), Err(InviteError::IncompletePaste));
        assert_eq!(parse_invite("", NOW), Err(InviteError::IncompletePaste));
    }

    #[test]
    fn other_text_is_not_our_code() {
        for t in [
            "https://example.com",
            "[1,2",
            "[1,2]",
            "42",
            "null",
            r#"{"v":1,"r":"room","s":"ws://x","k":"abc"}"#,
        ] {
            assert_eq!(parse_invite(t, NOW), Err(InviteError::NotOurCode), "{t}");
        }
    }

    #[test]
    fn debug_does_not_reveal_the_key() {
        let inv = parse_invite(&good(NOW + 5), NOW).unwrap();
        let dbg = format!("{inv:?}");
        assert!(!dbg.contains(&good_key()));
        assert!(!dbg.contains("[0, 1, 2"));
    }

    proptest! {
        #[test]
        fn parse_of_build_is_identity(
            psk in proptest::collection::vec(any::<u8>(), 32),
            hosts in proptest::collection::vec("[a-z0-9.\\-]{1,20}(:[0-9]{1,5})?(/[a-z0-9/_\\-]{0,12})?", 1..4),
            secure in any::<bool>(),
            expires in 1u64..4_000_000_000,
        ) {
            let scheme = if secure { "wss" } else { "ws" };
            let urls: Vec<String> = hosts.iter().map(|h| format!("{scheme}://{h}")).collect();
            let text = build_invite(&psk, &urls, expires).unwrap();
            let parsed = parse_invite(&format!(" {text}\n"), expires - 1).unwrap();
            prop_assert_eq!(parsed.psk().as_slice(), psk.as_slice());
            prop_assert_eq!(parsed.urls, urls);
            prop_assert_eq!(parsed.expires_at_epoch_s, expires);
        }

        #[test]
        fn arbitrary_text_never_panics(text in ".{0,200}", now in any::<u64>()) {
            let _ = parse_invite(&text, now);
        }
    }
}
