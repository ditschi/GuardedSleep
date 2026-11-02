//! Fuzzes the NIP-01 frames of the Nostr mailbox: arbitrary text from a public relay must be
//! refused or parsed without a panic, an accepted event must stay inside the permitted alphabets
//! (ASCII base64url content and tags, lowercase hex ids and keys), verifying it must never panic,
//! and nothing over the frame cap may parse. Run with: `cargo fuzz run nostr_frame` (needs the
//! nightly pinned in `fuzz/rust-toolchain.toml`).
#![no_main]

use guardedsleep_core::nostr::{MAX_FRAME_BYTES, RelayFrame, is_base64url, parse_relay_frame, verify_event};
use libfuzzer_sys::fuzz_target;

fn is_lower_hex(text: &str) -> bool {
    text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let parsed = parse_relay_frame(text);
    if text.len() > MAX_FRAME_BYTES {
        assert!(parsed.is_none(), "a frame over the cap parsed");
    }
    match parsed {
        Some(RelayFrame::Event { sub, event }) => {
            assert!(is_base64url(&sub));
            assert!(is_base64url(&event.content));
            assert_eq!(event.id.len(), 64);
            assert_eq!(event.pubkey.len(), 64);
            assert_eq!(event.sig.len(), 128);
            assert!(is_lower_hex(&event.id) && is_lower_hex(&event.pubkey) && is_lower_hex(&event.sig));
            assert!(event.tags.iter().flatten().all(|t| is_base64url(t)));
            // Any accepted event is safe to verify, whatever it claims.
            let _ = verify_event(&event);
            // Our own serialisation of it is a frame we would parse again, unchanged.
            let again = format!("[\"EVENT\",\"{sub}\",{}]", event.to_json());
            assert_eq!(
                parse_relay_frame(&again),
                Some(RelayFrame::Event { sub, event }),
                "round trip"
            );
        }
        Some(RelayFrame::Ok { id, .. }) => assert!(id.len() == 64 && is_lower_hex(&id)),
        Some(RelayFrame::Eose { sub }) | Some(RelayFrame::Closed { sub, .. }) => {
            assert!(is_base64url(&sub));
        }
        Some(RelayFrame::Notice { .. }) | Some(RelayFrame::Auth) | None => {}
    }
});
