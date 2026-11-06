#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use clap::Parser;
use guardedsleep_server::config::{Cli, Config, Limits};
use guardedsleep_server::turn::IceConfig;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if cli.healthcheck {
        std::process::exit(if guardedsleep_server::healthcheck(cli.port) {
            0
        } else {
            1
        });
    }
    let cfg = match Config::new(cli, Limits::from_env())
        .and_then(|cfg| IceConfig::from_env().map(|ice| cfg.with_ice(ice)))
    {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("guardedsleep-server: {e}");
            std::process::exit(1);
        }
    };
    // Never prints a URL, secret or credential.
    if cfg.ice.turn_enabled() {
        println!("turn credentials: enabled");
    } else {
        println!("turn credentials: off (set TURN_URLS and TURN_SECRET to enable)");
    }
    let server = match guardedsleep_server::start(cfg).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("guardedsleep-server: {e}");
            std::process::exit(1);
        }
    };
    println!(
        "guardedsleep-server listening on port {}",
        server.signaling_addr.port()
    );
    if let Some(stun) = server.stun_addr {
        println!("stun listening on udp port {}", stun.port());
    }
    for ui in &server.ui_addrs {
        println!("local ui on loopback port {}", ui.port());
    }
    wait_for_stop().await;
    server.shutdown().await;
}

async fn wait_for_stop() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut term) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = term.recv() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}
