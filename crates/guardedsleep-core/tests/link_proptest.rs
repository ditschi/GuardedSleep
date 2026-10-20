//! Random input sequences: the `Link` never panics and never has two live attempts (ADR-017 §3).

mod common;

use std::collections::HashSet;

use common::{World, BABY, PARENT, PSK_A};
use guardedsleep_core::link::{Link, LinkAction, LinkConfig, LinkInput, LinkState};
use guardedsleep_core::pairing::{
    derive_room_id, encode_psk, extract_sha256_fingerprint, normalize_dtls_fingerprint,
};
use guardedsleep_core::protocol::{DeviceStatus, IceCandidate, Role};
use proptest::prelude::*;
use serde_json::json;

const PSK: [u8; 32] = [5; 32];
const AIDS: [&str; 3] = [
    "00000000000000000000000000000001",
    "00000000000000000000000000000002",
    "ffffffffffffffffffffffffffffffff",
];

fn room() -> String {
    derive_room_id(&PSK)
}

fn server_text() -> impl Strategy<Value = String> {
    let id = prop::sample::select(vec!["a", "b", "c", "x"]);
    let aid = prop::sample::select(AIDS.to_vec());
    let sdp_type = prop::sample::select(vec!["offer", "answer"]);
    prop_oneof![
        Just(json!({"type":"welcome","clientId":"a","roomId":room()}).to_string()),
        Just(json!({"type":"welcome","clientId":"a","roomId":"other"}).to_string()),
        Just(json!({"type":"members","peers":[{"clientId":"a","role":"baby"},{"clientId":"b","role":"parent"}]}).to_string()),
        Just(json!({"type":"members","peers":[{"clientId":"a","role":"baby"}]}).to_string()),
        Just(json!({"type":"members","peers":[{"clientId":"b","role":"parent"},{"clientId":"a","role":"baby"},{"clientId":"c","role":"parent"}]}).to_string()),
        Just(json!({"type":"pong"}).to_string()),
        prop::sample::select(vec!["BABY_TAKEN", "PARENT_CAP", "NOT_JOINED", "UNSUPPORTED_PROTOCOL", "RATE_LIMITED"])
            .prop_map(|c| json!({"type":"error","code":c,"message":"x"}).to_string()),
        (id.clone(), aid.clone(), sdp_type).prop_map(|(from, aid, t)| json!({
            "type":"signal","from":from,
            "payload":{"type":"sdp","aid":aid,"sdp":{"type":t,"sdp":"v=0\r\nm=audio 9 X 1\r\na=sendonly\r\na=fingerprint:sha-256 AA:BB:CC:DD:AA:BB:CC:DD:AA:BB:CC:DD:AA:BB:CC:DD:AA:BB:CC:DD:AA:BB:CC:DD:AA:BB:CC:DD:AA:BB:CC:DD\r\n"}}
        }).to_string()),
        (id.clone(), aid.clone()).prop_map(|(from, aid)| json!({
            "type":"signal","from":from,"payload":{"type":"ice","aid":aid,"candidate":{"candidate":"c","sdpMid":"0","sdpMLineIndex":0}}
        }).to_string()),
        (id.clone(), aid).prop_map(|(from, aid)| json!({
            "type":"signal","from":from,"payload":{"type":"request_offer","aid":aid}
        }).to_string()),
        id.prop_map(|from| json!({"type":"signal","from":from,"payload":{"type":"stop_notice"}}).to_string()),
        ".{0,40}",
    ]
}

fn data_text() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(json!({"type":"heartbeat","v":3,"t":1,"seq":1}).to_string()),
        Just(json!({"type":"pair_proof","v":3,"mac":"0".repeat(64)}).to_string()),
        Just(json!({"type":"stop_notice","v":3}).to_string()),
        Just(json!({"type":"quiet_request","v":3}).to_string()),
        Just(json!({"type":"listen","v":3,"enabled":true}).to_string()),
        Just(json!({"type":"probe_test","v":3,"t":1}).to_string()),
        Just(json!({"type":"device_status","v":3,"battery_pct":3,"charging":false}).to_string()),
        Just(json!({"type":"missed_events","v":3,"count":1,"max_rms":0.5,"complete":true,"down_ms":9}).to_string()),
        Just(json!({"type":"noise_alert","v":3,"rms":0.5,"t":1}).to_string()),
        ".{0,40}",
    ]
}

fn input() -> impl Strategy<Value = LinkInput> {
    let attempt = 0u32..5;
    prop_oneof![
        any::<u64>().prop_map(|entropy| LinkInput::Start { entropy }),
        Just(LinkInput::Stop),
        Just(LinkInput::AcknowledgeAlarm),
        Just(LinkInput::NetworkAvailable),
        Just(LinkInput::SignalingOpened),
        Just(LinkInput::SignalingClosed),
        server_text().prop_map(|text| LinkInput::SignalingMessage { text }),
        (
            attempt.clone(),
            prop::sample::select(vec!["offer", "answer"])
        )
            .prop_map(|(attempt, t)| {
                LinkInput::AttemptLocalDescription {
                    attempt,
                    sdp_type: t.into(),
                    sdp: "v=0\r\nm=audio 9 X 1\r\na=sendonly\r\na=fingerprint:sha-256 CC:DD:CC:DD:CC:DD:CC:DD:CC:DD:CC:DD:CC:DD:CC:DD:CC:DD:CC:DD:CC:DD:CC:DD:CC:DD:CC:DD:CC:DD:CC:DD\r\n"
                        .into(),
                }
            }),
        attempt
            .clone()
            .prop_map(|attempt| LinkInput::AttemptLocalIce {
                attempt,
                candidate: IceCandidate {
                    candidate: "c".into(),
                    sdp_mid: None,
                    sdp_m_line_index: None,
                },
            }),
        attempt
            .clone()
            .prop_map(|attempt| LinkInput::AttemptDataOpen { attempt }),
        (attempt.clone(), data_text())
            .prop_map(|(attempt, text)| LinkInput::AttemptDataMessage { attempt, text }),
        attempt
            .clone()
            .prop_map(|attempt| LinkInput::AttemptDataClosed { attempt }),
        (
            attempt.clone(),
            prop::sample::select(vec![
                "AA:BB:CC:DD:AA:BB:CC:DD:AA:BB:CC:DD:AA:BB:CC:DD:AA:BB:CC:DD:AA:BB:CC:DD:AA:BB:CC:DD:AA:BB:CC:DD",
                "AA:BB",
                "",
            ])
        )
            .prop_map(|(attempt, fp)| LinkInput::AttemptDtlsFingerprint {
                attempt,
                fingerprint: fp.into()
            }),
        (
            attempt,
            prop::sample::select(vec!["connected", "disconnected", "failed", "closed"])
        )
            .prop_map(|(attempt, s)| LinkInput::AttemptPcState {
                attempt,
                state: s.into()
            }),
        (-1.0f64..2.0).prop_map(|rms| LinkInput::AudioLevel { rms }),
        Just(LinkInput::AudioLevel { rms: f64::NAN }),
        (0u8..=120, any::<bool>()).prop_map(|(b, c)| LinkInput::DeviceStatus {
            status: DeviceStatus {
                battery_pct: Some(b),
                charging: Some(c),
                ..Default::default()
            }
        }),
        any::<bool>().prop_map(|enabled| LinkInput::SetListen { enabled }),
        Just(LinkInput::RequestProbe),
        Just(LinkInput::RequestQuiet),
        prop::option::of(0u64..500_000).prop_map(|until_ms| LinkInput::SetSnooze { until_ms }),
        Just(LinkInput::QuietApplied {
            state: DeviceStatus::default()
        }),
        Just(LinkInput::Tick),
        Just(LinkInput::Tick),
        Just(LinkInput::Tick),
    ]
}

fn run(role: Role, steps: Vec<(u64, LinkInput)>) {
    let mut link = Link::new(LinkConfig::with_defaults(role, encode_psk(&PSK))).unwrap();
    let mut live: HashSet<u32> = HashSet::new();
    let mut now = 0u64;
    for (dt, input) in steps {
        now = now.saturating_add(dt);
        for a in link.handle(now, input) {
            match a {
                LinkAction::CreateAttempt { attempt, .. } => {
                    assert!(
                        live.is_empty(),
                        "create_attempt {attempt} while {live:?} is live"
                    );
                    live.insert(attempt);
                }
                LinkAction::CloseAttempt { attempt } => {
                    assert!(
                        live.remove(&attempt),
                        "close_attempt for non-live {attempt}"
                    );
                }
                LinkAction::CreateOffer { attempt }
                | LinkAction::CreateAnswer { attempt }
                | LinkAction::SetRemoteDescription { attempt, .. }
                | LinkAction::AddIceCandidate { attempt, .. }
                | LinkAction::SendData { attempt, .. } => {
                    assert!(
                        live.contains(&attempt),
                        "action for non-live attempt {attempt}"
                    );
                }
                _ => {}
            }
        }
        assert!(live.len() <= 1);
        assert_eq!(link.live_attempt().is_some(), !live.is_empty());
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 300, .. ProptestConfig::default() })]

    #[test]
    fn baby_never_panics_and_never_has_two_live_attempts(
        steps in prop::collection::vec((0u64..40_000, input()), 0..80)
    ) {
        run(Role::Baby, steps);
    }

    #[test]
    fn parent_never_panics_and_never_has_two_live_attempts(
        steps in prop::collection::vec((0u64..40_000, input()), 0..80)
    ) {
        run(Role::Parent, steps);
    }
}

// ---- a verified link survives hostile signaling --------------------------------------------------

fn hostile_signaling_text() -> impl Strategy<Value = String> {
    let id = prop::sample::select(vec!["c1", "c2", "c3", "x", ""]);
    let aid = prop::sample::select(AIDS.to_vec());
    let code = prop::sample::select(vec![
        "UNSUPPORTED_PROTOCOL",
        "BABY_TAKEN",
        "PARENT_CAP",
        "ROOM_FULL",
        "NOT_JOINED",
        "RATE_LIMITED",
        "BAD_JSON",
    ]);
    let fp = "AA:BB:CC:DD:AA:BB:CC:DD:AA:BB:CC:DD:AA:BB:CC:DD:AA:BB:CC:DD:AA:BB:CC:DD:AA:BB:CC:DD:AA:BB:CC:DD";
    let sdp = format!("v=0\r\nm=audio 9 X 1\r\na=sendonly\r\na=fingerprint:sha-256 {fp}\r\n");
    prop_oneof![
        code.prop_map(|c| json!({"type":"error","code":c,"message":"x"}).to_string()),
        Just(json!({"type":"welcome","clientId":"c9","roomId":room()}).to_string()),
        Just(json!({"type":"peer","peerId":"c2"}).to_string()),
        Just(json!({"type":"peer","peerId":null}).to_string()),
        Just(json!({"type":"members","peers":[]}).to_string()),
        Just(json!({"type":"members","peers":[{"clientId":"c1","role":"baby"}]}).to_string()),
        Just(json!({"type":"members","peers":[{"clientId":"c2","role":"parent"}]}).to_string()),
        Just(json!({"type":"members","peers":[{"clientId":"c1","role":"baby"},{"clientId":"c2","role":"parent"},{"clientId":"c3","role":"parent"}]}).to_string()),
        (id.clone(), aid.clone(), prop::sample::select(vec!["offer", "answer"])).prop_map(move |(from, aid, t)| json!({
            "type":"signal","from":from,
            "payload":{"type":"sdp","aid":aid,"re":aid,"sdp":{"type":t,"sdp":sdp}}
        }).to_string()),
        (id.clone(), aid.clone()).prop_map(|(from, aid)| json!({
            "type":"signal","from":from,"payload":{"type":"ice","aid":aid,"candidate":{"candidate":"c","sdpMid":"0","sdpMLineIndex":0}}
        }).to_string()),
        (id.clone(), aid.clone()).prop_map(|(from, aid)| json!({
            "type":"signal","from":from,"payload":{"type":"request_offer","aid":aid}
        }).to_string()),
        id.clone().prop_map(|from| json!({"type":"signal","from":from,"payload":{"type":"stop_notice"}}).to_string()),
        (id, aid, "[0-9a-f]{64}").prop_map(|(from, aid, mac)| json!({
            "type":"signal","from":from,"payload":{"type":"stop_notice","aid":aid,"mac":mac}
        }).to_string()),
        ".{0,60}",
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 120, .. ProptestConfig::default() })]

    /// After a real verification, nothing a (compromised) relay or stranger can say over
    /// signaling closes the live attempt or moves either side off `connected`.
    #[test]
    fn verified_links_ignore_any_signaling_text(
        to_parent in prop::collection::vec(hostile_signaling_text(), 0..40),
        to_baby in prop::collection::vec(hostile_signaling_text(), 0..40),
    ) {
        let mut w = World::pair(&PSK_A, &PSK_A);
        w.connect_pair();
        let handles = [
            w.nodes[BABY].link.live_attempt().unwrap(),
            w.nodes[PARENT].link.live_attempt().unwrap(),
        ];
        for i in 0..to_parent.len().max(to_baby.len()) {
            for (node, texts) in [(PARENT, &to_parent), (BABY, &to_baby)] {
                if let Some(text) = texts.get(i) {
                    w.input(node, LinkInput::SignalingMessage { text: text.clone() });
                }
            }
            w.advance(100);
            for (k, node) in [BABY, PARENT].into_iter().enumerate() {
                prop_assert_eq!(w.state(node), LinkState::Connected, "node {} at step {}", node, i);
                prop_assert_eq!(w.nodes[node].link.live_attempt(), Some(handles[k]));
            }
        }
    }
}

// ---- SDP fingerprint parser ----------------------------------------------------------------------

/// What a correct reader of the `a=fingerprint` lines must conclude from `sdp`.
fn reference_fingerprint(sdp: &str) -> Option<String> {
    let mut found: Option<String> = None;
    for raw in sdp.split('\n') {
        let line = raw.trim_end_matches('\r');
        let Some(rest) = line.strip_prefix("a=fingerprint:") else {
            continue;
        };
        let (alg, value) = rest.split_once(' ')?;
        if !alg.eq_ignore_ascii_case("sha-256") {
            if alg.is_empty() {
                return None;
            }
            continue;
        }
        let groups: Vec<&str> = value.trim().split(':').collect();
        let valid = groups.len() == 32
            && groups
                .iter()
                .all(|g| g.len() == 2 && g.bytes().all(|b| b.is_ascii_hexdigit()));
        if !valid {
            return None;
        }
        let v = groups.concat().to_ascii_lowercase();
        match &found {
            Some(f) if *f != v => return None,
            _ => found = Some(v),
        }
    }
    found
}

fn fp_colon(n: u8) -> String {
    (0..32u8)
        .map(|i| format!("{:02X}", n.wrapping_mul(7).wrapping_add(i)))
        .collect::<Vec<_>>()
        .join(":")
}

fn sdp_line() -> impl Strategy<Value = String> {
    prop_oneof![
        (0u8..3).prop_map(|n| format!("a=fingerprint:sha-256 {}", fp_colon(n))),
        (0u8..3).prop_map(|n| format!("a=fingerprint:SHA-256 {}", fp_colon(n).to_lowercase())),
        (0u8..3).prop_map(|n| format!("s=a=fingerprint:sha-256 {}", fp_colon(n))),
        (0u8..3).prop_map(|n| format!(" a=fingerprint:sha-256 {}", fp_colon(n))),
        (0u8..3).prop_map(|n| format!("a=fingerprint:sha-1 {}", &fp_colon(n)[..59])),
        (0u8..3).prop_map(|n| format!("a=fingerprint:sha-256 {}", &fp_colon(n)[..50])),
        Just("a=fingerprint:sha-256".to_owned()),
        Just("a=fingerprint:".to_owned()),
        Just("a=sendonly".to_owned()),
        Just("m=audio 9 UDP/TLS/RTP/SAVPF 111".to_owned()),
        "[ -~]{0,60}",
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 600, .. ProptestConfig::default() })]

    #[test]
    fn fingerprint_parser_agrees_with_the_reference_on_structured_sdps(
        lines in prop::collection::vec(sdp_line(), 0..8),
        crlf in any::<bool>(),
    ) {
        let sdp = lines.join(if crlf { "\r\n" } else { "\n" });
        prop_assert_eq!(extract_sha256_fingerprint(&sdp), reference_fingerprint(&sdp));
    }

    #[test]
    fn fingerprint_parser_never_panics_and_only_returns_a_single_valid_value(s in "(?s).{0,300}") {
        if let Some(v) = extract_sha256_fingerprint(&s) {
            prop_assert_eq!(v.len(), 64);
            prop_assert_eq!(reference_fingerprint(&s), Some(v.clone()));
            prop_assert_eq!(normalize_dtls_fingerprint(&v), Some(v));
        }
        let _ = normalize_dtls_fingerprint(&s);
    }
}
