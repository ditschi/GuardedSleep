mod common;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use clap::Parser;
use common::{Client, ROOM_V3, hello, http_get, http_request};
use guardedsleep_core::pairing::{admission_mac, derive_admission_key};
use guardedsleep_core::protocol::Role;
use guardedsleep_server::config::{Cli, Config, Limits};
use guardedsleep_server::{RunningServer, start};
use serde_json::json;
use tokio::net::UdpSocket;
use tokio::time::timeout;

async fn run(args: &[&str], env: &[(&str, &str)]) -> RunningServer {
    let mut argv = vec![
        "guardedsleep-server",
        "--bind",
        "127.0.0.1",
        "--port",
        "0",
        "--no-stun",
    ];
    argv.extend_from_slice(args);
    let env: HashMap<String, String> = env
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    let limits = Limits::from_lookup(|k| env.get(k).cloned());
    let cfg = Config::new(Cli::parse_from(argv), limits).unwrap();
    start(cfg).await.unwrap()
}

/// Limits off, like the retired Node tests.
const NO_LIMITS: [(&str, &str); 2] = [
    ("SIGNALING_HELLO_RATE_PER_MIN", "0"),
    ("SIGNALING_NEW_ROOM_RATE_PER_MIN", "0"),
];

#[tokio::test]
async fn health_endpoint_and_banner() {
    let srv = run(&[], &NO_LIMITS).await;
    let addr = srv.signaling_addr;
    let (status, head, body) = http_get(addr, "/healthz", &[]).await;
    assert_eq!((status, body.as_str()), (200, "ok\n"));
    assert!(head.contains("cache-control: no-store"));
    assert!(head.contains("x-content-type-options: nosniff"));
    let (_, _, body) = http_get(addr, "/signaling/healthz?probe=1", &[]).await;
    assert_eq!(body, "ok\n");
    let (status, head, body) = http_get(addr, "/", &[]).await;
    assert_eq!((status, body.as_str()), (200, "GuardedSleep signaling\n"));
    assert!(head.contains("x-frame-options: deny"));
    srv.shutdown().await;
}

#[tokio::test]
async fn websocket_upgrades_on_any_path_and_answers_ping() {
    let srv = run(&[], &NO_LIMITS).await;
    for path in ["/", "/signaling", "/a/b/c"] {
        let mut c = Client::connect(srv.signaling_addr, path).await;
        c.send(json!({"type":"ping"})).await;
        assert_eq!(c.recv().await["type"], "pong");
    }
    srv.shutdown().await;
}

#[tokio::test]
async fn hello_and_signal_relay_between_baby_and_parent() {
    let srv = run(&[], &NO_LIMITS).await;
    let mut baby = Client::connect(srv.signaling_addr, "/").await;
    baby.send(hello("baby", ROOM_V3, 3)).await;
    let welcome = baby.recv().await;
    assert_eq!(welcome["type"], "welcome");
    assert_eq!(baby.recv().await["type"], "members");
    assert_eq!(baby.recv().await["type"], "peer");

    let mut parent = Client::connect(srv.signaling_addr, "/").await;
    parent.send(hello("parent", ROOM_V3, 3)).await;
    assert_eq!(parent.recv().await["type"], "welcome");
    assert_eq!(parent.recv().await["type"], "members");
    let peer = parent.recv().await;
    assert_eq!(peer["peerId"], welcome["clientId"]);
    assert_eq!(baby.recv().await["type"], "members");
    assert_eq!(baby.recv().await["type"], "peer");

    baby.send(json!({"type":"signal","roomId":ROOM_V3,"payload":{"smoke":true}}))
        .await;
    let relayed = parent.recv().await;
    assert_eq!(relayed["type"], "signal");
    assert_eq!(relayed["from"], welcome["clientId"]);
    assert_eq!(relayed["payload"]["smoke"], true);

    parent
        .send(json!({"type":"signal","roomId":ROOM_V3,"payload":2}))
        .await;
    assert_eq!(baby.recv().await["payload"], 2);
    srv.shutdown().await;
}

#[tokio::test]
async fn second_baby_is_refused_and_closed() {
    let srv = run(&[], &NO_LIMITS).await;
    let mut a = Client::connect(srv.signaling_addr, "/").await;
    a.send(hello("baby", "r-baby-twice", 2)).await;
    assert_eq!(a.recv().await["type"], "welcome");
    let mut b = Client::connect(srv.signaling_addr, "/").await;
    b.send(hello("baby", "r-baby-twice", 2)).await;
    assert_eq!(b.recv().await["code"], "BABY_TAKEN");
    assert_eq!(b.close_code().await, Some(1008));
    srv.shutdown().await;
}

#[tokio::test]
async fn one_parent_per_room_by_default_and_configurable() {
    let srv = run(&[], &NO_LIMITS).await;
    let mut p1 = Client::connect(srv.signaling_addr, "/").await;
    p1.send(hello("parent", "r-cap", 2)).await;
    assert_eq!(p1.recv().await["type"], "welcome");
    let mut p2 = Client::connect(srv.signaling_addr, "/").await;
    p2.send(hello("parent", "r-cap", 2)).await;
    assert_eq!(p2.recv().await["code"], "PARENT_CAP");
    srv.shutdown().await;

    let mut env = NO_LIMITS.to_vec();
    env.push(("SIGNALING_MAX_PARENTS", "2"));
    let srv = run(&[], &env).await;
    let mut p1 = Client::connect(srv.signaling_addr, "/").await;
    p1.send(hello("parent", "r-cap", 2)).await;
    assert_eq!(p1.recv().await["type"], "welcome");
    let mut p2 = Client::connect(srv.signaling_addr, "/").await;
    p2.send(hello("parent", "r-cap", 2)).await;
    assert_eq!(p2.recv().await["type"], "welcome");
    srv.shutdown().await;
}

#[tokio::test]
async fn hello_attempts_are_rate_limited_per_client_address() {
    let srv = run(
        &[],
        &[
            ("SIGNALING_HELLO_RATE_PER_MIN", "2"),
            ("SIGNALING_NEW_ROOM_RATE_PER_MIN", "0"),
        ],
    )
    .await;
    for i in 0..2 {
        let mut c = Client::connect(srv.signaling_addr, "/").await;
        c.send(hello("baby", &format!("r-hello-{i}"), 2)).await;
        assert_eq!(c.recv().await["type"], "welcome");
    }
    let mut third = Client::connect(srv.signaling_addr, "/").await;
    third.send(hello("baby", "r-hello-3", 2)).await;
    assert_eq!(third.recv().await["code"], "RATE_LIMITED");
    assert_eq!(third.close_code().await, Some(1008));
    srv.shutdown().await;
}

#[tokio::test]
async fn spoofed_forwarded_for_gives_no_fresh_rate_limit_budget() {
    // Without SIGNALING_TRUST_PROXY, a spoofed header must not give a fresh rate-limit budget.
    let srv = run(
        &[],
        &[
            ("SIGNALING_HELLO_RATE_PER_MIN", "1"),
            ("SIGNALING_NEW_ROOM_RATE_PER_MIN", "0"),
        ],
    )
    .await;
    let mut a = Client::connect(srv.signaling_addr, "/").await;
    a.send(hello("baby", "r-xff-1", 2)).await;
    assert_eq!(a.recv().await["type"], "welcome");
    let mut b = Client::connect(srv.signaling_addr, "/").await;
    b.send(hello("baby", "r-xff-2", 2)).await;
    assert_eq!(b.recv().await["code"], "RATE_LIMITED");
    srv.shutdown().await;
}

#[tokio::test]
async fn origin_allowlist_rejects_browser_origins_but_not_native_clients() {
    let env = [
        ("SIGNALING_ALLOWED_ORIGINS", "https://good.example"),
        ("SIGNALING_HELLO_RATE_PER_MIN", "0"),
        ("SIGNALING_NEW_ROOM_RATE_PER_MIN", "0"),
    ];
    let srv = run(&[], &env).await;
    assert!(
        Client::try_connect(srv.signaling_addr, "/", Some("https://evil.example"))
            .await
            .is_err()
    );
    assert!(
        Client::try_connect(srv.signaling_addr, "/", Some("https://good.example"))
            .await
            .is_ok()
    );
    assert!(
        Client::try_connect(srv.signaling_addr, "/", None)
            .await
            .is_ok()
    );
    srv.shutdown().await;
}

#[tokio::test]
async fn pings_do_not_close_a_responsive_client() {
    // tungstenite answers pings on its own; a 2 s interval must keep an idle client connected.
    let env = [("SIGNALING_PING_INTERVAL_MS", "2000")];
    let srv = run(&[], &env).await;
    let mut c = Client::connect(srv.signaling_addr, "/").await;
    c.send(hello("baby", "r-live", 2)).await;
    assert_eq!(c.recv().await["type"], "welcome");
    assert_eq!(c.recv().await["type"], "members");
    assert_eq!(c.recv().await["type"], "peer");
    c.pump(Duration::from_millis(5_500)).await;
    c.send(json!({"type":"ping"})).await;
    assert_eq!(c.recv().await["type"], "pong");
    srv.shutdown().await;
}

fn web_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("gs-web-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("index.html"),
        "<!doctype html><title>gs</title>index-marker",
    )
    .unwrap();
    std::fs::write(dir.join("app.js"), "console.log('asset-marker')").unwrap();
    dir
}

#[tokio::test]
async fn serves_the_web_ui_with_spa_fallback_and_keeps_health_and_websocket() {
    let dir = web_dir("public");
    let srv = run(&["--serve-web", dir.to_str().unwrap()], &NO_LIMITS).await;
    let addr = srv.signaling_addr;
    let (status, _, body) = http_get(addr, "/", &[]).await;
    assert_eq!(status, 200);
    assert!(body.contains("index-marker"));
    let (_, _, body) = http_get(addr, "/app.js", &[]).await;
    assert!(body.contains("asset-marker"));
    let (status, _, body) = http_get(addr, "/some/client/route", &[]).await;
    assert_eq!(status, 200);
    assert!(body.contains("index-marker"));
    let (status, head, _) = http_get(addr, "/../../etc/passwd", &[]).await;
    assert!(
        status == 200 || status == 404 || status == 400,
        "status {status}"
    );
    assert!(!head.contains("root:"));
    let (_, _, body) = http_get(addr, "/healthz", &[]).await;
    assert_eq!(body, "ok\n");
    let mut c = Client::connect(addr, "/signaling").await;
    c.send(json!({"type":"ping"})).await;
    assert_eq!(c.recv().await["type"], "pong");
    srv.shutdown().await;
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn missing_web_dir_is_a_startup_error() {
    let cli = Cli::parse_from([
        "x",
        "--bind",
        "127.0.0.1",
        "--port",
        "0",
        "--no-stun",
        "--serve-web",
        "/nonexistent/gs",
    ]);
    let cfg = Config::new(cli, Limits::from_lookup(|_| None)).unwrap();
    assert!(start(cfg).await.is_err());
}

// Local mode (ADR-021 section 1).

const PSK: [u8; 32] = [5; 32];

struct Local {
    srv: RunningServer,
    lan: SocketAddr,
    ui: SocketAddr,
    dir: std::path::PathBuf,
}

async fn local(name: &str) -> Local {
    let dir = web_dir(name);
    let srv = run(
        &[
            "--local",
            "--ui-port",
            "0",
            "--serve-web",
            dir.to_str().unwrap(),
        ],
        &NO_LIMITS,
    )
    .await;
    let lan = srv.signaling_addr;
    let ui = srv.ui_addrs[0];
    Local { srv, lan, ui, dir }
}

async fn register(l: &Local, room: &str, key: &str) -> u16 {
    let host = format!("127.0.0.1:{}", l.ui.port());
    http_request(
        l.ui,
        "PUT",
        &format!("/admission/{room}"),
        &[("Host", &host)],
        key.as_bytes(),
    )
    .await
    .0
}

#[tokio::test]
async fn local_mode_binds_the_ui_to_loopback_and_checks_host_and_origin() {
    let l = local("host").await;
    assert!(l.ui.ip().is_loopback());
    let port = l.ui.port();
    let ok_host = format!("localhost:{port}");
    let (status, _, body) = http_get(l.ui, "/", &[("Host", &ok_host)]).await;
    assert_eq!(status, 200);
    assert!(body.contains("index-marker"));

    // DNS rebinding: a foreign name pointing at 127.0.0.1.
    let (status, _, _) = http_get(l.ui, "/", &[("Host", &format!("evil.example:{port}"))]).await;
    assert_eq!(status, 403);
    // Right host, wrong port.
    let (status, _, _) = http_get(l.ui, "/", &[("Host", "localhost:1")]).await;
    assert_eq!(status, 403);
    // Cross-site page talking to the loopback UI.
    let host = format!("127.0.0.1:{port}");
    let (status, _, _) = http_get(
        l.ui,
        "/",
        &[("Host", &host), ("Origin", "https://evil.example")],
    )
    .await;
    assert_eq!(status, 403);
    let (status, _, _) = http_get(
        l.ui,
        "/",
        &[
            ("Host", &host),
            ("Origin", &format!("http://localhost:{port}")),
        ],
    )
    .await;
    assert_eq!(status, 200);
    // The same applies to the WebSocket upgrade on loopback.
    assert!(
        Client::try_connect(l.ui, "/", Some("https://evil.example"))
            .await
            .is_err()
    );
    assert!(
        Client::try_connect(l.ui, "/", Some(&format!("http://localhost:{port}")))
            .await
            .is_ok()
    );

    // The LAN port serves no UI and no registration endpoint.
    let (_, _, body) = http_get(l.lan, "/", &[]).await;
    assert_eq!(body, "GuardedSleep signaling\n");
    let (status, _, _) =
        http_request(l.lan, "PUT", &format!("/admission/{ROOM_V3}"), &[], b"x").await;
    assert_eq!(
        status, 200,
        "falls through to the banner; nothing is registered"
    );
    l.srv.shutdown().await;
    std::fs::remove_dir_all(l.dir).unwrap();
}

#[tokio::test]
async fn local_mode_admits_only_joiners_that_hold_the_registered_key() {
    let l = local("admit").await;
    let key = derive_admission_key(&PSK);
    assert_eq!(register(&l, ROOM_V3, &key.to_b64url()).await, 204);
    assert_eq!(
        register(&l, ROOM_V3, &key.to_b64url()).await,
        204,
        "idempotent"
    );
    assert_eq!(
        register(&l, ROOM_V3, &derive_admission_key(&[6; 32]).to_b64url()).await,
        409
    );
    assert_eq!(register(&l, ROOM_V3, "not-a-key").await, 400);
    assert_eq!(register(&l, "short", &key.to_b64url()).await, 400);

    // Wrong key.
    let mut bad = Client::connect(l.lan, "/").await;
    let nonce = bad.recv().await["nonce"].as_str().unwrap().to_owned();
    let mac = admission_mac(
        &derive_admission_key(&[6; 32]),
        &nonce,
        Role::Parent,
        ROOM_V3,
    )
    .unwrap();
    bad.send(
        json!({"type":"admit","role":"parent","roomId":ROOM_V3,"protocolVersion":3,"mac":mac}),
    )
    .await;
    assert_eq!(bad.recv().await["code"], "ADMISSION_FAILED");
    assert_eq!(bad.close_code().await, Some(1008));

    // A plain hello is refused.
    let mut plain = Client::connect(l.lan, "/").await;
    assert_eq!(plain.recv().await["type"], "challenge");
    plain.send(hello("parent", ROOM_V3, 3)).await;
    assert_eq!(plain.recv().await["code"], "ADMISSION_REQUIRED");

    // Right key; the replayed first message on a new connection fails.
    let mut good = Client::connect(l.lan, "/").await;
    let nonce = good.recv().await["nonce"].as_str().unwrap().to_owned();
    let mac = admission_mac(&key, &nonce, Role::Parent, ROOM_V3).unwrap();
    let admit =
        json!({"type":"admit","role":"parent","roomId":ROOM_V3,"protocolVersion":3,"mac":mac});
    good.send(admit.clone()).await;
    assert_eq!(good.recv().await["type"], "welcome");
    let mut replay = Client::connect(l.lan, "/").await;
    assert_eq!(replay.recv().await["type"], "challenge");
    replay.send(admit).await;
    assert_eq!(replay.recv().await["code"], "ADMISSION_FAILED");

    // Unregistering stops admission for the room.
    let host = format!("localhost:{}", l.ui.port());
    let (status, _, _) = http_request(
        l.ui,
        "DELETE",
        &format!("/admission/{ROOM_V3}"),
        &[("Host", &host)],
        b"",
    )
    .await;
    assert_eq!(status, 204);
    let mut late = Client::connect(l.lan, "/").await;
    let nonce = late.recv().await["nonce"].as_str().unwrap().to_owned();
    let mac = admission_mac(&key, &nonce, Role::Baby, ROOM_V3).unwrap();
    late.send(json!({"type":"admit","role":"baby","roomId":ROOM_V3,"protocolVersion":3,"mac":mac}))
        .await;
    assert_eq!(late.recv().await["code"], "ADMISSION_FAILED");
    l.srv.shutdown().await;
    std::fs::remove_dir_all(l.dir).unwrap();
}

#[tokio::test]
async fn unauthenticated_lan_sockets_are_closed_after_five_seconds() {
    let l = local("timeout").await;
    let mut idle = Client::connect(l.lan, "/").await;
    assert_eq!(idle.recv().await["type"], "challenge");
    let started = std::time::Instant::now();
    assert_eq!(idle.close_code().await, Some(1008));
    let waited = started.elapsed();
    assert!(
        waited >= Duration::from_secs(4) && waited < Duration::from_secs(8),
        "{waited:?}"
    );
    l.srv.shutdown().await;
    std::fs::remove_dir_all(l.dir).unwrap();
}

// STUN.

fn binding_request(padding: usize) -> Vec<u8> {
    let mut attr = vec![0x80, 0x22, 0x00, padding as u8];
    attr.extend(std::iter::repeat_n(b'x', padding));
    let body = if padding == 0 { Vec::new() } else { attr };
    let mut b = vec![0x00, 0x01];
    b.extend_from_slice(&(body.len() as u16).to_be_bytes());
    b.extend_from_slice(&0x2112_A442u32.to_be_bytes());
    b.extend_from_slice(&[7u8; 12]);
    b.extend_from_slice(&body);
    b
}

async fn stun_roundtrip(sock: &UdpSocket, to: SocketAddr, req: &[u8]) -> Option<Vec<u8>> {
    sock.send_to(req, to).await.unwrap();
    let mut buf = [0u8; 600];
    match timeout(Duration::from_millis(400), sock.recv_from(&mut buf)).await {
        Ok(Ok((n, _))) => Some(buf[..n].to_vec()),
        _ => None,
    }
}

async fn run_stun(args: &[&str]) -> RunningServer {
    let mut argv = vec![
        "guardedsleep-server",
        "--bind",
        "127.0.0.1",
        "--port",
        "0",
        "--stun-port",
        "0",
    ];
    argv.extend_from_slice(args);
    let cfg = Config::new(Cli::parse_from(argv), Limits::from_lookup(|_| None)).unwrap();
    start(cfg).await.unwrap()
}

#[tokio::test]
async fn stun_answers_binding_requests_with_the_senders_address_and_never_grows_them() {
    let srv = run_stun(&[]).await;
    let to = srv.stun_addr.unwrap();
    let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let me = sock.local_addr().unwrap();

    let req = binding_request(16);
    let resp = stun_roundtrip(&sock, to, &req).await.expect("response");
    assert!(resp.len() <= req.len());
    assert_eq!(&resp[0..2], &[0x01, 0x01]);
    let port = u16::from_be_bytes([resp[26], resp[27]]) ^ 0x2112;
    assert_eq!(port, me.port());
    assert_eq!(&resp[28..32], &[127 ^ 0x21, 0x12, 0xa4, 0x42 ^ 1]);

    // A bare 20-byte request would need a longer response: no answer by default.
    assert!(
        stun_roundtrip(&sock, to, &binding_request(0))
            .await
            .is_none()
    );
    // Other STUN message types and junk get no answer either.
    let mut indication = binding_request(16);
    indication[0..2].copy_from_slice(&0x0011u16.to_be_bytes());
    assert!(stun_roundtrip(&sock, to, &indication).await.is_none());
    assert!(
        stun_roundtrip(&sock, to, b"GET / HTTP/1.1\r\n\r\n")
            .await
            .is_none()
    );
    srv.shutdown().await;
}

#[tokio::test]
async fn stun_can_answer_bare_requests_and_rate_limits_per_address() {
    let srv = run_stun(&["--stun-allow-short-requests", "--stun-rate-per-min", "2"]).await;
    let to = srv.stun_addr.unwrap();
    let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let bare = binding_request(0);
    assert_eq!(stun_roundtrip(&sock, to, &bare).await.unwrap().len(), 32);
    assert!(stun_roundtrip(&sock, to, &bare).await.is_some());
    assert!(
        stun_roundtrip(&sock, to, &bare).await.is_none(),
        "over the per-address budget"
    );
    srv.shutdown().await;
}

#[tokio::test]
async fn stun_can_be_disabled() {
    let srv = run(&[], &NO_LIMITS).await;
    assert!(srv.stun_addr.is_none());
    srv.shutdown().await;
}

#[tokio::test]
async fn healthcheck_probe_reports_a_running_server() {
    let srv = run(&[], &NO_LIMITS).await;
    let port = srv.signaling_addr.port();
    assert!(
        tokio::task::spawn_blocking(move || guardedsleep_server::healthcheck(port))
            .await
            .unwrap()
    );
    srv.shutdown().await;
    assert!(
        !tokio::task::spawn_blocking(move || guardedsleep_server::healthcheck(port))
            .await
            .unwrap()
    );
}
