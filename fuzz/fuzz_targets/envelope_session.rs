//! Drives a `SessionState` with a fuzzer-chosen sequence of events (peer announcements and
//! messages, challenge echoes, our own sealing, retirement, the passage of time) and checks the
//! security invariants after every step:
//! - a session is confirmed only by a `here` that echoes the challenge issued for it, within 60 s;
//! - a retired session is never accepted again;
//! - nothing panics. Run with: `cargo fuzz run envelope_session` (nightly, see
//!   `fuzz/rust-toolchain.toml`).
#![no_main]

use std::collections::BTreeMap;

use chacha20::ChaCha20Rng;
use chacha20::rand_core::SeedableRng;
use guardedsleep_core::envelope::{
    CHALLENGE_LEN, CHALLENGE_VALID_MS, Frame, MessageClass, NONCE_LEN, OpenError,
    Purpose, SessionId, SessionState, derive_key, seal_with_nonce,
};
use guardedsleep_core::protocol::Role;
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
enum Op {
    /// A `here` from peer session `peer` (of 4), naming us or nobody, optionally echoing the last
    /// challenge we issued to that session (or a wrong one) and optionally carrying its own.
    Here {
        peer: u8,
        names_us: bool,
        echo: Echo,
        brings_challenge: bool,
        seq: u8,
    },
    /// A non-`here` message from `peer`.
    Message { peer: u8, seq: u8 },
    /// Our own message to the confirmed peer, if any.
    Send,
    /// The caller processed a stop from `peer`.
    Retire { peer: u8 },
    /// Time passes.
    Advance { ms: u16 },
}

#[derive(Arbitrary, Debug)]
enum Echo {
    None,
    LastIssued,
    Wrong,
}

fuzz_target!(|ops: Vec<Op>| {
    let Some(key) = derive_key(&[3u8; 32], Purpose::Seal) else {
        return;
    };
    let mut rng = ChaCha20Rng::from_seed([9; 32]);
    let own = SessionId::new([1; 16]);
    let mut state = SessionState::new(Role::Baby, own);
    let session = |n: u8| SessionId::new([0x10 + (n % 4); 16]);
    // What the model knows: the challenge handed to each peer session and when.
    let mut issued: BTreeMap<u8, ([u8; CHALLENGE_LEN], u64)> = BTreeMap::new();
    let mut retired: Vec<u8> = Vec::new();
    let mut now = 0u64;
    let mut nonce = 0u64;

    for op in ops.iter().take(64) {
        nonce += 1;
        let mut n = [0u8; NONCE_LEN];
        if let Some(s) = n.get_mut(..8) {
            s.copy_from_slice(&nonce.to_be_bytes());
        }
        match op {
            Op::Advance { ms } => now += u64::from(*ms) * 8,
            Op::Send => {
                let _ = state.seal_to_peer(&key, &mut rng, MessageClass::Control, b"x");
            }
            Op::Retire { peer } => {
                let p = peer % 4;
                state.retire(session(p));
                if !retired.contains(&p) {
                    retired.push(p);
                }
            }
            Op::Message { peer, seq } => {
                let p = peer % 4;
                let frame = Frame {
                    role: Role::Parent,
                    class: MessageClass::Control,
                    recipient: own,
                    sender: session(p),
                    seq: u32::from(*seq),
                    challenge: [0; CHALLENGE_LEN],
                    echo: [0; CHALLENGE_LEN],
                    tag_key: None,
                };
                let Ok(s) = seal_with_nonce(&key, &frame, b"m", n) else {
                    continue;
                };
                let before = state.peer_session();
                let r = state.open(&key, &mut rng, now, &s.bytes);
                if retired.contains(&p) {
                    assert_eq!(r.err(), Some(OpenError::RetiredSender));
                }
                // A message never changes who the confirmed peer is.
                assert_eq!(state.peer_session(), before);
            }
            Op::Here {
                peer,
                names_us,
                echo,
                brings_challenge,
                seq,
            } => {
                let p = peer % 4;
                let echo_value = match echo {
                    Echo::None => [0; CHALLENGE_LEN],
                    Echo::LastIssued => issued.get(&p).map_or([1; CHALLENGE_LEN], |(c, _)| *c),
                    Echo::Wrong => [0xAA; CHALLENGE_LEN],
                };
                let frame = Frame {
                    role: Role::Parent,
                    class: MessageClass::Here,
                    recipient: if *names_us { own } else { SessionId::NONE },
                    sender: session(p),
                    seq: u32::from(*seq),
                    challenge: if *brings_challenge {
                        [0x55; CHALLENGE_LEN]
                    } else {
                        [0; CHALLENGE_LEN]
                    },
                    echo: echo_value,
                    tag_key: None,
                };
                let Ok(s) = seal_with_nonce(&key, &frame, b"h", n) else {
                    continue;
                };
                let was_retired = retired.contains(&p);
                match state.open(&key, &mut rng, now, &s.bytes) {
                    Ok(o) => {
                        assert!(!was_retired, "a retired session was accepted");
                        if o.confirmed {
                            // Only a valid echo of an issued challenge confirms.
                            let (c, at) = issued.get(&p).copied().expect("challenge was issued");
                            assert_eq!(c, echo_value, "confirmed with a wrong echo");
                            assert!(now - at <= CHALLENGE_VALID_MS, "confirmed after expiry");
                            assert_eq!(state.peer_session(), Some(session(p)));
                            issued.remove(&p);
                            // A confirmed session replaces and retires the previous one.
                            for q in 0..4u8 {
                                if q != p && state.is_retired(&session(q)) && !retired.contains(&q) {
                                    retired.push(q);
                                }
                            }
                        }
                        if let Some(r) = o.reply {
                            if r.challenge != [0; CHALLENGE_LEN] {
                                issued.entry(p).or_insert((r.challenge, now));
                                if let Some(e) = issued.get_mut(&p) {
                                    if e.0 != r.challenge {
                                        *e = (r.challenge, now);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if was_retired {
                            assert_eq!(e, OpenError::RetiredSender);
                        }
                    }
                }
            }
        }
    }
});
