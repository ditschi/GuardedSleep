//! Regenerates `expect` in `vectors/conformance.json` from the Rust core (the golden reference).
//! Run after an intentional behaviour change, then review the diff:
//!   cargo run -p guardedsleep-core --example bless_vectors

use serde_json::Value;

fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/conformance.json");
    let raw = std::fs::read_to_string(path).expect("read vectors");
    let mut cases: Vec<Value> = serde_json::from_str(&raw).expect("parse vectors");
    for case in &mut cases {
        let mut input = case.clone();
        if let Some(o) = input.as_object_mut() {
            o.remove("expect");
        }
        let got: Value = serde_json::from_str(&guardedsleep_core::conformance::evaluate(
            &input.to_string(),
        ))
        .expect("evaluate");
        case["expect"] = got;
    }
    let mut out = serde_json::to_string_pretty(&cases).expect("serialize");
    out.push('\n');
    std::fs::write(path, out).expect("write vectors");
    println!("blessed {} vectors", cases.len());
}
