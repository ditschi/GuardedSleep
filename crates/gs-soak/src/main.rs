//! gs-soak: go/no-go tooling for the public Nostr mailboxes (`alpha-connection-model` tasks 4.1 and
//! 4.2). Not part of any release; it does the I/O the sans-IO core must not (sockets, clock,
//! randomness) and consumes the core's public API only.
//!
//!   gs-soak soak   two real Links over real relays for hours, forced link losses, a verdict
//!   gs-soak burst  the reconnect burst and the tag-only filter against real relays
//!   gs-soak sign   signed throwaway events for the Node measurement scripts (scripts/spike)
//!
//! Runbook: docs/contributing/relay-soak.md

#![forbid(unsafe_code)]

mod burst;
mod lists;
mod report;
mod sign;
mod soak;
mod stats;
mod ws;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Args, Parser, Subcommand, ValueEnum};
use guardedsleep_core::relay_pick::{PICK_COUNT, pick_relays};

#[derive(Parser)]
#[command(
    name = "gs-soak",
    about = "Go/no-go tooling for the public Nostr mailboxes"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Args)]
struct RelayArgs {
    /// Relays to use, `wss://host` each, comma separated. Default: `--pick` relays chosen from
    /// the active entries of data/nostr-relays.json like the core's `pick_relays`.
    #[arg(long, value_delimiter = ',')]
    relays: Vec<String>,
    /// How many relays to pick from the list.
    #[arg(long, default_value_t = PICK_COUNT)]
    pick: usize,
    /// Directory of the curated lists.
    #[arg(long, default_value = "data")]
    data: PathBuf,
}

#[derive(Clone, Copy, ValueEnum)]
enum RelayMailboxArg {
    Fail,
    Silent,
}

#[derive(Subcommand)]
enum Command {
    /// Two Links (baby, parent) over real relays for a duration, with forced link losses.
    Soak {
        #[command(flatten)]
        relays: RelayArgs,
        /// Duration in hours (default 8).
        #[arg(long, default_value_t = 8.0)]
        hours: f64,
        /// Minutes between a verified link and the next forced loss; 0 turns losses off.
        /// Default: 10, or a sixth of the run when that is shorter (smoke runs).
        #[arg(long)]
        loss_every_min: Option<f64>,
        /// Subscribe by tag only: the `authors` member of every REQ is dropped (spike unknown 3).
        #[arg(long)]
        tag_only: bool,
        /// The parent's wall clock is this many seconds off (relay `created_at` tolerance).
        #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
        parent_wall_offset_s: i64,
        /// Time the fake ICE takes once descriptions and a remote candidate are in.
        #[arg(long, default_value_t = 150)]
        ice_ms: u64,
        /// What the harness answers when the core opens its own signaling relay (mailbox 0):
        /// `silent` never answers (the default: the public relays are measured alone), `fail`
        /// reports every open as failed, as a platform without an own server may. Setups must take
        /// as long as with `silent` (see the runbook).
        #[arg(long, value_enum, default_value_t = RelayMailboxArg::Silent)]
        relay_mailbox: RelayMailboxArg,
        /// Print a timeline of the core's actions and the relays' answers (kinds and sizes only).
        #[arg(long)]
        trace: bool,
        /// Report file (JSON). Default: .cache/relay-soak/soak-<unix time>.json
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Offer, answer and candidates within about a second from two clients behind this IP.
    Burst {
        #[command(flatten)]
        relays: RelayArgs,
        /// Candidate bundles per client on top of its offer or answer (2 x 6 = 12 events).
        #[arg(long, default_value_t = 5)]
        candidates: usize,
        /// Milliseconds between two events of one client; 0 sends them back to back.
        #[arg(long, default_value_t = 170)]
        spacing_ms: u64,
        /// Subscribe by tag only (spike unknown 3).
        #[arg(long)]
        tag_only: bool,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Print signed throwaway events as JSON lines. Each SPEC is `BYTES[:OFFSET_S]`: the content
    /// length and the shift of `created_at` against now.
    Sign {
        /// The `t` tag; default: random.
        #[arg(long)]
        tag: Option<String>,
        #[arg(required = true, allow_hyphen_values = true)]
        specs: Vec<String>,
    },
}

fn resolve_relays(args: &RelayArgs) -> Result<Vec<String>, String> {
    if !args.relays.is_empty() {
        if let Some(bad) = args.relays.iter().find(|r| ws::split_url(r).is_none()) {
            return Err(format!("`{bad}` is not a wss:// URL"));
        }
        return Ok(args.relays.clone());
    }
    let list = lists::active_relays(&args.data)?;
    // A fresh random key per run: the pick differs from run to run, so repeated runs cover the list.
    let psk = sign::random_bytes::<32>()?;
    let picked = pick_relays(&psk, &list, &[], args.pick);
    if picked.is_empty() {
        return Err("no active relay in the list".into());
    }
    Ok(picked.into_iter().map(|m| m.url).collect())
}

fn default_out(kind: &str) -> PathBuf {
    PathBuf::from(format!(
        ".cache/relay-soak/{kind}-{}.json",
        sign::now_unix_s()
    ))
}

fn run(cli: Cli) -> Result<ExitCode, String> {
    match cli.command {
        Command::Sign { tag, specs } => {
            sign::run(tag, &specs)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Burst {
            relays,
            candidates,
            spacing_ms,
            tag_only,
            out,
        } => {
            let relays = resolve_relays(&relays)?;
            eprintln!("[burst] relays: {}", relays.join(", "));
            let result = burst::run(&burst::BurstOptions {
                relays,
                candidates,
                spacing_ms,
                tag_only,
            })?;
            let path = out.unwrap_or_else(|| default_out("burst"));
            report::write_json(&path, &result)?;
            println!("{}", report::burst_markdown(&result));
            eprintln!("[burst] report: {}", path.display());
            Ok(ExitCode::SUCCESS)
        }
        Command::Soak {
            relays,
            hours,
            loss_every_min,
            tag_only,
            parent_wall_offset_s,
            ice_ms,
            relay_mailbox,
            trace,
            out,
        } => {
            if !(hours > 0.0 && hours.is_finite()) {
                return Err("--hours must be positive".into());
            }
            let duration = Duration::from_secs_f64(hours * 3_600.0);
            let every_min = loss_every_min.unwrap_or_else(|| (hours * 60.0 / 6.0).min(10.0));
            if !(every_min >= 0.0 && every_min.is_finite()) {
                return Err("--loss-every-min must not be negative".into());
            }
            let relays = resolve_relays(&relays)?;
            let path = out.unwrap_or_else(|| default_out("soak"));
            if every_min > 0.0 && every_min < 4.0 {
                // The core sends the opening presence to one relay at most every 3 minutes
                // (OPEN_HERE_MIN_MS in link/nostr_link.rs); a recovery sooner than that waits for
                // the 5 s presence repeat instead, so its time says nothing about the relays.
                eprintln!(
                    "[soak] warning: a forced loss every {every_min:.1} min is inside the core's 3 min presence throttle; recovery times are inflated by about 5 s. Use 4 min or more for the go/no-go numbers."
                );
            }
            eprintln!(
                "[soak] {:.2} h on {}; forced loss every {:.1} min; report: {}",
                hours,
                relays.join(", "),
                every_min,
                path.display()
            );
            let mut soak = soak::Soak::new(soak::SoakOptions {
                relays,
                duration,
                loss_every: Duration::from_secs_f64(every_min * 60.0),
                tag_only,
                parent_wall_offset_s,
                ice_ms,
                relay_mailbox: match relay_mailbox {
                    RelayMailboxArg::Fail => soak::RelayMailbox::Fail,
                    RelayMailboxArg::Silent => soak::RelayMailbox::Silent,
                },
                trace,
                out: Some(path.clone()),
            })?;
            let report = soak.run()?;
            println!("{}", report::soak_markdown(&report));
            eprintln!("[soak] report: {}", path.display());
            // 0 = go, 3 = no-go, 4 = inconclusive: scripts can tell them apart.
            Ok(match report.verdict.outcome {
                stats::Outcome::Go => ExitCode::SUCCESS,
                stats::Outcome::NoGo => ExitCode::from(3),
                stats::Outcome::Inconclusive => ExitCode::from(4),
            })
        }
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("gs-soak: {e}");
            ExitCode::from(2)
        }
    }
}
