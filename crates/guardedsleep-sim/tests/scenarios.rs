//! Runs every committed scenario with 20 seeds, checks that scenarios marked `expected_fail`
//! still fail, lints the committed traces and checks determinism.

use guardedsleep_sim::lint::lint_trace;
use guardedsleep_sim::{load_scenarios, run_scenario, traces_dir, Verdict};

const SEEDS: u64 = 20;

#[test]
fn all_scenarios_pass_with_every_seed() {
    let scenarios = load_scenarios().expect("scenarios load");
    assert!(scenarios.len() >= 25, "scenario set shrank");
    let mut problems = Vec::new();
    for sc in &scenarios {
        for seed in 1..=SEEDS {
            let res = run_scenario(sc, seed, false).expect("scenario builds");
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
