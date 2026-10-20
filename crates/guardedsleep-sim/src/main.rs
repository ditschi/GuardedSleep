//! `gs-sim`: run `gs-scenario/1` files against the core, record `gs-trace/1` documents and lint
//! them. See README.md.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use guardedsleep_sim::lint::lint_trace;
use guardedsleep_sim::scenario::Scenario;
use guardedsleep_sim::{Verdict, load_scenarios, run_scenario, scenarios_dir, traces_dir};

const USAGE: &str = "usage:
  gs-sim list
  gs-sim run <scenario-name|path.json> [--seed N] [--trace-out DIR]
  gs-sim check [--seeds N]               run every scenario with seeds 1..=N (default 20)
  gs-sim bless-traces [--out DIR]        regenerate the curated traces (default: core vectors/traces)
  gs-sim lint-trace <file|dir>...";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("list") => list(),
        Some("run") => run(&args[1..]),
        Some("check") => check(&args[1..]),
        Some("bless-traces") => bless(&args[1..]),
        Some("lint-trace") => lint(&args[1..]),
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}

fn opt(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn list() -> Result<bool, String> {
    for sc in load_scenarios()? {
        let xf = if sc.expected_fail.is_some() {
            "  [expected_fail]"
        } else {
            ""
        };
        println!("{:<44} {}{xf}", sc.id, sc.description);
    }
    Ok(true)
}

fn find(name: &str) -> Result<Scenario, String> {
    let path = if Path::new(name).exists() {
        PathBuf::from(name)
    } else {
        scenarios_dir().join(format!("{name}.json"))
    };
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Scenario::parse(&text)
}

fn run(args: &[String]) -> Result<bool, String> {
    let name = args.first().ok_or(USAGE)?;
    let sc = find(name)?;
    let seed = opt(args, "--seed")
        .map(|s| s.parse::<u64>().map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or(sc.seed);
    let out = opt(args, "--trace-out").map(PathBuf::from);
    let res = run_scenario(&sc, seed, out.is_some())?;
    println!("{} (seed {seed}, {} ms simulated)", sc.id, res.world.end);
    print!("{}", res.report());
    if args.iter().any(|a| a == "--events") {
        for a in &res.world.actors {
            for e in &a.events {
                println!(
                    "  [{:>7}] {:<8} {} (contact+{})",
                    e.t, a.name, e.ev, e.since_contact
                );
            }
        }
    }
    if let Some(dir) = out {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        for rec in &res.world.recs {
            let id = format!("sim-{}-{}-{}-seed{seed}", sc.id, rec.actor, rec.instance);
            let path = dir.join(format!("{id}.json"));
            std::fs::write(&path, rec.to_json(&id, seed, &sc.id)).map_err(|e| e.to_string())?;
            println!("wrote {}", path.display());
        }
    }
    let verdict = res.verdict(&sc);
    println!("=> {verdict:?}");
    Ok(matches!(verdict, Verdict::Pass | Verdict::ExpectedFail))
}

fn check(args: &[String]) -> Result<bool, String> {
    let seeds: u64 = opt(args, "--seeds")
        .map(|s| {
            s.parse()
                .map_err(|e: std::num::ParseIntError| e.to_string())
        })
        .transpose()?
        .unwrap_or(20);
    let mut ok = true;
    for sc in load_scenarios()? {
        let mut bad = Vec::new();
        for seed in 1..=seeds {
            let res = run_scenario(&sc, seed, false)?;
            if !matches!(res.verdict(&sc), Verdict::Pass | Verdict::ExpectedFail) {
                bad.push(seed);
            }
        }
        let tag = if sc.expected_fail.is_some() {
            " (expected_fail)"
        } else {
            ""
        };
        if bad.is_empty() {
            println!("ok    {}{tag}", sc.id);
        } else {
            ok = false;
            println!("FAIL  {}{tag}: seeds {bad:?}", sc.id);
        }
    }
    Ok(ok)
}

fn bless(args: &[String]) -> Result<bool, String> {
    let dir = opt(args, "--out")
        .map(PathBuf::from)
        .unwrap_or_else(traces_dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut n = 0;
    for sc in load_scenarios()? {
        if sc.export_traces.is_empty() {
            continue;
        }
        let res = run_scenario(&sc, sc.seed, true)?;
        if !res.clean() {
            return Err(format!(
                "scenario {} does not pass; refusing to export\n{}",
                sc.id,
                res.report()
            ));
        }
        for ex in &sc.export_traces {
            let rec = res
                .world
                .recs
                .iter()
                .find(|r| r.actor == ex.peer && r.instance == ex.instance)
                .ok_or(format!(
                    "{}: no trace for {} #{}",
                    sc.id, ex.peer, ex.instance
                ))?;
            let text = rec.to_json(&ex.id, sc.seed, &sc.id);
            let errs = lint_trace(&text);
            if !errs.is_empty() {
                return Err(format!("{}: lint failed: {errs:?}", ex.id));
            }
            std::fs::write(dir.join(format!("{}.json", ex.id)), text).map_err(|e| e.to_string())?;
            println!("wrote {}.json ({} steps)", ex.id, rec.steps.len());
            n += 1;
        }
    }
    println!("blessed {n} traces into {}", dir.display());
    Ok(true)
}

fn lint(args: &[String]) -> Result<bool, String> {
    if args.is_empty() {
        return Err(USAGE.to_owned());
    }
    let mut files = Vec::new();
    for a in args {
        let p = PathBuf::from(a);
        if p.is_dir() {
            let mut v: Vec<_> = std::fs::read_dir(&p)
                .map_err(|e| e.to_string())?
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "json"))
                .collect();
            v.sort();
            files.extend(v);
        } else {
            files.push(p);
        }
    }
    let mut ok = true;
    for f in files {
        let text = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        let errs = lint_trace(&text);
        if errs.is_empty() {
            println!("ok    {}", f.display());
        } else {
            ok = false;
            println!("FAIL  {}", f.display());
            for e in errs {
                println!("        {e}");
            }
        }
    }
    Ok(ok)
}
