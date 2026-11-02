//! Fuzzes the receive side of the sealed signaling envelope: arbitrary bytes from a mailbox, and
//! arbitrary full-bucket plaintext sealed under the real key (so the plaintext parser and the
//! session rules see fields no honest sender produces), must be refused or opened without a panic;
//! a state that never issued a challenge must never confirm a session; and any single-byte change
//! to a sealed message must be refused. Run with: `cargo fuzz run envelope_open` (needs the nightly
//! pinned in `fuzz/rust-toolchain.toml`).
#![no_main]

use chacha20::ChaCha20Rng;
use chacha20::rand_core::SeedableRng;
use guardedsleep_core::envelope::{
    CHALLENGE_LEN, Frame, MessageClass, NONCE_LEN, Purpose, SessionId, SessionState,
    derive_key, seal_raw_plaintext, seal_with_nonce,
};
use guardedsleep_core::protocol::Role;
use libfuzzer_sys::fuzz_target;

const CLASSES: [MessageClass; 4] = [
    MessageClass::Here,
    MessageClass::Control,
    MessageClass::Description,
    MessageClass::Candidates,
];

fuzz_target!(|data: &[u8]| {
    let Some(key) = derive_key(&[3u8; 32], Purpose::Seal) else {
        return;
    };
    let mut rng = ChaCha20Rng::from_seed([9; 32]);
    let own = SessionId::new([1; 16]);
    let peer = SessionId::new([2; 16]);

    // 1. Arbitrary bytes straight from a mailbox.
    let mut state = SessionState::new(Role::Baby, own);
    let _ = state.open(&key, &mut rng, 0, data);

    // 2. Arbitrary plaintext of a full bucket, sealed by "the parent" under the real key. A fresh
    // state has issued no challenge, so nothing the parent sends can confirm its session.
    let class = CLASSES[usize::from(data.first().copied().unwrap_or(0)) % CLASSES.len()];
    let mut plain = data.get(1..).unwrap_or(&[]).to_vec();
    plain.resize(class.bucket(), 0);
    let Ok(sealed) = seal_raw_plaintext(&key, Role::Parent, class, &plain, [5; NONCE_LEN]) else {
        return;
    };
    let mut fresh = SessionState::new(Role::Baby, own);
    if let Ok(opened) = fresh.open(&key, &mut rng, 1_000, &sealed.bytes) {
        assert!(!opened.confirmed, "confirmed without a challenge");
    }

    // 3. A well-formed `here` round trips, and one flipped byte, chosen by the input, never opens.
    let message = data
        .get(..MessageClass::Here.max_message().min(data.len()))
        .unwrap_or(data);
    let frame = Frame {
        role: Role::Parent,
        class: MessageClass::Here,
        recipient: own,
        sender: peer,
        seq: 0,
        challenge: [0; CHALLENGE_LEN],
        echo: [0; CHALLENGE_LEN],
        tag_key: None,
    };
    let Ok(good) = seal_with_nonce(&key, &frame, message, [7; NONCE_LEN]) else {
        return;
    };
    let mut state = SessionState::new(Role::Baby, own);
    let opened = state.open(&key, &mut rng, 2_000, &good.bytes);
    assert_eq!(
        opened.map(|o| o.message.to_vec()),
        Ok(message.to_vec()),
        "round trip"
    );
    let at = data.first().map_or(0, |b| usize::from(*b)) % good.bytes.len();
    let mut tampered = good.bytes.clone();
    if let Some(byte) = tampered.get_mut(at) {
        *byte ^= 0x80;
    }
    let mut other = SessionState::new(Role::Baby, own);
    assert!(other.open(&key, &mut rng, 2_000, &tampered).is_err());
});
