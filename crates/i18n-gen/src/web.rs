use crate::catalogue::{Catalogue, Flat, LOCALES, MASTER};
use crate::icu::{parse, serialize, Case, Node};
use serde_json::{Map, Value};

fn accent(c: char) -> char {
    const LOWER: &str = "áƀçðéƒĝĥíĵķļɱñöþǫŕšţüṽŵẋýž";
    const UPPER: &str = "ÁƁÇÐÉƑĜĤÍĴĶĻṀÑÖÞǪŔŠŢÜṼŴẊÝŽ";
    if c.is_ascii_lowercase() {
        LOWER.chars().nth((c as u8 - b'a') as usize).unwrap_or(c)
    } else if c.is_ascii_uppercase() {
        UPPER.chars().nth((c as u8 - b'A') as usize).unwrap_or(c)
    } else {
        c
    }
}

fn pseudo_nodes(nodes: &[Node]) -> Vec<Node> {
    let mut visible = 0;
    let mut out: Vec<Node> = nodes
        .iter()
        .map(|n| match n {
            Node::Text(t) => {
                visible += t.chars().count();
                Node::Text(t.chars().map(accent).collect())
            }
            Node::Plural { name, cases } => Node::Plural {
                name: name.clone(),
                cases: pseudo_cases(cases),
            },
            Node::Select { name, cases } => Node::Select {
                name: name.clone(),
                cases: pseudo_cases(cases),
            },
            other => other.clone(),
        })
        .collect();
    if visible > 0 {
        out.push(Node::Text("~".repeat((visible * 2).div_ceil(5))));
    }
    out
}

fn pseudo_cases(cases: &[Case]) -> Vec<Case> {
    cases
        .iter()
        .map(|c| Case {
            sel: c.sel.clone(),
            body: pseudo_nodes(&c.body),
        })
        .collect()
}

/// Accented, ~40 % longer, wrapped in brackets; arguments and ICU syntax are preserved.
pub fn pseudo_message(msg: &str) -> Result<String, String> {
    let mut nodes = vec![Node::Text("⟦".into())];
    nodes.extend(pseudo_nodes(&parse(msg)?));
    nodes.push(Node::Text("⟧".into()));
    Ok(serialize(&nodes, false))
}

/// Rebuilds the nested catalogue shape from flat dot paths, preserving order.
pub fn nest(flat: &Flat, map: &dyn Fn(&str) -> Result<String, String>) -> Result<Value, String> {
    let mut root = Map::new();
    for (path, msg) in flat {
        let parts: Vec<&str> = path.split('.').collect();
        let mut cur = &mut root;
        for p in &parts[..parts.len() - 1] {
            cur = cur
                .entry(p.to_string())
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .ok_or_else(|| format!("key \"{path}\" collides with a message"))?;
        }
        cur.insert(parts[parts.len() - 1].to_string(), Value::String(map(msg)?));
    }
    Ok(Value::Object(root))
}

fn pretty(v: &Value) -> String {
    format!(
        "{}\n",
        serde_json::to_string_pretty(v).expect("serializable")
    )
}

/// File name to JSON text: one per locale plus `pseudo.json`.
pub fn render_web(cat: &Catalogue) -> Result<Vec<(String, String)>, String> {
    let mut files = Vec::new();
    for l in LOCALES {
        files.push((
            format!("{l}.json"),
            pretty(&nest(cat.get(l), &|s| Ok(s.to_string()))?),
        ));
    }
    files.push((
        "pseudo.json".into(),
        pretty(&nest(cat.get(MASTER), &pseudo_message)?),
    ));
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icu::collect_args;

    #[test]
    fn pseudo_keeps_icu_structure_and_wraps() {
        let src = "Hello {name}: {n, plural, one {# apple} other {# apples}} l'été";
        let out = pseudo_message(src).unwrap();
        assert!(out.starts_with('⟦') && out.ends_with('⟧'));
        assert_eq!(
            collect_args(&parse(&out).unwrap()),
            collect_args(&parse(src).unwrap())
        );
        assert!(out.contains("{name}") && out.contains("plural"));
        assert!(!out.contains("Hello"));
    }

    #[test]
    fn pseudo_is_about_forty_percent_longer() {
        let src = "A reasonably long sentence for measuring";
        let ratio =
            pseudo_message(src).unwrap().chars().count() as f64 / src.chars().count() as f64;
        assert!((1.4..1.6).contains(&ratio), "{ratio}");
    }

    #[test]
    fn nest_rebuilds_shape_and_web_output_lists_all_files() {
        let flat: Flat = vec![
            ("a.b".into(), "x".into()),
            ("a.c".into(), "y".into()),
            ("d".into(), "z".into()),
        ];
        let v = nest(&flat, &|s| Ok(s.to_string())).unwrap();
        assert_eq!(v, serde_json::json!({"a": {"b": "x", "c": "y"}, "d": "z"}));
        let one: Flat = vec![("a.b".into(), "x {n}".into())];
        let cat = Catalogue {
            locales: LOCALES
                .iter()
                .map(|l| (l.to_string(), one.clone()))
                .collect(),
        };
        let names: Vec<String> = render_web(&cat)
            .unwrap()
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert_eq!(
            names,
            ["en.json", "de.json", "fr.json", "es.json", "pseudo.json"]
        );
    }
}
