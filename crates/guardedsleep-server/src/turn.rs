//! ICE servers the signaling server hands out (`signaling-relay` "ICE servers from the relay"):
//! the settings from the environment and the signing of short-lived TURN credentials in coturn's
//! REST scheme. The sans-IO relay only decides *when* (`RelayEffect::IssueIceServers`); this module
//! holds the secret, reads the clock and draws the random id. HMAC-SHA1 lives here and nowhere
//! else (AGENTS.md section 7.3); `just deny` keeps `sha1` out of the core and relay graphs.

use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use secrecy::{ExposeSecret, SecretString};
use serde_json::json;
use sha1::Sha1;

use crate::config::{csv_list, parse_in};

/// Longest credential lifetime (`signaling-relay` "Short-lived TURN credentials"). The only way to
/// raise it is the 12-hour fallback of task 9.1, which did not apply.
const MAX_TTL_S: u32 = 600;
const DEFAULT_TTL_S: u32 = 600;
const DEFAULT_MAX_LIVE_PER_ADDR: usize = 3;

/// A signed TURN credential. Never printed: `Debug` hides both fields.
pub struct Credential {
    pub username: String,
    pub password: String,
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Credential(redacted)")
    }
}

/// Signs coturn REST credentials (`--use-auth-secret`).
#[derive(Debug)]
pub struct TurnIssuer {
    secret: SecretString,
    urls: Vec<String>,
    ttl_s: u32,
}

impl TurnIssuer {
    pub fn new(secret: SecretString, urls: Vec<String>, ttl_s: u32) -> Self {
        Self {
            secret,
            urls,
            ttl_s: ttl_s.clamp(1, MAX_TTL_S),
        }
    }

    /// User name `<expiry unix s>:<16 random bytes base64url>`, password
    /// `base64(HMAC-SHA1(secret, user name))`, exactly as coturn derives it with `--use-auth-secret`
    /// (the "TURN REST API" section of
    /// <https://github.com/coturn/coturn/blob/master/README.turnserver>). The id is random and never
    /// derived from a room id, because coturn logs user names at some levels.
    pub fn credential(&self, now_unix_s: u64, id: [u8; 16]) -> Option<Credential> {
        let expiry = now_unix_s.checked_add(u64::from(self.ttl_s))?;
        let username = format!("{expiry}:{}", URL_SAFE_NO_PAD.encode(id));
        let mut mac = Hmac::<Sha1>::new_from_slice(self.secret.expose_secret().as_bytes()).ok()?;
        mac.update(username.as_bytes());
        let password = STANDARD.encode(mac.finalize().into_bytes());
        Some(Credential { username, password })
    }

    /// The `ice_servers` frame for one member.
    pub fn frame(&self, now_unix_s: u64, id: [u8; 16]) -> Option<String> {
        let c = self.credential(now_unix_s, id)?;
        Some(
            json!({
                "type": "ice_servers",
                "urls": self.urls,
                "username": c.username,
                "credential": c.password,
                "ttl": self.ttl_s,
            })
            .to_string(),
        )
    }
}

/// ICE-related settings. Without `TURN_SECRET` and `TURN_URLS` no TURN credential is ever sent;
/// with only `STUN_URLS` the `welcome` field is the whole feature.
#[derive(Debug, Clone, Default)]
pub struct IceConfig {
    pub stun_urls: Vec<String>,
    turn: Option<TurnSettings>,
    pub max_live_per_addr: usize,
}

#[derive(Debug, Clone)]
struct TurnSettings {
    urls: Vec<String>,
    secret: SecretString,
    ttl_s: u32,
}

/// Host of a `stun:`/`turn:`/`turns:` URL (`<scheme>:<host>[:<port>][?<query>]`), lower-cased.
fn url_host(url: &str, schemes: &[&str]) -> Option<String> {
    let rest = schemes
        .iter()
        .find_map(|s| url.strip_prefix(s))?
        .split('?')
        .next()?;
    let host = match rest.strip_prefix('[') {
        Some(v6) => v6.split(']').next()?,
        None => rest.split(':').next()?,
    };
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

fn check_urls(
    name: &str,
    urls: &[String],
    schemes: &[&str],
    public_host: Option<&str>,
) -> Result<(), String> {
    if urls.is_empty() {
        return Ok(());
    }
    let Some(public_host) = public_host else {
        return Err(format!("{name} needs PUBLIC_HOST"));
    };
    for url in urls {
        // Clients drop a whole set whose host differs from the relay's, so refuse to start
        // instead of silently handing out unusable servers.
        if url_host(url, schemes).as_deref() != Some(public_host) {
            return Err(format!(
                "{name} must be {} URLs of PUBLIC_HOST ({public_host})",
                schemes.join("/")
            ));
        }
    }
    Ok(())
}

impl IceConfig {
    /// Reads `PUBLIC_HOST`, `STUN_URLS`, `TURN_URLS`, `TURN_SECRET`, `TURN_TTL_S` and
    /// `TURN_MAX_LIVE_PER_ADDR`. Errors name the variable, never a value.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let public_host = get("PUBLIC_HOST")
            .map(|h| h.trim().to_ascii_lowercase())
            .filter(|h| !h.is_empty());
        let stun_urls = csv_list(get("STUN_URLS"));
        let turn_urls = csv_list(get("TURN_URLS"));
        check_urls("STUN_URLS", &stun_urls, &["stun:"], public_host.as_deref())?;
        check_urls(
            "TURN_URLS",
            &turn_urls,
            &["turn:", "turns:"],
            public_host.as_deref(),
        )?;
        // coturn REST secret shared through the deployment environment, never from a file in the
        // image: https://github.com/coturn/coturn/blob/master/README.turnserver (--static-auth-secret)
        let secret = get("TURN_SECRET")
            .filter(|s| !s.trim().is_empty())
            .map(SecretString::from);
        // Credential lifetime in seconds, default and maximum 600 (signaling-relay spec); a larger
        // value is clamped, an unusable one falls back to the default.
        let ttl_s = get("TURN_TTL_S")
            .and_then(|s| s.trim().parse::<u32>().ok())
            .map_or(DEFAULT_TTL_S, |n| n.clamp(1, MAX_TTL_S));
        // Live credentials per client address (IPv4, IPv6 /64); many families can share one CGNAT
        // address, so it is configurable. Default 8, as in design section 7 "Gate and abuse bounds".
        let max_live_per_addr = parse_in(
            get("TURN_MAX_LIVE_PER_ADDR"),
            1,
            10_000,
            DEFAULT_MAX_LIVE_PER_ADDR,
        );
        let turn = match (secret, turn_urls.is_empty()) {
            (Some(secret), false) => Some(TurnSettings {
                urls: turn_urls,
                secret,
                ttl_s,
            }),
            _ => None,
        };
        Ok(Self {
            stun_urls,
            turn,
            max_live_per_addr,
        })
    }

    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|k| std::env::var(k).ok())
    }

    pub fn turn_enabled(&self) -> bool {
        self.turn.is_some()
    }

    pub fn turn_ttl_s(&self) -> Option<u32> {
        self.turn.as_ref().map(|t| t.ttl_s)
    }

    pub fn issuer(&self) -> Option<TurnIssuer> {
        self.turn
            .as_ref()
            .map(|t| TurnIssuer::new(t.secret.clone(), t.urls.clone(), t.ttl_s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn cfg(pairs: &[(&str, &str)]) -> Result<IceConfig, String> {
        let m: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        IceConfig::from_lookup(|k| m.get(k).cloned())
    }

    const FULL: [(&str, &str); 4] = [
        ("PUBLIC_HOST", "gs.example.org"),
        ("STUN_URLS", "stun:gs.example.org:3478"),
        (
            "TURN_URLS",
            "turn:gs.example.org:3478?transport=udp, turns:GS.example.org:5349?transport=tcp",
        ),
        ("TURN_SECRET", "s3cret-value-for-tests"),
    ];

    /// Reference value computed outside this code with the algorithm of coturn's "TURN REST API"
    /// (https://github.com/coturn/coturn/blob/master/README.turnserver): user name
    /// `<timestamp>:<id>`, password `base64(hmac-sha1(secret, user name))`:
    /// `printf '%s' '1700000600:AAECAwQFBgcICQoLDA0ODw' | openssl dgst -sha1 -hmac 'coturn-test-secret' -binary | base64`
    #[test]
    fn credential_matches_the_coturn_rest_algorithm() {
        let issuer = TurnIssuer::new(SecretString::from("coturn-test-secret"), vec![], 600);
        let id: [u8; 16] = core::array::from_fn(|i| i as u8);
        let c = issuer.credential(1_700_000_000, id).unwrap();
        assert_eq!(c.username, "1700000600:AAECAwQFBgcICQoLDA0ODw");
        assert_eq!(c.password, "YzN0SkguFxM1GrLNWBDVU/nRTR0=");
    }

    #[test]
    fn frame_has_the_wire_shape() {
        let issuer = TurnIssuer::new(
            SecretString::from("k"),
            vec!["turn:h:3478?transport=udp".to_owned()],
            90,
        );
        let v: serde_json::Value =
            serde_json::from_str(&issuer.frame(1_000, [7; 16]).unwrap()).unwrap();
        assert_eq!(v["type"], "ice_servers");
        assert_eq!(v["urls"], json!(["turn:h:3478?transport=udp"]));
        assert_eq!(v["ttl"], 90);
        assert!(v["username"].as_str().unwrap().starts_with("1090:"));
        assert_eq!(v["credential"].as_str().unwrap().len(), 28);
    }

    #[test]
    fn user_names_are_random_and_never_the_same_twice() {
        let issuer = TurnIssuer::new(SecretString::from("k"), vec![], 600);
        let a = issuer.credential(5, [1; 16]).unwrap();
        let b = issuer.credential(5, [2; 16]).unwrap();
        assert_ne!(a.username, b.username);
        assert_ne!(a.password, b.password);
    }

    #[test]
    fn ttl_is_clamped_to_600_and_defaults_to_600() {
        let ttl = |v: &str| {
            let mut p = FULL.to_vec();
            p.push(("TURN_TTL_S", v));
            cfg(&p).unwrap().turn_ttl_s()
        };
        assert_eq!(cfg(&FULL).unwrap().turn_ttl_s(), Some(600));
        assert_eq!(ttl("43200"), Some(600));
        assert_eq!(ttl("60"), Some(60));
        assert_eq!(ttl("0"), Some(1));
        assert_eq!(ttl("soon"), Some(600));
    }

    #[test]
    fn defaults_and_the_per_address_cap() {
        assert_eq!(cfg(&FULL).unwrap().max_live_per_addr, 3);
        let mut p = FULL.to_vec();
        p.push(("TURN_MAX_LIVE_PER_ADDR", "3"));
        assert_eq!(cfg(&p).unwrap().max_live_per_addr, 3);
        p.push(("TURN_MAX_LIVE_PER_ADDR", "0"));
        assert_eq!(cfg(&p).unwrap().max_live_per_addr, 3);
    }

    #[test]
    fn a_url_of_another_host_refuses_to_start() {
        for (k, v) in [
            ("STUN_URLS", "stun:other.example:3478"),
            ("TURN_URLS", "turn:gs.example.org.evil.example:3478"),
            (
                "TURN_URLS",
                "turn:gs.example.org:3478,turn:other.example:3478",
            ),
            ("TURN_URLS", "http://gs.example.org"),
            ("STUN_URLS", "turn:gs.example.org:3478"),
        ] {
            let mut p = FULL.to_vec();
            p.push((k, v));
            let err = cfg(&p).unwrap_err();
            assert!(err.contains(k), "{err}");
            assert!(!err.contains("s3cret"), "{err}");
        }
        // URLs without a PUBLIC_HOST cannot be checked.
        let err = cfg(&[("STUN_URLS", "stun:a.example:3478")]).unwrap_err();
        assert!(err.contains("PUBLIC_HOST"));
    }

    #[test]
    fn host_comparison_ignores_case_and_handles_ipv6_literals() {
        let c = cfg(&[("PUBLIC_HOST", "::1"), ("STUN_URLS", "stun:[::1]:3478")]).unwrap();
        assert_eq!(c.stun_urls, ["stun:[::1]:3478"]);
        assert!(cfg(&FULL).is_ok());
    }

    #[test]
    fn turn_needs_secret_and_urls_and_stun_alone_is_welcome_only() {
        let no_secret: Vec<_> = FULL
            .iter()
            .copied()
            .filter(|(k, _)| *k != "TURN_SECRET")
            .collect();
        assert!(!cfg(&no_secret).unwrap().turn_enabled());
        let no_urls: Vec<_> = FULL
            .iter()
            .copied()
            .filter(|(k, _)| *k != "TURN_URLS")
            .collect();
        let c = cfg(&no_urls).unwrap();
        assert!(!c.turn_enabled());
        assert_eq!(c.stun_urls.len(), 1);
        assert!(cfg(&[]).unwrap().stun_urls.is_empty());
        assert!(cfg(&FULL).unwrap().turn_enabled());
        assert!(!cfg(&[("TURN_SECRET", "  ")]).unwrap().turn_enabled());
    }

    #[test]
    fn the_secret_and_credentials_never_appear_in_debug_output() {
        let c = cfg(&FULL).unwrap();
        let dbg = format!("{c:?} {:?}", c.issuer().unwrap());
        assert!(!dbg.contains("s3cret-value-for-tests"), "{dbg}");
        let cred = c.issuer().unwrap().credential(1, [9; 16]).unwrap();
        let dbg = format!("{cred:?}");
        assert!(!dbg.contains(&cred.username) && !dbg.contains(&cred.password));
    }
}
