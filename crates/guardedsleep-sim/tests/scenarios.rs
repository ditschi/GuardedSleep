//! Runs every committed scenario with 20 seeds, checks that scenarios marked `expected_fail`
//! still fail, lints the committed traces and checks determinism.

use guardedsleep_sim::lint::lint_trace;
use guardedsleep_sim::{
    Verdict, load_scenarios, run_scenario, run_scenario_with_canary, traces_dir,
};

const SEEDS: u64 = 20;

#[test]
fn all_scenarios_pass_with_every_seed() {
    let scenarios = load_scenarios().expect("scenarios load");
    assert!(scenarios.len() >= 25, "scenario set shrank");
    let mut problems = Vec::new();
    for sc in &scenarios {
        for seed in 1..=sc.max_seeds.map_or(SEEDS, |m| m.min(SEEDS)) {
            // The log canary rides along: a secret in anything a platform could log is a violation.
            let res = run_scenario_with_canary(sc, seed).expect("scenario builds");
            match res.verdict(sc) {
                Verdict::Pass | Verdict::ExpectedFail => {}
                Verdict::UnexpectedPass => problems.push(format!(
                    "{} seed {seed}: passes but is marked expected_fail; remove the marker",
                    sc.id
                )),
                Verdict::Fail => problems.push(format!("{} seed {seed}:\n{}", sc.id, res.report())),
            }
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn a_seed_fully_determines_the_run() {
    let sc = load_scenarios()
        .unwrap()
        .into_iter()
        .find(|s| s.id == "wifi-baby-90s")
        .unwrap();
    let a = run_scenario(&sc, 7, true).unwrap();
    let b = run_scenario(&sc, 7, true).unwrap();
    assert_eq!(a.world.recs.len(), b.world.recs.len());
    for (x, y) in a.world.recs.iter().zip(&b.world.recs) {
        assert_eq!(x.to_json("t", 7, "s"), y.to_json("t", 7, "s"));
    }
}

#[test]
fn committed_traces_pass_the_shareable_lint() {
    let mut n = 0;
    for entry in std::fs::read_dir(traces_dir()).unwrap().flatten() {
        let p = entry.path();
        if p.extension().is_some_and(|x| x == "json") {
            let errs = lint_trace(&std::fs::read_to_string(&p).unwrap());
            assert!(errs.is_empty(), "{}: {errs:?}", p.display());
            n += 1;
        }
    }
    assert!(n >= 8);
}

#[test]
fn recorded_traces_of_every_exported_scenario_are_lint_clean() {
    for sc in load_scenarios().unwrap() {
        if sc.export_traces.is_empty() {
            continue;
        }
        let res = run_scenario(&sc, sc.seed, true).unwrap();
        for rec in &res.world.recs {
            let errs = lint_trace(&rec.to_json("x", sc.seed, &sc.id));
            assert!(errs.is_empty(), "{} {}: {errs:?}", sc.id, rec.actor);
        }
    }
}

fn scenario(id: &str) -> guardedsleep_sim::scenario::Scenario {
    load_scenarios()
        .unwrap()
        .into_iter()
        .find(|s| s.id == id)
        .unwrap()
}

#[test]
fn the_canary_list_of_a_run_has_the_run_time_parent_secret() {
    let res = run_scenario_with_canary(&scenario("happy-path"), 1).unwrap();
    assert!(res.clean(), "{}", res.report());
    assert!(res.world.canary_lines() > 10, "the canary scanned nothing");
    let secret = res
        .world
        .canary_parent_secret()
        .expect("the first pairing draws a parent secret");
    assert_eq!(secret.len(), 32);
    let set = res.world.canary_set().unwrap();
    for name in [
        "psk",
        "envelope_key",
        "admission_key",
        "room_id",
        "invite",
        "parent_secret",
        "parent_subkey_parent_tag",
        "parent_subkey_baby_tag",
        "parent_subkey_parent_proof",
        "parent_subkey_baby_proof",
        "parent_pin_message",
        "plaintext_sdp",
    ] {
        assert!(set.has(name), "canary `{name}` missing from a pairing run");
    }
    assert!(set.names().iter().any(|n| n.starts_with("seed_")));
    set.self_check().expect("scanner self check");
}

/// A planted leak goes through the same path as the core's own output and fails the run, whatever
/// the encoding; the message names the canary and never the value.
#[test]
fn a_planted_leak_fails_the_run_and_the_message_hides_the_value() {
    use base64::Engine;
    let sc = scenario("happy-path");
    let clean = run_scenario_with_canary(&sc, 1).unwrap();
    let psk = guardedsleep_sim::world::synthetic_psk("A");
    let secret = clean.world.canary_parent_secret().unwrap().to_vec();
    let b64url = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let planted: Vec<(&str, String)> = vec![
        ("psk", b64url.encode(&psk)),
        ("psk", psk.iter().map(|b| format!("{b:02x}")).collect()),
        (
            "parent_secret",
            secret.iter().map(|b| format!("{b:02X}")).collect(),
        ),
        (
            "parent_secret",
            base64::engine::general_purpose::STANDARD.encode(&secret),
        ),
        (
            "plaintext_sdp",
            "offer v=0\r\no=- 1 2 IN IP4 0.0.0.0".to_owned(),
        ),
    ];
    // One world for every plant: the run-time parent secret is known after the run, and each
    // plant is a distinct (canary, encoding, kind) so none hides behind another.
    let mut world = guardedsleep_sim::world::World::new(&sc, 1, false).unwrap();
    world.enable_log_canary();
    world.run();
    assert!(world.violations.is_empty(), "{:?}", world.violations);
    for (n, (name, text)) in planted.into_iter().enumerate() {
        let before = world.violations.len();
        world.log_line(
            0,
            &format!("planted{n}"),
            &format!("INFO value={text}"),
            false,
        );
        let new = &world.violations[before..];
        let violation = new
            .iter()
            .find(|v| v.contains(&format!("canary: {name} ")))
            .unwrap_or_else(|| panic!("planted `{name}` leak not caught: {new:?}"));
        assert!(!violation.contains(&text), "the violation prints the value");
    }
}

#[test]
fn a_leak_in_a_wire_payload_is_caught_for_secrets_but_not_for_room_ids() {
    let sc = scenario("happy-path");
    let mut world = guardedsleep_sim::world::World::new(&sc, 1, false).unwrap();
    world.enable_log_canary();
    let room =
        guardedsleep_core::pairing::derive_room_id(&guardedsleep_sim::world::synthetic_psk("A"));
    world.log_line(0, "wire", &format!("{{\"room\":\"{room}\"}}"), true);
    assert!(world.violations.is_empty(), "{:?}", world.violations);
    world.log_line(0, "wire", &format!("{{\"room\":\"{room}\"}}"), false);
    assert!(!world.violations.is_empty());
}

#[test]
fn issued_turn_credentials_are_canaries_and_the_wire_exemption_does_not_cover_them() {
    let sc = scenario("turn-credentials-renewed-across-a-long-outage");
    let res = run_scenario_with_canary(&sc, 1).unwrap();
    assert!(res.clean(), "{}", res.report());
    let set = res.world.canary_set().unwrap();
    assert!(
        set.has("turn_password") && set.has("turn_password_1"),
        "{:?}",
        set.names()
    );
    // What an unredacted Debug of the frame would print, as wire text and as log text.
    let mut world = guardedsleep_sim::world::World::new(&sc, 1, false).unwrap();
    world.enable_log_canary();
    world.run();
    assert!(world.violations.is_empty(), "{:?}", world.violations);
    let leak = r#"MailboxFrame { id: 0, bytes: "{\"type\":\"ice_servers\",\"credential\":\"simcred1\"}" }"#;
    world.log_line(0, "planted", leak, true);
    assert!(
        world
            .violations
            .iter()
            .any(|v| v.contains("canary: turn_password ")),
        "{:?}",
        world.violations
    );
}
