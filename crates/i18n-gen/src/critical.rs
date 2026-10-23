//! `locales/critical.json`: which keys are safety copy and how long their values may be.
//!
//! Critical keys must exist in every shipped locale (an alarm is never mixed-language) and stay
//! short enough for a notification or a full-screen alert. The list is one reviewable file, so
//! the catalogues stay plain message JSON.

use crate::icu::Node;
use serde::Deserialize;

/// A placeholder or `#` counts as this many characters: a short name, a duration or a number.
pub const PLACEHOLDER_WIDTH: usize = 4;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pattern {
    /// An exact key (`native.alertNoiseTitle`) or a group prefix (`native.notif.*`).
    pub pattern: String,
    /// Maximum length in characters, measured on the `other` plural case.
    pub budget: usize,
}

impl Pattern {
    pub fn matches(&self, key: &str) -> bool {
        match self.pattern.strip_suffix(".*") {
            Some(prefix) => key
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with('.')),
            None => self.pattern == key,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Critical {
    pub patterns: Vec<Pattern>,
}

impl Critical {
    pub fn parse(json: &str) -> Result<Self, String> {
        let c: Critical = serde_json::from_str(json).map_err(|e| e.to_string())?;
        for p in &c.patterns {
            if p.pattern.is_empty() || p.pattern == "*" || p.pattern.contains(' ') {
                return Err(format!("invalid pattern \"{}\"", p.pattern));
            }
            if p.budget == 0 {
                return Err(format!("pattern \"{}\" has a budget of 0", p.pattern));
            }
        }
        Ok(c)
    }

    pub fn is_critical(&self, key: &str) -> bool {
        self.patterns.iter().any(|p| p.matches(key))
    }

    /// The tightest budget among the patterns that match the key.
    pub fn budget(&self, key: &str) -> Option<usize> {
        self.patterns
            .iter()
            .filter(|p| p.matches(key))
            .map(|p| p.budget)
            .min()
    }

    /// Patterns that match none of the given keys (a renamed or removed key).
    pub fn stale<'a>(&'a self, keys: &[&str]) -> Vec<&'a str> {
        self.patterns
            .iter()
            .filter(|p| !keys.iter().any(|k| p.matches(k)))
            .map(|p| p.pattern.as_str())
            .collect()
    }
}

/// Length of the message as shown, using the `other` case of plurals and selects.
pub fn rendered_len(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|n| match n {
            Node::Text(t) => t.chars().count(),
            Node::Arg(_) | Node::Pound => PLACEHOLDER_WIDTH,
            Node::Plural { cases, .. } | Node::Select { cases, .. } => cases
                .iter()
                .find(|c| c.sel == "other")
                .or_else(|| cases.first())
                .map_or(0, |c| rendered_len(&c.body)),
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icu::parse;

    fn crit(p: &[(&str, usize)]) -> Critical {
        Critical {
            patterns: p
                .iter()
                .map(|(pattern, budget)| Pattern {
                    pattern: pattern.to_string(),
                    budget: *budget,
                })
                .collect(),
        }
    }

    #[test]
    fn exact_and_prefix_patterns() {
        let c = crit(&[("a.b", 10), ("g.*", 20)]);
        assert!(c.is_critical("a.b") && c.is_critical("g.x") && c.is_critical("g.x.y"));
        assert!(!c.is_critical("a.bc") && !c.is_critical("a") && !c.is_critical("gx.y"));
        assert!(!c.is_critical("g"));
    }

    #[test]
    fn tightest_budget_wins() {
        let c = crit(&[("g.*", 40), ("g.title", 20)]);
        assert_eq!(c.budget("g.title"), Some(20));
        assert_eq!(c.budget("g.body"), Some(40));
        assert_eq!(c.budget("h.x"), None);
    }

    #[test]
    fn stale_patterns_are_reported() {
        let c = crit(&[("a.b", 10), ("g.*", 20), ("gone.*", 5)]);
        assert_eq!(c.stale(&["a.b", "g.x"]), ["gone.*"]);
    }

    #[test]
    fn parse_rejects_bad_files() {
        assert!(Critical::parse(r#"{"patterns":[{"pattern":"a.b","budget":3}]}"#).is_ok());
        assert!(Critical::parse(r#"{"patterns":[{"pattern":"a.b","budget":0}]}"#).is_err());
        assert!(Critical::parse(r#"{"patterns":[{"pattern":"*","budget":3}]}"#).is_err());
        assert!(Critical::parse(r#"{"patterns":[{"pattern":"a","budget":3,"x":1}]}"#).is_err());
    }

    #[test]
    fn length_uses_other_case_and_placeholder_width() {
        let n = parse("ab {x} {n, plural, one {# sound} other {# sounds}}").unwrap();
        // "ab " (3) + {x} (4) + " " (1) + "# sounds" (4 + 7)
        assert_eq!(rendered_len(&n), 3 + 4 + 1 + 4 + 7);
    }
}
