//! Fuzzes the receive side of the sealed signaling envelope: arbitrary bytes from a mailbox must
//! be refused or opened, never panic, and a sealed message must round trip while any single-byte
//! change to it is refused. Run with: `cargo fuzz run envelope_open` (needs the nightly pinned in
//! `fuzz/rust-toolchain.toml`).
#![no_main]

use guardedsleep_core::envelope::{
    Frame, MessageClass, NONCE_LEN, Purpose, SessionId, SessionState, derive_key, seal_with_nonce,
};
use guardedsleep_core::protocol::Role;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some(key) = derive_key(&[3u8; 32], Purpose::Seal) else {
        return;
    };
    let own = SessionId::new([1; 16]);
    let peer = SessionId::new([2; 16]);

    // 1. Arbitrary bytes straight from a mailbox.
    let mut state = SessionState::new(Role::Baby, own);
    let _ = state.open(&key, 0, data);

    // 2. The same bytes as a message: a `here` that answers our session must open and round trip.
    let class = MessageClass::Here;
    let message = data
        .get(..class.max_message().min(data.len()))
        .unwrap_or(data);
    let frame = Frame {
        role: Role::Parent,
        class,
        recipient: own,
        sender: peer,
        seq: 0,
    };
    let Ok(sealed) = seal_with_nonce(&key, &frame, message, [7; NONCE_LEN]) else {
        return;
    };
    let mut state = SessionState::new(Role::Baby, own);
    let opened = state.open(&key, 0, &sealed.bytes);
    assert_eq!(opened.map(|o| o.message), Ok(message.to_vec()));

    // 3. One flipped byte, chosen by the input, must never open.
    let at = data.first().map_or(0, |b| usize::from(*b)) % sealed.bytes.len();
    let mut tampered = sealed.bytes.clone();
    if let Some(byte) = tampered.get_mut(at) {
        *byte ^= 0x80;
    }
    let mut fresh = SessionState::new(Role::Baby, own);
    assert!(fresh.open(&key, 0, &tampered).is_err());
});
