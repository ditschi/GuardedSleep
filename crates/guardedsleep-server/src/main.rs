use clap::Parser;
use guardedsleep_server::config::{Cli, Config, Limits};

#[tokio::main]
async fn main() {
    let cfg = match Config::new(Cli::parse(), Limits::from_env()) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("guardedsleep-server: {e}");
            std::process::exit(1);
        }
    };
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
