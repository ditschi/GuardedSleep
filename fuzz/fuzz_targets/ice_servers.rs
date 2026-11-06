//! Fuzzes the relay's `welcome.iceServers` and `ice_servers` frames (design section 7 of
//! `alpha-connection-model`): arbitrary text, and a structured frame built from arbitrary parts,
//! must never panic, and whatever the core accepts must obey the bounds (at most 8 URLs, only
//! `stun:`/`turn:`/`turns:`, every host exactly the mailbox host, user name and credential at most
//! 128 ASCII characters). Run with: `cargo fuzz run ice_servers` (or `just fuzz ice_servers`; needs
//! the nightly pinned in `tools/nightly/rust-toolchain.toml`).
#![no_main]

use arbitrary::Arbitrary;
use guardedsleep_core::ice::{IceServer, servers_of_frame};
use libfuzzer_sys::fuzz_target;
use serde_json::json;

const RELAY: &str = "wss://relay.example:8443/path";
const HOST: &str = "relay.example";

#[derive(Arbitrary, Debug)]
struct Parts {
    welcome: bool,
    urls: Vec<String>,
    username: String,
    credential: String,
    ttl: u64,
    /// Hosts that are right half of the time, so accepted frames occur.
    good_host: bool,
    scheme: u8,
}

fn check(list: &[IceServer]) {
    for s in list {
        assert!(!s.urls.is_empty() && s.urls.len() <= 8);
        for u in &s.urls {
            let rest = ["stun:", "turn:", "turns:"]
                .iter()
                .find_map(|p| u.strip_prefix(p))
                .expect("scheme");
            let host = rest.split([':', '/', '?']).next().unwrap_or("");
            assert_eq!(host, HOST, "{u}");
            assert!(u.is_ascii());
        }
        for f in [&s.username, &s.credential].into_iter().flatten() {
            assert!(!f.is_empty() && f.len() <= 128 && f.is_ascii());
        }
        assert_eq!(s.username.is_some(), s.credential.is_some());
    }
}

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        check(&servers_of_frame(RELAY, text, 1_000));
    }
    let mut u = arbitrary::Unstructured::new(data);
    let Ok(p) = Parts::arbitrary(&mut u) else {
        return;
    };
    let scheme = ["stun:", "turn:", "turns:", "http:"][usize::from(p.scheme) % 4];
    let urls: Vec<String> = p
        .urls
        .iter()
        .map(|x| {
            if p.good_host {
                format!("{scheme}{HOST}{x}")
            } else {
                format!("{scheme}{x}")
            }
        })
        .collect();
    let frame = if p.welcome {
        json!({"type":"welcome","clientId":"c","roomId":"r","iceServers":urls})
    } else {
        json!({"type":"ice_servers","urls":urls,"username":p.username,
            "credential":p.credential,"ttl":p.ttl})
    };
    check(&servers_of_frame(RELAY, &frame.to_string(), 1_000));
});
