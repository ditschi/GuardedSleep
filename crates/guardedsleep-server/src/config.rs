//! Configuration from environment variables and command-line flags.
//!
//! The `SIGNALING_*` and `PORT` variables keep the names of the retired Node server, so existing
//! deployments keep working. A value that is missing, malformed (`5000ms` included) or out of range
//! falls back to the default. New options are flags with `GS_*` / `STUN_*` environment
//! counterparts. Docs: https://docs.rs/clap/latest/clap/_derive/index.html

use std::net::IpAddr;
use std::path::PathBuf;

use clap::Parser;
use guardedsleep_relay::RelayConfig;

#[derive(Parser, Debug, Clone)]
#[command(name = "guardedsleep-server", version, about)]
pub struct Cli {
    /// Port of the signaling WebSocket and HTTP endpoint (TCP).
    #[arg(long, env = "PORT", default_value_t = 8787)]
    pub port: u16,

    /// Address to bind the signaling (and in public mode the HTTP) listener and STUN to.
    #[arg(long, env = "GS_BIND", default_value = "0.0.0.0")]
    pub bind: IpAddr,

    /// Local mode (ADR-021): the UI and registration endpoint bind to loopback only, `Origin` and
    /// `Host` are checked there, only the signaling port binds to `--bind`, and the relay requires
    /// admission (keys are registered over loopback with `PUT /admission/<room>`).
    #[arg(long, env = "GS_LOCAL", value_parser = clap::builder::BoolishValueParser::new())]
    pub local: bool,

    /// Port of the loopback UI listener in local mode.
    #[arg(long, env = "GS_UI_PORT", default_value_t = 8080)]
    pub ui_port: u16,

    /// Serve the static web UI from this directory (SPA fallback to `index.html`).
    #[arg(long, env = "GS_SERVE_WEB", value_name = "DIR")]
    pub serve_web: Option<PathBuf>,

    /// UDP port of the built-in STUN Binding responder.
    #[arg(long, env = "STUN_PORT", default_value_t = 3478)]
    pub stun_port: u16,

    /// Do not start the STUN responder.
    #[arg(long, env = "STUN_DISABLED", value_parser = clap::builder::BoolishValueParser::new())]
    pub no_stun: bool,

    /// Binding requests answered per client address and minute (with the 44-byte and 2.2x response
    /// bound this keeps the responder useless as a traffic amplifier).
    #[arg(long, env = "STUN_RATE_PER_MIN", default_value_t = 60)]
    pub stun_rate_per_min: u32,

    /// Probe `GET /healthz` on 127.0.0.1:<port> and exit 0 when healthy, 1 otherwise. For the
    /// container HEALTHCHECK: the distroless image has no shell or curl.
    #[arg(long)]
    pub healthcheck: bool,

    /// Print relay events (connect, join, leave, close reason; never addresses or ids) to stderr.
    #[arg(long, env = "GS_LOG_EVENTS", value_parser = clap::builder::BoolishValueParser::new())]
    pub log_events: bool,
}

/// Limits with the retired Node server's names and defaults (`git show ae71762d:apps/signaling-server/src/config.ts`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    pub hello_timeout_ms: u64,
    pub max_parents: usize,
    pub ping_interval_ms: u64,
    pub hello_rate_per_min: u32,
    pub new_room_rate_per_min: u32,
    pub max_connections_per_ip: usize,
    pub frame_rate_unjoined_per_min: u32,
    pub frame_rate_per_min: u32,
    pub trust_proxy: bool,
    pub trusted_proxy_hops: usize,
    pub allowed_origins: Vec<String>,
    pub production: bool,
}

fn parse_in<T: std::str::FromStr + PartialOrd>(raw: Option<String>, lo: T, hi: T, default: T) -> T {
    match raw.and_then(|s| s.trim().parse::<T>().ok()) {
        Some(n) if n >= lo && n <= hi => n,
        _ => default,
    }
}

impl Limits {
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Self {
        let allowed_origins = get("SIGNALING_ALLOWED_ORIGINS")
            .map(|raw| {
                raw.split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        Self {
            hello_timeout_ms: parse_in(get("SIGNALING_HELLO_TIMEOUT_MS"), 1_000, 120_000, 30_000),
            max_parents: parse_in(get("SIGNALING_MAX_PARENTS"), 1, 256, 1),
            ping_interval_ms: match get("SIGNALING_PING_INTERVAL_MS")
                .and_then(|s| s.trim().parse::<u64>().ok())
            {
                Some(n) if n >= 2_000 => n,
                _ => 10_000,
            },
            hello_rate_per_min: parse_in(get("SIGNALING_HELLO_RATE_PER_MIN"), 0, 100_000, 20),
            new_room_rate_per_min: parse_in(get("SIGNALING_NEW_ROOM_RATE_PER_MIN"), 0, 100_000, 10),
            max_connections_per_ip: parse_in(
                get("SIGNALING_MAX_CONNECTIONS_PER_IP"),
                0,
                100_000,
                16,
            ),
            frame_rate_unjoined_per_min: parse_in(
                get("SIGNALING_FRAME_RATE_UNJOINED_PER_MIN"),
                0,
                1_000_000,
                120,
            ),
            frame_rate_per_min: parse_in(get("SIGNALING_FRAME_RATE_PER_MIN"), 0, 1_000_000, 600),
            trust_proxy: get("SIGNALING_TRUST_PROXY").as_deref() == Some("1"),
            trusted_proxy_hops: match get("SIGNALING_TRUSTED_PROXY_HOPS")
                .and_then(|s| s.trim().parse::<usize>().ok())
            {
                Some(n) if n >= 1 => n,
                _ => 1,
            },
            allowed_origins,
            production: get("NODE_ENV").as_deref() == Some("production"),
        }
    }

    pub fn from_env() -> Self {
        Self::from_lookup(|k| std::env::var(k).ok())
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub cli: Cli,
    pub limits: Limits,
}

impl Config {
    pub fn new(cli: Cli, limits: Limits) -> Result<Self, String> {
        if limits.production && limits.allowed_origins.is_empty() {
            return Err("SIGNALING_ALLOWED_ORIGINS must be set when NODE_ENV=production".into());
        }
        if cli.local && cli.port != 0 && cli.port == cli.ui_port {
            return Err("--port and --ui-port must differ in local mode".into());
        }
        Ok(Self { cli, limits })
    }

    pub fn relay_config(&self) -> RelayConfig {
        use guardedsleep_relay::Admission;
        let l = &self.limits;
        RelayConfig {
            admission: if self.cli.local {
                Admission::Required
            } else {
                Admission::Open
            },
            max_parents_per_room: l.max_parents,
            hello_timeout_ms: l.hello_timeout_ms,
            ping_interval_ms: l.ping_interval_ms,
            hello_rate_per_min: l.hello_rate_per_min,
            new_room_rate_per_min: l.new_room_rate_per_min,
            max_connections_per_client: l.max_connections_per_ip,
            frame_rate_unjoined_per_min: l.frame_rate_unjoined_per_min,
            frame_rate_per_min: l.frame_rate_per_min,
            ..RelayConfig::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn limits(pairs: &[(&str, &str)]) -> Limits {
        let m: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        Limits::from_lookup(|k| m.get(k).cloned())
    }

    #[test]
    fn defaults_match_the_node_server() {
        let l = limits(&[]);
        assert_eq!(l.hello_timeout_ms, 30_000);
        assert_eq!(l.max_parents, 1);
        assert_eq!(l.ping_interval_ms, 10_000);
        assert_eq!(l.hello_rate_per_min, 20);
        assert_eq!(l.new_room_rate_per_min, 10);
        assert_eq!(l.max_connections_per_ip, 16);
        assert_eq!(l.frame_rate_unjoined_per_min, 120);
        assert_eq!(l.frame_rate_per_min, 600);
        assert!(!l.trust_proxy);
        assert_eq!(l.trusted_proxy_hops, 1);
        assert!(l.allowed_origins.is_empty());
    }

    #[test]
    fn out_of_range_or_malformed_values_fall_back() {
        let l = limits(&[
            ("SIGNALING_HELLO_TIMEOUT_MS", "999"),
            ("SIGNALING_MAX_PARENTS", "0"),
            ("SIGNALING_PING_INTERVAL_MS", "1999"),
            ("SIGNALING_HELLO_RATE_PER_MIN", "-1"),
            ("SIGNALING_NEW_ROOM_RATE_PER_MIN", "x"),
            ("SIGNALING_MAX_CONNECTIONS_PER_IP", "-3"),
            ("SIGNALING_TRUSTED_PROXY_HOPS", "0"),
        ]);
        assert_eq!(l, limits(&[]));
    }

    #[test]
    fn valid_values_are_taken() {
        let l = limits(&[
            ("SIGNALING_HELLO_TIMEOUT_MS", "5000"),
            ("SIGNALING_MAX_PARENTS", "4"),
            ("SIGNALING_PING_INTERVAL_MS", "2000"),
            ("SIGNALING_HELLO_RATE_PER_MIN", "0"),
            ("SIGNALING_NEW_ROOM_RATE_PER_MIN", "7"),
            ("SIGNALING_MAX_CONNECTIONS_PER_IP", "4"),
            ("SIGNALING_FRAME_RATE_UNJOINED_PER_MIN", "30"),
            ("SIGNALING_FRAME_RATE_PER_MIN", "90"),
            ("SIGNALING_TRUST_PROXY", "1"),
            ("SIGNALING_TRUSTED_PROXY_HOPS", "2"),
            (
                "SIGNALING_ALLOWED_ORIGINS",
                " https://a.example , ,https://b.example",
            ),
        ]);
        assert_eq!(l.hello_timeout_ms, 5000);
        assert_eq!(l.max_parents, 4);
        assert_eq!(l.ping_interval_ms, 2000);
        assert_eq!(l.hello_rate_per_min, 0);
        assert_eq!(l.new_room_rate_per_min, 7);
        assert_eq!(l.max_connections_per_ip, 4);
        assert_eq!(l.frame_rate_unjoined_per_min, 30);
        assert_eq!(l.frame_rate_per_min, 90);
        assert!(l.trust_proxy);
        assert_eq!(l.trusted_proxy_hops, 2);
        assert_eq!(
            l.allowed_origins,
            ["https://a.example", "https://b.example"]
        );
    }

    #[test]
    fn production_requires_an_origin_allowlist() {
        let cli = Cli::parse_from(["guardedsleep-server"]);
        let prod = limits(&[("NODE_ENV", "production")]);
        assert!(Config::new(cli.clone(), prod).is_err());
        let ok = limits(&[
            ("NODE_ENV", "production"),
            ("SIGNALING_ALLOWED_ORIGINS", "https://a.example"),
        ]);
        assert!(Config::new(cli, ok).is_ok());
    }

    #[test]
    fn relay_limits_reach_the_relay_config() {
        let cfg = Config::new(
            Cli::parse_from(["x"]),
            limits(&[
                ("SIGNALING_MAX_CONNECTIONS_PER_IP", "3"),
                ("SIGNALING_FRAME_RATE_PER_MIN", "50"),
            ]),
        )
        .unwrap();
        let r = cfg.relay_config();
        assert_eq!(r.max_connections_per_client, 3);
        assert_eq!(r.frame_rate_per_min, 50);
        assert_eq!(r.frame_rate_unjoined_per_min, 120);
    }

    #[test]
    fn local_mode_requires_admission() {
        use guardedsleep_relay::Admission;
        let local = Config::new(Cli::parse_from(["x", "--local"]), limits(&[])).unwrap();
        assert_eq!(local.relay_config().admission, Admission::Required);
        let public = Config::new(Cli::parse_from(["x"]), limits(&[])).unwrap();
        assert_eq!(public.relay_config().admission, Admission::Open);
    }
}
