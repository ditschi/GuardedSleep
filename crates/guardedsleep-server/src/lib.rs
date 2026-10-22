//! GuardedSleep signaling server (ADR-020, ADR-021): axum and tokio around the sans-IO
//! `guardedsleep-relay`. It carries connection metadata only, never media, and never logs client
//! addresses (AGENTS.md section 7).

#![forbid(unsafe_code)]
// Untrusted HTTP, WebSocket and UDP input is parsed here: return errors instead of panicking
// (AGENTS.md section 5.3).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub mod config;
pub mod http;
pub mod hub;
pub mod stun;

use std::io;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::config::Config;
use crate::http::{AppState, loopback_router, public_router, static_service};
use crate::hub::Hub;

/// How often the relay clock runs; bounds the accuracy of timeouts.
const TICK: Duration = Duration::from_millis(500);

pub struct RunningServer {
    /// The signaling listener (LAN-facing in local mode).
    pub signaling_addr: SocketAddr,
    /// Loopback UI listeners of local mode (IPv4, plus IPv6 where available).
    pub ui_addrs: Vec<SocketAddr>,
    pub stun_addr: Option<SocketAddr>,
    shutdown: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
}

impl RunningServer {
    /// Stops accepting, closes all sockets and waits for the tasks.
    pub async fn shutdown(self) {
        let _ = self.shutdown.send(true);
        for t in self.tasks {
            let _ = t.await;
        }
    }
}

async fn bind(addr: SocketAddr, what: &str) -> io::Result<TcpListener> {
    TcpListener::bind(addr)
        .await
        .map_err(|e| io::Error::new(e.kind(), format!("cannot bind {what}: {e}")))
}

fn serve(
    listener: TcpListener,
    app: axum::Router,
    mut shutdown: watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            let _ = shutdown.wait_for(|stop| *stop).await;
        })
        .await;
    })
}

/// Whether binding `[::1]` failed because the host has no IPv6 loopback (as opposed to a conflict).
fn ipv6_loopback_unavailable(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::AddrNotAvailable | io::ErrorKind::Unsupported
    ) || e.raw_os_error() == Some(97) // EAFNOSUPPORT on Linux: IPv6 compiled out or disabled
}

/// Blocking probe of `GET /healthz` on loopback, for `--healthcheck`.
pub fn healthcheck(port: u16) -> bool {
    use std::io::{Read, Write};
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut s) = std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(2)) else {
        return false;
    };
    let timeout = Some(Duration::from_secs(2));
    let _ = s.set_read_timeout(timeout);
    let _ = s.set_write_timeout(timeout);
    let req = "GET /healthz HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
    let mut raw = String::new();
    s.write_all(req.as_bytes()).is_ok()
        && s.read_to_string(&mut raw).is_ok()
        && raw.starts_with("HTTP/1.1 200")
        && raw.ends_with("ok\n")
}

pub async fn start(cfg: Config) -> io::Result<RunningServer> {
    let cli = &cfg.cli;
    let hub = Arc::new(Hub::new(cfg.relay_config(), cli.log_events));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let relay_cfg = cfg.relay_config();
    let web = match &cli.serve_web {
        Some(dir) if !dir.is_dir() => {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("--serve-web: {} is not a directory", dir.display()),
            ));
        }
        Some(dir) => Some(static_service(dir)),
        None => None,
    };
    let state = |web, loopback_port| AppState {
        hub: hub.clone(),
        origins: Arc::new(cfg.limits.allowed_origins.clone()),
        trust_proxy: cfg.limits.trust_proxy,
        trusted_hops: cfg.limits.trusted_proxy_hops,
        max_message_bytes: relay_cfg.max_frame_bytes,
        shutdown: shutdown_rx.clone(),
        web,
        loopback_port,
    };

    let mut tasks = Vec::new();
    let mut ui_addrs = Vec::new();
    let signaling_addr;
    if cli.local {
        let lan = bind(SocketAddr::new(cli.bind, cli.port), "the signaling port").await?;
        signaling_addr = lan.local_addr()?;
        tasks.push(serve(
            lan,
            public_router(state(None, None)),
            shutdown_rx.clone(),
        ));

        let v4 = bind(
            SocketAddr::new(IpAddr::from([127, 0, 0, 1]), cli.ui_port),
            "the UI port",
        )
        .await?;
        let ui_port = v4.local_addr()?.port();
        ui_addrs.push(v4.local_addr()?);
        let app = loopback_router(state(web, Some(ui_port)));
        tasks.push(serve(v4, app.clone(), shutdown_rx.clone()));
        // `localhost` may resolve to ::1 first. Hosts without IPv6 loopback skip it; any other
        // failure (another process already holds the port) is an error, so a foreign page cannot
        // answer for `localhost` and phish the key hand-over.
        match TcpListener::bind(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), ui_port)).await {
            Ok(v6) => {
                ui_addrs.push(v6.local_addr()?);
                tasks.push(serve(v6, app, shutdown_rx.clone()));
            }
            Err(e) if ipv6_loopback_unavailable(&e) => {}
            Err(e) => {
                return Err(io::Error::new(
                    e.kind(),
                    format!("cannot bind the UI port on [::1]: {e}"),
                ));
            }
        }
    } else {
        let l = bind(SocketAddr::new(cli.bind, cli.port), "the signaling port").await?;
        signaling_addr = l.local_addr()?;
        tasks.push(serve(
            l,
            public_router(state(web, None)),
            shutdown_rx.clone(),
        ));
    }

    let mut stun_addr = None;
    if !cli.no_stun {
        let socket = UdpSocket::bind(SocketAddr::new(cli.bind, cli.stun_port))
            .await
            .map_err(|e| {
                io::Error::new(
                    e.kind(),
                    format!("cannot bind the STUN port (use --no-stun to disable): {e}"),
                )
            })?;
        stun_addr = Some(socket.local_addr()?);
        tasks.push(tokio::spawn(stun::serve(
            Arc::new(socket),
            cli.stun_rate_per_min,
            shutdown_rx.clone(),
        )));
    }

    let ticker_hub = hub.clone();
    let mut ticker_stop = shutdown_rx.clone();
    tasks.push(tokio::spawn(async move {
        let mut interval = tokio::time::interval(TICK);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = interval.tick() => ticker_hub.tick(),
                _ = ticker_stop.changed() => return,
            }
        }
    }));

    Ok(RunningServer {
        signaling_addr,
        ui_addrs,
        stun_addr,
        shutdown: shutdown_tx,
        tasks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_missing_ipv6_loopback_is_skipped_not_a_port_conflict() {
        assert!(ipv6_loopback_unavailable(&io::Error::from(
            io::ErrorKind::AddrNotAvailable
        )));
        assert!(ipv6_loopback_unavailable(&io::Error::from_raw_os_error(97)));
        assert!(!ipv6_loopback_unavailable(&io::Error::from(
            io::ErrorKind::AddrInUse
        )));
        assert!(!ipv6_loopback_unavailable(&io::Error::from(
            io::ErrorKind::PermissionDenied
        )));
    }
}
