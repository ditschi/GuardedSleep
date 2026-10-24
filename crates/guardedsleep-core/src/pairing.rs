//! QR pairing material, the PSK-derived room id and the direction-bound `pair_proof` MAC that
//! binds the PSK to both DTLS certificates (ADR-004 amendment, ADR-017 §1).

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
use hkdf::Hkdf;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

use crate::protocol::{LEGACY_PROTOCOL_VERSION, Role};

const ROOM_INFO: &[u8] = b"gs-room/3";
const PROOF_PREFIX: &str = "gs-pair-proof/3|";
const ADMISSION_INFO: &[u8] = b"guardedsleep relay admission v1";
const ADMISSION_PREFIX: &str = "gs-relay-admit/1|";

/// The DTLS fingerprint an SDP commits to, lowercase hex without separators (64 characters).
///
/// libwebrtc authenticates DTLS against the real `a=fingerprint` attribute lines, so this parser
/// reads exactly those: every line that *starts with* `a=fingerprint:` at session or media level
/// (CRLF or LF). Only `sha-256` lines count; they must all be well-formed (`XX:XX:...`, 32 bytes)
/// and carry **one distinct value**. Anything else (none, several different values, a malformed
/// sha-256 line, a fingerprint hidden inside `s=` or another attribute) yields `None`, and the
/// caller fails the attempt with `fingerprint_invalid`.
pub fn extract_sha256_fingerprint(sdp: &str) -> Option<String> {
    let mut found: Option<String> = None;
    for raw in sdp.split('\n') {
        let line = raw.trim_end_matches('\r');
        let Some(rest) = line.strip_prefix("a=fingerprint:") else {
            continue;
        };
        let (algorithm, value) = rest.split_once(' ')?;
        if !algorithm.eq_ignore_ascii_case("sha-256") {
            if algorithm.is_empty() {
                return None;
            }
            continue;
        }
        let fp = normalize_sha256_fingerprint(value.trim(), true)?;
        match &found {
            Some(existing) if *existing != fp => return None,
            _ => found = Some(fp),
        }
    }
    found
}

/// Normalizes a SHA-256 certificate fingerprint to lowercase hex without separators.
/// Accepts the WebRTC colon form (`AB:CD:...`, as in `RTCCertificateStats.fingerprint`) and plain
/// 64-character hex; anything that is not exactly 32 bytes yields `None`.
pub fn normalize_dtls_fingerprint(value: &str) -> Option<String> {
    let v = value.trim();
    if v.contains(':') {
        normalize_sha256_fingerprint(v, true)
    } else {
        normalize_sha256_fingerprint(v, false)
    }
}

fn normalize_sha256_fingerprint(value: &str, colons: bool) -> Option<String> {
    let hex: String = if colons {
        let groups: Vec<&str> = value.split(':').collect();
        if groups.len() != 32 || groups.iter().any(|g| g.len() != 2) {
            return None;
        }
        groups.concat()
    } else {
        value.to_owned()
    };
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| hex.to_ascii_lowercase())
}

/// The v2 TypeScript extractor: first `a=fingerprint:sha-256` substring anywhere (also inside
/// `s=`), hex up to the first other character. **Insecure**; kept only so the golden `pair_mac`
/// v2 vectors keep cross-checking against the old client and so the simulator can model the
/// attacker's view. Never used on a live connection.
pub fn extract_sha256_fingerprint_v2_legacy(sdp: &str) -> Option<String> {
    const PREFIX: &str = "a=fingerprint:sha-256";
    let lower = sdp.to_ascii_lowercase();
    let start = lower.find(PREFIX)? + PREFIX.len();
    let rest = &sdp[start..];
    if !rest.starts_with(|c: char| c.is_whitespace()) {
        return None;
    }
    let value: String = rest
        .chars()
        .skip_while(|c| c.is_whitespace())
        .take_while(|c| c.is_ascii_hexdigit() || *c == ':' || c.is_whitespace())
        .filter(|c| c.is_ascii_hexdigit())
        .collect::<String>()
        .to_ascii_lowercase();
    (!value.is_empty()).then_some(value)
}

/// Accepts base64url with or without padding, like `decodeBase64UrlToBytes`.
pub fn decode_psk(b64url: &str) -> Option<Vec<u8>> {
    let trimmed = b64url.trim().trim_end_matches('=');
    URL_SAFE_NO_PAD.decode(trimmed).ok()
}

pub fn encode_psk(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Room id derived from the PSK: base64url (no padding) of HKDF-SHA256(ikm = psk, empty salt,
/// info `gs-room/3`) truncated to 128 bit, i.e. 22 characters. Nobody without the PSK can guess it.
pub fn derive_room_id(psk: &[u8]) -> String {
    let hk = Hkdf::<Sha256>::new(None, psk);
    let mut okm = [0u8; 16];
    // Infallible: HKDF-SHA256 only rejects outputs longer than 255 * 32 bytes.
    #[allow(clippy::expect_used)]
    hk.expand(ROOM_INFO, &mut okm)
        .expect("16 bytes is a valid HKDF-SHA256 output length");
    URL_SAFE_NO_PAD.encode(okm)
}

/// Direction-bound proof (v3): HMAC-SHA256(psk, "gs-pair-proof/3|role|room|fp_sender|fp_receiver")
/// as lowercase hex. `fp_*` are normalized fingerprints (see [`extract_sha256_fingerprint`]).
/// Each side sends its own proof and verifies the peer's with roles and fingerprints swapped, so
/// a reflected proof never verifies.
pub fn pair_proof_mac_v3(
    psk: &[u8],
    sender_role: Role,
    room_id: &str,
    fp_sender: &str,
    fp_receiver: &str,
) -> Option<String> {
    let canonical = format!(
        "{PROOF_PREFIX}{}|{room_id}|{fp_sender}|{fp_receiver}",
        sender_role.as_str()
    );
    let mut mac = Hmac::<Sha256>::new_from_slice(psk).ok()?;
    mac.update(canonical.as_bytes());
    Some(hex(&mac.finalize().into_bytes()))
}

/// v2 symmetric proof: HMAC-SHA256(psk, "room|min(fpA,fpB)|max(fpA,fpB)|2"). **Only** kept so the
/// golden `pair_mac` vectors still cross-check against the TypeScript v2 client; never used on a
/// live connection because a peer that knows the room id can reflect it.
pub fn pair_proof_mac_v2_legacy(
    psk: &[u8],
    room_id: &str,
    local_sdp: &str,
    remote_sdp: &str,
) -> Option<String> {
    let a = extract_sha256_fingerprint_v2_legacy(local_sdp)?;
    let b = extract_sha256_fingerprint_v2_legacy(remote_sdp)?;
    let (x, y) = if a < b { (a, b) } else { (b, a) };
    let canonical = format!("{room_id}|{x}|{y}|{LEGACY_PROTOCOL_VERSION}");
    let mut mac = Hmac::<Sha256>::new_from_slice(psk).ok()?;
    mac.update(canonical.as_bytes());
    Some(hex(&mac.finalize().into_bytes()))
}

/// Constant-time comparison so a remote peer cannot probe the expected MAC byte by byte.
pub fn macs_equal(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// Key a relay uses to admit joiners (ADR-021 section 1): HKDF-SHA256(psk, empty salt,
/// info `guardedsleep relay admission v1`), 32 bytes. It cannot be turned back into the PSK, so a
/// relay that holds it never learns the pairing secret. `Debug` never prints the key.
#[derive(Clone, PartialEq, Eq)]
pub struct AdmissionKey([u8; 32]);

impl std::fmt::Debug for AdmissionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AdmissionKey(..)")
    }
}

impl AdmissionKey {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// base64url without padding (43 characters), the form handed to a relay over loopback.
    pub fn to_b64url(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }

    pub fn from_b64url(s: &str) -> Option<Self> {
        let bytes = URL_SAFE_NO_PAD.decode(s.trim()).ok()?;
        Some(Self(bytes.try_into().ok()?))
    }
}

pub fn derive_admission_key(psk: &[u8]) -> AdmissionKey {
    let hk = Hkdf::<Sha256>::new(None, psk);
    let mut okm = [0u8; 32];
    // Infallible: HKDF-SHA256 only rejects outputs longer than 255 * 32 bytes.
    #[allow(clippy::expect_used)]
    hk.expand(ADMISSION_INFO, &mut okm)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    AdmissionKey(okm)
}

/// Joiner's answer to a relay `challenge`:
/// HMAC-SHA256(key, "gs-relay-admit/1|nonce|role|room") as lowercase hex. The nonce is fresh per
/// connection and the role (direction) and room are bound in, so a recorded answer is useless on
/// any other connection, role or room.
pub fn admission_mac(key: &AdmissionKey, nonce: &str, role: Role, room_id: &str) -> Option<String> {
    let canonical = format!("{ADMISSION_PREFIX}{nonce}|{}|{room_id}", role.as_str());
    let mut mac = Hmac::<Sha256>::new_from_slice(&key.0).ok()?;
    mac.update(canonical.as_bytes());
    Some(hex(&mac.finalize().into_bytes()))
}

pub fn verify_admission(
    key: &AdmissionKey,
    nonce: &str,
    role: Role,
    room_id: &str,
    presented_mac: &str,
) -> bool {
    admission_mac(key, nonce, role, room_id).is_some_and(|m| macs_equal(&m, presented_mac))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Short values are fine here: the v2 legacy extractor does not validate the length.
    const SDP_A: &str = "v=0\r\na=fingerprint:sha-256 AB:CD:EF:01\r\n";
    const SDP_B: &str = "v=0\r\na=fingerprint:sha-256 12:34:56:78\r\n";

    fn fp_hex(seed: u8) -> String {
        (0..32)
            .map(|i| format!("{:02x}", seed.wrapping_add(i)))
            .collect()
    }

    fn fp_colon(seed: u8) -> String {
        fp_hex(seed)
            .as_bytes()
            .chunks(2)
            .map(|c| std::str::from_utf8(c).unwrap().to_ascii_uppercase())
            .collect::<Vec<_>>()
            .join(":")
    }

    fn sdp_with(lines: &[String]) -> String {
        format!(
            "v=0\r\n{}\r\nm=audio 9 X 1\r\na=sendonly\r\n",
            lines.join("\r\n")
        )
    }

    #[test]
    fn fingerprint_is_normalized() {
        let sdp = sdp_with(&[format!("a=fingerprint:sha-256 {}", fp_colon(1))]);
        assert_eq!(extract_sha256_fingerprint(&sdp), Some(fp_hex(1)));
        assert_eq!(extract_sha256_fingerprint("v=0\r\n"), None);
        // LF only, upper-case algorithm, session and media level carrying the same value.
        let lf = format!(
            "v=0\na=fingerprint:SHA-256 {c}\nm=audio 9 X 1\na=fingerprint:sha-256 {c}\n",
            c = fp_colon(1)
        );
        assert_eq!(extract_sha256_fingerprint(&lf), Some(fp_hex(1)));
    }

    #[test]
    fn fingerprint_inside_another_line_is_invalid() {
        // The v2 extractor read the value inside `s=`, while DTLS authenticates the real line.
        let sdp = format!(
            "v=0\r\ns=a=fingerprint:sha-256 {}\r\nm=audio 9 X 1\r\na=fingerprint:sha-256 {}\r\n",
            fp_colon(1),
            fp_colon(2)
        );
        assert_eq!(extract_sha256_fingerprint(&sdp), Some(fp_hex(2)));
        assert_eq!(
            extract_sha256_fingerprint_v2_legacy(&sdp),
            Some(fp_hex(1)),
            "the legacy extractor is the one that is fooled"
        );
        let only_hidden = format!("v=0\r\ns=x a=fingerprint:sha-256 {}\r\n", fp_colon(1));
        assert_eq!(extract_sha256_fingerprint(&only_hidden), None);
        let indented = format!("v=0\r\n a=fingerprint:sha-256 {}\r\n", fp_colon(1));
        assert_eq!(extract_sha256_fingerprint(&indented), None);
    }

    #[test]
    fn two_different_fingerprints_are_invalid_but_repeats_are_fine() {
        let different = sdp_with(&[
            format!("a=fingerprint:sha-256 {}", fp_colon(1)),
            format!("a=fingerprint:sha-256 {}", fp_colon(2)),
        ]);
        assert_eq!(extract_sha256_fingerprint(&different), None);
        let same = sdp_with(&[
            format!("a=fingerprint:sha-256 {}", fp_colon(1)),
            format!("a=fingerprint:sha-256 {}", fp_colon(1).to_ascii_lowercase()),
        ]);
        assert_eq!(extract_sha256_fingerprint(&same), Some(fp_hex(1)));
    }

    #[test]
    fn only_sha256_lines_count_and_they_must_be_single() {
        let sha1 = "AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD";
        let mixed = sdp_with(&[
            format!("a=fingerprint:sha-1 {sha1}"),
            format!("a=fingerprint:sha-256 {}", fp_colon(3)),
        ]);
        assert_eq!(extract_sha256_fingerprint(&mixed), Some(fp_hex(3)));
        let only_sha1 = sdp_with(&[format!("a=fingerprint:sha-1 {sha1}")]);
        assert_eq!(extract_sha256_fingerprint(&only_sha1), None);
        let sha1_and_two = sdp_with(&[
            format!("a=fingerprint:sha-1 {sha1}"),
            format!("a=fingerprint:sha-256 {}", fp_colon(3)),
            format!("a=fingerprint:sha-256 {}", fp_colon(4)),
        ]);
        assert_eq!(extract_sha256_fingerprint(&sha1_and_two), None);
    }

    #[test]
    fn malformed_fingerprints_are_invalid() {
        for bad in [
            "a=fingerprint:sha-256 AA:BB".to_owned(),
            "a=fingerprint:sha-256 ".to_owned(),
            "a=fingerprint:sha-256".to_owned(),
            "a=fingerprint:".to_owned(),
            format!("a=fingerprint:sha-256 {}:00", fp_colon(1)),
            format!("a=fingerprint:sha-256 {}", fp_hex(1)),
            format!("a=fingerprint:sha-256 {}", fp_colon(1).replace("0", "g")),
            format!("a=fingerprint: sha-256 {}", fp_colon(1)),
        ] {
            assert_eq!(
                extract_sha256_fingerprint(&sdp_with(std::slice::from_ref(&bad))),
                None,
                "{bad}"
            );
        }
    }

    #[test]
    fn dtls_fingerprint_reports_are_normalized() {
        assert_eq!(normalize_dtls_fingerprint(&fp_colon(5)), Some(fp_hex(5)));
        assert_eq!(normalize_dtls_fingerprint(&fp_hex(5)), Some(fp_hex(5)));
        assert_eq!(
            normalize_dtls_fingerprint(&fp_hex(5).to_ascii_uppercase()),
            Some(fp_hex(5))
        );
        assert_eq!(normalize_dtls_fingerprint("AA:BB"), None);
        assert_eq!(normalize_dtls_fingerprint(""), None);
        assert_eq!(normalize_dtls_fingerprint(&"zz".repeat(32)), None);
    }

    #[test]
    fn room_id_matches_independent_hkdf_computation() {
        // Expected value computed with Python hmac/hashlib (HKDF extract with a zero salt, one
        // expand block), independent of the hkdf crate.
        let psk: Vec<u8> = (0u8..32).collect();
        assert_eq!(derive_room_id(&psk), "FrcBBWi_uGBOkaphqPhl_w");
        assert_eq!(derive_room_id(&psk).len(), 22);
    }

    #[test]
    fn room_id_depends_on_psk_only() {
        assert_ne!(derive_room_id(&[1u8; 32]), derive_room_id(&[2u8; 32]));
        assert_eq!(derive_room_id(&[1u8; 32]), derive_room_id(&[1u8; 32]));
    }

    #[test]
    fn v2_mac_is_symmetric_between_peers() {
        let psk = [7u8; 32];
        let a = pair_proof_mac_v2_legacy(&psk, "room", SDP_A, SDP_B).unwrap();
        let b = pair_proof_mac_v2_legacy(&psk, "room", SDP_B, SDP_A).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn v3_mac_is_direction_bound() {
        let psk = [7u8; 32];
        let baby = pair_proof_mac_v3(&psk, Role::Baby, "room", "aa", "bb").unwrap();
        let parent = pair_proof_mac_v3(&psk, Role::Parent, "room", "bb", "aa").unwrap();
        assert_ne!(baby, parent);
        // Reflection: the parent's expectation of the baby proof is mac(baby, fp_b=aa, fp_p=bb).
        assert!(!macs_equal(&baby, &parent));
        assert_eq!(baby.len(), 64);
        let same_role_swapped = pair_proof_mac_v3(&psk, Role::Baby, "room", "bb", "aa").unwrap();
        assert_ne!(baby, same_role_swapped);
    }

    #[test]
    fn v3_mac_binds_psk_room_and_fingerprints() {
        let base = pair_proof_mac_v3(&[1u8; 32], Role::Baby, "r", "aa", "bb").unwrap();
        for other in [
            pair_proof_mac_v3(&[2u8; 32], Role::Baby, "r", "aa", "bb").unwrap(),
            pair_proof_mac_v3(&[1u8; 32], Role::Baby, "s", "aa", "bb").unwrap(),
            pair_proof_mac_v3(&[1u8; 32], Role::Baby, "r", "ac", "bb").unwrap(),
            pair_proof_mac_v3(&[1u8; 32], Role::Baby, "r", "aa", "bc").unwrap(),
        ] {
            assert!(!macs_equal(&base, &other));
        }
    }

    #[test]
    fn v3_mac_known_answer() {
        // HMAC-SHA256(key = 32 x 0x07, "gs-pair-proof/3|baby|room|aa|bb") computed with Python.
        assert_eq!(
            pair_proof_mac_v3(&[7u8; 32], Role::Baby, "room", "aa", "bb").unwrap(),
            "7dc37decc3d8364d712278ceae6b0d636dda429ce8e239e560d1d8aa4e36ae72"
        );
    }

    #[test]
    fn wrong_psk_changes_v2_mac() {
        let a = pair_proof_mac_v2_legacy(&[1u8; 32], "room", SDP_A, SDP_B).unwrap();
        let b = pair_proof_mac_v2_legacy(&[2u8; 32], "room", SDP_A, SDP_B).unwrap();
        assert!(!macs_equal(&a, &b));
    }

    #[test]
    fn macs_equal_requires_equal_length_and_content() {
        assert!(macs_equal("abc", "abc"));
        assert!(!macs_equal("abc", "and"));
        assert!(!macs_equal("abc", "abcd"));
    }

    #[test]
    fn psk_round_trips_with_and_without_padding() {
        let raw = [0xfbu8, 0xff, 0x01];
        let enc = encode_psk(&raw);
        assert_eq!(decode_psk(&enc).unwrap(), raw);
        assert_eq!(decode_psk(&format!("{enc}=")).unwrap(), raw);
        assert!(decode_psk("***").is_none());
    }

    proptest! {
        #[test]
        fn fingerprint_parser_never_panics(s in ".{0,400}") {
            if let Some(fp) = extract_sha256_fingerprint(&s) {
                prop_assert_eq!(fp.len(), 64);
                prop_assert!(fp.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
            }
            let _ = extract_sha256_fingerprint_v2_legacy(&s);
            let _ = normalize_dtls_fingerprint(&s);
        }

        #[test]
        fn v2_mac_symmetric_for_any_psk(psk in proptest::collection::vec(any::<u8>(), 1..64), room in "[a-z0-9-]{1,32}") {
            let a = pair_proof_mac_v2_legacy(&psk, &room, SDP_A, SDP_B);
            let b = pair_proof_mac_v2_legacy(&psk, &room, SDP_B, SDP_A);
            prop_assert_eq!(a, b);
        }

        #[test]
        fn v3_mac_never_equal_across_roles(psk in proptest::collection::vec(any::<u8>(), 1..64), room in "[a-z0-9-]{1,32}", fa in "[0-9a-f]{8}", fb in "[0-9a-f]{8}") {
            let baby = pair_proof_mac_v3(&psk, Role::Baby, &room, &fa, &fb).unwrap();
            let parent = pair_proof_mac_v3(&psk, Role::Parent, &room, &fa, &fb).unwrap();
            prop_assert_ne!(baby, parent);
        }
    }

    #[test]
    fn admission_key_is_stable_and_not_the_psk() {
        let psk = [7u8; 32];
        let key = derive_admission_key(&psk);
        assert_eq!(key, derive_admission_key(&psk));
        assert_ne!(key.to_b64url(), encode_psk(&psk));
        assert_ne!(key, derive_admission_key(&[8u8; 32]));
        assert_eq!(
            AdmissionKey::from_b64url(&key.to_b64url()),
            Some(key.clone())
        );
        assert_eq!(AdmissionKey::from_b64url("short"), None);
        assert_eq!(format!("{key:?}"), "AdmissionKey(..)");
    }

    #[test]
    fn admission_mac_binds_nonce_role_and_room() {
        let key = derive_admission_key(&[1u8; 32]);
        let n1 = "AAAAAAAAAAAAAAAAAAAAAA";
        let n2 = "BBBBBBBBBBBBBBBBBBBBBB";
        let mac = admission_mac(&key, n1, Role::Parent, "room").unwrap();
        assert_eq!(mac.len(), 64);
        assert!(verify_admission(&key, n1, Role::Parent, "room", &mac));
        assert!(
            !verify_admission(&key, n2, Role::Parent, "room", &mac),
            "replayed nonce"
        );
        assert!(
            !verify_admission(&key, n1, Role::Baby, "room", &mac),
            "other direction"
        );
        assert!(
            !verify_admission(&key, n1, Role::Parent, "other", &mac),
            "other room"
        );
        let other = derive_admission_key(&[2u8; 32]);
        assert!(
            !verify_admission(&other, n1, Role::Parent, "room", &mac),
            "other key"
        );
    }
}
