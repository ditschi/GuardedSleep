//! The v4 pairing invite: the text a baby device shows as a QR code (or copies) and a parent device
//! scans or pastes. One codec for every client (Android via UniFFI, the browser via WASM), so no
//! platform parses invite text itself.
//!
//! Wire form (UTF-8 JSON, at most [`MAX_INVITE_BYTES`]): `{"v":4,"k":<PSK, unpadded base64url of 32
//! bytes>,"m":[<mailbox>, ...],"x":<expiry, epoch seconds>}`. One to [`MAX_MAILBOXES`] compact mailbox
//! entries, each `<kind>:<address>`:
//!
//! - `n:<host>`: a Nostr relay, always `wss://<host>`;
//! - `r:<host[:port][/path]>`: an own relay, always `wss://`;
//! - `l:<ip>:<port>`: a LAN mailbox, always `ws://`; the address is RFC 1918 or RFC 4193 (ULA, written
//!   `[fd00::1]:port`). Link-local, public, CGNAT (100.64/10) and host names are refused.
//!
//! No room id, fingerprint, parent id, device name or public IP address is carried. The clock is a
//! parameter (sans-IO) and nothing here emits prose. Any other version, v3 included, is not our code.
//!
//! Development only: with `allow_dev_loopback` an `l:` entry may also name a loopback address or
//! `localhost` (`just dev` and the browser E2E run the signaling server on loopback). Release builds
//! of the platforms pass `false`.

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

use std::net::{Ipv4Addr, Ipv6Addr};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use zeroize::Zeroizing;

use crate::mailbox::MailboxKind;

/// Length of the pairing key in bytes.
pub const PSK_LEN: usize = 32;
/// The only invite version this core accepts and emits.
pub const INVITE_VERSION: u64 = 4;
/// Most mailbox entries an invite may carry.
pub const MAX_MAILBOXES: usize = 6;
/// Largest encoded invite in bytes (about QR version 13 at level M).
pub const MAX_INVITE_BYTES: usize = 300;
/// The drop order keeps at least this many Nostr relays.
const MIN_NOSTR: usize = 2;

/// Why a text is not an acceptable invite. The platform maps each variant to its own copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteError {
    /// Not a v4 GuardedSleep invite (foreign text, wrong version, bad key, bad mailbox, too long, no
    /// expiry).
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

/// One mailbox of an invite: its kind and the URL the platform opens (`wss://host` for Nostr,
/// `wss://host[:port][/path]` for an own relay, `ws://ip:port` for a LAN mailbox).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct InviteMailbox {
    pub kind: MailboxKind,
    pub url: String,
}

/// A parsed invite. The PSK is wiped on drop and never printed (`Debug` is redacted).
#[derive(Clone)]
pub struct Invite {
    psk: Zeroizing<[u8; PSK_LEN]>,
    pub mailboxes: Vec<InviteMailbox>,
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
            && self.mailboxes == other.mailboxes
            && self.expires_at_epoch_s == other.expires_at_epoch_s
    }
}

impl Eq for Invite {}

impl std::fmt::Debug for Invite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Invite")
            .field("psk", &"..")
            .field("mailboxes", &self.mailboxes)
            .field("expires_at_epoch_s", &self.expires_at_epoch_s)
            .finish()
    }
}

/// Builds the invite text. Rejects a key that is not 32 bytes, an empty mailbox list, a mailbox the
/// grammar refuses and a zero expiry (`NotOurCode`), so the output always parses back. When the
/// list exceeds the budget (6 entries, 300 bytes) entries are dropped in this order: extra own
/// relays (last first), then Nostr relays (last first) down to two. A LAN mailbox and the first own
/// relay are never dropped; if the invite is still over budget the build fails (`NotOurCode`).
pub fn build_invite(
    psk: &[u8],
    mailboxes: &[InviteMailbox],
    expires_at_epoch_s: u64,
    allow_dev_loopback: bool,
) -> Result<String, InviteError> {
    if psk.len() != PSK_LEN || mailboxes.is_empty() || expires_at_epoch_s == 0 {
        return Err(InviteError::NotOurCode);
    }
    let mut kept: Vec<(InviteMailbox, String)> = mailboxes
        .iter()
        .map(|m| {
            let entry = entry_text(m).ok_or(InviteError::NotOurCode)?;
            let canonical =
                parse_entry(&entry, allow_dev_loopback).ok_or(InviteError::NotOurCode)?;
            Ok((canonical, entry))
        })
        .collect::<Result<_, InviteError>>()?;
    let key = URL_SAFE_NO_PAD.encode(psk);
    loop {
        let text = json!({
            "v": INVITE_VERSION,
            "k": key,
            "m": kept.iter().map(|(_, e)| e.as_str()).collect::<Vec<_>>(),
            "x": expires_at_epoch_s,
        })
        .to_string();
        if kept.len() <= MAX_MAILBOXES && text.len() <= MAX_INVITE_BYTES {
            return Ok(text);
        }
        let kinds: Vec<MailboxKind> = kept.iter().map(|(m, _)| m.kind).collect();
        let idx = drop_candidate(&kinds).ok_or(InviteError::NotOurCode)?;
        kept.remove(idx);
    }
}

/// Index of the entry the budget drops next, or `None` when only protected entries remain.
fn drop_candidate(kinds: &[MailboxKind]) -> Option<usize> {
    let first_relay = kinds.iter().position(|k| *k == MailboxKind::Relay);
    let extra_relay = kinds
        .iter()
        .enumerate()
        .rev()
        .find(|(i, k)| **k == MailboxKind::Relay && Some(*i) != first_relay)
        .map(|(i, _)| i);
    if extra_relay.is_some() {
        return extra_relay;
    }
    // "Nostr beyond 3, then down to 2" is one rule: the last Nostr entry goes while more than two
    // remain; the invite stops shrinking as soon as it fits.
    let nostr = kinds.iter().filter(|k| **k == MailboxKind::Nostr).count();
    if nostr > MIN_NOSTR {
        return kinds
            .iter()
            .enumerate()
            .rev()
            .find(|(_, k)| **k == MailboxKind::Nostr)
            .map(|(i, _)| i);
    }
    None
}

/// Parses a scanned QR text or a pasted code. Surrounding whitespace is ignored. `now_epoch_s` is
/// the platform's wall clock in epoch seconds; an invite is expired when `x <= now_epoch_s`.
pub fn parse_invite(
    text: &str,
    now_epoch_s: u64,
    allow_dev_loopback: bool,
) -> Result<Invite, InviteError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(InviteError::IncompletePaste);
    }
    if trimmed.len() > MAX_INVITE_BYTES {
        return Err(InviteError::NotOurCode);
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
    let mailboxes = obj
        .get("m")
        .and_then(Value::as_array)
        .filter(|a| (1..=MAX_MAILBOXES).contains(&a.len()))
        .and_then(|a| {
            a.iter()
                .map(|e| e.as_str().and_then(|e| parse_entry(e, allow_dev_loopback)))
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
        mailboxes,
        expires_at_epoch_s,
    })
}

/// The compact entry for a mailbox URL, or `None` when the scheme does not match the kind. The
/// address itself is validated by [`parse_entry`].
fn entry_text(m: &InviteMailbox) -> Option<String> {
    let (prefix, scheme) = match m.kind {
        MailboxKind::Nostr => ("n:", "wss://"),
        MailboxKind::Relay => ("r:", "wss://"),
        MailboxKind::Lan => ("l:", "ws://"),
    };
    m.url
        .strip_prefix(scheme)
        .map(|rest| format!("{prefix}{rest}"))
}

/// Validates one compact entry and returns the mailbox in canonical form (IPv6 re-written).
fn parse_entry(entry: &str, allow_dev_loopback: bool) -> Option<InviteMailbox> {
    let (kind, rest) = entry.split_once(':')?;
    match kind {
        "n" => is_hostname(rest).then(|| InviteMailbox {
            kind: MailboxKind::Nostr,
            url: format!("wss://{rest}"),
        }),
        "r" => is_relay_address(rest).then(|| InviteMailbox {
            kind: MailboxKind::Relay,
            url: format!("wss://{rest}"),
        }),
        "l" => lan_authority(rest, allow_dev_loopback).map(|authority| InviteMailbox {
            kind: MailboxKind::Lan,
            url: format!("ws://{authority}"),
        }),
        _ => None,
    }
}

/// An ASCII DNS name: dot-separated labels of letters, digits and inner hyphens (an IPv4 literal
/// passes as such a name, which is fine for a public host).
fn is_hostname(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

/// A port as canonical decimal text: 1..=65535, no sign, no leading zero.
fn is_port(port: &str) -> bool {
    !port.starts_with('0')
        && port.len() <= 5
        && port.bytes().all(|b| b.is_ascii_digit())
        && port.parse::<u32>().is_ok_and(|p| (1..=65535).contains(&p))
}

/// `host[:port][/path]` with a conservative path alphabet (no query, fragment or whitespace).
fn is_relay_address(rest: &str) -> bool {
    let (authority, path) = match rest.find('/') {
        Some(i) => rest.split_at(i),
        None => (rest, ""),
    };
    let host_ok = match authority.split_once(':') {
        Some((host, port)) => is_hostname(host) && is_port(port),
        None => is_hostname(authority),
    };
    host_ok
        && path.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'.' | b'_' | b'~' | b'%')
        })
}

/// The canonical `ip:port` (or `[ipv6]:port`) of an acceptable LAN address.
fn lan_authority(rest: &str, allow_dev_loopback: bool) -> Option<String> {
    if let Some(inner) = rest.strip_prefix('[') {
        let (ip, port) = inner.split_once("]:")?;
        let ip: Ipv6Addr = ip.parse().ok()?;
        let unique_local = ip.segments().first().is_some_and(|s| s & 0xfe00 == 0xfc00);
        let ok = unique_local || (allow_dev_loopback && ip.is_loopback());
        return (ok && is_port(port)).then(|| format!("[{ip}]:{port}"));
    }
    let (ip, port) = rest.rsplit_once(':')?;
    if !is_port(port) {
        return None;
    }
    if allow_dev_loopback && ip == "localhost" {
        return Some(format!("localhost:{port}"));
    }
    let ip: Ipv4Addr = ip.parse().ok()?;
    let ok = ip.is_private() || (allow_dev_loopback && ip.is_loopback());
    ok.then(|| format!("{ip}:{port}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const NOW: u64 = 1_700_000_000;

    fn psk() -> Vec<u8> {
        (0..32).collect()
    }

    fn mb(kind: MailboxKind, url: &str) -> InviteMailbox {
        InviteMailbox {
            kind,
            url: url.into(),
        }
    }

    fn relay(host: &str) -> InviteMailbox {
        mb(MailboxKind::Relay, &format!("wss://{host}"))
    }

    fn nostr(host: &str) -> InviteMailbox {
        mb(MailboxKind::Nostr, &format!("wss://{host}"))
    }

    fn lan(authority: &str) -> InviteMailbox {
        mb(MailboxKind::Lan, &format!("ws://{authority}"))
    }

    fn mailboxes() -> Vec<InviteMailbox> {
        vec![lan("10.0.2.2:8787"), relay("a.example/sig")]
    }

    fn payload(v: &str, k: &str, m: &str, x: &str) -> String {
        format!(r#"{{"v":{v},"k":"{k}","m":{m},"x":{x}}}"#)
    }

    fn good_key() -> String {
        URL_SAFE_NO_PAD.encode(psk())
    }

    fn good(x: u64) -> String {
        payload(
            "4",
            &good_key(),
            r#"["l:10.0.2.2:8787","r:a.example/sig"]"#,
            &x.to_string(),
        )
    }

    fn parse(text: &str) -> Result<Invite, InviteError> {
        parse_invite(text, NOW, false)
    }

    fn parse_m(entries: &str) -> Result<Invite, InviteError> {
        parse(&payload("4", &good_key(), entries, "4000000000"))
    }

    #[test]
    fn round_trip() {
        let text = build_invite(&psk(), &mailboxes(), NOW + 300, false).unwrap();
        let parsed = parse(&text).unwrap();
        assert_eq!(parsed.psk().as_slice(), psk().as_slice());
        assert_eq!(parsed.mailboxes, mailboxes());
        assert_eq!(parsed.expires_at_epoch_s, NOW + 300);
    }

    #[test]
    fn build_emits_the_documented_shape() {
        let text = build_invite(&psk(), &[nostr("relay.example.org")], 42, false).unwrap();
        assert_eq!(
            text,
            format!(
                r#"{{"v":4,"k":"{}","m":["n:relay.example.org"],"x":42}}"#,
                good_key()
            )
        );
    }

    #[test]
    fn every_kind_parses() {
        let inv = parse_m(
            r#"["n:relay.example.org","r:gs.example.com:8443/sig","l:192.168.1.5:8787","l:[fd12:3456::1]:9000"]"#,
        )
        .unwrap();
        assert_eq!(
            inv.mailboxes,
            vec![
                nostr("relay.example.org"),
                relay("gs.example.com:8443/sig"),
                lan("192.168.1.5:8787"),
                lan("[fd12:3456::1]:9000"),
            ]
        );
    }

    #[test]
    fn lan_accepts_only_private_ranges() {
        for ok in [
            "10.0.0.1:1",
            "10.255.255.255:65535",
            "172.16.0.1:80",
            "172.31.255.254:80",
            "192.168.0.1:80",
            "[fc00::1]:80",
            "[fdff::1]:80",
        ] {
            assert!(parse_m(&format!(r#"["l:{ok}"]"#)).is_ok(), "{ok}");
        }
        for bad in [
            "169.254.1.1:80",
            "8.8.8.8:80",
            "100.64.0.1:80",
            "100.127.255.255:80",
            "172.15.0.1:80",
            "172.32.0.1:80",
            "127.0.0.1:80",
            "0.0.0.0:80",
            "192.169.0.1:80",
            "[fe80::1]:80",
            "[::1]:80",
            "[2001:db8::1]:80",
            "[fbff::1]:80",
            "[fe00::1]:80",
            "gs.local:80",
            "localhost:80",
            "10.0.0.1",
            "10.0.0.1:0",
            "10.0.0.1:65536",
            "10.0.0.1:08080",
            "010.0.0.1:80",
            "fd00::1:80",
            "[fd00::1]80",
        ] {
            assert_eq!(
                parse_m(&format!(r#"["l:{bad}"]"#)),
                Err(InviteError::NotOurCode),
                "{bad}"
            );
        }
    }

    #[test]
    fn dev_loopback_is_opt_in() {
        let text = payload(
            "4",
            &good_key(),
            r#"["l:127.0.0.1:8787","l:[::1]:8787","l:localhost:8787"]"#,
            "4000000000",
        );
        assert_eq!(parse(&text), Err(InviteError::NotOurCode));
        let inv = parse_invite(&text, NOW, true).unwrap();
        assert_eq!(
            inv.mailboxes,
            vec![lan("127.0.0.1:8787"), lan("[::1]:8787"), lan("localhost:8787")]
        );
        // Public and link-local stay refused in development.
        for bad in ["l:8.8.8.8:80", "l:[fe80::1]:80"] {
            let t = payload("4", &good_key(), &format!(r#"["{bad}"]"#), "4000000000");
            assert_eq!(parse_invite(&t, NOW, true), Err(InviteError::NotOurCode));
        }
        assert!(build_invite(&psk(), &[lan("127.0.0.1:1")], NOW, true).is_ok());
        assert_eq!(
            build_invite(&psk(), &[lan("127.0.0.1:1")], NOW, false),
            Err(InviteError::NotOurCode)
        );
    }

    #[test]
    fn nostr_and_relay_entries_are_strict() {
        for bad in [
            "n:",
            "n:host:443",
            "n:host/path",
            "n:-host.example",
            "n:host..example",
            "n:ho st",
            "n:ho_st.example",
            "n:[::1]",
            "n:relay.example\\u00e9",
            "r:",
            "r:host:",
            "r:host:0",
            "r:host:99999",
            "r:host?q=1",
            "r:host/a#b",
            "r:host/a b",
            "r:[::1]:80",
            "r:user@host",
            "x:host",
            "host",
            "wss://host",
            "n:wss://host",
            "r:ws://host",
        ] {
            assert_eq!(
                parse_m(&format!(r#"["{bad}"]"#)),
                Err(InviteError::NotOurCode),
                "{bad}"
            );
        }
        assert!(parse_m(r#"["r:host:8443","r:host/sig","r:host:1/a/b-c_d.e~f%20"]"#).is_ok());
    }

    #[test]
    fn schemes_must_match_the_kind() {
        let bad = [
            mb(MailboxKind::Relay, "ws://a.example"),
            mb(MailboxKind::Nostr, "ws://a.example"),
            mb(MailboxKind::Lan, "wss://10.0.0.1:80"),
            mb(MailboxKind::Relay, "http://a.example"),
            mb(MailboxKind::Nostr, "wss://a.example:443"),
            mb(MailboxKind::Lan, "ws://8.8.8.8:80"),
            mb(MailboxKind::Lan, "ws://[fe80::1]:80"),
            mb(MailboxKind::Lan, "ws://gs.local:80"),
        ];
        for m in bad {
            assert_eq!(
                build_invite(&psk(), std::slice::from_ref(&m), NOW, false),
                Err(InviteError::NotOurCode),
                "{m:?}"
            );
        }
    }

    #[test]
    fn entry_count_is_one_to_six() {
        assert_eq!(parse_m("[]"), Err(InviteError::NotOurCode));
        let six = r#"["n:a.example","n:b.example","n:c.example","n:d.example","n:e.example","n:f.example"]"#;
        assert!(parse_m(six).is_ok());
        let seven = r#"["n:a.example","n:b.example","n:c.example","n:d.example","n:e.example","n:f.example","n:g.example"]"#;
        assert_eq!(parse_m(seven), Err(InviteError::NotOurCode));
        assert_eq!(parse_m(r#""n:a.example""#), Err(InviteError::NotOurCode));
        assert_eq!(parse_m(r#"["n:a.example",5]"#), Err(InviteError::NotOurCode));
    }

    #[test]
    fn size_limit_is_300_bytes_trimmed() {
        // Pad the host to land on the limit; labels stay at most 50 characters (valid DNS name).
        let pad = |len: usize| -> String {
            (0..len)
                .map(|i| if (i + 1) % 51 == 0 { '.' } else { 'a' })
                .collect()
        };
        let make = |len: usize| {
            let host = format!("{}.example", pad(len));
            payload(
                "4",
                &good_key(),
                &format!(r#"["n:{host}"]"#),
                "4000000000",
            )
        };
        let base = make(1).len();
        let at_limit = make(1 + MAX_INVITE_BYTES - base);
        assert_eq!(at_limit.len(), MAX_INVITE_BYTES);
        assert!(parse(&at_limit).is_ok());
        assert!(parse(&format!("  {at_limit}\n")).is_ok());
        let over = make(2 + MAX_INVITE_BYTES - base);
        assert_eq!(over.len(), MAX_INVITE_BYTES + 1);
        assert_eq!(parse(&over), Err(InviteError::NotOurCode));
    }

    #[test]
    fn build_rejects_what_parse_would_reject() {
        assert_eq!(
            build_invite(&psk()[..16], &mailboxes(), NOW, false),
            Err(InviteError::NotOurCode)
        );
        assert_eq!(
            build_invite(&psk(), &[], NOW, false),
            Err(InviteError::NotOurCode)
        );
        assert_eq!(
            build_invite(&psk(), &[mb(MailboxKind::Relay, "http://h")], NOW, false),
            Err(InviteError::NotOurCode)
        );
        assert_eq!(
            build_invite(&psk(), &mailboxes(), 0, false),
            Err(InviteError::NotOurCode)
        );
    }

    fn built(list: &[InviteMailbox]) -> Vec<InviteMailbox> {
        let text = build_invite(&psk(), list, 4_000_000_000, false).unwrap();
        assert!(text.len() <= MAX_INVITE_BYTES, "{}", text.len());
        parse(&text).unwrap().mailboxes
    }

    #[test]
    fn fitting_lists_are_kept_whole() {
        let list = vec![
            relay("gs.example.com"),
            nostr("a.example.org"),
            nostr("b.example.org"),
            nostr("c.example.org"),
            lan("192.168.1.5:8787"),
        ];
        assert_eq!(built(&list), list);
    }

    #[test]
    fn more_than_six_entries_are_dropped_to_six() {
        let list: Vec<_> = ["a", "b", "c", "d", "e", "f", "g"]
            .iter()
            .map(|h| nostr(&format!("{h}.example.org")))
            .collect();
        assert_eq!(built(&list), list[..6].to_vec());
    }

    #[test]
    fn drops_extra_own_relays_before_nostr() {
        let long = |c: &str| relay(&format!("{}.relay.example.com", c.repeat(61)));
        let list = vec![
            long("a"),
            long("b"),
            long("c"),
            nostr("n1.example.org"),
            nostr("n2.example.org"),
            nostr("n3.example.org"),
        ];
        let kept = built(&list);
        // Both extra own relays go (last first); every Nostr relay survives while it fits.
        assert_eq!(
            kept,
            vec![
                long("a"),
                nostr("n1.example.org"),
                nostr("n2.example.org"),
                nostr("n3.example.org")
            ]
        );
    }

    #[test]
    fn drops_nostr_down_to_two_but_never_lan_or_first_relay() {
        let long = |c: &str| nostr(&format!("{}.nostr.example.org", c.repeat(45)));
        let list = vec![
            relay("gs.example.com"),
            long("a"),
            long("b"),
            long("c"),
            lan("192.168.1.5:8787"),
        ];
        let kept = built(&list);
        assert!(kept.contains(&relay("gs.example.com")));
        assert!(kept.contains(&lan("192.168.1.5:8787")));
        let nostrs: Vec<_> = kept.iter().filter(|m| m.kind == MailboxKind::Nostr).collect();
        assert_eq!(nostrs.len(), 2);
        // The earlier Nostr entries survive, the last goes first.
        assert_eq!(nostrs[0], &long("a"));
        assert_eq!(nostrs[1], &long("b"));
    }

    #[test]
    fn build_fails_when_protected_entries_alone_are_too_big() {
        let big = |c: &str| relay(&format!("{}.example.com/{}", c.repeat(60), "p".repeat(60)));
        // One own relay, never dropped, plus three LAN entries.
        let list = vec![
            big("a"),
            big("b"),
            lan("[fd12:3456:789a:bcde:f012:3456:789a:bcde]:65535"),
            lan("[fd12:3456:789a:bcde:f012:3456:789a:bcdf]:65535"),
            lan("[fd12:3456:789a:bcde:f012:3456:789a:bcd0]:65535"),
        ];
        assert_eq!(
            build_invite(&psk(), &list, 4_000_000_000, false),
            Err(InviteError::NotOurCode)
        );
    }

    #[test]
    fn two_nostr_floor_is_not_crossed() {
        let long = |c: &str| nostr(&format!("{}.{}.nostr.example.org", c.repeat(60), c.repeat(60)));
        let list = vec![long("a"), long("b")];
        assert_eq!(
            build_invite(&psk(), &list, 4_000_000_000, false),
            Err(InviteError::NotOurCode)
        );
    }

    #[test]
    fn surrounding_whitespace_is_ignored() {
        assert!(parse(&format!("  \n{}\n ", good(NOW + 5))).is_ok());
    }

    #[test]
    fn expiry_boundary() {
        assert!(parse(&good(NOW + 1)).is_ok());
        assert_eq!(parse(&good(NOW)), Err(InviteError::Expired));
        assert_eq!(parse(&good(NOW - 1)), Err(InviteError::Expired));
    }

    #[test]
    fn rejects_other_versions_including_v3() {
        let t = payload("2", &good_key(), r#"["r:h.example"]"#, "4000000000");
        assert_eq!(parse(&t), Err(InviteError::NotOurCode));
        let t = payload(r#""4""#, &good_key(), r#"["r:h.example"]"#, "4000000000");
        assert_eq!(parse(&t), Err(InviteError::NotOurCode));
        let v3 = format!(
            r#"{{"v":3,"k":"{}","s":["wss://h.example"],"x":4000000000}}"#,
            good_key()
        );
        assert_eq!(parse(&v3), Err(InviteError::NotOurCode));
        let v3_with_m = payload("3", &good_key(), r#"["r:h.example"]"#, "4000000000");
        assert_eq!(parse(&v3_with_m), Err(InviteError::NotOurCode));
    }

    #[test]
    fn rejects_bad_key() {
        let short = URL_SAFE_NO_PAD.encode([0u8; 16]);
        let long = URL_SAFE_NO_PAD.encode([0u8; 33]);
        let padded = format!("{}=", good_key());
        for k in [short, long, padded, "not base64!".into(), String::new()] {
            let t = payload("4", &k, r#"["r:h.example"]"#, "4000000000");
            assert_eq!(parse(&t), Err(InviteError::NotOurCode), "{k}");
        }
    }

    #[test]
    fn rejects_missing_or_bad_expiry() {
        for x in ["0", "-5", "1.5", r#""4000000000""#, "null"] {
            let t = payload("4", &good_key(), r#"["r:h.example"]"#, x);
            assert_eq!(parse(&t), Err(InviteError::NotOurCode), "{x}");
        }
        let no_x = format!(r#"{{"v":4,"k":"{}","m":["r:h.example"]}}"#, good_key());
        assert_eq!(parse(&no_x), Err(InviteError::NotOurCode));
    }

    #[test]
    fn cut_off_json_is_incomplete() {
        let t = good(NOW + 5);
        assert_eq!(parse(&t[..t.len() - 10]), Err(InviteError::IncompletePaste));
        assert_eq!(parse("   "), Err(InviteError::IncompletePaste));
        assert_eq!(parse(""), Err(InviteError::IncompletePaste));
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
            assert_eq!(parse(t), Err(InviteError::NotOurCode), "{t}");
        }
    }

    #[test]
    fn debug_does_not_reveal_the_key() {
        let inv = parse(&good(NOW + 5)).unwrap();
        let dbg = format!("{inv:?}");
        assert!(!dbg.contains(&good_key()));
        assert!(!dbg.contains("[0, 1, 2"));
    }

    fn host_strategy() -> impl Strategy<Value = String> {
        "[a-z0-9]{1,8}(\\.[a-z0-9]{1,8}){0,3}"
    }

    fn mailbox_strategy() -> impl Strategy<Value = InviteMailbox> {
        prop_oneof![
            host_strategy().prop_map(|h| nostr(&h)),
            (
                host_strategy(),
                proptest::option::of(1u16..),
                proptest::option::of("/[a-z0-9/_\\-]{0,12}")
            )
                .prop_map(|(h, port, path)| {
                    let port = port.map(|p| format!(":{p}")).unwrap_or_default();
                    relay(&format!("{h}{port}{}", path.unwrap_or_default()))
                }),
            (any::<[u8; 2]>(), 1u16..).prop_map(|(b, p)| lan(&format!("10.{}.{}.1:{p}", b[0], b[1]))),
            (1u16.., 1u16..).prop_map(|(s, p)| lan(&format!("[fd00::{s:x}]:{p}"))),
        ]
    }

    proptest! {
        #[test]
        fn parse_of_build_keeps_a_protected_subsequence(
            psk in proptest::collection::vec(any::<u8>(), 32),
            list in proptest::collection::vec(mailbox_strategy(), 1..9),
            expires in 1u64..4_000_000_000,
        ) {
            match build_invite(&psk, &list, expires, false) {
                Ok(text) => {
                    prop_assert!(text.len() <= MAX_INVITE_BYTES);
                    let parsed = parse_invite(&format!(" {text}\n"), expires - 1, false).unwrap();
                    prop_assert_eq!(parsed.psk().as_slice(), psk.as_slice());
                    prop_assert_eq!(parsed.expires_at_epoch_s, expires);
                    prop_assert!(parsed.mailboxes.len() <= MAX_MAILBOXES);
                    // The result is the input minus dropped entries, in order.
                    let mut rest = list.iter();
                    for kept in &parsed.mailboxes {
                        prop_assert!(rest.any(|m| m == kept));
                    }
                    // Nothing protected was dropped.
                    let lan_in = list.iter().filter(|m| m.kind == MailboxKind::Lan).count();
                    let lan_out = parsed.mailboxes.iter().filter(|m| m.kind == MailboxKind::Lan).count();
                    prop_assert_eq!(lan_in, lan_out);
                    let first_relay = list.iter().find(|m| m.kind == MailboxKind::Relay);
                    prop_assert_eq!(
                        first_relay,
                        parsed.mailboxes.iter().find(|m| m.kind == MailboxKind::Relay)
                    );
                    // An input within the budget comes back unchanged.
                    if list.len() <= MAX_MAILBOXES {
                        let whole = json!({"v": 4, "k": URL_SAFE_NO_PAD.encode(&psk), "m": list.iter().map(|m| entry_text(m).unwrap()).collect::<Vec<_>>(), "x": expires}).to_string();
                        if whole.len() <= MAX_INVITE_BYTES {
                            prop_assert_eq!(parsed.mailboxes, list);
                        }
                    }
                }
                Err(e) => prop_assert_eq!(e, InviteError::NotOurCode),
            }
        }

        #[test]
        fn arbitrary_text_never_panics(text in ".{0,200}", now in any::<u64>(), dev in any::<bool>()) {
            let _ = parse_invite(&text, now, dev);
        }
    }
}
