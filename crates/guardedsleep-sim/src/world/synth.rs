//! Synthetic key material and fingerprints: recognisable by the lint, never real secrets.

use crate::rng::Rng;

/// Synthetic PSK material: recognisable by the lint, never a real secret.
pub const SYNTH_PSK_PREFIX: &[u8; 16] = b"GS-SYNTHETIC-PSK";
/// First four bytes of every synthetic DTLS fingerprint ("SYNT"), lowercase hex.
pub const SYNTH_FP_PREFIX_HEX: &str = "53594e54";
/// Every simulation `Link` seed starts with these 16 bytes, so the lint can tell a test seed from
/// a real one.
pub const SYNTH_SEED_PREFIX: &[u8; 16] = b"GS-SYNTH-SEED-v1";

pub fn synthetic_psk(name: &str) -> Vec<u8> {
    let fill = match name {
        "B" => 0xB2,
        _ => 0xA1,
    };
    let mut k = SYNTH_PSK_PREFIX.to_vec();
    k.extend_from_slice(&[fill; 16]);
    k
}

/// The SDP as a fingerprint-confusing relay forwards it: the real value moves into an `s=` line
/// (the first occurrence a substring search finds) and the relay's own fingerprint takes the real
/// `a=fingerprint` line, so DTLS ends at the relay.
pub(super) fn confuse_fingerprint(sdp: &str, mitm_fp: &str) -> String {
    let mut hidden = None;
    let mut body = String::new();
    for line in sdp.split_inclusive("\r\n") {
        if let Some(v) = line.strip_prefix("a=fingerprint:sha-256 ") {
            hidden = Some(v.trim_end().to_owned());
            body.push_str(&format!(
                "a=fingerprint:sha-256 {}\r\n",
                colon_upper(mitm_fp)
            ));
        } else {
            body.push_str(line);
        }
    }
    match (hidden, body.strip_prefix("v=0\r\n")) {
        (Some(h), Some(rest)) => format!("v=0\r\ns=a=fingerprint:sha-256 {h}\r\n{rest}"),
        _ => body,
    }
}

pub(super) fn synthetic_fp(n: u64) -> String {
    let mut bytes = [0u8; 32];
    bytes[..4].copy_from_slice(b"SYNT");
    bytes[24..].copy_from_slice(&n.to_be_bytes());
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn colon_upper(fp: &str) -> String {
    (0..fp.len())
        .step_by(2)
        .map(|i| fp[i..i + 2].to_uppercase())
        .collect::<Vec<_>>()
        .join(":")
}

/// Test seed of one `Link` run, derived from the scenario seed, the actor and the run number.
/// It uses its own generator, so the world's random stream (latencies, faults) is untouched.
/// Simulation traces record it; real seeds are never recorded.
pub(super) fn link_seed(world_seed: u64, actor: &str, run: u32) -> Vec<u8> {
    let name_hash = actor.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    let mut rng = Rng::new(world_seed ^ name_hash ^ (u64::from(run) << 48));
    let mut seed = SYNTH_SEED_PREFIX.to_vec();
    seed.extend((0..2).flat_map(|_| rng.next_u64().to_le_bytes()));
    seed
}

pub(super) fn valid_v3_room(room: &str) -> bool {
    room.len() == 22
        && room
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
