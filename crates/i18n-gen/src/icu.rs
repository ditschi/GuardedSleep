//! Fixed ICU subset: literal text, `{arg}`, plural, select, `#`, apostrophe quoting. Nothing else.

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Text(String),
    Arg(String),
    Pound,
    Plural { name: String, cases: Vec<Case> },
    Select { name: String, cases: Vec<Case> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Case {
    pub sel: String,
    pub body: Vec<Node>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgKind {
    Simple,
    Plural,
    Select,
}

impl ArgKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ArgKind::Simple => "simple",
            ArgKind::Plural => "plural",
            ArgKind::Select => "select",
        }
    }
}

pub fn parse(src: &str) -> Result<Vec<Node>, String> {
    let mut p = Parser {
        s: src.chars().collect(),
        pos: 0,
    };
    p.message(false, true)
}

struct Parser {
    s: Vec<char>,
    pos: usize,
}

fn is_name(w: &str) -> bool {
    let mut it = w.chars();
    matches!(it.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && it.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_select_key(w: &str) -> bool {
    !w.is_empty() && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_plural_selector(w: &str) -> bool {
    matches!(w, "zero" | "one" | "two" | "few" | "many" | "other")
        || (w.len() > 1 && w.starts_with('=') && w[1..].chars().all(|c| c.is_ascii_digit()))
}

impl Parser {
    fn fail(&self, msg: &str) -> String {
        format!("{msg} at offset {}", self.pos)
    }

    fn peek(&self, off: usize) -> Option<char> {
        self.s.get(self.pos + off).copied()
    }

    fn ws(&mut self) {
        while self.peek(0).is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }

    fn word(&mut self, ok: fn(&str) -> bool, what: &str) -> Result<String, String> {
        self.ws();
        let start = self.pos;
        while self
            .peek(0)
            .is_some_and(|c| !c.is_whitespace() && !matches!(c, ',' | '{' | '}'))
        {
            self.pos += 1;
        }
        let w: String = self.s[start..self.pos].iter().collect();
        if w.is_empty() || !ok(&w) {
            return Err(self.fail(&format!("invalid {what} \"{w}\"")));
        }
        Ok(w)
    }

    fn message(&mut self, in_plural: bool, top: bool) -> Result<Vec<Node>, String> {
        let mut out = Vec::new();
        let mut text = String::new();
        fn flush(out: &mut Vec<Node>, text: &mut String) {
            if !text.is_empty() {
                out.push(Node::Text(std::mem::take(text)));
            }
        }
        while let Some(c) = self.peek(0) {
            match c {
                '}' => {
                    if top {
                        return Err(self.fail("unmatched \"}\""));
                    }
                    break;
                }
                '{' => {
                    flush(&mut out, &mut text);
                    out.push(self.argument(in_plural)?);
                }
                '#' if in_plural => {
                    flush(&mut out, &mut text);
                    out.push(Node::Pound);
                    self.pos += 1;
                }
                '\'' => match self.peek(1) {
                    Some('\'') => {
                        text.push('\'');
                        self.pos += 2;
                    }
                    Some('{') | Some('}') => self.quoted(&mut text)?,
                    Some('#') if in_plural => self.quoted(&mut text)?,
                    _ => {
                        text.push('\'');
                        self.pos += 1;
                    }
                },
                _ => {
                    text.push(c);
                    self.pos += 1;
                }
            }
        }
        flush(&mut out, &mut text);
        Ok(out)
    }

    fn quoted(&mut self, text: &mut String) -> Result<(), String> {
        self.pos += 1;
        loop {
            match self.peek(0) {
                None => return Err(self.fail("unterminated apostrophe quote")),
                Some('\'') if self.peek(1) == Some('\'') => {
                    text.push('\'');
                    self.pos += 2;
                }
                Some('\'') => {
                    self.pos += 1;
                    return Ok(());
                }
                Some(c) => {
                    text.push(c);
                    self.pos += 1;
                }
            }
        }
    }

    fn argument(&mut self, in_plural: bool) -> Result<Node, String> {
        self.pos += 1;
        let name = self.word(is_name, "argument name")?;
        self.ws();
        match self.peek(0) {
            Some('}') => {
                self.pos += 1;
                return Ok(Node::Arg(name));
            }
            Some(',') => self.pos += 1,
            _ => return Err(self.fail("expected \",\" or \"}\"")),
        }
        self.ws();
        let start = self.pos;
        while self.peek(0).is_some_and(|c| c.is_ascii_alphabetic()) {
            self.pos += 1;
        }
        let kind: String = self.s[start..self.pos].iter().collect();
        let is_plural = match kind.as_str() {
            "plural" => true,
            "select" => false,
            _ => {
                return Err(self.fail(&format!(
                    "unsupported argument type \"{kind}\" (only plural and select)"
                )));
            }
        };
        self.ws();
        if self.peek(0) != Some(',') {
            return Err(self.fail("expected \",\" after argument type"));
        }
        self.pos += 1;
        let mut cases: Vec<Case> = Vec::new();
        loop {
            self.ws();
            match self.peek(0) {
                Some('}') => {
                    self.pos += 1;
                    break;
                }
                None => return Err(self.fail("unterminated argument")),
                _ => {}
            }
            let sel = if is_plural {
                self.word(is_plural_selector, "plural selector")?
            } else {
                self.word(is_select_key, "select selector")?
            };
            if cases.iter().any(|c| c.sel == sel) {
                return Err(self.fail(&format!("duplicate selector \"{sel}\"")));
            }
            self.ws();
            if self.peek(0) != Some('{') {
                return Err(self.fail(&format!("expected \"{{\" after selector \"{sel}\"")));
            }
            self.pos += 1;
            let body = self.message(is_plural || in_plural, false)?;
            if self.peek(0) != Some('}') {
                return Err(self.fail(&format!("unterminated case \"{sel}\"")));
            }
            self.pos += 1;
            cases.push(Case { sel, body });
        }
        if !cases.iter().any(|c| c.sel == "other") {
            return Err(self.fail(&format!("{kind} \"{name}\" is missing the \"other\" case")));
        }
        Ok(if is_plural {
            Node::Plural { name, cases }
        } else {
            Node::Select { name, cases }
        })
    }
}

/// Argument names with their kind, in order of first appearance.
pub fn collect_args(nodes: &[Node]) -> Vec<(String, ArgKind)> {
    fn add(into: &mut Vec<(String, ArgKind)>, name: &str, kind: ArgKind) {
        match into.iter_mut().find(|(n, _)| n == name) {
            None => into.push((name.to_string(), kind)),
            Some((_, k)) if *k == ArgKind::Simple => *k = kind,
            Some(_) => {}
        }
    }
    fn walk(nodes: &[Node], into: &mut Vec<(String, ArgKind)>) {
        for n in nodes {
            match n {
                Node::Arg(name) => add(into, name, ArgKind::Simple),
                Node::Plural { name, cases } | Node::Select { name, cases } => {
                    let kind = if matches!(n, Node::Plural { .. }) {
                        ArgKind::Plural
                    } else {
                        ArgKind::Select
                    };
                    add(into, name, kind);
                    for c in cases {
                        walk(&c.body, into);
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(nodes, &mut out);
    out
}

/// All plural nodes in the message, nested ones included.
pub fn plural_nodes(nodes: &[Node]) -> Vec<&Node> {
    let mut out = Vec::new();
    fn walk<'a>(nodes: &'a [Node], out: &mut Vec<&'a Node>) {
        for n in nodes {
            match n {
                Node::Plural { cases, .. } => {
                    out.push(n);
                    cases.iter().for_each(|c| walk(&c.body, out));
                }
                Node::Select { cases, .. } => cases.iter().for_each(|c| walk(&c.body, out)),
                _ => {}
            }
        }
    }
    walk(nodes, &mut out);
    out
}

pub fn has_select(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Select { .. } => true,
        Node::Plural { cases, .. } => cases.iter().any(|c| has_select(&c.body)),
        _ => false,
    })
}

pub fn serialize(nodes: &[Node], in_plural: bool) -> String {
    let mut out = String::new();
    for n in nodes {
        match n {
            Node::Text(t) => {
                for c in t.chars() {
                    match c {
                        '\'' => out.push_str("''"),
                        '{' | '}' => {
                            out.push('\'');
                            out.push(c);
                            out.push('\'');
                        }
                        '#' if in_plural => out.push_str("'#'"),
                        _ => out.push(c),
                    }
                }
            }
            Node::Arg(name) => out.push_str(&format!("{{{name}}}")),
            Node::Pound => out.push('#'),
            Node::Plural { name, cases } | Node::Select { name, cases } => {
                let (kind, inner) = if matches!(n, Node::Plural { .. }) {
                    ("plural", true)
                } else {
                    ("select", in_plural)
                };
                let body: Vec<String> = cases
                    .iter()
                    .map(|c| format!("{} {{{}}}", c.sel, serialize(&c.body, inner)))
                    .collect();
                out.push_str(&format!("{{{name}, {kind}, {}}}", body.join(" ")));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> Node {
        Node::Text(s.into())
    }

    #[test]
    fn parses_literal_text_and_arguments() {
        assert_eq!(
            parse("Hi {name}!").unwrap(),
            vec![text("Hi "), Node::Arg("name".into()), text("!")]
        );
    }

    #[test]
    fn apostrophes_double_lone_and_quoted() {
        assert_eq!(
            parse("it''s l'unité '{x}' ok").unwrap(),
            vec![text("it's l'unité {x} ok")]
        );
        assert_eq!(parse("a'#b").unwrap(), vec![text("a'#b")]);
    }

    #[test]
    fn plural_with_exact_match_and_pound() {
        let nodes = parse("{count, plural, =0 {none} one {# item} other {# items}}").unwrap();
        let Node::Plural { cases, .. } = &nodes[0] else {
            panic!("not a plural")
        };
        let sels: Vec<&str> = cases.iter().map(|c| c.sel.as_str()).collect();
        assert_eq!(sels, ["=0", "one", "other"]);
        assert_eq!(cases[1].body[0], Node::Pound);
    }

    #[test]
    fn pound_is_literal_outside_plural_and_quotable_inside() {
        assert_eq!(
            parse("No. #{n}").unwrap(),
            vec![text("No. #"), Node::Arg("n".into())]
        );
        let nodes = parse("{c, plural, other {'#' #}}").unwrap();
        let Node::Plural { cases, .. } = &nodes[0] else {
            panic!()
        };
        assert_eq!(cases[0].body, vec![text("# "), Node::Pound]);
    }

    #[test]
    fn select_needs_other() {
        assert!(parse("{kind, select, baby {Baby} other {Parent}}").is_ok());
        assert!(
            parse("{kind, select, baby {Baby}}")
                .unwrap_err()
                .contains("other")
        );
    }

    #[test]
    fn rejects_everything_outside_the_subset() {
        for bad in [
            "{n, number}",
            "{d, date, short}",
            "{n, selectordinal, other {x}}",
            "{n, plural, offset:1 other {x}}",
            "{n, plural, one {x}}",
            "{n, plural, other {x} other {y}}",
            "{n, plural, many2 {x} other {y}}",
            "{ unclosed",
            "stray }",
            "{}",
            "{1bad}",
            "'{unterminated",
        ] {
            assert!(parse(bad).is_err(), "{bad} should not parse");
        }
    }

    #[test]
    fn collects_arg_kinds_in_order_of_first_appearance() {
        let nodes =
            parse("{b} {n, plural, one {# x {c}} other {y}} {k, select, a {1} other {2}}").unwrap();
        let got: Vec<(String, &str)> = collect_args(&nodes)
            .into_iter()
            .map(|(n, k)| (n, k.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                ("b".into(), "simple"),
                ("n".into(), "plural"),
                ("c".into(), "simple"),
                ("k".into(), "select")
            ]
        );
    }

    #[test]
    fn serialize_round_trips() {
        for src in [
            "l''unité {x}",
            "{n, plural, =0 {none} one {# one} other {# many '#'}}",
            "{k, select, a {A '{'} other {B}}",
        ] {
            let nodes = parse(src).unwrap();
            assert_eq!(parse(&serialize(&nodes, false)).unwrap(), nodes, "{src}");
        }
    }
}
