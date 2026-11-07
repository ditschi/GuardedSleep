//! ICE servers from the signaling relays (`alpha-connection-model`, design section 7). Each relay
//! mailbox may supply STUN URLs in `welcome.iceServers` and a TURN set in an `ice_servers` frame.
//! The core validates them against that mailbox's own host, keeps the latest valid set per relay
//! with its monotonic receipt time and hands every attempt the union as `CreateAttempt.ice_servers`.
//! The platform adds a user-entered TURN server and builds the WebRTC configuration; credentials
//! are redacted in `Debug`, never logged, never persisted and never part of an invite or a
//! mailbox update.

// Relay-supplied (untrusted) text flows through here: return errors instead of panicking.
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

use serde::{Deserialize, Serialize};

use crate::protocol::IceServersFrame;

/// Most URLs one set may carry.
pub const MAX_ICE_URLS: usize = 8;
/// Longest user name and credential, in ASCII characters.
pub const MAX_ICE_FIELD_LEN: usize = 128;
/// Largest accepted `ttl`, in seconds (design section 7, "Allocation lifetime").
pub const MAX_ICE_TTL_S: u64 = 600;
/// A set counts as valid until this long before its TTL ends, so a call never starts on a
/// credential that expires mid-handshake.
pub const ICE_EXPIRY_MARGIN_MS: u64 = 30_000;
/// The built-in STUN port of `guardedsleep-server`.
pub const STUN_PORT: u16 = 3478;

/// One WebRTC ICE server entry of an attempt. STUN entries carry no user name or credential.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct IceServer {
    pub urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

impl std::fmt::Debug for IceServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let shown = |o: &Option<String>| if o.is_some() { "<redacted>" } else { "<none>" };
        f.debug_struct("IceServer")
            .field("urls", &self.urls)
            .field("username", &shown(&self.username))
            .field("credential", &shown(&self.credential))
            .finish()
    }
}

/// `host` of a `ws://`/`wss://` mailbox URL or of a `stun:`/`turn:`/`turns:` URL (after the
/// scheme), without port, path or query; IPv6 literals lose their brackets.
fn host_of(rest: &str) -> Option<&str> {
    let host = if let Some(v6) = rest.strip_prefix('[') {
        // Brackets only wrap an IPv6 literal.
        v6.split_once(']')
            .map(|(h, _)| h)
            .filter(|h| h.contains(':'))?
    } else {
        rest.split([':', '/', '?']).next()?
    };
    (!host.is_empty()).then_some(host)
}

/// The lower-case host of a relay mailbox URL, `None` for anything that is not `ws(s)://host...`.
pub(crate) fn mailbox_host(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("wss://")
        .or_else(|| url.strip_prefix("ws://"))?;
    host_of(rest).map(str::to_ascii_lowercase)
}

fn url_ok(host: &str, url: &str) -> bool {
    let Some(rest) = ["stun:", "turn:", "turns:"]
        .iter()
        .find_map(|scheme| url.strip_prefix(scheme))
    else {
        return false;
    };
    url.len() <= 256 && url.chars().all(|c| c.is_ascii_graphic()) && host_of(rest) == Some(host)
}

fn urls_ok(host: &str, urls: &[String]) -> bool {
    !urls.is_empty() && urls.len() <= MAX_ICE_URLS && urls.iter().all(|u| url_ok(host, u))
}

fn field_ok(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_ICE_FIELD_LEN && s.chars().all(|c| c.is_ascii_graphic())
}

/// The built-in STUN of a relay host, for a relay that supplied no set.
fn fallback_stun(host: &str) -> IceServer {
    let url = if host.contains(':') {
        format!("stun:[{host}]:{STUN_PORT}")
    } else {
        format!("stun:{host}:{STUN_PORT}")
    };
    IceServer {
        urls: vec![url],
        username: None,
        credential: None,
    }
}

/// What one relay mailbox supplied, as the core last accepted it.
#[derive(Debug, Default)]
pub(crate) struct RelayIce {
    /// `welcome.iceServers`: replaced by every accepted `welcome`; no expiry (it has no credential).
    stun: Option<IceServer>,
    /// The latest `ice_servers` set and the monotonic time (ms) after which it is not offered.
    turn: Option<(IceServer, u64)>,
}

impl RelayIce {
    /// An accepted `welcome` of the relay at `host`. A missing or invalid field clears the set.
    pub fn on_welcome(&mut self, host: Option<&str>, urls: Option<Vec<String>>) {
        self.stun = match (host, urls) {
            (Some(h), Some(urls)) if urls_ok(h, &urls) => Some(IceServer {
                urls,
                username: None,
                credential: None,
            }),
            _ => None,
        };
    }

    /// An `ice_servers` frame received at `now` (monotonic ms). An invalid frame is dropped whole
    /// and leaves the previous set in place.
    pub fn on_ice_servers(&mut self, now: u64, host: Option<&str>, f: IceServersFrame) {
        let Some(host) = host else { return };
        let valid = urls_ok(host, &f.urls)
            && field_ok(&f.username)
            && field_ok(&f.credential)
            && (1..=MAX_ICE_TTL_S).contains(&f.ttl);
        if !valid {
            return;
        }
        let end = now.saturating_add(f.ttl * 1000);
        // Expiry minus the margin; a TTL shorter than the margin is never offered.
        let Some(until) = end.checked_sub(ICE_EXPIRY_MARGIN_MS) else {
            return;
        };
        self.turn = Some((
            IceServer {
                urls: f.urls,
                username: Some(f.username),
                credential: Some(f.credential),
            },
            until,
        ));
    }

    /// Appends the still-valid sets at `now` (STUN, then TURN); an expired TURN set is dropped.
    fn valid_sets(&mut self, now: u64, out: &mut Vec<IceServer>) {
        if self.turn.as_ref().is_some_and(|(_, until)| now > *until) {
            self.turn = None;
        }
        out.extend(self.stun.iter().cloned());
        out.extend(self.turn.iter().map(|(s, _)| s.clone()));
    }

    /// Appends this relay's contribution at `now`: its still-valid sets, or the STUN fallback
    /// when it has none. Expired sets are dropped.
    pub fn collect(&mut self, now: u64, host: &str, out: &mut Vec<IceServer>) {
        let before = out.len();
        self.valid_sets(now, out);
        if out.len() == before {
            out.push(fallback_stun(host));
        }
    }
}

/// The ICE servers one raw frame from the relay at `relay_url` would contribute at `now_ms`:
/// what a fresh `Link` holds after receiving just that frame, without the STUN fallback. Empty
/// for anything that is not an accepted `welcome.iceServers` or `ice_servers` frame. This is the
/// entry the fuzz target exercises (`fuzz/fuzz_targets/ice_servers.rs`); the `Link` uses the same
/// validation.
pub fn servers_of_frame(relay_url: &str, raw: &str, now_ms: u64) -> Vec<IceServer> {
    let host = mailbox_host(relay_url);
    let mut r = RelayIce::default();
    match crate::protocol::parse_server_message(raw) {
        Some(crate::protocol::ServerMessage::Welcome { ice_servers, .. }) => {
            r.on_welcome(host.as_deref(), ice_servers);
        }
        Some(crate::protocol::ServerMessage::IceServers(f)) => {
            r.on_ice_servers(now_ms, host.as_deref(), f);
        }
        _ => {}
    }
    let mut out = Vec::new();
    r.valid_sets(now_ms, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: &str = "relay.example";

    fn frame(urls: &[&str], ttl: u64) -> IceServersFrame {
        IceServersFrame {
            urls: urls.iter().map(|u| (*u).to_owned()).collect(),
            username: "1700000600:AbCd_-".into(),
            credential: "c2VjcmV0+/=".into(),
            ttl,
        }
    }

    fn good() -> IceServersFrame {
        frame(
            &[
                "stun:relay.example:3478",
                "turn:relay.example:3478?transport=udp",
                "turns:relay.example:5349?transport=tcp",
            ],
            600,
        )
    }

    fn list(r: &mut RelayIce, now: u64) -> Vec<IceServer> {
        let mut out = Vec::new();
        r.collect(now, H, &mut out);
        out
    }

    #[test]
    fn mailbox_host_forms() {
        assert_eq!(mailbox_host("wss://Relay.Example:8443/x"), Some(H.into()));
        assert_eq!(mailbox_host("ws://[::1]:8787"), Some("::1".into()));
        assert_eq!(mailbox_host("https://relay.example"), None);
        assert_eq!(mailbox_host("wss://"), None);
        assert_eq!(mailbox_host("wss://[relay.example]"), None);
    }

    #[test]
    fn valid_frame_is_kept() {
        let mut r = RelayIce::default();
        r.on_ice_servers(1_000, Some(H), good());
        let got = list(&mut r, 2_000);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].username.as_deref(), Some("1700000600:AbCd_-"));
        assert_eq!(got[0].urls.len(), 3);
    }

    #[test]
    fn foreign_host_drops_the_whole_frame() {
        let mut r = RelayIce::default();
        let f = frame(&["turn:relay.example:3478", "turn:evil.example:3478"], 600);
        r.on_ice_servers(0, Some(H), f);
        // Fallback only: nothing of the frame was kept.
        assert_eq!(list(&mut r, 1)[0].urls, vec!["stun:relay.example:3478"]);
        assert!(list(&mut r, 1)[0].credential.is_none());
        // A host that merely starts with the relay host is foreign too, as is a different case.
        for u in ["turn:relay.example.invalid:3478", "turn:RELAY.example:3478"] {
            r.on_ice_servers(0, Some(H), frame(&[u], 600));
            assert!(list(&mut r, 1)[0].credential.is_none(), "{u}");
        }
    }

    #[test]
    fn invalid_frame_keeps_the_previous_set() {
        let mut r = RelayIce::default();
        r.on_ice_servers(0, Some(H), good());
        r.on_ice_servers(5, Some(H), frame(&["turn:evil.example:1"], 600));
        assert_eq!(list(&mut r, 10)[0].urls.len(), 3);
    }

    #[test]
    fn expired_set_is_not_attached() {
        let mut r = RelayIce::default();
        r.on_ice_servers(10_000, Some(H), frame(&["turn:relay.example:3478"], 60));
        // Valid while receipt + ttl - 30 s >= now: until 40_000.
        assert!(list(&mut r, 40_000)[0].credential.is_some());
        let late = list(&mut r, 40_001);
        assert!(late[0].credential.is_none());
        assert_eq!(late[0].urls, vec!["stun:relay.example:3478"]);
        // Dropped for good, not just hidden.
        assert!(r.turn.is_none());
    }

    #[test]
    fn ttl_shorter_than_the_margin_is_never_offered() {
        let mut r = RelayIce::default();
        r.on_ice_servers(0, Some(H), frame(&["turn:relay.example:3478"], 29));
        assert!(list(&mut r, 0)[0].credential.is_none());
        // ttl 30 is valid exactly at receipt (receipt + ttl - 30 s >= now), not a millisecond later.
        r.on_ice_servers(0, Some(H), frame(&["turn:relay.example:3478"], 30));
        assert!(list(&mut r, 0)[0].credential.is_some());
        assert!(list(&mut r, 1)[0].credential.is_none());
    }

    #[test]
    fn bounds() {
        let mut r = RelayIce::default();
        let ok = |r: &mut RelayIce, f: IceServersFrame| {
            r.turn = None;
            r.on_ice_servers(0, Some(H), f);
            r.turn.is_some()
        };
        assert!(ok(&mut r, good()));
        // At most 8 URLs, at least 1.
        let u = "turn:relay.example:3478";
        assert!(ok(&mut r, frame(&[u; 8], 600)));
        assert!(!ok(&mut r, frame(&[u; 9], 600)));
        assert!(!ok(&mut r, frame(&[], 600)));
        // Schemes.
        for bad in [
            "http://relay.example",
            "stuns:relay.example:5349",
            "TURN:relay.example",
            "relay.example:3478",
            "turn:",
            "turn:relay.example:3478 ",
            "turn:relay.example:3478\u{e9}",
        ] {
            assert!(!ok(&mut r, frame(&[bad], 600)), "{bad}");
        }
        // ttl 1..=600 (1..30 parses but never offers; checked separately above).
        assert!(!ok(&mut r, frame(&[u], 0)));
        assert!(!ok(&mut r, frame(&[u], 601)));
        assert!(ok(&mut r, frame(&[u], 600)));
        // Field lengths and alphabet.
        let mut f = frame(&[u], 600);
        f.username = "a".repeat(128);
        f.credential = "b".repeat(128);
        assert!(ok(&mut r, f.clone()));
        f.credential = "b".repeat(129);
        assert!(!ok(&mut r, f.clone()));
        f.credential = "b".repeat(128);
        f.username = "a".repeat(129);
        assert!(!ok(&mut r, f.clone()));
        f.username = String::new();
        assert!(!ok(&mut r, f.clone()));
        f.username = "user\u{e9}".into();
        assert!(!ok(&mut r, f.clone()));
        f.username = "a b".into();
        assert!(!ok(&mut r, f));
    }

    #[test]
    fn welcome_stun_set_and_fallback() {
        let mut r = RelayIce::default();
        assert_eq!(list(&mut r, 0)[0].urls, vec!["stun:relay.example:3478"]);
        r.on_welcome(Some(H), Some(vec!["stun:relay.example:3479".into()]));
        let got = list(&mut r, 0);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].urls, vec!["stun:relay.example:3479"]);
        // Credentials in `welcome` are not a thing; a foreign host drops the field.
        r.on_welcome(Some(H), Some(vec!["stun:evil.example:3478".into()]));
        assert_eq!(list(&mut r, 0)[0].urls, vec!["stun:relay.example:3478"]);
        // A welcome without the field clears the previous set.
        r.on_welcome(Some(H), Some(vec!["stun:relay.example:3479".into()]));
        r.on_welcome(Some(H), None);
        assert_eq!(list(&mut r, 0)[0].urls, vec!["stun:relay.example:3478"]);
    }

    #[test]
    fn ipv6_fallback_is_bracketed() {
        let mut r = RelayIce::default();
        let mut out = Vec::new();
        r.collect(0, "::1", &mut out);
        assert_eq!(out[0].urls, vec!["stun:[::1]:3478"]);
    }

    #[test]
    fn servers_of_frame_accepts_only_valid_frames_of_the_relay_host() {
        let url = "wss://Relay.Example:8443";
        let ok = r#"{"type":"ice_servers","urls":["turn:relay.example:3478"],"username":"1:a","credential":"x","ttl":600}"#;
        assert_eq!(servers_of_frame(url, ok, 0).len(), 1);
        let foreign = ok.replace("relay.example", "other.example");
        assert!(servers_of_frame(url, &foreign, 0).is_empty());
        let welcome = r#"{"type":"welcome","clientId":"c","roomId":"r","iceServers":["stun:relay.example:3478"]}"#;
        assert_eq!(servers_of_frame(url, welcome, 0).len(), 1);
        assert!(servers_of_frame(url, "not json", 0).is_empty());
        assert!(servers_of_frame("https://relay.example", ok, 0).is_empty());
    }

    #[test]
    fn debug_redacts_credentials() {
        let s = IceServer {
            urls: vec!["turn:relay.example:3478".into()],
            username: Some("1700000600:id".into()),
            credential: Some("TOPSECRET".into()),
        };
        let shown = format!("{s:?} {:?}", good());
        assert!(!shown.contains("TOPSECRET"));
        assert!(!shown.contains("c2VjcmV0"));
        assert!(!shown.contains("1700000600"));
    }
}
