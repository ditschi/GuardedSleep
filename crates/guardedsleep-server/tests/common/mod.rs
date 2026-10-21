#![allow(dead_code)]

use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

pub const STEP: Duration = Duration::from_secs(5);
pub const ROOM_V3: &str = "AAAAAAAAAAAAAAAAAAAAAA";

pub struct Client {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl Client {
    pub async fn connect(addr: SocketAddr, path: &str) -> Client {
        Self::try_connect(addr, path, None)
            .await
            .expect("websocket handshake")
    }

    pub async fn try_connect(
        addr: SocketAddr,
        path: &str,
        origin: Option<&str>,
    ) -> Result<Client, tokio_tungstenite::tungstenite::Error> {
        let mut req = format!("ws://{addr}{path}").into_client_request().unwrap();
        if let Some(o) = origin {
            req.headers_mut().insert("origin", o.parse().unwrap());
        }
        let (ws, _) = timeout(STEP, connect_async(req))
            .await
            .expect("handshake timed out")?;
        Ok(Client { ws })
    }

    pub async fn send(&mut self, v: Value) {
        self.ws
            .send(Message::text(v.to_string()))
            .await
            .expect("send");
    }

    /// Next JSON text message; `None` when the socket closed first.
    pub async fn next(&mut self) -> Option<Value> {
        loop {
            match timeout(STEP, self.ws.next()).await.expect("recv timed out") {
                Some(Ok(Message::Text(t))) => return Some(serde_json::from_str(&t).unwrap()),
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return None,
                Some(Ok(_)) => {}
            }
        }
    }

    /// Keeps reading (which also answers server pings) for `d`; panics if the server closes.
    pub async fn pump(&mut self, d: Duration) {
        let end = tokio::time::Instant::now() + d;
        while let Some(left) = end.checked_duration_since(tokio::time::Instant::now()) {
            if let Ok(msg) = timeout(left, self.ws.next()).await {
                assert!(matches!(msg, Some(Ok(_))), "server closed the connection");
            }
        }
    }

    pub async fn recv(&mut self) -> Value {
        self.next().await.expect("socket closed")
    }

    /// Close code once the server closes the socket (drains pending messages).
    pub async fn close_code(&mut self) -> Option<u16> {
        loop {
            match timeout(STEP * 2, self.ws.next())
                .await
                .expect("close timed out")
            {
                Some(Ok(Message::Close(frame))) => return frame.map(|f| u16::from(f.code)),
                Some(Ok(_)) => {}
                Some(Err(_)) | None => return None,
            }
        }
    }
}

pub fn hello(role: &str, room: &str, version: u32) -> Value {
    serde_json::json!({"type":"hello","role":role,"roomId":room,"protocolVersion":version})
}

/// Minimal HTTP/1.1 GET; returns the status line code, headers (lowercase) and body.
pub async fn http_get(
    addr: SocketAddr,
    path: &str,
    extra_headers: &[(&str, &str)],
) -> (u16, String, String) {
    http_request(addr, "GET", path, extra_headers, b"").await
}

pub async fn http_request(
    addr: SocketAddr,
    method: &str,
    path: &str,
    extra_headers: &[(&str, &str)],
    body: &[u8],
) -> (u16, String, String) {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let mut req = format!("{method} {path} HTTP/1.1\r\n");
    if !extra_headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("host"))
    {
        req.push_str(&format!("Host: {addr}\r\n"));
    }
    for (k, v) in extra_headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    ));
    s.write_all(req.as_bytes()).await.unwrap();
    s.write_all(body).await.unwrap();
    let mut raw = Vec::new();
    timeout(STEP, s.read_to_end(&mut raw))
        .await
        .expect("http read timed out")
        .unwrap();
    let raw = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((&raw, ""));
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    (status, head.to_ascii_lowercase(), body.to_owned())
}
