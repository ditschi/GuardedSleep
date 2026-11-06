//! The command line the build and `just lint` run: minimums fail, staleness warns, unlisted STUN
//! hosts fail (and `stun.l.google.com` is caught).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_public-lists"))
}

fn stun(host: &str, operator: &str) -> String {
    format!(
        r#"{{"host":"{host}","port":3478,"operator":"{operator}","country":"DE","privacy_policy":null,"last_vetted":"2026-10-23","status":"active","notes":""}}"#
    )
}

const PROJECT: &str = r#"{"host":"gs.listed.net","path":"/signaling","operator":"Project","country":"DE","hoster":"Hoster","privacy_policy":null,"imprint":null,"last_vetted":"2026-10-23","status":"pending","notes":""}"#;

fn data_dir(name: &str, stun_entries: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("public-lists-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let s: Vec<String> = stun_entries.iter().map(|(h, o)| stun(h, o)).collect();
    std::fs::write(
        dir.join("stun-servers.json"),
        format!(r#"{{"servers":[{}]}}"#, s.join(",")),
    )
    .unwrap();
    std::fs::write(dir.join("project-server.json"), PROJECT).unwrap();
    dir
}

fn run(args: &[&str], data: &Path) -> Output {
    bin()
        .args(args)
        .arg("--data")
        .arg(data)
        .args(["--today", "2026-10-25"])
        .output()
        .unwrap()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

const TWO_STUN: &[(&str, &str)] = &[("stun.a.listed.net", "A"), ("stun.b.listed.net", "B")];

#[test]
fn the_shipped_lists_pass() {
    let o = bin().arg("check").output().unwrap();
    assert!(o.status.success(), "{}", stderr(&o));
}

#[test]
fn a_big_enough_list_passes() {
    let o = run(&["check"], &data_dir("ok", TWO_STUN));
    assert!(o.status.success(), "{}", stderr(&o));
}

#[test]
fn an_empty_stun_list_fails() {
    let o = run(&["check"], &data_dir("stun0", &[]));
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("STUN list is empty"), "{}", stderr(&o));
}

#[test]
fn a_single_stun_server_passes() {
    let o = run(
        &["check"],
        &data_dir("stun1", &[("stun.a.listed.net", "A")]),
    );
    assert!(o.status.success(), "{}", stderr(&o));
}

#[test]
fn a_live_project_server_without_policy_links_fails_the_build_check() {
    let dir = data_dir("live", TWO_STUN);
    let path = dir.join("project-server.json");
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("\"pending\"", "\"live\"");
    std::fs::write(&path, text).unwrap();
    let o = run(&["check"], &dir);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("status live needs"), "{}", stderr(&o));
}

#[test]
fn a_missing_project_server_file_fails() {
    let dir = data_dir("noproject", TWO_STUN);
    std::fs::remove_file(dir.join("project-server.json")).unwrap();
    let o = run(&["check"], &dir);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("project-server.json"), "{}", stderr(&o));
}

#[test]
fn a_stale_entry_warns_and_still_passes() {
    let dir = data_dir("stale", TWO_STUN);
    let path = dir.join("stun-servers.json");
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replacen("2026-10-23", "2024-01-01", 1);
    std::fs::write(&path, text).unwrap();
    let o = run(&["check"], &dir);
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(stderr(&o).contains("WARNING"), "{}", stderr(&o));
}

fn scan_root(name: &str, file: &str, content: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("public-lists-src-{name}-{}", std::process::id()));
    let path = root.join(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
    root
}

#[test]
fn a_google_stun_host_in_client_source_fails_the_lint() {
    let data = data_dir("scan", TWO_STUN);
    let root = scan_root(
        "google",
        "apps/web/src/ice.ts",
        "const ice = { urls: 'stun:stun.l.google.com:19302' };\n",
    );
    let o = run(&["stun-sources", "--root", root.to_str().unwrap()], &data);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("stun.l.google.com"), "{}", stderr(&o));
    assert!(
        stderr(&o).contains("apps/web/src/ice.ts:1"),
        "{}",
        stderr(&o)
    );
}

#[test]
fn a_listed_stun_host_passes_the_lint() {
    let data = data_dir("scan-ok", TWO_STUN);
    let root = scan_root(
        "listed",
        "apps/web/src/ice.ts",
        "const u = 'stun:stun.a.listed.net:3478';\n",
    );
    let o = run(&["stun-sources", "--root", root.to_str().unwrap()], &data);
    assert!(o.status.success(), "{}", stderr(&o));
}

#[test]
fn the_repository_has_no_unlisted_stun_host() {
    let o = bin().arg("stun-sources").output().unwrap();
    assert!(o.status.success(), "{}", stderr(&o));
}

#[test]
fn generation_writes_the_two_outputs() {
    let data = data_dir("gen", TWO_STUN);
    let out = std::env::temp_dir().join(format!("public-lists-out-{}", std::process::id()));
    let o = run(
        &["web", "--web-out", out.join("web").to_str().unwrap()],
        &data,
    );
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(out.join("web/public-lists.ts").is_file());
    assert!(!out.join("csp").exists());
    let o = run(
        &["android", "--out", out.join("kt").to_str().unwrap()],
        &data,
    );
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(out.join("kt/PublicLists.kt").is_file());
}
