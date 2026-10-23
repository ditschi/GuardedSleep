//! Every key in `message_keys()` must exist in the master catalogue as `core.<camelCase key>`
//! (openspec `expand-locales`, requirement "Core message keys exist"). Test-only: the core itself
//! stays free of file and catalogue access.

use guardedsleep_core::link::message_keys;
use serde_json::Value;

const EN: &str = include_str!("../../../locales/en.json");

/// `server_unreachable` -> `serverUnreachable`.
fn camel_case(snake: &str) -> String {
    let mut out = String::new();
    for (i, part) in snake.split('_').enumerate() {
        if i == 0 {
            out.push_str(part);
        } else {
            let mut chars = part.chars();
            if let Some(first) = chars.next() {
                out.extend(first.to_uppercase());
                out.push_str(chars.as_str());
            }
        }
    }
    out
}

/// Catalogue keys (`core.<camelCase>`) of `keys` that `catalogue` does not hold as a string.
fn missing_keys(catalogue: &Value, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .map(|k| camel_case(k))
        .filter(|name| {
            !catalogue
                .get("core")
                .and_then(|g| g.get(name.as_str()))
                .is_some_and(|v| v.as_str().is_some_and(|s| !s.is_empty()))
        })
        .map(|name| format!("core.{name}"))
        .collect()
}

#[test]
fn snake_case_becomes_camel_case() {
    assert_eq!(camel_case("server_unreachable"), "serverUnreachable");
    assert_eq!(camel_case("pairing_key_mismatch"), "pairingKeyMismatch");
    assert_eq!(camel_case("wrong_key_attempt"), "wrongKeyAttempt");
    assert_eq!(camel_case("single"), "single");
}

#[test]
fn a_missing_key_is_reported_by_its_catalogue_name() {
    let catalogue: Value = serde_json::from_str(r#"{"core":{"peerAbsent":"x"}}"#).unwrap();
    assert_eq!(
        missing_keys(&catalogue, &["peer_absent", "media_blocked"]),
        vec!["core.mediaBlocked".to_string()]
    );
    let empty: Value = serde_json::from_str("{}").unwrap();
    assert_eq!(
        missing_keys(&empty, &["peer_absent"]),
        vec!["core.peerAbsent".to_string()]
    );
}

#[test]
fn every_core_message_key_exists_in_the_master_catalogue() {
    let catalogue: Value = serde_json::from_str(EN).unwrap();
    let missing = missing_keys(&catalogue, message_keys());
    assert!(
        missing.is_empty(),
        "locales/en.json lacks core message keys: {}",
        missing.join(", ")
    );
}
