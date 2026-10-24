//! End-to-end smoke (`just smoke`): starts the built binary as a child process, configured through
//! the environment only, and checks hello and signal relay over a real WebSocket. It replaces
//! the retired Node `smoke-relay.ts` (`git show ae71762d:apps/signaling-server/scripts/smoke-relay.ts`).

mod common;

use std::net::{SocketAddr, TcpListener};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::{Client, hello, http_get};
use serde_json::json;

struct Guard(Child);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn spawn_server() -> (Guard, SocketAddr) {
    let port = free_port();
    let child = Command::new(env!("CARGO_BIN_EXE_guardedsleep-server"))
        .env("PORT", port.to_string())
        .env("GS_BIND", "127.0.0.1")
        .env("STUN_DISABLED", "1")
        .env_remove("NODE_ENV")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while tokio::net::TcpStream::connect(addr).await.is_err() {
        assert!(Instant::now() < deadline, "server did not start");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    (Guard(child), addr)
}

#[tokio::test]
async fn hello_and_signal_relay_through_the_built_binary() {
    let (_server, addr) = spawn_server().await;
    let (status, _, body) = http_get(addr, "/healthz", &[]).await;
    assert_eq!((status, body.as_str()), (200, "ok\n"));

    let room = "smoke-room";
    let mut a = Client::connect(addr, "/").await;
    a.send(hello("baby", room, 2)).await;
    let wa = a.recv().await;
    assert_eq!(wa["type"], "welcome");
    assert_eq!(a.recv().await["type"], "members");
    assert_eq!(a.recv().await["type"], "peer");

    let mut b = Client::connect(addr, "/").await;
    b.send(hello("parent", room, 2)).await;
    let wb = b.recv().await;
    assert_eq!(wb["type"], "welcome");
    assert_eq!(b.recv().await["type"], "members");
    assert_eq!(b.recv().await["peerId"], wa["clientId"]);
    let members = a.recv().await;
    assert!(
        members["peers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["clientId"] == wb["clientId"])
    );
    assert_eq!(a.recv().await["peerId"], wb["clientId"]);

    // The payload is a sealed envelope `{"env": base64url}` since the alpha connection model; the
    // relay forwards it unread.
    let envelope = json!({"env": "AQEBsmoke_smoke-smoke0123456789"});
    a.send(json!({"type":"signal","roomId":room,"payload":envelope}))
        .await;
    let relayed = b.recv().await;
    assert_eq!(relayed["type"], "signal");
    assert_eq!(relayed["from"], wa["clientId"]);
    assert_eq!(relayed["payload"], envelope);

    let v3_room = "smokeSmokeSmokeSmoke22";
    let mut c = Client::connect(addr, "/").await;
    c.send(hello("baby", v3_room, 3)).await;
    assert_eq!(c.recv().await["type"], "welcome");
    let mut d = Client::connect(addr, "/").await;
    d.send(hello("parent", v3_room, 2)).await;
    assert_eq!(d.recv().await["code"], "UNSUPPORTED_PROTOCOL");
}
