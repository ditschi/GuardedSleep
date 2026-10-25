//! Unused-key check: a catalogue key must be referenced by a client or be a core message key.
//!
//! Usage rules (openspec `expand-locales`, spec "No unused keys"):
//! - Android: a `R.string.<name>` / `R.plurals.<name>` / `@string/<name>` / `@plurals/<name>`
//!   reference in `apps/android` source (build output excluded) uses the key whose generated
//!   resource name is `<name>`. Only keys of the Android groups can be used this way.
//! - Web: a quoted literal equal to the dot path in `apps/web/src` (test files and the generated
//!   bundles excluded) uses the key. This covers `t("a.b")` and the typed key constants and maps
//!   (`AMBIENT_KEY`, `LANGUAGE_LABELS`, ...), because `MessageKey` forces every key to be written
//!   out as a literal. A key assembled from parts is invisible to the scan, so the clients must not
//!   build keys by concatenation; the one rule-based mapping is `core.*` below.
//! - Core: every key of the `core` group is used. The core derives it from `message_keys()`
//!   (`core.<camelCase>`, a core integration test fails when one is missing), and the clients look
//!   it up by that rule, so no literal exists to find.

use crate::android::{android_name, is_android_key};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Group whose keys are looked up by rule from the core's message keys.
pub const CORE_GROUP: &str = "core";

const ANDROID_PREFIXES: [&str; 4] = ["R.string.", "R.plurals.", "@string/", "@plurals/"];

/// Files below `dir` whose name satisfies `keep`, skipping build output and generated bundles.
fn files(dir: &Path, keep: &dyn Fn(&str) -> bool, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !matches!(
                name.as_str(),
                "build" | "node_modules" | "generated" | ".gradle"
            ) {
                files(&path, keep, out);
            }
        } else if keep(&name) {
            out.push(path);
        }
    }
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Resource names referenced in one source text.
fn android_refs_in(src: &str, out: &mut HashSet<String>) {
    for prefix in ANDROID_PREFIXES {
        for (i, _) in src.match_indices(prefix) {
            let name: String = src[i + prefix.len()..]
                .chars()
                .take_while(|c| is_name_char(*c))
                .collect();
            if !name.is_empty() {
                out.insert(name);
            }
        }
    }
}

/// Resource names referenced anywhere in the Android project sources.
pub fn android_refs(android_dir: &Path) -> HashSet<String> {
    let mut paths = Vec::new();
    files(
        android_dir,
        &|n| n.ends_with(".kt") || n.ends_with(".java") || n.ends_with(".xml"),
        &mut paths,
    );
    let mut out = HashSet::new();
    for p in paths {
        if let Ok(src) = fs::read_to_string(&p) {
            android_refs_in(&src, &mut out);
        }
    }
    out
}

/// Quoted dotted tokens (`"a.b.c"`) of one source text.
fn web_literals_in(src: &str, out: &mut HashSet<String>) {
    let mut chars = src.chars();
    while let Some(q) = chars.next() {
        if !matches!(q, '"' | '\'' | '`') {
            continue;
        }
        let mut tok = String::new();
        let mut closed = false;
        for c in chars.by_ref() {
            if c == q {
                closed = true;
                break;
            }
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
                tok.push(c);
            } else {
                break;
            }
        }
        if closed && tok.contains('.') {
            out.insert(tok);
        }
    }
}

/// Dotted string literals in the web client sources (tests and generated bundles excluded).
pub fn web_literals(web_src: &Path) -> HashSet<String> {
    let mut paths = Vec::new();
    let keep = |n: &str| {
        (n.ends_with(".ts") || n.ends_with(".svelte"))
            && !n.ends_with(".test.ts")
            && !n.ends_with(".spec.ts")
            && !n.ends_with(".d.ts")
    };
    files(web_src, &keep, &mut paths);
    let mut out = HashSet::new();
    for p in paths {
        if let Ok(src) = fs::read_to_string(&p) {
            web_literals_in(&src, &mut out);
        }
    }
    out
}

/// Where each client references keys.
pub struct Usage {
    pub android: HashSet<String>,
    pub web: HashSet<String>,
}

impl Usage {
    pub fn scan(android_dir: &Path, web_src: &Path) -> Self {
        Usage {
            android: android_refs(android_dir),
            web: web_literals(web_src),
        }
    }

    pub fn is_used(&self, key: &str) -> bool {
        key.split('.').next() == Some(CORE_GROUP)
            || self.web.contains(key)
            || (is_android_key(key) && self.android.contains(&android_name(key)))
    }
}

/// Problems for every master key that nothing references.
pub fn unused_keys<'a>(keys: impl Iterator<Item = &'a str>, usage: &Usage) -> Vec<String> {
    keys.filter(|k| !usage.is_used(k))
        .map(|k| {
            format!(
                "unused key \"{k}\" (no Android resource, web literal or core message key uses it; delete it)"
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    fn fixture() -> (tempdir::Dir, Usage) {
        let d = tempdir::Dir::new();
        write(
            d.path(),
            "android/app/src/main/kotlin/A.kt",
            "Text(stringResource(R.string.native_ui_title))\n\
             pluralStringResource(R.plurals.native_ui_missed, n, n)\n",
        );
        write(
            d.path(),
            "android/app/src/main/res/xml/c.xml",
            "<x android:label=\"@string/native_ui_label\"/>",
        );
        // Build output is not source: its references do not count.
        write(
            d.path(),
            "android/app/build/gen/B.kt",
            "R.string.native_ui_only_in_build",
        );
        write(
            d.path(),
            "web/src/Screen.svelte",
            "<p>{$t(\"web.a.title\")}</p>\n<script>const MAP = { x: 'web.a.mapped', y: `web.a.tpl` };</script>",
        );
        write(d.path(), "web/src/a.test.ts", "t(\"web.a.only_in_test\")");
        write(d.path(), "web/src/generated/en.ts", "\"web.a.generated\"");
        let u = Usage::scan(&d.path().join("android"), &d.path().join("web/src"));
        (d, u)
    }

    #[test]
    fn android_plural_and_xml_references_count() {
        let (_d, u) = fixture();
        assert!(u.is_used("native.ui.title"));
        assert!(u.is_used("native.ui.missed"));
        assert!(u.is_used("native.ui.label"));
        assert!(!u.is_used("native.ui.onlyInBuild"));
    }

    #[test]
    fn web_literals_in_calls_maps_and_templates_count() {
        let (_d, u) = fixture();
        assert!(u.is_used("web.a.title") && u.is_used("web.a.mapped") && u.is_used("web.a.tpl"));
    }

    #[test]
    fn tests_and_generated_bundles_do_not_count() {
        let (_d, u) = fixture();
        assert!(!u.is_used("web.a.only_in_test"));
        assert!(!u.is_used("web.a.generated"));
    }

    #[test]
    fn android_names_do_not_use_web_only_groups() {
        // `web.*` keys are not Android resources, even if a same-named resource is referenced.
        let u = Usage {
            android: HashSet::from(["web_a_title".to_string()]),
            web: HashSet::new(),
        };
        assert!(!u.is_used("web.a.title"));
    }

    #[test]
    fn core_group_is_used_by_rule() {
        let (_d, u) = fixture();
        assert!(u.is_used("core.serverUnreachable"));
        assert!(!u.is_used("corex.serverUnreachable"));
    }

    #[test]
    fn unused_keys_are_named() {
        let (_d, u) = fixture();
        let keys = [
            "native.ui.title",
            "native.ui.dead",
            "core.peerAbsent",
            "web.a.dead",
        ];
        let problems = unused_keys(keys.iter().copied(), &u);
        assert_eq!(problems.len(), 2);
        assert!(problems[0].contains("native.ui.dead") && problems[1].contains("web.a.dead"));
    }

    /// Minimal temp dir (no extra dependency): removed on drop.
    mod tempdir {
        use std::path::{Path, PathBuf};
        pub struct Dir(PathBuf);
        impl Dir {
            pub fn new() -> Self {
                use std::sync::atomic::{AtomicU32, Ordering};
                static N: AtomicU32 = AtomicU32::new(0);
                let p = std::env::temp_dir().join(format!(
                    "i18n-gen-usage-{}-{}",
                    std::process::id(),
                    N.fetch_add(1, Ordering::Relaxed)
                ));
                std::fs::create_dir_all(&p).unwrap();
                Dir(p)
            }
            pub fn path(&self) -> &Path {
                &self.0
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }
}
