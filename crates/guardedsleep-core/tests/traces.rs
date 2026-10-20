//! Replays every committed `gs-trace/1` document (vectors/traces/*.json) through
//! `conformance::evaluate` and compares with `expect`. Kotlin and WASM do the same.

use guardedsleep_core::conformance::evaluate;
use guardedsleep_core::link::{Link, LinkConfig, LinkInput};
use serde_json::Value;

fn trace_files() -> Vec<std::path::PathBuf> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("vectors/traces");
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .expect("vectors/traces exists")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    v.sort();
    v
}

#[test]
fn committed_traces_replay_identically() {
    let files = trace_files();
    assert!(files.len() >= 8, "expected the curated trace set");
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        let trace: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(trace["schema"], "gs-trace/1");
        assert_eq!(trace["privacy"], "shareable");
        let got: Value = serde_json::from_str(&evaluate(&text)).unwrap();
        assert_eq!(got, trace["expect"], "trace {}", f.display());
        assert_eq!(
            got.as_array().unwrap().len(),
            trace["steps"].as_array().unwrap().len()
        );
    }
}

#[test]
fn committed_traces_end_in_the_recorded_state() {
    for f in trace_files() {
        let trace: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        let Some(state) = trace["final"]["state"].as_str() else {
            continue;
        };
        let cfg: LinkConfig = serde_json::from_value(trace["config"].clone()).unwrap();
        let mut link = Link::new(cfg).unwrap();
        for s in trace["steps"].as_array().unwrap() {
            let input: LinkInput = serde_json::from_value(s["input"].clone()).unwrap();
            link.handle(s["now"].as_u64().unwrap(), input);
        }
        let got = serde_json::to_value(link.status().unwrap().state).unwrap();
        assert_eq!(got, state, "trace {}", f.display());
    }
}
