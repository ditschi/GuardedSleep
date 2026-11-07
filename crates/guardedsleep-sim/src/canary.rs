//! The log canary (AGENTS.md section 7.6, `alpha-connection-model` task 7.1, `add-quality-gates`
//! task 1.3).
//!
//! A run pairs with a known PSK. This module builds every secret value such a run could leak (the
//! PSK, the keys derived from it, the seed, the parent secret with its four subkeys and the
//! `parent_pin` message that carries it, the room id, an issued TURN password) and a scanner that
//! looks for each one in a log in every encoding a careless log line could produce. Plaintext SDP
//! has no known value, so it is found by its markers.
//!
//! Failure messages name the canary and the encoding plus the place, never the value.
//!
//! Encodings searched for every binary value: the raw bytes, lowercase and uppercase hex, standard
//! and URL-safe base64 (padded or not, at any of the three alignments, so a value inside a longer
//! base64 blob is found too) and the percent-escaped form of standard base64. A textual value (the
//! PSK as the config and the invite carry it) is searched as its UTF-8 bytes in the same encodings.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

use base64::Engine;
use base64::engine::general_purpose::{STANDARD_NO_PAD, URL_SAFE_NO_PAD};
use guardedsleep_core::envelope::{Purpose, derive_key};
use guardedsleep_core::invite::{InviteMailbox, build_invite};
use guardedsleep_core::mailbox::MailboxKind;
use guardedsleep_core::pairing::{PinPurpose, derive_admission_key, derive_room_id, pin_subkey};

/// How a needle must sit in the text to count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Boundary {
    /// Any occurrence.
    Anywhere,
    /// Not glued to a letter or digit on the left, and followed by a line end (a real or an
    /// escaped one) or the end of the text: `v=0` must not match `rev=0`.
    SdpLine,
}

#[derive(Debug, Clone)]
struct Needle {
    label: String,
    bytes: Vec<u8>,
    boundary: Boundary,
}

#[derive(Debug, Clone)]
pub struct Canary {
    pub name: String,
    /// The value legitimately travels on the wire (room id in a `hello`, SDP in an offer), so it is
    /// only forbidden in log text, not in the wire payloads the simulation hands to the platform.
    pub wire_ok: bool,
    /// The raw bytes of a binary secret, kept for the alignment self-check.
    secret: Option<Vec<u8>>,
    needles: Vec<Needle>,
}

/// One finding. `Display` carries names and places only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub canary: String,
    pub encoding: String,
    pub source: String,
    pub line: usize,
}

impl std::fmt::Display for Hit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "canary `{}` ({}) in {}:{}",
            self.canary, self.encoding, self.source, self.line
        )
    }
}

/// Needles by their first three bytes, so a scan walks the text once instead of once per needle.
type Index = HashMap<[u8; 3], Vec<(usize, usize)>>;

#[derive(Debug, Clone, Default)]
pub struct CanarySet {
    canaries: Vec<Canary>,
    /// Built on the first scan after a canary was added.
    index: RefCell<Option<Index>>,
}

fn hex(bytes: &[u8], upper: bool) -> String {
    bytes
        .iter()
        .map(|b| {
            if upper {
                format!("{b:02X}")
            } else {
                format!("{b:02x}")
            }
        })
        .collect()
}

/// The unpadded base64 texts of `bytes` as they can appear inside a longer base64 text: the value
/// sits at offset 0, 1 or 2 within a 3-byte group, so the characters that depend on the
/// neighbouring bytes are cut off at both ends. (Padding is a suffix of these texts, so searching
/// the unpadded text finds padded forms too.)
fn base64_inner_variants(bytes: &[u8], url_safe: bool) -> Vec<String> {
    const SKIP: [usize; 3] = [0, 2, 3];
    (0..3)
        .filter_map(|offset| {
            let mut buf = vec![0u8; offset];
            buf.extend_from_slice(bytes);
            let text = if url_safe {
                URL_SAFE_NO_PAD.encode(&buf)
            } else {
                STANDARD_NO_PAD.encode(&buf)
            };
            let end = if buf.len() % 3 == 0 {
                text.len()
            } else {
                text.len().saturating_sub(1)
            };
            let start = SKIP[offset];
            (start < end).then(|| text[start..end].to_owned())
        })
        .collect()
}

fn needles_for(bytes: &[u8]) -> Vec<Needle> {
    let mut by_bytes: BTreeMap<Vec<u8>, Vec<String>> = BTreeMap::new();
    let mut add = |label: &str, text: Vec<u8>| {
        let labels = by_bytes.entry(text).or_default();
        if !labels.iter().any(|l| l == label) {
            labels.push(label.to_owned());
        }
    };
    add("raw", bytes.to_vec());
    add("hex", hex(bytes, false).into_bytes());
    add("HEX", hex(bytes, true).into_bytes());
    for v in base64_inner_variants(bytes, false) {
        add("base64", v.clone().into_bytes());
        add(
            "base64+percent",
            v.replace('+', "%2B").replace('/', "%2F").into_bytes(),
        );
        add(
            "base64+percent",
            v.replace('+', "%2b").replace('/', "%2f").into_bytes(),
        );
    }
    for v in base64_inner_variants(bytes, true) {
        add("base64url", v.into_bytes());
    }
    by_bytes
        .into_iter()
        .map(|(bytes, labels)| Needle {
            label: labels.join("/"),
            bytes,
            boundary: Boundary::Anywhere,
        })
        .collect()
}

fn boundary_ok(hay: &[u8], at: usize, len: usize, boundary: Boundary) -> bool {
    match boundary {
        Boundary::Anywhere => true,
        Boundary::SdpLine => {
            let left_ok = at == 0 || !hay[at - 1].is_ascii_alphanumeric();
            let rest = &hay[at + len..];
            let right_ok = rest.is_empty()
                || rest[0] == b'\r'
                || rest[0] == b'\n'
                || rest.starts_with(b"\\r")
                || rest.starts_with(b"\\n");
            left_ok && right_ok
        }
    }
}

impl CanarySet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn names(&self) -> Vec<&str> {
        self.canaries.iter().map(|c| c.name.as_str()).collect()
    }

    pub fn has(&self, name: &str) -> bool {
        self.canaries.iter().any(|c| c.name == name)
    }

    fn push(&mut self, name: &str, wire_ok: bool, secret: Option<Vec<u8>>, needles: Vec<Needle>) {
        if self.has(name) {
            return;
        }
        self.canaries.push(Canary {
            name: name.to_owned(),
            wire_ok,
            secret,
            needles,
        });
        *self.index.borrow_mut() = None;
    }

    /// A binary secret, searched in every encoding.
    pub fn add_secret(&mut self, name: &str, bytes: &[u8]) {
        self.push(name, false, Some(bytes.to_vec()), needles_for(bytes));
    }

    /// A textual secret (searched as its UTF-8 bytes, in every encoding of those bytes).
    pub fn add_text(&mut self, name: &str, text: &str) {
        self.add_secret(name, text.as_bytes());
    }

    /// Like [`add_secret`](Self::add_secret) for a value that is allowed on the wire.
    pub fn add_wire_value(&mut self, name: &str, bytes: &[u8]) {
        self.push(name, true, Some(bytes.to_vec()), needles_for(bytes));
    }

    /// The PSK, everything derived from it and the invite that carries it.
    ///
    /// `suffix` tells two pairings apart in the names (`""` for the one under test).
    pub fn add_pairing(&mut self, suffix: &str, psk: &[u8]) {
        self.add_secret(&format!("psk{suffix}"), psk);
        self.add_text(&format!("psk_b64url{suffix}"), &URL_SAFE_NO_PAD.encode(psk));
        self.add_text(
            &format!("psk_b64{suffix}"),
            &base64::engine::general_purpose::STANDARD.encode(psk),
        );
        if let Some(seal) = derive_key(psk, Purpose::Seal) {
            self.add_secret(&format!("envelope_key{suffix}"), seal.as_bytes());
        }
        if let Ok(adm) = URL_SAFE_NO_PAD.decode(derive_admission_key(psk).to_b64url()) {
            self.add_secret(&format!("admission_key{suffix}"), &adm);
        }
        // The room id is the base64url text of 16 derived bytes: the raw bytes cover the wire form
        // (alignment 0 of base64url) and every other encoding.
        if let Ok(room) = URL_SAFE_NO_PAD.decode(derive_room_id(psk)) {
            self.add_wire_value(&format!("room_id{suffix}"), &room);
        }
        // What the QR code and the invite link carry. The relay label is a placeholder: only the
        // key inside decides the verdict.
        let relay = InviteMailbox {
            kind: MailboxKind::Relay,
            url: "wss://relay-0.invalid".to_owned(),
        };
        if let Ok(invite) = build_invite(psk, &[relay], 1, true) {
            self.add_text(&format!("invite{suffix}"), &invite);
        }
    }

    /// The seed of one `Link` run (it predicts every nonce and secret the core draws).
    pub fn add_seed(&mut self, name: &str, seed: &[u8]) {
        self.add_secret(name, seed);
    }

    /// The parent secret and its four HKDF subkeys. Drawn at run time by the parent's core, so the
    /// simulation calls this when the `parent_pinned` event comes out of the first pairing.
    pub fn add_parent_secret(&mut self, secret: &[u8]) {
        self.add_secret("parent_secret", secret);
        // The hex text is what `ParentPinned` serializes to; covered by the hex encoding above.
        for (name, purpose) in [
            ("parent_subkey_parent_tag", PinPurpose::ParentTag),
            ("parent_subkey_baby_tag", PinPurpose::BabyTag),
            ("parent_subkey_parent_proof", PinPurpose::ParentProof),
            ("parent_subkey_baby_proof", PinPurpose::BabyProof),
        ] {
            if let Some(key) = pin_subkey(secret, purpose) {
                self.add_secret(name, &key);
            }
        }
    }

    /// The `parent_pin` DataChannel message text (it carries the secret and the device name).
    pub fn add_pin_message(&mut self, text: &str) {
        self.add_text("parent_pin_message", text);
    }

    /// An issued TURN credential. Every one is its own canary (the first keeps the plain name), so
    /// a renewed credential is covered as well as the first.
    pub fn add_turn_password(&mut self, password: &str) {
        let n = self
            .canaries
            .iter()
            .filter(|c| c.name.starts_with("turn_password"))
            .count();
        let name = if n == 0 {
            "turn_password".to_owned()
        } else {
            format!("turn_password_{n}")
        };
        self.add_text(&name, password);
    }

    /// Plaintext SDP has no known value: its markers are searched instead. SDP legitimately
    /// travels as an offer or answer, so only log text is checked.
    pub fn add_sdp_markers(&mut self) {
        let needles = [
            (
                "sdp marker a=fingerprint:",
                "a=fingerprint:",
                Boundary::Anywhere,
            ),
            (
                "sdp marker a=ice-ufrag:",
                "a=ice-ufrag:",
                Boundary::Anywhere,
            ),
            ("sdp marker a=ice-pwd:", "a=ice-pwd:", Boundary::Anywhere),
            ("sdp marker m=audio", "m=audio ", Boundary::Anywhere),
            ("sdp marker v=0", "v=0", Boundary::SdpLine),
        ]
        .into_iter()
        .map(|(label, text, boundary)| Needle {
            label: label.to_owned(),
            bytes: text.as_bytes().to_vec(),
            boundary,
        })
        .collect();
        self.push("plaintext_sdp", true, None, needles);
    }

    /// Searches `data` (log text, possibly not UTF-8). `wire` marks text that is a wire payload by
    /// design (a mailbox frame, an SDP in transit): values that may travel on the wire are skipped
    /// there. One hit per canary and encoding.
    pub fn scan(&self, source: &str, data: &[u8], wire: bool) -> Vec<Hit> {
        let mut index = self.index.borrow_mut();
        let index = index.get_or_insert_with(|| {
            let mut m: Index = HashMap::new();
            for (ci, c) in self.canaries.iter().enumerate() {
                for (ni, n) in c.needles.iter().enumerate() {
                    if let Some(key) = n.bytes.first_chunk::<3>() {
                        m.entry(*key).or_default().push((ci, ni));
                    }
                }
            }
            m
        });
        // First match position per (canary, needle).
        let mut found: BTreeMap<(usize, usize), usize> = BTreeMap::new();
        for (at, window) in data.windows(3).enumerate() {
            let Some(candidates) = window.first_chunk::<3>().and_then(|k| index.get(k)) else {
                continue;
            };
            for &(ci, ni) in candidates {
                let Some(canary) = self.canaries.get(ci) else {
                    continue;
                };
                if (wire && canary.wire_ok) || found.contains_key(&(ci, ni)) {
                    continue;
                }
                let Some(needle) = canary.needles.get(ni) else {
                    continue;
                };
                if data[at..].starts_with(&needle.bytes)
                    && boundary_ok(data, at, needle.bytes.len(), needle.boundary)
                {
                    found.insert((ci, ni), at);
                }
            }
        }
        found
            .into_iter()
            .filter_map(|((ci, ni), at)| {
                let canary = self.canaries.get(ci)?;
                let needle = canary.needles.get(ni)?;
                Some(Hit {
                    canary: canary.name.clone(),
                    encoding: needle.label.clone(),
                    source: source.to_owned(),
                    line: 1 + data[..at].iter().filter(|&&b| b == b'\n').count(),
                })
            })
            .collect()
    }

    /// Every value of every canary, planted in every encoding and at every base64 alignment inside
    /// noise, must be found; a clean text must not. `Err` names what the scanner missed.
    pub fn self_check(&self) -> Result<(), String> {
        let clean = b"heartbeat ok t=1234 level=0 peer=connected\nstats packets=50 jitter=3\n";
        if let Some(hit) = self.scan("self-check clean text", clean, false).first() {
            return Err(format!("clean text flagged: {hit}"));
        }
        let mut missed = Vec::new();
        for canary in &self.canaries {
            for needle in &canary.needles {
                let mut planted = b"noise before ".to_vec();
                planted.extend_from_slice(&needle.bytes);
                planted.extend_from_slice(b"\nnoise after\n");
                if !self
                    .scan("self-check", &planted, false)
                    .iter()
                    .any(|h| h.canary == canary.name)
                {
                    missed.push(format!("{} ({})", canary.name, needle.label));
                }
            }
            if let Some(secret) = &canary.secret {
                // Inside a longer base64 or hex text, at every offset and with every neighbour.
                for lead in 0..3usize {
                    for tail in 0..3usize {
                        let mut buf = vec![b'q'; lead];
                        buf.extend_from_slice(secret);
                        buf.extend(std::iter::repeat_n(b'Z', tail));
                        let planted = [
                            ("base64", STANDARD_NO_PAD.encode(&buf)),
                            ("base64url", URL_SAFE_NO_PAD.encode(&buf)),
                            (
                                "base64 padded",
                                base64::engine::general_purpose::STANDARD.encode(&buf),
                            ),
                            ("hex", hex(&buf, false)),
                            ("HEX", hex(&buf, true)),
                        ];
                        for (label, text) in planted {
                            let line = format!("data={text}\n");
                            if !self
                                .scan("self-check", line.as_bytes(), false)
                                .iter()
                                .any(|h| h.canary == canary.name)
                            {
                                missed.push(format!(
                                    "{} ({label}, lead {lead}, tail {tail})",
                                    canary.name
                                ));
                            }
                        }
                    }
                }
            }
        }
        if missed.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "scanner missed planted values: {}",
                missed.join("; ")
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_set() -> CanarySet {
        let mut set = CanarySet::new();
        set.add_pairing("", &[0xA1; 32]);
        set.add_seed(
            "seed",
            b"GS-SYNTH-SEED-v1\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10",
        );
        set.add_parent_secret(&[0x5C; 32]);
        set.add_pin_message(r#"{"type":"parent_pin","secret":"x","name":"Mum's phone"}"#);
        set.add_turn_password("turn-password-for-the-test");
        set.add_sdp_markers();
        set
    }

    #[test]
    fn the_canary_list_has_every_required_value() {
        let set = full_set();
        for name in [
            "psk",
            "psk_b64url",
            "psk_b64",
            "envelope_key",
            "admission_key",
            "room_id",
            "invite",
            "seed",
            "parent_secret",
            "parent_subkey_parent_tag",
            "parent_subkey_baby_tag",
            "parent_subkey_parent_proof",
            "parent_subkey_baby_proof",
            "parent_pin_message",
            "turn_password",
            "plaintext_sdp",
        ] {
            assert!(set.has(name), "canary `{name}` missing");
        }
    }

    #[test]
    fn a_planted_value_is_found_in_every_encoding_and_a_clean_log_passes() {
        full_set().self_check().expect("self check");
    }

    #[test]
    fn a_planted_psk_in_each_encoding_names_the_canary_and_not_the_value() {
        let set = full_set();
        let psk = [0xA1u8; 32];
        let texts = [
            hex(&psk, false),
            hex(&psk, true),
            STANDARD_NO_PAD.encode(psk),
            base64::engine::general_purpose::STANDARD.encode(psk),
            URL_SAFE_NO_PAD.encode(psk),
        ];
        for (n, text) in texts.iter().enumerate() {
            let log = format!("INFO paired with key {text}\nnext line\n");
            let hits = set.scan("planted.log", log.as_bytes(), false);
            assert!(
                hits.iter().any(|h| h.canary.starts_with("psk")),
                "encoding {n} not found"
            );
            for hit in &hits {
                let shown = hit.to_string();
                assert!(
                    !shown.contains(text.as_str()),
                    "the message leaks the value"
                );
                assert_eq!(hit.line, 1);
            }
        }
        let raw = [b"bytes: ".as_slice(), &psk, b"\n"].concat();
        assert!(!set.scan("planted.bin", &raw, false).is_empty());
    }

    #[test]
    fn the_wire_flag_exempts_the_room_id_and_sdp_only() {
        let set = full_set();
        let room = derive_room_id(&[0xA1; 32]);
        let hello = format!(r#"{{"type":"hello","room":"{room}"}}"#);
        assert!(set.scan("wire", hello.as_bytes(), true).is_empty());
        assert!(!set.scan("log", hello.as_bytes(), false).is_empty());
        let sdp = "v=0\r\na=fingerprint:sha-256 AA:BB\r\n";
        assert!(set.scan("wire", sdp.as_bytes(), true).is_empty());
        assert!(!set.scan("log", sdp.as_bytes(), false).is_empty());
        // A secret on the wire is still a leak.
        let psk = URL_SAFE_NO_PAD.encode([0xA1u8; 32]);
        assert!(!set.scan("wire", psk.as_bytes(), true).is_empty());
    }

    #[test]
    fn sdp_marker_v0_needs_a_line_end_and_no_glued_letter() {
        let set = full_set();
        assert!(set.scan("l", b"rev=0 done", false).is_empty());
        assert!(set.scan("l", b"v=0 is the version", false).is_empty());
        assert!(!set.scan("l", b"offer: v=0\\r\\no=- 1", false).is_empty());
        assert!(!set.scan("l", b"v=0\r\no=- 1", false).is_empty());
    }

    #[test]
    fn hits_carry_the_line_number() {
        let set = full_set();
        let log = format!("a\nb\nkey={}\n", hex(&[0x5C; 32], false));
        let hits = set.scan("x.log", log.as_bytes(), false);
        assert!(hits.iter().all(|h| h.line == 3 && h.source == "x.log"));
        assert!(hits.iter().any(|h| h.canary == "parent_secret"));
    }

    #[test]
    fn an_empty_set_finds_nothing() {
        assert!(CanarySet::new().scan("x", b"anything", false).is_empty());
    }
}
