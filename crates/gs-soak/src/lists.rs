//! The curated relay list (`data/nostr-relays.json`) as far as the tools need it.

use std::path::Path;

use guardedsleep_core::relay_pick::RelayCandidate;
use serde_json::Value;

/// Active entries of the relay list, as the platforms hand them to `pick_relays`.
pub fn active_relays(data_dir: &Path) -> Result<Vec<RelayCandidate>, String> {
    let path = data_dir.join("nostr-relays.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    candidates_from_json(&text)
}

fn candidates_from_json(text: &str) -> Result<Vec<RelayCandidate>, String> {
    let root: Value = serde_json::from_str(text).map_err(|e| format!("nostr-relays.json: {e}"))?;
    let entries = root
        .get("relays")
        .and_then(Value::as_array)
        .ok_or("nostr-relays.json: no `relays` array")?;
    let mut out = Vec::new();
    for entry in entries {
        if entry.get("status").and_then(Value::as_str) != Some("active") {
            continue;
        }
        let text = |key: &str| entry.get(key).and_then(Value::as_str).map(str::to_owned);
        let (Some(host), Some(operator)) = (text("host"), text("operator")) else {
            continue;
        };
        out.push(RelayCandidate {
            host,
            operator,
            hoster: text("hoster"),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::candidates_from_json;

    #[test]
    fn keeps_active_entries_only() {
        let json = r#"{"relays":[
          {"host":"a.example","operator":"A","hoster":null,"status":"active"},
          {"host":"b.example","operator":"B","hoster":"H","status":"suspended"},
          {"host":"c.example","operator":"C","hoster":"H","status":"active"}]}"#;
        let list = candidates_from_json(json).unwrap_or_default();
        let hosts: Vec<&str> = list.iter().map(|c| c.host.as_str()).collect();
        assert_eq!(hosts, ["a.example", "c.example"]);
        assert_eq!(list[1].hoster.as_deref(), Some("H"));
    }
}
