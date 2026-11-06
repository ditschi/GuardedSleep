//! Curated public lists: validates `data/stun-servers.json`, enforces the build minimums, and emits
//! the web module and the Android object.

#![forbid(unsafe_code)]

mod emit;
mod lists;
mod stun_scan;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use lists::{Date, Lists};

const USAGE: &str = "usage: public-lists <check|stun-sources|web|android> [options]
  check          validate the lists and the build minimums; stale entries only warn
  stun-sources   fail when a STUN host outside data/stun-servers.json appears in client source or config
  web            write public-lists.ts to --web-out
  android        write PublicLists.kt to --out
options: --data <dir> (default: data/)  --root <dir> (stun-sources; default: repository root)
         --today <YYYY-MM-DD> (default: system date)";

fn arg(argv: &[String], name: &str) -> Option<PathBuf> {
    argv.iter()
        .position(|a| a == name)
        .and_then(|i| argv.get(i + 1))
        .map(PathBuf::from)
}

fn write(dir: &Path, name: &str, content: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(name), content)
}

fn today(argv: &[String]) -> Result<Date, String> {
    match arg(argv, "--today") {
        Some(text) => Date::parse(&text.to_string_lossy())
            .ok_or_else(|| format!("--today {} is not YYYY-MM-DD", text.display())),
        None => {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            Ok(Date::from_unix_seconds(secs))
        }
    }
}

/// Loads and validates; prints warnings, and every error before returning `Err`.
fn load_checked(data: &Path, argv: &[String]) -> Result<Lists, ExitCode> {
    let fail = |msg: String| {
        eprintln!("public-lists: {msg}");
        ExitCode::from(1)
    };
    let lists = Lists::load(data).map_err(fail)?;
    let report = lists::validate(&lists, today(argv).map_err(fail)?);
    for w in &report.warnings {
        eprintln!("public-lists: WARNING {w}");
    }
    if !report.errors.is_empty() {
        eprintln!("public-lists: {} problem(s)", report.errors.len());
        for e in &report.errors {
            eprintln!("  - {e}");
        }
        return Err(ExitCode::from(1));
    }
    Ok(lists)
}

fn run(argv: &[String]) -> Result<(), ExitCode> {
    let cmd = argv.first().map(String::as_str).unwrap_or("");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let data = arg(argv, "--data").unwrap_or_else(|| root.join("data"));
    match cmd {
        "check" => {
            let lists = load_checked(&data, argv)?;
            println!(
                "public-lists: ok ({} STUN servers)",
                lists.active_stun().count()
            );
            Ok(())
        }
        "stun-sources" => {
            if let Err(e) = stun_scan::self_test() {
                eprintln!("public-lists: {e}");
                return Err(ExitCode::from(1));
            }
            let lists = load_checked(&data, argv)?;
            let allowed = lists.active_stun().map(|s| s.host.to_lowercase()).collect();
            let scan_root = arg(argv, "--root").unwrap_or(root);
            let findings = stun_scan::scan_tree(&scan_root, &allowed);
            if findings.is_empty() {
                return Ok(());
            }
            eprintln!(
                "public-lists: {} unlisted STUN host(s) in client source or config",
                findings.len()
            );
            for f in &findings {
                eprintln!("  - {f}");
            }
            eprintln!(
                "public-lists: use a server from data/stun-servers.json (AGENTS.md section 7.1)"
            );
            Err(ExitCode::from(1))
        }
        "web" => {
            let lists = load_checked(&data, argv)?;
            let web_out = arg(argv, "--web-out")
                .unwrap_or_else(|| root.join("apps/web/src/lib/public-lists/generated"));
            let io = |e: std::io::Error| {
                eprintln!("public-lists: {e}");
                ExitCode::from(1)
            };
            write(&web_out, "public-lists.ts", &emit::web_module(&lists)).map_err(io)?;
            Ok(())
        }
        "android" => {
            let Some(out) = arg(argv, "--out") else {
                eprintln!("android: --out <dir> is required");
                return Err(ExitCode::from(2));
            };
            let lists = load_checked(&data, argv)?;
            write(&out, "PublicLists.kt", &emit::kotlin_object(&lists)).map_err(|e| {
                eprintln!("public-lists: {e}");
                ExitCode::from(1)
            })
        }
        _ => {
            eprintln!("{USAGE}");
            Err(ExitCode::from(2))
        }
    }
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match run(&argv) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}
