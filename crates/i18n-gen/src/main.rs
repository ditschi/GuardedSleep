//! Localization generator: validates `locales/*.json` and emits web JSON and Android resources.

mod android;
mod catalogue;
mod icu;
mod web;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "usage: i18n-gen <check|web|android|all> [--out <dir>] [--web-out <dir>] [--android-out <dir>] [--locales <dir>]
  check     validate the catalogue and Kotlin UI literals
  web       write web JSON bundles (+ pseudo.json) to --out
  android   write Android res/values*/strings.xml to --out
  all       web + android (--web-out / --android-out, defaults to the app build dirs)";

fn arg(argv: &[String], name: &str) -> Option<PathBuf> {
    argv.iter()
        .position(|a| a == name)
        .and_then(|i| argv.get(i + 1))
        .map(PathBuf::from)
}

fn write(dir: &Path, files: &[(String, String)]) -> std::io::Result<()> {
    for (rel, content) in files {
        let target = dir.join(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(target, content)?;
    }
    Ok(())
}

fn fail(issues: &[String]) -> ExitCode {
    eprintln!("i18n: {} problem(s)", issues.len());
    for i in issues {
        eprintln!("  - {i}");
    }
    ExitCode::from(1)
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let cmd = argv.first().map(String::as_str).unwrap_or("");
    if !["check", "web", "android", "all"].contains(&cmd) {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = arg(&argv, "--out");
    if matches!(cmd, "web" | "android") && out.is_none() {
        eprintln!("{cmd}: --out <dir> is required");
        return ExitCode::from(2);
    }
    let web_out = arg(&argv, "--web-out")
        .or_else(|| if cmd == "web" { out.clone() } else { None })
        .unwrap_or_else(|| root.join("apps/mobile/src/lib/i18n/generated"));
    let android_out = arg(&argv, "--android-out")
        .or_else(|| if cmd == "android" { out.clone() } else { None })
        .unwrap_or_else(|| root.join("apps/android/app/build/generated/i18n/res"));

    let mut issues = Vec::new();
    let cat = catalogue::load(
        &arg(&argv, "--locales").unwrap_or_else(|| root.join("locales")),
        &mut issues,
    );
    issues.extend(catalogue::validate(&cat));
    let android_files = android::render_android(&cat, &mut issues);
    let web_files = match web::render_web(&cat) {
        Ok(f) => f,
        Err(e) => {
            issues.push(format!("web: {e}"));
            Vec::new()
        }
    };
    if !issues.is_empty() {
        return fail(&issues);
    }

    let io = |r: std::io::Result<()>| r.map_err(|e| eprintln!("i18n: write failed: {e}")).is_ok();
    if matches!(cmd, "web" | "all") && !io(write(&web_out, &web_files)) {
        return ExitCode::from(1);
    }
    if matches!(cmd, "android" | "all") && !io(write(&android_out, &android_files)) {
        return ExitCode::from(1);
    }
    if cmd == "check" {
        match Command::new("bash")
            .arg(root.join("scripts/lint-ui-literals.sh"))
            .status()
        {
            Ok(s) if s.success() => {}
            Ok(s) => return ExitCode::from(s.code().unwrap_or(1) as u8),
            Err(e) => {
                eprintln!("i18n: cannot run lint-ui-literals.sh: {e}");
                return ExitCode::from(1);
            }
        }
        println!(
            "i18n: {} messages x {} locales OK",
            cat.get(catalogue::MASTER).len(),
            cat.locales.len()
        );
    }
    ExitCode::SUCCESS
}
