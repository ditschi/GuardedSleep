use crate::critical::{Critical, rendered_len};
use crate::icu::{Node, collect_args, parse, plural_nodes};
use serde_json::Value;
use std::path::Path;

pub const MASTER: &str = "en";
/// Top-level groups emitted to Android; every other group is web-only.
pub const ANDROID_GROUPS: [&str; 2] = ["native", "shared"];
/// Reserved top-level key of non-English catalogues: review markers, never output.
pub const META_KEY: &str = "$meta";
/// File in `locales/` that is not a catalogue.
pub const CRITICAL_FILE: &str = "critical.json";
pub const MARKERS: [&str; 2] = ["machineTranslated", "reviewed"];

/// Messages as (dot path, ICU string) in file order.
pub type Flat = Vec<(String, String)>;

/// `$meta` of a non-English catalogue: every value is `default` unless its key is `reviewed`.
#[derive(Debug, Clone, PartialEq)]
pub struct Meta {
    pub default: Option<String>,
    pub reviewed: Vec<String>,
}

pub struct Catalogue {
    pub locales: Vec<(String, Flat)>,
    /// Same order as `locales`; `None` when the file has no `$meta`.
    pub meta: Vec<(String, Option<Meta>)>,
    pub critical: Critical,
}

impl Catalogue {
    /// A catalogue without critical keys; non-English locales get a valid default marker.
    #[cfg(test)]
    pub fn new(locales: Vec<(String, Flat)>) -> Self {
        let meta = locales
            .iter()
            .map(|(l, _)| {
                let m = (l != MASTER).then(|| Meta {
                    default: Some(MARKERS[0].to_string()),
                    reviewed: Vec::new(),
                });
                (l.clone(), m)
            })
            .collect();
        Catalogue {
            locales,
            meta,
            critical: Critical::default(),
        }
    }

    pub fn get(&self, locale: &str) -> &Flat {
        &self
            .locales
            .iter()
            .find(|(l, _)| l == locale)
            .expect("known locale")
            .1
    }

    pub fn tags(&self) -> Vec<&str> {
        self.locales.iter().map(|(l, _)| l.as_str()).collect()
    }

    /// The catalogue without the given locales (those that are not shipped).
    pub fn without(&self, drop: &[String]) -> Catalogue {
        let keep = |l: &String| !drop.contains(l);
        Catalogue {
            locales: self
                .locales
                .iter()
                .filter(|(l, _)| keep(l))
                .cloned()
                .collect(),
            meta: self.meta.iter().filter(|(l, _)| keep(l)).cloned().collect(),
            critical: self.critical.clone(),
        }
    }
}

pub fn lookup<'a>(flat: &'a Flat, key: &str) -> Option<&'a str> {
    flat.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// CLDR plural categories required per locale (CLDR 42+). Unknown locales fail loudly.
pub fn plural_categories(locale: &str) -> Result<&'static [&'static str], String> {
    match locale {
        "en" | "de" | "nl" | "sv" | "da" | "tr" | "hi" => Ok(&["one", "other"]),
        "fr" | "es" | "it" | "pt" => Ok(&["one", "many", "other"]),
        "ro" => Ok(&["one", "few", "other"]),
        "pl" | "cs" | "uk" => Ok(&["one", "few", "many", "other"]),
        "ar" => Ok(&["zero", "one", "two", "few", "many", "other"]),
        "zh-Hans" | "ja" | "ko" | "id" => Ok(&["other"]),
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

fn parse_meta(v: &Value, locale: &str, issues: &mut Vec<String>) -> Meta {
    let mut meta = Meta {
        default: None,
        reviewed: Vec::new(),
    };
    let Some(obj) = v.as_object() else {
        issues.push(format!("{locale}: \"{META_KEY}\" must be an object"));
        return meta;
    };
    for (k, v) in obj {
        match (k.as_str(), v) {
            ("default", Value::String(s)) => meta.default = Some(s.clone()),
            ("reviewed", Value::Array(items)) => {
                for i in items {
                    match i.as_str() {
                        Some(key) => meta.reviewed.push(key.to_string()),
                        None => issues.push(format!(
                            "{locale}: \"{META_KEY}.reviewed\" must list key strings"
                        )),
                    }
                }
            }
            _ => issues.push(format!(
                "{locale}: unexpected \"{META_KEY}.{k}\" (allowed: default, reviewed)"
            )),
        }
    }
    meta
}

/// Reads every `*.json` catalogue in `dir` (`en` first, then by tag) and `critical.json`.
pub fn load(dir: &Path, issues: &mut Vec<String>) -> Catalogue {
    let mut tags: Vec<String> = match std::fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with(".json") && n != CRITICAL_FILE)
            .map(|n| n.trim_end_matches(".json").to_string())
            .collect(),
        Err(e) => {
            issues.push(format!("cannot read {}: {e}", dir.display()));
            Vec::new()
        }
    };
    tags.sort_by(|a, b| (a != MASTER, a).cmp(&(b != MASTER, b)));
    if !tags.iter().any(|t| t == MASTER) {
        issues.push(format!(
            "{MASTER}: master catalogue not found in {}",
            dir.display()
        ));
    }
    let mut locales = Vec::new();
    let mut meta = Vec::new();
    for l in tags {
        let file = dir.join(format!("{l}.json"));
        let mut flat = Flat::new();
        let mut m = None;
        match std::fs::read_to_string(&file)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str::<Value>(&s).map_err(|e| e.to_string()))
        {
            Ok(mut v) => {
                if let Some(raw) = v.as_object_mut().and_then(|o| o.shift_remove(META_KEY)) {
                    m = Some(parse_meta(&raw, &l, issues));
                }
                flatten(&v, &l, issues, "", &mut flat);
            }
            Err(e) => issues.push(format!("{l}: cannot read {}: {e}", file.display())),
        }
        meta.push((l.clone(), m));
        locales.push((l, flat));
    }
    let critical_file = dir.join(CRITICAL_FILE);
    let critical = match std::fs::read_to_string(&critical_file)
        .map_err(|e| e.to_string())
        .and_then(|s| Critical::parse(&s))
    {
        Ok(c) => c,
        Err(e) => {
            issues.push(format!("cannot read {}: {e}", critical_file.display()));
            Critical::default()
        }
    };
    Catalogue {
        locales,
        meta,
        critical,
    }
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

/// Outcome of [`validate`].
#[derive(Debug, Default, PartialEq)]
pub struct Report {
    /// Problems that fail every command.
    pub errors: Vec<String>,
    /// Locales that miss or break a critical key: `(locale, problem)`. Such a locale is not
    /// generated; the check fails and names the key.
    pub unshipped: Vec<(String, String)>,
}

impl Report {
    #[cfg(test)]
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty() && self.unshipped.is_empty()
    }

    /// Every problem as text.
    #[cfg(test)]
    pub fn all(&self) -> Vec<String> {
        let mut v = self.errors.clone();
        v.extend(
            self.unshipped
                .iter()
                .map(|(l, m)| format!("{l}: not shipped, {m}")),
        );
        v
    }

    pub fn unshipped_locales(&self) -> Vec<String> {
        let mut v: Vec<String> = self.unshipped.iter().map(|(l, _)| l.clone()).collect();
        v.dedup();
        v
    }
}

fn validate_meta(cat: &Catalogue, errors: &mut Vec<String>) {
    for (l, m) in &cat.meta {
        if l == MASTER {
            if m.is_some() {
                errors.push(format!(
                    "{l}: \"{META_KEY}\" is reserved for non-{MASTER} catalogues"
                ));
            }
            continue;
        }
        let Some(m) = m else {
            errors.push(format!(
                "{l}: missing \"{META_KEY}\" with \"default\": \"machineTranslated\" or \"reviewed\""
            ));
            continue;
        };
        match m.default.as_deref() {
            Some(d) if MARKERS.contains(&d) => {}
            Some(d) => errors.push(format!(
                "{l}: \"{META_KEY}.default\" is \"{d}\", expected machineTranslated or reviewed"
            )),
            None => errors.push(format!("{l}: \"{META_KEY}.default\" is missing")),
        }
        let flat = cat.get(l);
        for k in &m.reviewed {
            if lookup(flat, k).is_none() {
                errors.push(format!(
                    "{l}: \"{META_KEY}.reviewed\" lists unknown key \"{k}\""
                ));
            }
        }
    }
}

/// Critical-key completeness, argument and plural checks, length budgets and `$meta`.
/// Missing non-critical keys are fine: they fall back to the master in the output.
pub fn validate(cat: &Catalogue) -> Report {
    let mut rep = Report::default();
    let master = cat.get(MASTER);
    let master_keys: Vec<&str> = master.iter().map(|(k, _)| k.as_str()).collect();
    for p in cat.critical.stale(&master_keys) {
        rep.errors.push(format!(
            "{CRITICAL_FILE}: pattern \"{p}\" matches no key in {MASTER}.json (renamed or removed key?)"
        ));
    }
    validate_meta(cat, &mut rep.errors);
    // Parse errors are attributed to the key: critical ones make the locale unshipped.
    let mut parsed: Vec<(String, Parsed)> = Vec::new();
    for (l, flat) in &cat.locales {
        let mut ok = Parsed::new();
        for (k, m) in flat {
            match parse(m) {
                Ok(n) => ok.push((k.clone(), n)),
                Err(e) => {
                    let msg = format!("\"{k}\": parse error: {e}");
                    if l != MASTER && cat.critical.is_critical(k) {
                        rep.unshipped.push((l.clone(), msg));
                    } else {
                        rep.errors.push(format!("{l}: {msg}"));
                    }
                }
            }
        }
        parsed.push((l.clone(), ok));
    }
    let master_ast = &parsed.iter().find(|(l, _)| l == MASTER).expect("master").1;
    for (l, flat) in &cat.locales {
        if l == MASTER {
            continue;
        }
        for (k, _) in master {
            if lookup(flat, k).is_none() && cat.critical.is_critical(k) {
                rep.unshipped
                    .push((l.clone(), format!("missing critical key \"{k}\"")));
            }
        }
        for (k, _) in flat {
            if lookup(master, k).is_none() {
                rep.errors
                    .push(format!("{l}: extra key \"{k}\" (not in {MASTER})"));
            }
        }
    }
    for (l, msgs) in &parsed {
        let required = match plural_categories(l) {
            Ok(r) => r,
            Err(e) => {
                rep.errors.push(e);
                continue;
            }
        };
        for (key, nodes) in msgs {
            let critical = l != MASTER && cat.critical.is_critical(key);
            let mut problem = |msg: String| {
                if critical {
                    rep.unshipped.push((l.clone(), msg));
                } else {
                    rep.errors.push(format!("{l}: {msg}"));
                }
            };
            if l != MASTER
                && let Some((_, m)) = master_ast.iter().find(|(k, _)| k == key)
            {
                let (a, b) = (sig(m), sig(nodes));
                if a != b {
                    problem(format!(
                        "\"{key}\": arguments differ from {MASTER} ({MASTER}: [{a}], {l}: [{b}])"
                    ));
                }
            }
            for pl in plural_nodes(nodes) {
                let Node::Plural { name, cases } = pl else {
                    continue;
                };
                for c in required {
                    if !cases.iter().any(|x| x.sel == *c) {
                        problem(format!(
                            "\"{key}\": plural \"{name}\" lacks required category \"{c}\" for locale {l}"
                        ));
                    }
                }
            }
            if let Some(budget) = cat.critical.budget(key) {
                let len = rendered_len(nodes);
                if len > budget {
                    rep.errors.push(format!(
                        "{l}: \"{key}\": {len} characters exceed the critical length budget of {budget}"
                    ));
                }
            }
        }
    }
    rep
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::critical::Pattern;

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
        Catalogue::new(locales)
    }

    pub fn with_critical(mut c: Catalogue, p: &[(&str, usize)]) -> Catalogue {
        c.critical.patterns = p
            .iter()
            .map(|(pattern, budget)| Pattern {
                pattern: pattern.to_string(),
                budget: *budget,
            })
            .collect();
        c
    }

    #[test]
    fn consistent_catalogue_is_valid() {
        assert_eq!(validate(&cat(&[])), Report::default());
    }

    #[test]
    fn cldr_categories_per_locale_and_unknown_locale_fails() {
        assert_eq!(plural_categories("en").unwrap(), ["one", "other"]);
        assert_eq!(plural_categories("de").unwrap(), ["one", "other"]);
        assert_eq!(plural_categories("fr").unwrap(), ["one", "many", "other"]);
        assert_eq!(plural_categories("es").unwrap(), ["one", "many", "other"]);
        assert_eq!(plural_categories("pl").unwrap().len(), 4);
        assert_eq!(plural_categories("ar").unwrap().len(), 6);
        assert_eq!(plural_categories("ja").unwrap(), ["other"]);
        assert!(plural_categories("ru").is_err());
    }

    #[test]
    fn french_plural_without_many_is_rejected() {
        let rep = validate(&cat(&[(
            "fr",
            &[
                ("a.hello", "Bonjour {name}"),
                (
                    "a.items",
                    "{count, plural, one {# object} other {# objects}}",
                ),
            ],
        )]));
        let issues = rep.all();
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(issues[0].contains("fr: \"a.items\"") && issues[0].contains("\"many\""));
    }

    #[test]
    fn missing_non_critical_key_is_allowed_and_extra_key_fails() {
        let rep = validate(&cat(&[(
            "de",
            &[("a.hello", "Hallo {name}"), ("a.extra", "x")],
        )]));
        // a.items is missing but not critical: the locale still ships.
        assert!(rep.unshipped.is_empty(), "{rep:?}");
        assert!(
            rep.errors
                .iter()
                .any(|i| i.starts_with("de: extra key \"a.extra\""))
        );
        assert!(!rep.errors.iter().any(|i| i.contains("a.items")));
    }

    #[test]
    fn missing_critical_key_leaves_the_locale_unshipped_and_names_it() {
        let c = with_critical(
            cat(&[("de", &[("a.items", "{count} Dinge")])]),
            &[("a.hello", 40)],
        );
        // a.items in de has no plural but is not critical; only a.hello is.
        let rep = validate(&c);
        assert_eq!(
            rep.unshipped,
            [(
                "de".to_string(),
                "missing critical key \"a.hello\"".to_string()
            )]
        );
        assert_eq!(rep.unshipped_locales(), ["de"]);
        assert!(
            rep.all()
                .iter()
                .any(|i| i.contains("de: not shipped") && i.contains("a.hello"))
        );
        // The locale is dropped from generation; the others stay.
        let shipped = c.without(&rep.unshipped_locales());
        assert_eq!(shipped.tags(), ["en", "fr", "es"]);
    }

    #[test]
    fn critical_key_with_wrong_placeholder_or_plural_category_is_unshipped() {
        let items = "{count, plural, one {# Ding} other {# Dinge}}";
        let renamed = validate(&with_critical(
            cat(&[("de", &[("a.hello", "Hallo {nombre}"), ("a.items", items)])]),
            &[("a.hello", 40)],
        ));
        assert!(renamed.errors.is_empty());
        assert!(renamed.unshipped[0].1.contains("arguments differ"));
        let plural = validate(&with_critical(
            cat(&[(
                "fr",
                &[
                    ("a.hello", "Bonjour {name}"),
                    ("a.items", "{count, plural, one {# o} other {# os}}"),
                ],
            )]),
            &[("a.items", 40)],
        ));
        assert!(plural.errors.is_empty());
        assert_eq!(plural.unshipped_locales(), ["fr"]);
        assert!(plural.unshipped[0].1.contains("\"many\""));
    }

    #[test]
    fn missing_non_critical_key_does_not_unship() {
        let c = with_critical(
            cat(&[("de", &[("a.hello", "Hallo {name}")])]),
            &[("a.hello", 40)],
        );
        assert_eq!(validate(&c), Report::default());
    }

    #[test]
    fn stale_critical_pattern_is_named() {
        let rep = validate(&with_critical(
            cat(&[]),
            &[("a.hello", 40), ("a.renamed", 40)],
        ));
        assert_eq!(rep.errors.len(), 1, "{rep:?}");
        assert!(rep.errors[0].contains("pattern \"a.renamed\" matches no key"));
    }

    #[test]
    fn over_budget_value_fails_generation_per_locale() {
        let c = with_critical(
            cat(&[(
                "de",
                &[
                    ("a.hello", "Hallo {name} xxxxxxxxxxxxxxxxxxxxxxxxx"),
                    ("a.items", "{count, plural, one {# Ding} other {# Dinge}}"),
                ],
            )]),
            &[("a.hello", 20)],
        );
        let rep = validate(&c);
        assert_eq!(rep.errors.len(), 1, "{rep:?}");
        assert!(
            rep.errors[0].starts_with("de: \"a.hello\": ")
                && rep.errors[0].contains("budget of 20")
        );
    }

    #[test]
    fn meta_is_validated() {
        let mut c = cat(&[]);
        c.meta[1].1 = None;
        c.meta[2].1 = Some(Meta {
            default: Some("guessed".into()),
            reviewed: vec!["a.hello".into(), "a.gone".into()],
        });
        c.meta[0].1 = Some(Meta {
            default: None,
            reviewed: vec![],
        });
        let errors = validate(&c).errors;
        assert!(
            errors
                .iter()
                .any(|e| e.contains("en: \"$meta\" is reserved")),
            "{errors:?}"
        );
        assert!(errors.iter().any(|e| e.contains("de: missing \"$meta\"")));
        assert!(
            errors
                .iter()
                .any(|e| e.contains("fr: \"$meta.default\" is \"guessed\""))
        );
        assert!(errors.iter().any(|e| e.contains("unknown key \"a.gone\"")));
        assert!(!errors.iter().any(|e| e.contains("\"a.hello\"")));
    }

    #[test]
    fn argument_name_and_kind_mismatches() {
        let items = "{count, plural, one {# Ding} other {# Dinge}}";
        let renamed = validate(&cat(&[(
            "de",
            &[("a.hello", "Hallo {nombre}"), ("a.items", items)],
        )]))
        .all();
        assert!(
            renamed
                .iter()
                .any(|i| i.contains("de: \"a.hello\": arguments differ"))
        );
        let kind = validate(&cat(&[(
            "de",
            &[("a.hello", "Hallo {name}"), ("a.items", "{count} Dinge")],
        )]))
        .all();
        assert!(
            kind.iter()
                .any(|i| i.contains("de: \"a.items\": arguments differ"))
        );
    }

    #[test]
    fn parse_errors_name_locale_and_key() {
        let issues = validate(&cat(&[(
            "de",
            &[
                ("a.hello", "Hallo {name"),
                ("a.items", "{count, plural, one {x} other {y}}"),
            ],
        )]))
        .all();
        assert!(
            issues
                .iter()
                .any(|i| i.contains("de: \"a.hello\": parse error"))
        );
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

    #[test]
    fn load_reads_meta_and_critical_and_orders_en_first() {
        let dir = std::env::temp_dir().join(format!("i18n-gen-load-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let w = |n: &str, c: &str| std::fs::write(dir.join(n), c).unwrap();
        w("en.json", r#"{"a":{"b":"B"}}"#);
        w(
            "de.json",
            r#"{"$meta":{"default":"machineTranslated","reviewed":["a.b"]},"a":{"b":"Bee"}}"#,
        );
        w(
            "critical.json",
            r#"{"patterns":[{"pattern":"a.*","budget":9}]}"#,
        );
        let mut issues = Vec::new();
        let c = load(&dir, &mut issues);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(c.tags(), ["en", "de"]);
        assert_eq!(c.get("de"), &vec![("a.b".to_string(), "Bee".to_string())]);
        assert_eq!(c.meta[1].1.as_ref().unwrap().reviewed, ["a.b"]);
        assert!(c.meta[0].1.is_none());
        assert!(c.critical.is_critical("a.b"));
        assert!(validate(&c).is_ok());
    }
}
