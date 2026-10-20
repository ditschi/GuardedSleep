//! Deterministic host-side system simulation of the GuardedSleep link (ADR-015 AC-9/AC-10,
//! ADR-016 Stage 1): two `Link`s, a relay that mirrors the Node signaling server, a fake media
//! layer and a seeded fault injector, driven by `gs-scenario/1` files.

#![forbid(unsafe_code)]

pub mod check;
pub mod lint;
pub mod raw;
pub mod rng;
pub mod scenario;
pub mod trace;
pub mod world;

use check::Check;
use scenario::Scenario;
use world::World;

pub struct RunResult {
    pub scenario: String,
    pub seed: u64,
    pub world: World,
    pub checks: Vec<Check>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    /// Failed as announced by `expected_fail`.
    ExpectedFail,
    /// Passed although `expected_fail` is set: remove the marker.
    UnexpectedPass,
}

impl RunResult {
    pub fn clean(&self) -> bool {
        self.world.violations.is_empty() && self.checks.iter().all(|c| c.ok)
    }

    pub fn verdict(&self, sc: &Scenario) -> Verdict {
        match (self.clean(), sc.expected_fail.is_some()) {
            (true, false) => Verdict::Pass,
            (false, false) => Verdict::Fail,
            (false, true) => Verdict::ExpectedFail,
            (true, true) => Verdict::UnexpectedPass,
        }
    }

    pub fn report(&self) -> String {
        let mut s = String::new();
        for v in &self.world.violations {
            s.push_str(&format!("  VIOLATION {v}\n"));
        }
        for c in &self.checks {
            s.push_str(&format!(
                "  {} {}\n",
                if c.ok { "ok  " } else { "FAIL" },
                c.text
            ));
        }
        s
    }
}

pub fn run_scenario(sc: &Scenario, seed: u64, record: bool) -> Result<RunResult, String> {
    let mut world = World::new(sc, seed, record)?;
    world.run();
    let checks = check::evaluate(&world, sc);
    Ok(RunResult {
        scenario: sc.id.clone(),
        seed,
        world,
        checks,
    })
}

pub fn scenarios_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios")
}

pub fn traces_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../guardedsleep-core/vectors/traces")
}

pub fn load_scenarios() -> Result<Vec<Scenario>, String> {
    let mut paths: Vec<_> = std::fs::read_dir(scenarios_dir())
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|p| {
            let text = std::fs::read_to_string(p).map_err(|e| e.to_string())?;
            Scenario::parse(&text).map_err(|e| format!("{}: {e}", p.display()))
        })
        .collect()
}
