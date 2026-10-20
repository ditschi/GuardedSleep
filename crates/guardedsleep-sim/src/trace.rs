//! Recording of `gs-trace/1` documents: for each `Link` instance the exact `(now, input)` steps
//! and the actions it returned.

use guardedsleep_core::link::{LinkAction, LinkConfig, LinkInput, LinkState};
use serde_json::{Value, json};

pub const TRACE_SCHEMA: &str = "gs-trace/1";

pub struct TraceRec {
    pub actor: String,
    pub instance: u32,
    pub config: LinkConfig,
    pub steps: Vec<(u64, LinkInput)>,
    pub expect: Vec<Vec<LinkAction>>,
    pub final_state: Option<LinkState>,
}

impl TraceRec {
    pub fn new(actor: &str, instance: u32, config: LinkConfig) -> Self {
        Self {
            actor: actor.to_owned(),
            instance,
            config,
            steps: Vec::new(),
            expect: Vec::new(),
            final_state: None,
        }
    }

    pub fn push(&mut self, now: u64, input: LinkInput, actions: &[LinkAction]) {
        self.steps.push((now, input));
        self.expect.push(actions.to_vec());
    }

    /// One top-level field per line, one step and one expectation per line, so diffs stay readable.
    pub fn to_json(&self, id: &str, seed: u64, scenario: &str) -> String {
        let steps: Vec<String> = self
            .steps
            .iter()
            .enumerate()
            .map(|(n, (now, input))| json!({"n": n, "now": now, "input": input}).to_string())
            .collect();
        let expect: Vec<String> = self
            .expect
            .iter()
            .map(|a| serde_json::to_string(a).unwrap_or_default())
            .collect();
        let origin = json!({
            "core_version": env!("CARGO_PKG_VERSION"),
            "harness": "sim/1",
            "scenario": scenario,
        });
        let mut out = String::from("{\n");
        out.push_str(&format!("  \"schema\": \"{TRACE_SCHEMA}\",\n"));
        out.push_str(&format!("  \"id\": {},\n", Value::from(id)));
        out.push_str("  \"class\": \"simulated\",\n");
        out.push_str("  \"privacy\": \"shareable\",\n");
        out.push_str(&format!("  \"origin\": {origin},\n"));
        out.push_str(&format!("  \"seed\": {seed},\n"));
        out.push_str(&format!(
            "  \"config\": {},\n",
            serde_json::to_string(&self.config).unwrap_or_default()
        ));
        out.push_str("  \"steps\": [\n");
        out.push_str(&indent_lines(&steps));
        out.push_str("\n  ],\n  \"expect\": [\n");
        out.push_str(&indent_lines(&expect));
        out.push_str("\n  ]");
        if let Some(state) = self.final_state {
            out.push_str(&format!(",\n  \"final\": {}", json!({ "state": state })));
        }
        out.push_str("\n}\n");
        out
    }
}

fn indent_lines(lines: &[String]) -> String {
    lines
        .iter()
        .map(|l| format!("    {l}"))
        .collect::<Vec<_>>()
        .join(",\n")
}
