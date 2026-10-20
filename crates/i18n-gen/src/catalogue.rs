use crate::icu::{collect_args, parse, plural_nodes, Node};
use serde_json::Value;
use std::path::Path;

pub const LOCALES: [&str; 4] = ["en", "de", "fr", "es"];
pub const MASTER: &str = "en";
/// Top-level groups emitted to Android; every other group is web-only.
pub const ANDROID_GROUPS: [&str; 2] = ["native", "shared"];

/// Messages as (dot path, ICU string) in file order.
pub type Flat = Vec<(String, String)>;

pub struct Catalogue {
    pub locales: Vec<(String, Flat)>,
}

impl Catalogue {
    pub fn get(&self, locale: &str) -> &Flat {
        &self
            .locales
            .iter()
            .find(|(l, _)| l == locale)
            .expect("known locale")
            .1
    }
}

pub fn lookup<'a>(flat: &'a Flat, key: &str) -> Option<&'a str> {
    flat.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// CLDR plural categories required per locale (CLDR 42+). Unknown locales fail loudly.
pub fn plural_categories(locale: &str) -> Result<&'static [&'static str], String> {
    match locale {
        "en" | "de" => Ok(&["one", "other"]),
        "fr" | "es" => Ok(&["one", "many", "other"]),
        _ => Err(format!(
            "no CLDR plural table for locale \"{locale}\"; add it to plural_categories()"
        )),
    }
}

fn valid_segment(k: &str) -> bool {
    let mut it = k.chars();
    matches!(it.next(), Some(c) if c.is_ascii_alphabetic()) && it.all(|c| c.is_ascii_alphanumeric())
}

pub fn flatten(v: &Value, locale: &str, issues: &mut Vec<String>, prefix: &str, out: &mut Flat) {
    let Some(obj) = v.as_object() else {
        issues.push(format!(
            "{locale}: \"{}\" must be an object",
            if prefix.is_empty() { "<root>" } else { prefix }
        ));
        return;
    };
    for (k, v) in obj {
        let p = if prefix.is_empty() {
            k.clone()
        } else {
            format!("{prefix}.{k}")
        };
        if !valid_segment(k) {
            issues.push(format!(
                "{locale}: invalid key segment \"{k}\" in \"{p}\" (use camelCase letters/digits)"
            ));
        }
        match v {
            Value::String(s) if s.trim().is_empty() => {
                issues.push(format!("{locale}: empty message \"{p}\""))
            }
            Value::String(s) => out.push((p, s.clone())),
            Value::Object(_) => flatten(v, locale, issues, &p, out),
            _ => issues.push(format!("{locale}: \"{p}\" must be a string or an object")),
        }
    }
}

pub fn load(dir: &Path, issues: &mut Vec<String>) -> Catalogue {
    let mut locales = Vec::new();
    for l in LOCALES {
        let file = dir.join(format!("{l}.json"));
        let mut flat = Flat::new();
        match std::fs::read_to_string(&file)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str::<Value>(&s).map_err(|e| e.to_string()))
        {
            Ok(v) => flatten(&v, l, issues, "", &mut flat),
            Err(e) => issues.push(format!("{l}: cannot read {}: {e}", file.display())),
        }
        locales.push((l.to_string(), flat));
    }
    Catalogue { locales }
}

pub type Parsed = Vec<(String, Vec<Node>)>;

pub fn parse_all(flat: &Flat, locale: &str, issues: &mut Vec<String>) -> Parsed {
    flat.iter()
        .filter_map(|(k, m)| match parse(m) {
            Ok(n) => Some((k.clone(), n)),
            Err(e) => {
                issues.push(format!("{locale}: \"{k}\": parse error: {e}"));
                None
            }
        })
        .collect()
}

fn sig(nodes: &[Node]) -> String {
    let mut v: Vec<String> = collect_args(nodes)
        .into_iter()
        .map(|(n, k)| format!("{n}:{}", k.as_str()))
        .collect();
    v.sort();
    v.join(", ")
}

/// Key parity, argument parity and plural categories; returns human-readable issues.
pub fn validate(cat: &Catalogue) -> Vec<String> {
    let mut issues = Vec::new();
    let parsed: Vec<(String, Parsed)> = cat
        .locales
        .iter()
        .map(|(l, f)| (l.clone(), parse_all(f, l, &mut issues)))
        .collect();
    let master = cat.get(MASTER);
    for (l, flat) in &cat.locales {
        if l == MASTER {
            continue;
        }
        for (k, _) in master {
            if lookup(flat, k).is_none() {
                issues.push(format!("{l}: missing key \"{k}\""));
            }
        }
        for (k, _) in flat {
            if lookup(master, k).is_none() {
                issues.push(format!("{l}: extra key \"{k}\" (not in {MASTER})"));
            }
        }
    }
    let master_ast = &parsed.iter().find(|(l, _)| l == MASTER).expect("master").1;
    for (l, msgs) in &parsed {
        let required = match plural_categories(l) {
            Ok(r) => r,
            Err(e) => {
                issues.push(e);
                continue;
            }
        };
        for (key, nodes) in msgs {
            if l != MASTER {
                if let Some((_, m)) = master_ast.iter().find(|(k, _)| k == key) {
                    let (a, b) = (sig(m), sig(nodes));
                    if a != b {
                        issues.push(format!(
                            "{l}: \"{key}\": arguments differ from {MASTER} ({MASTER}: [{a}], {l}: [{b}])"
                        ));
                    }
                }
            }
            for pl in plural_nodes(nodes) {
                let Node::Plural { name, cases } = pl else {
                    continue;
                };
                for c in required {
                    if !cases.iter().any(|x| x.sel == *c) {
                        issues.push(format!(
                            "{l}: \"{key}\": plural \"{name}\" lacks required category \"{c}\" for locale {l}"
                        ));
                    }
                }
            }
        }
    }
    issues
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn cat(over: &[(&str, &[(&str, &str)])]) -> Catalogue {
        let base: [(&str, Vec<(&str, &str)>); 4] = [
            (
                "en",
                vec![
                    ("a.hello", "Hello {name}"),
                    ("a.items", "{count, plural, one {# item} other {# items}}"),
                ],
            ),
            (
                "de",
                vec![
                    ("a.hello", "Hallo {name}"),
                    ("a.items", "{count, plural, one {# Ding} other {# Dinge}}"),
                ],
            ),
            (
                "fr",
                vec![
                    ("a.hello", "Bonjour {name}"),
                    (
                        "a.items",
                        "{count, plural, one {# object} many {# d'objects} other {# objects}}",
                    ),
                ],
            ),
            (
                "es",
                vec![
                    ("a.hello", "Hola {name}"),
                    (
                        "a.items",
                        "{count, plural, one {# cosa} many {# de cosas} other {# cosas}}",
                    ),
                ],
            ),
        ];
        let locales = base
            .iter()
            .map(|(l, m)| {
                let m = over
                    .iter()
                    .find(|(o, _)| o == l)
                    .map(|(_, m)| m.to_vec())
                    .unwrap_or_else(|| m.clone());
                (
                    l.to_string(),
                    m.iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect(),
                )
            })
            .collect();
        Catalogue { locales }
    }

    #[test]
    fn consistent_catalogue_is_valid() {
        assert_eq!(validate(&cat(&[])), Vec::<String>::new());
    }

    #[test]
    fn cldr_categories_per_locale_and_unknown_locale_fails() {
        assert_eq!(plural_categories("en").unwrap(), ["one", "other"]);
        assert_eq!(plural_categories("de").unwrap(), ["one", "other"]);
        assert_eq!(plural_categories("fr").unwrap(), ["one", "many", "other"]);
        assert_eq!(plural_categories("es").unwrap(), ["one", "many", "other"]);
        assert!(plural_categories("ru").is_err());
    }

    #[test]
    fn french_plural_without_many_is_rejected() {
        let issues = validate(&cat(&[(
            "fr",
            &[
                ("a.hello", "Bonjour {name}"),
                (
                    "a.items",
                    "{count, plural, one {# object} other {# objects}}",
                ),
            ],
        )]));
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(issues[0].contains("fr: \"a.items\"") && issues[0].contains("\"many\""));
    }

    #[test]
    fn missing_and_extra_keys() {
        let issues = validate(&cat(&[(
            "de",
            &[("a.hello", "Hallo {name}"), ("a.extra", "x")],
        )]));
        assert!(issues.contains(&"de: missing key \"a.items\"".to_string()));
        assert!(issues
            .iter()
            .any(|i| i.starts_with("de: extra key \"a.extra\"")));
    }

    #[test]
    fn argument_name_and_kind_mismatches() {
        let items = "{count, plural, one {# Ding} other {# Dinge}}";
        let renamed = validate(&cat(&[(
            "de",
            &[("a.hello", "Hallo {nombre}"), ("a.items", items)],
        )]));
        assert!(renamed
            .iter()
            .any(|i| i.contains("de: \"a.hello\": arguments differ")));
        let kind = validate(&cat(&[(
            "de",
            &[("a.hello", "Hallo {name}"), ("a.items", "{count} Dinge")],
        )]));
        assert!(kind
            .iter()
            .any(|i| i.contains("de: \"a.items\": arguments differ")));
    }

    #[test]
    fn parse_errors_name_locale_and_key() {
        let issues = validate(&cat(&[(
            "de",
            &[
                ("a.hello", "Hallo {name"),
                ("a.items", "{count, plural, one {x} other {y}}"),
            ],
        )]));
        assert!(issues
            .iter()
            .any(|i| i.contains("de: \"a.hello\": parse error")));
    }

    #[test]
    fn flatten_rejects_empty_non_string_and_bad_keys() {
        let v: Value = serde_json::from_str(
            r#"{"ok":"x","empty":" ","n":1,"bad-key":"y","nested":{"fine":"z"}}"#,
        )
        .unwrap();
        let (mut issues, mut out) = (Vec::new(), Flat::new());
        flatten(&v, "en", &mut issues, "", &mut out);
        assert_eq!(issues.len(), 3, "{issues:?}");
        assert_eq!(out.len(), 3);
    }
}
