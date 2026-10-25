//! The soak report: a JSON file for the record and a markdown table for the humans.

use std::path::Path;

use serde_json::Value;

use crate::stats::{RelayStats, Verdict, percentile};

pub struct SoakReport {
    pub json: Value,
    pub relays: Vec<RelayStats>,
    pub verdict: Verdict,
}

/// Writes through a temporary file so that a killed run never leaves half a report.
pub fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, format!("{text}\n")).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

fn ms(value: Option<u64>) -> String {
    value.map_or("n/a".to_owned(), |v| v.to_string())
}

fn host(url: &str) -> &str {
    url.strip_prefix("wss://").unwrap_or(url)
}

pub fn soak_markdown(report: &SoakReport) -> String {
    let link = &report.json["link"];
    let rec = &link["recoveries"];
    let mut out = String::new();
    out.push_str(&format!(
        "Verdict: **{}**\n\n",
        report.verdict.outcome.as_str()
    ));
    for reason in &report.verdict.reasons {
        out.push_str(&format!("- {reason}\n"));
    }
    out.push_str(&format!(
        "\nRun: {} s of {} s planned{}. Initial setup {} ms. Forced losses {}, recoveries {} ({} failed): p50 {} ms, p95 {} ms, max {} ms.\n\n",
        report.json["elapsed_s"],
        report.json["planned_s"],
        if report.json["complete"] == Value::Bool(true) {
            ""
        } else {
            " (snapshot, run not complete)"
        },
        link["initial_setup_ms"],
        link["cuts"],
        rec["n"],
        rec["failed"],
        rec["p50"],
        rec["p95"],
        rec["max"],
    ));
    out.push_str("| Relay | Opens | Open fails | Unexpected closes | Sent | OK | Refused (created_at / park) | Deliverable | Delivered | Lost | Delivery p50 / p95 ms | Connect p50 ms |\n");
    out.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for r in &report.relays {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} ({} / {}) | {} | {} | {} | {} / {} | {} |\n",
            host(&r.url),
            r.opens,
            r.open_failures,
            r.unexpected_closes,
            r.events_sent,
            r.ok_true,
            r.refused,
            r.refused_clock,
            r.refused_park,
            r.eligible,
            r.delivered,
            r.lost,
            ms(percentile(&r.latency_ms, 50)),
            ms(percentile(&r.latency_ms, 95)),
            ms(percentile(&r.connect_ms, 50)),
        ));
    }
    out.push('\n');
    out
}

pub fn burst_markdown(report: &Value) -> String {
    let mut out = String::from(
        "| Relay | Result | Events | Accepted | Refused | No OK | Delivered | Delivery p50 / p95 ms | Refusal messages |\n| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |\n",
    );
    for r in report["relays"].as_array().into_iter().flatten() {
        let msgs = r["refusal_messages"]
            .as_array()
            .map(|m| {
                m.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default();
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} / {} | {} |\n",
            host(r["url"].as_str().unwrap_or("?")),
            r["result"].as_str().unwrap_or("?"),
            r["events"],
            r["accepted"],
            r["refused"],
            r["no_ok"],
            r["delivered"],
            r["delivery_ms"]["p50"],
            r["delivery_ms"]["p95"],
            msgs.replace('|', "/"),
        ));
    }
    out.push('\n');
    out
}
