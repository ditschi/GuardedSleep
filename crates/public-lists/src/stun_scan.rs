//! Lint: no public STUN or TURN host outside `data/stun-servers.json` and the project server in
//! client source or configuration (AGENTS.md section 7.1, spec `signaling-relay` "Client ICE
//! configuration").
//!
//! A TURN host shows up only as a `turn:` / `turns:` URL. Test hosts under `apps/e2e`, `docker/`
//! and `scripts/poc/` may name any TURN host (the gate and the e2e suite run their own coturn).
//!
//! A STUN host shows up in two shapes: a `stun:` / `stuns:` URL, or a bare host name that starts
//! with `stun` (`stun.l.google.com`, `stun1.example.org`). Dynamic hosts (`stun:${host}:3478`, the
//! built-in STUN of a configured relay), reserved documentation and test names, loopback and
//! private literals are fine: they are not a public service.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Directories scanned below the repository root: the clients, the server defaults, the images and
/// the scripts. Documentation and OpenSpec text are covered by `scripts/lib/no-google-stun.sh`.
const ROOTS: &[&str] = &["apps", "crates", "docker", "scripts", ".github"];

/// Directory names never entered (build output, dependencies, generated files).
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "build",
    "dist",
    "dist-e2e",
    "generated",
    ".gradle",
    ".git",
    "pkg",
    "pkg-node",
    "test-results",
    "playwright-report",
];

/// This crate holds the self-test sample.
const SKIP_PATHS: &[&str] = &["crates/public-lists/"];

/// Paths where `turn:` / `turns:` URLs may name any host (test fixtures and the gate stack).
const TURN_TEST_PATHS: &[&str] = &["apps/e2e/", "docker/", "scripts/poc/"];

const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

fn host_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'.' || b == b'-'
}

/// Reserved or private names that are never a public STUN service. A name without a dot is a
/// placeholder or a LAN host name, not a public host.
fn exempt(host: &str) -> bool {
    let h = host.to_ascii_lowercase();
    let reserved = [
        "example",
        "invalid",
        "test",
        "localhost",
        "local",
        "internal",
    ];
    let documentation = ["example.com", "example.org", "example.net"];
    !h.contains('.')
        || h.rsplit('.')
            .next()
            .is_some_and(|tld| reserved.contains(&tld))
        || documentation
            .iter()
            .any(|d| h == *d || h.ends_with(&format!(".{d}")))
        || h.starts_with(|c: char| c.is_ascii_digit())
        || h.starts_with('[')
}

fn push_once(found: &mut Vec<(usize, String)>, line: usize, host: String) {
    let key = (line, host);
    if !found.contains(&key) {
        found.push(key);
    }
}

/// True when byte `i` starts a word (not preceded by a host character or `_`).
fn at_word_start(bytes: &[u8], i: usize) -> bool {
    i == 0
        || !bytes
            .get(i - 1)
            .is_some_and(|b| host_byte(*b) || *b == b'_')
}

/// The host of a URL whose scheme name ended just before `rest`: `rest` starts with `:` or `s:`
/// (`stun:` / `stuns:`, `turn:` / `turns:`), then an optional `//`, then host and port. Returns
/// `None` when `rest` is not the URL shape (the caller may then try other shapes). The host is
/// not validated here; an IPv6 literal keeps its brackets.
fn scheme_url_host(rest: &str) -> Option<String> {
    let after = rest.strip_prefix("s:").or_else(|| rest.strip_prefix(':'))?;
    let after = after.strip_prefix("//").unwrap_or(after);
    let host: String = after
        .bytes()
        .take_while(|b| host_byte(*b) || *b == b'[' || *b == b']' || *b == b':')
        .map(char::from)
        .collect();
    Some(if host.starts_with('[') {
        host
    } else {
        host.split(':').next().unwrap_or("").to_owned()
    })
}

/// Every STUN host named in `text`, with its 1-based line.
fn hosts_in(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if at_word_start(bytes, i)
                && bytes
                    .get(i..i + 4)
                    .is_some_and(|w| w.eq_ignore_ascii_case(b"stun"))
            {
                let rest = line.get(i + 4..).unwrap_or("");
                if let Some(host) = scheme_url_host(rest) {
                    // stun: / stuns: URL
                    if !host.is_empty() && host.contains(|c: char| c.is_ascii_alphanumeric()) {
                        push_once(&mut found, idx + 1, host);
                    }
                } else {
                    // stun.example / stun1.example bare host name
                    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
                    if rest.as_bytes().get(digits) == Some(&b'.') {
                        let host: String = line
                            .get(i..)
                            .unwrap_or("")
                            .bytes()
                            .take_while(|b| host_byte(*b))
                            .map(char::from)
                            .collect();
                        let host = host.trim_end_matches(['.', '-']).to_owned();
                        // Two dots: `stun.rs` (a file name in a comment) is not a host.
                        if host.matches('.').count() >= 2 {
                            push_once(&mut found, idx + 1, host);
                        }
                    }
                }
                i += 4;
            } else {
                i += 1;
            }
        }
    }
    found
}

/// Every host of a `turn:` / `turns:` URL in `text`, with its 1-based line.
fn turn_hosts_in(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let bytes = line.as_bytes();
        for i in 0..bytes.len() {
            if !at_word_start(bytes, i)
                || !bytes
                    .get(i..i + 4)
                    .is_some_and(|w| w.eq_ignore_ascii_case(b"turn"))
            {
                continue;
            }
            let Some(host) = scheme_url_host(line.get(i + 4..).unwrap_or("")) else {
                continue;
            };
            // A name that does not start like a host (`turn:.host`, a malformed-input fixture) is
            // not one.
            if host.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '[') {
                push_once(&mut found, idx + 1, host);
            }
        }
    }
    found
}

/// Findings in one text: each is `"<path>:<line>: <host>"`.
/// `allowed` holds the `data/stun-servers.json` hosts and the project server host.
pub fn scan_text(path: &str, text: &str, allowed: &BTreeSet<String>) -> Vec<String> {
    let unlisted = |(line, host): (usize, String)| {
        let h = host.to_ascii_lowercase();
        (!allowed.contains(&h) && !exempt(&h)).then_some((line, host))
    };
    let mut out: Vec<String> = hosts_in(text)
        .into_iter()
        .filter_map(unlisted)
        .map(|(line, host)| {
            format!("{path}:{line}: STUN host {host} is not in data/stun-servers.json")
        })
        .collect();
    if !TURN_TEST_PATHS.iter().any(|p| path.starts_with(p)) {
        out.extend(turn_hosts_in(text).into_iter().filter_map(unlisted).map(
            |(line, host)| {
                format!(
                    "{path}:{line}: TURN host {host} is not the project server or in data/stun-servers.json"
                )
            },
        ));
    }
    out
}

fn walk(dir: &Path, root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if kind.is_dir() {
            if !SKIP_DIRS.contains(&name.as_str())
                && !SKIP_PATHS.iter().any(|p| format!("{rel}/").starts_with(p))
            {
                walk(&path, root, files);
            }
        } else if kind.is_file() {
            files.push(path);
        }
    }
}

/// Scans the client, server, image and script trees below `root`.
pub fn scan_tree(root: &Path, allowed: &BTreeSet<String>) -> Vec<String> {
    let mut files = Vec::new();
    for r in ROOTS {
        walk(&root.join(r), root, &mut files);
    }
    files.sort();
    let mut findings = Vec::new();
    for file in files {
        if std::fs::metadata(&file).is_ok_and(|m| m.len() > MAX_FILE_BYTES) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue; // binary
        };
        let rel = file.strip_prefix(root).unwrap_or(&file).to_string_lossy();
        findings.extend(scan_text(&rel, &text, allowed));
    }
    findings
}

/// The check proves itself before it is trusted: the Google STUN host must be caught and a
/// listed host must pass. Returns an error text when the scanner is broken.
pub fn self_test() -> Result<(), String> {
    let allowed: BTreeSet<String> = ["stun.nextcloud.com".to_owned()].into();
    let caught = scan_text(
        "self-test",
        "const s = 'stun:stun.l.google.com:19302'; // and stun1.l.google.com",
        &allowed,
    );
    if caught.len() != 2 {
        return Err(format!(
            "self-test: stun.l.google.com was not caught ({caught:?})"
        ));
    }
    let turn = scan_text(
        "self-test",
        "urls: ['turn:turn.other.net:3478', \"turns://TURN.Other.net:5349?transport=tcp\"]",
        &allowed,
    );
    if turn.len() != 2 {
        return Err(format!(
            "self-test: a foreign turn: URL was not caught ({turn:?})"
        ));
    }
    let turn_fine = scan_text(
        "self-test",
        "'turn:stun.nextcloud.com:3478' 'turns:relay.example.org:5349' `turn:${host}:3478`",
        &allowed,
    );
    let turn_test_path = scan_text("apps/e2e/x.ts", "'turn:turn.other.net:3478'", &allowed);
    if !turn_fine.is_empty() || !turn_test_path.is_empty() {
        return Err(format!(
            "self-test: turn: false positives ({turn_fine:?} {turn_test_path:?})"
        ));
    }
    let fine = scan_text(
        "self-test",
        "urls: ['stun:stun.nextcloud.com:443', `stun:${host}:3478`, 'stun:relay.example:3478']",
        &allowed,
    );
    if !fine.is_empty() {
        return Err(format!("self-test: false positives ({fine:?})"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed() -> BTreeSet<String> {
        [
            "stun.nextcloud.com".to_owned(),
            "stun.t-online.de".to_owned(),
        ]
        .into()
    }

    #[test]
    fn google_stun_is_caught_in_every_shape() {
        for text in [
            "iceServers: [{ urls: 'stun:stun.l.google.com:19302' }]",
            "stuns://stun.l.google.com",
            "host = \"stun1.l.google.com\"",
            "STUN:STUN.L.GOOGLE.COM:19302",
            "url(\"stun:stun.l.google.com\")",
        ] {
            assert!(!scan_text("f", text, &allowed()).is_empty(), "{text}");
        }
    }

    #[test]
    fn the_self_test_passes() {
        assert_eq!(self_test(), Ok(()));
    }

    #[test]
    fn listed_dynamic_and_reserved_hosts_pass() {
        for text in [
            "stun:stun.nextcloud.com:443",
            "'stun:stun.t-online.de:3478'",
            "`stun:${host}:3478`",
            "\"stun:$host:$PORT\"",
            "stun:[$host]:3478",
            "stun:[::1]:3478",
            "stun:127.0.0.1:3478",
            "stun:relay.example.org:3478",
            "stun:box.example:3478",
            "a built-in STUN server",
            "stun: nothing here",
            "const stunUrls = []",
            "fn run_stun_server()",
            "nostun.example.invalid",
        ] {
            assert_eq!(
                scan_text("f", text, &allowed()),
                Vec::<String>::new(),
                "{text}"
            );
        }
    }

    #[test]
    fn foreign_turn_hosts_are_caught_listed_and_test_paths_pass() {
        for text in [
            "urls: 'turn:turn.other.net:3478'",
            "turns:turn.other.net:5349?transport=tcp",
            "TURN://turn.other.net",
            "turns://[2001:db8::1]:5349",
        ] {
            let got = scan_text("a.ts", text, &allowed());
            // the IPv6 literal is exempt like for STUN; the rest is caught
            assert_eq!(got.len(), usize::from(!text.contains('[')), "{text}");
        }
        assert!(scan_text("a.ts", "turn:turn.other.net:3478", &allowed())[0].contains("TURN host"));
        for text in [
            "turn:stun.nextcloud.com:3478",
            "turn:${host}:3478",
            "turns:relay.example.org:5349",
            "turn: nothing, turnstile, return turn:",
            "turn_credentials",
        ] {
            assert!(scan_text("a.ts", text, &allowed()).is_empty(), "{text}");
        }
        for path in ["apps/e2e/t.ts", "docker/turn/c.yaml", "scripts/poc/g.sh"] {
            assert!(scan_text(path, "turn:turn.other.net:3478", &allowed()).is_empty());
        }
    }

    #[test]
    fn a_finding_names_file_line_and_host() {
        let got = scan_text(
            "a/b.kt",
            "x\nval u = \"stun:stun.provider.net:3478\"",
            &allowed(),
        );
        assert_eq!(
            got,
            ["a/b.kt:2: STUN host stun.provider.net is not in data/stun-servers.json"]
        );
    }
}
