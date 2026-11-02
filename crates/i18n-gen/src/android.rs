use crate::catalogue::{ANDROID_GROUPS, Catalogue, MASTER, parse_all};
use crate::icu::{ArgKind, Case, Node, collect_args, has_select, plural_nodes};

const QUANTITIES: [&str; 6] = ["zero", "one", "two", "few", "many", "other"];

pub fn android_name(key: &str) -> String {
    key.split('.')
        .map(|seg| {
            let mut out = String::new();
            let mut prev: Option<char> = None;
            for c in seg.chars() {
                if c.is_ascii_uppercase()
                    && prev.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit())
                {
                    out.push('_');
                }
                out.push(c.to_ascii_lowercase());
                prev = Some(c);
            }
            out
        })
        .collect::<Vec<_>>()
        .join("_")
}

/// Resource directory of a locale tag: `values-de`, `values-b+zh+Hans` (BCP 47 for tags with a
/// script or region).
pub fn values_dir(locale: &str) -> String {
    if locale == MASTER {
        "values".to_string()
    } else if locale.contains('-') {
        format!("values-b+{}", locale.replace('-', "+"))
    } else {
        format!("values-{locale}")
    }
}

pub fn is_android_key(key: &str) -> bool {
    ANDROID_GROUPS.contains(&key.split('.').next().unwrap_or(""))
}

pub fn escape_text(s: &str, formatted: bool) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '"' => out.push_str("\\\""),
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '%' if formatted => out.push_str("%%"),
            _ => out.push(c),
        }
    }
    out
}

struct Ctx<'a> {
    args: &'a [(String, ArgKind)],
    formatted: bool,
}

fn spec(ctx: &Ctx, name: &str) -> Result<String, String> {
    let i = ctx
        .args
        .iter()
        .position(|(n, _)| n == name)
        .ok_or_else(|| format!("unknown argument \"{name}\""))?;
    let t = if ctx.args[i].1 == ArgKind::Plural {
        'd'
    } else {
        's'
    };
    Ok(format!("%{}${t}", i + 1))
}

/// Returns (escaped text, plain text used for whitespace decisions).
fn render(nodes: &[Node], ctx: &Ctx, count: Option<&str>) -> Result<(String, String), String> {
    let (mut text, mut plain) = (String::new(), String::new());
    for n in nodes {
        match n {
            Node::Text(t) => {
                text.push_str(&escape_text(t, ctx.formatted));
                plain.push_str(t);
            }
            Node::Arg(name) => {
                let s = spec(ctx, name)?;
                text.push_str(&s);
                plain.push_str(&s);
            }
            Node::Pound => {
                let s = spec(ctx, count.ok_or("# outside plural")?)?;
                text.push_str(&s);
                plain.push_str(&s);
            }
            _ => return Err("nested plural/select".into()),
        }
    }
    Ok((text, plain))
}

fn finish((text, plain): (String, String)) -> String {
    let mut t = text;
    if t.starts_with('@') || t.starts_with('?') {
        t.insert(0, '\\');
    }
    let chars: Vec<char> = plain.chars().collect();
    let needs_quote = chars.first().is_some_and(|c| c.is_whitespace())
        || chars.last().is_some_and(|c| c.is_whitespace())
        || chars
            .windows(2)
            .any(|w| w[0].is_whitespace() && w[1].is_whitespace());
    if needs_quote {
        t = format!("\"{t}\"");
    }
    t
}

/// `values[-xx]/strings.xml` contents per locale; problems are pushed into `issues`.
pub fn render_android(cat: &Catalogue, issues: &mut Vec<String>) -> Vec<(String, String)> {
    let master_flat = cat.get(MASTER);
    let master_keys: Vec<&String> = master_flat
        .iter()
        .map(|(k, _)| k)
        .filter(|k| is_android_key(k))
        .collect();
    let mut names: Vec<(String, &String)> = Vec::new();
    for key in &master_keys {
        let name = android_name(key);
        if let Some((_, prev)) = names.iter().find(|(n, _)| *n == name) {
            issues.push(format!(
                "android: resource name \"{name}\" is produced by both \"{prev}\" and \"{key}\""
            ));
        } else {
            names.push((name, key));
        }
    }
    let master_ast = parse_all(master_flat, MASTER, &mut Vec::new());
    let mut files = Vec::new();
    for l in cat.tags() {
        let ast = parse_all(cat.get(l), l, &mut Vec::new());
        let mut lines: Vec<String> = Vec::new();
        for key in &master_keys {
            let (Some((_, nodes)), Some((_, mnodes))) = (
                ast.iter().find(|(k, _)| k == *key),
                master_ast.iter().find(|(k, _)| k == *key),
            ) else {
                continue;
            };
            let name = android_name(key);
            let args = collect_args(mnodes);
            let mut fail = |m: &str| issues.push(format!("{l}: \"{key}\": {m}"));
            if has_select(nodes) {
                fail("select is not supported on Android (move it to a web-only group)");
                continue;
            }
            let plurals = plural_nodes(nodes);
            if plurals.is_empty() {
                let formatted = !args.is_empty();
                match render(
                    nodes,
                    &Ctx {
                        args: &args,
                        formatted,
                    },
                    None,
                ) {
                    Ok(r) => {
                        let body = finish(r);
                        let attr = if formatted || !body.contains('%') {
                            ""
                        } else {
                            " formatted=\"false\""
                        };
                        lines.push(format!("    <string name=\"{name}\"{attr}>{body}</string>"));
                    }
                    Err(e) => fail(&e),
                }
                continue;
            }
            let top = nodes.iter().position(|n| matches!(n, Node::Plural { .. }));
            let (Some(top), 1) = (top, plurals.len()) else {
                fail("Android supports exactly one top-level plural per message");
                continue;
            };
            let Node::Plural { name: pname, cases } = &nodes[top] else {
                continue;
            };
            if let Some(bad) = cases.iter().find(|c| !QUANTITIES.contains(&c.sel.as_str())) {
                fail(&format!(
                    "Android plurals do not support selector \"{}\"",
                    bad.sel
                ));
                continue;
            }
            let mut items = Vec::new();
            for Case { sel, body } in cases {
                let mut merged: Vec<Node> = nodes[..top].to_vec();
                merged.extend(body.iter().cloned());
                merged.extend(nodes[top + 1..].iter().cloned());
                match render(
                    &merged,
                    &Ctx {
                        args: &args,
                        formatted: true,
                    },
                    Some(pname),
                ) {
                    Ok(r) => items.push(format!(
                        "        <item quantity=\"{sel}\">{}</item>",
                        finish(r)
                    )),
                    Err(e) => fail(&e),
                }
            }
            lines.push(format!(
                "    <plurals name=\"{name}\">\n{}\n    </plurals>",
                items.join("\n")
            ));
        }
        // Keys a locale lacks (non-critical only) fall back to `values/` at runtime; lint must not
        // treat that as an error.
        let attrs = if l == MASTER {
            ""
        } else {
            " xmlns:tools=\"http://schemas.android.com/tools\" tools:ignore=\"MissingTranslation\""
        };
        files.push((
            format!("{}/strings.xml", values_dir(l)),
            format!(
                "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!-- Generated by crates/i18n-gen from locales/*.json. Do not edit. -->\n<resources{attrs}>\n{}\n</resources>\n",
                lines.join("\n")
            ),
        ));
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(en: &[(&str, &str)], other: &[(&str, &str)]) -> Catalogue {
        let f = |v: &[(&str, &str)]| {
            v.iter()
                .map(|(k, m)| (k.to_string(), m.to_string()))
                .collect::<Vec<_>>()
        };
        Catalogue::new(vec![
            ("en".into(), f(en)),
            ("de".into(), f(other)),
            ("fr".into(), f(other)),
            ("es".into(), f(other)),
        ])
    }

    fn xml(en: &[(&str, &str)], locale_file: &str) -> (String, Vec<String>) {
        let mut issues = Vec::new();
        let files = render_android(&mk(en, en), &mut issues);
        (
            files.into_iter().find(|(p, _)| p == locale_file).unwrap().1,
            issues,
        )
    }

    #[test]
    fn resource_name_mapping() {
        assert_eq!(
            android_name("native.alertNoiseTitle"),
            "native_alert_noise_title"
        );
        assert_eq!(android_name("shared.a2B.cD"), "shared_a2_b_c_d");
    }

    #[test]
    fn values_dir_uses_bcp47_for_scripts() {
        assert_eq!(values_dir("en"), "values");
        assert_eq!(values_dir("de"), "values-de");
        assert_eq!(values_dir("zh-Hans"), "values-b+zh+Hans");
    }

    #[test]
    fn missing_non_critical_keys_fall_back_to_the_default_resources() {
        let files = render_android(
            &mk(
                &[("native.a", "A"), ("native.b", "B")],
                &[("native.a", "Ah")],
            ),
            &mut Vec::new(),
        );
        let de = &files
            .iter()
            .find(|(p, _)| p == "values-de/strings.xml")
            .unwrap()
            .1;
        assert!(de.contains("native_a") && !de.contains("native_b"), "{de}");
        assert!(de.contains("tools:ignore=\"MissingTranslation\""), "{de}");
        let en = &files
            .iter()
            .find(|(p, _)| p == "values/strings.xml")
            .unwrap()
            .1;
        assert!(!en.contains("tools:ignore") && en.contains("native_b"));
    }

    #[test]
    fn escaping() {
        assert_eq!(
            escape_text("it's \"q\" & <b>\nx\\", false),
            "it\\'s \\\"q\\\" &amp; &lt;b&gt;\\nx\\\\"
        );
        assert_eq!(escape_text("50%", true), "50%%");
        assert_eq!(escape_text("50%", false), "50%");
    }

    #[test]
    fn named_arguments_become_positional_in_en_order() {
        let (x, issues) = xml(
            &[("native.t", "{b} then {a} then {b}")],
            "values/strings.xml",
        );
        assert!(issues.is_empty());
        assert!(
            x.contains("<string name=\"native_t\">%1$s then %2$s then %1$s</string>"),
            "{x}"
        );
    }

    #[test]
    fn other_locales_reuse_en_positions_when_reordered() {
        let files = render_android(
            &mk(&[("native.t", "{a} {b}")], &[("native.t", "{b} {a}")]),
            &mut Vec::new(),
        );
        let de = &files
            .iter()
            .find(|(p, _)| p == "values-de/strings.xml")
            .unwrap()
            .1;
        assert!(de.contains(">%2$s %1$s<"), "{de}");
    }

    #[test]
    fn plurals_expand_with_prefix_suffix_and_d_for_count() {
        let msg = "Got {count, plural, one {# packet} other {# packets}} from {who}.";
        let (x, issues) = xml(&[("native.p", msg)], "values/strings.xml");
        assert!(issues.is_empty(), "{issues:?}");
        assert!(x.contains("<plurals name=\"native_p\">"));
        assert!(
            x.contains("<item quantity=\"one\">Got %1$d packet from %2$s.</item>"),
            "{x}"
        );
        assert!(x.contains("<item quantity=\"other\">Got %1$d packets from %2$s.</item>"));
    }

    #[test]
    fn select_exact_match_are_errors_and_web_group_is_skipped() {
        let (_, issues) = xml(
            &[
                ("native.s", "{k, select, a {A} other {B}}"),
                ("native.e", "{n, plural, =0 {none} other {# x}}"),
                ("web.s", "{k, select, a {A} other {B}}"),
            ],
            "values/strings.xml",
        );
        assert!(
            issues
                .iter()
                .any(|i| i.contains("\"native.s\": select is not supported"))
        );
        assert!(
            issues
                .iter()
                .any(|i| i.contains("\"native.e\"") && i.contains("=0"))
        );
        assert!(!issues.iter().any(|i| i.contains("web.s")));
    }

    #[test]
    fn leading_at_question_and_surrounding_whitespace_are_protected() {
        let (x, _) = xml(
            &[
                ("native.a", "@home"),
                ("native.q", "?!"),
                ("native.w", " pad "),
            ],
            "values/strings.xml",
        );
        assert!(
            x.contains(">\\@home<") && x.contains(">\\?!<") && x.contains(">\" pad \"<"),
            "{x}"
        );
    }

    #[test]
    fn literal_percent_without_arguments_is_not_a_format() {
        let (x, _) = xml(&[("native.pc", "100% sure")], "values/strings.xml");
        assert!(
            x.contains("<string name=\"native_pc\" formatted=\"false\">100% sure</string>"),
            "{x}"
        );
    }

    #[test]
    fn resource_name_collisions_are_reported() {
        let (_, issues) = xml(
            &[("native.aB.c", "x"), ("native.a.bC", "y")],
            "values/strings.xml",
        );
        assert!(
            issues
                .iter()
                .any(|i| i.contains("native_a_b_c") && i.contains("both")),
            "{issues:?}"
        );
    }
}
