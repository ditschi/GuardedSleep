//! HTTP surface: WebSocket signaling on any path, `/healthz`, the optional static web UI and, in
//! local mode, the loopback-only admission-key registration.
//!
//! Like the Node server, WebSocket upgrades are accepted on any path so that a reverse proxy can
//! route e.g. `/signaling` here without stripping the prefix, and `/healthz` answers under any
//! prefix.

use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, DefaultBodyLimit, Path as UrlPath, Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::put;
use guardedsleep_core::pairing::AdmissionKey;
use tokio::sync::{mpsc, watch};
use tokio::time::timeout;
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

use crate::csp;
use crate::hub::{Hub, KeyError, Out};

/// Longest a single send to a socket may block before the socket is given up.
const SEND_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct AppState {
    pub hub: Arc<Hub>,
    pub origins: Arc<Vec<String>>,
    pub trust_proxy: bool,
    pub trusted_hops: usize,
    pub max_message_bytes: usize,
    pub shutdown: watch::Receiver<bool>,
    pub web: Option<WebUi>,
    /// Port of the loopback UI listener (local mode); enables the Host/Origin guard.
    pub loopback_port: Option<u16>,
}

/// The static web UI and the Content-Security-Policy it is served with.
#[derive(Clone)]
pub struct WebUi {
    files: ServeDir<ServeFile>,
    csp: HeaderValue,
}

/// `extra_connect`: the WebSocket origins the web build allows besides the serving host.
pub fn static_service(dir: &Path, extra_connect: &[String]) -> WebUi {
    WebUi {
        files: ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html"))),
        // The origins were validated when they were loaded, so the value is plain visible ASCII.
        csp: HeaderValue::from_str(&csp::header_value(extra_connect))
            .unwrap_or_else(|_| HeaderValue::from_static("default-src 'none'")),
    }
}

/// Signaling plus health and (optionally) the static UI on one listener.
pub fn public_router(state: AppState) -> Router {
    Router::new().fallback(endpoint).with_state(state)
}

/// Loopback listener of local mode: Host/Origin guard, key registration, UI and signaling.
pub fn loopback_router(state: AppState) -> Router {
    Router::new()
        .route(
            "/admission/{room}",
            put(register_key).delete(unregister_key),
        )
        .layer(DefaultBodyLimit::max(256))
        .fallback(endpoint)
        .layer(middleware::from_fn_with_state(state.clone(), guard))
        .with_state(state)
}

const SECURITY_HEADERS: [(HeaderName, &str); 3] = [
    (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    (header::X_FRAME_OPTIONS, "DENY"),
    (header::REFERRER_POLICY, "no-referrer"),
];

fn with_security_headers(mut res: Response) -> Response {
    for (name, value) in SECURITY_HEADERS {
        res.headers_mut()
            .insert(name, HeaderValue::from_static(value));
    }
    res
}

fn text(status: StatusCode, body: &'static str) -> Response {
    let mut res = (
        status,
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body,
    )
        .into_response();
    res = with_security_headers(res);
    res
}

/// Empty allowlist allows everything. Native clients and tooling send no `Origin`; they pass even
/// when an allowlist is set (the same rule as the Node server).
pub fn origin_allowed(allowed: &[String], origin: Option<&str>) -> bool {
    if allowed.is_empty() {
        return true;
    }
    match origin.map(str::trim) {
        None | Some("") => true,
        Some(o) => allowed.iter().any(|a| a == o),
    }
}

/// The client address behind `hops` trusted proxies: proxies append the address they saw to
/// `X-Forwarded-For`, so only the rightmost `hops` entries can be trusted.
pub fn client_ip(peer: IpAddr, forwarded_for: &str, trust_proxy: bool, hops: usize) -> IpAddr {
    if !trust_proxy {
        return peer;
    }
    let entries: Vec<&str> = forwarded_for
        .split(',')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .collect();
    entries
        .len()
        .checked_sub(hops)
        .and_then(|i| entries.get(i))
        .and_then(|e| e.parse::<IpAddr>().ok())
        .unwrap_or(peer)
}

fn forwarded_for(headers: &HeaderMap) -> String {
    headers
        .get_all(header::HeaderName::from_static("x-forwarded-for"))
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect::<Vec<_>>()
        .join(",")
}

fn host_allowed(host: &str, port: u16) -> bool {
    let host = host.to_ascii_lowercase();
    [
        format!("localhost:{port}"),
        format!("127.0.0.1:{port}"),
        format!("[::1]:{port}"),
    ]
    .contains(&host)
}

/// DNS-rebinding and cross-site protection of the loopback listener (ADR-021 section 1): the `Host`
/// must be a loopback name with the listener's port, and a present `Origin` must be a page served
/// from such a host.
async fn guard(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let Some(port) = st.loopback_port else {
        return next.run(req).await;
    };
    let host_ok = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| host_allowed(h, port));
    let origin_ok = match req.headers().get(header::ORIGIN) {
        None => true,
        Some(o) => o
            .to_str()
            .ok()
            .and_then(|o| o.strip_prefix("http://"))
            .is_some_and(|h| host_allowed(h, port)),
    };
    if !host_ok || !origin_ok {
        return text(StatusCode::FORBIDDEN, "forbidden\n");
    }
    next.run(req).await
}

fn is_v3_room_id(room: &str) -> bool {
    room.len() == 22
        && room
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// `PUT /admission/<room>` with the base64url admission key as the body (loopback only). Idempotent
/// for the same key; a different key for a known room is refused until `DELETE`.
async fn register_key(
    State(st): State<AppState>,
    UrlPath(room): UrlPath<String>,
    body: Bytes,
) -> Response {
    let key = std::str::from_utf8(&body)
        .ok()
        .and_then(AdmissionKey::from_b64url);
    let (Some(key), true) = (key, is_v3_room_id(&room)) else {
        return text(StatusCode::BAD_REQUEST, "bad room or key\n");
    };
    match st.hub.set_admission_key(&room, key) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(KeyError::Conflict) => text(StatusCode::CONFLICT, "room has another key\n"),
        Err(KeyError::Full) => text(StatusCode::SERVICE_UNAVAILABLE, "too many rooms\n"),
    }
}

async fn unregister_key(State(st): State<AppState>, UrlPath(room): UrlPath<String>) -> Response {
    st.hub.remove_admission_key(&room);
    StatusCode::NO_CONTENT.into_response()
}

async fn endpoint(
    State(st): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
    req: Request,
) -> Response {
    if let Ok(ws) = ws {
        let origin = req
            .headers()
            .get(header::ORIGIN)
            .and_then(|o| o.to_str().ok());
        if !origin_allowed(&st.origins, origin) {
            return text(StatusCode::FORBIDDEN, "origin not allowed\n");
        }
        let ip = client_ip(
            peer.ip(),
            &forwarded_for(req.headers()),
            st.trust_proxy,
            st.trusted_hops,
        );
        let max = st.max_message_bytes;
        return ws
            .max_message_size(max)
            .max_frame_size(max)
            .on_upgrade(move |socket| session(socket, st, ip));
    }
    let path = req.uri().path();
    if path == "/healthz" || path.ends_with("/healthz") {
        return text(StatusCode::OK, "ok\n");
    }
    match st.web.clone() {
        Some(web) => match web.files.oneshot(req).await {
            Ok(res) => {
                let mut res = with_security_headers(res.into_response());
                res.headers_mut()
                    .insert(header::CONTENT_SECURITY_POLICY, web.csp);
                res
            }
            Err(never) => match never {},
        },
        None => text(StatusCode::OK, "GuardedSleep signaling\n"),
    }
}

async fn send(socket: &mut WebSocket, msg: Message) -> bool {
    matches!(timeout(SEND_TIMEOUT, socket.send(msg)).await, Ok(Ok(())))
}

async fn session(mut socket: WebSocket, st: AppState, ip: IpAddr) {
    let (id, mut outbox): (_, mpsc::Receiver<Out>) = st.hub.connect(ip);
    let mut shutdown = st.shutdown.clone();
    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Text(t))) => st.hub.frame(id, t.as_bytes()),
                Some(Ok(Message::Binary(b))) => st.hub.frame(id, &b),
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => st.hub.pong(id),
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
            },
            out = outbox.recv() => match out {
                Some(Out::Text(t)) => {
                    if !send(&mut socket, Message::Text(t.into())).await {
                        break;
                    }
                }
                Some(Out::Ping) => {
                    if !send(&mut socket, Message::Ping(Bytes::new())).await {
                        break;
                    }
                }
                Some(Out::Close(reason)) => {
                    let frame = CloseFrame { code: reason.code(), reason: reason.text().into() };
                    let _ = send(&mut socket, Message::Close(Some(frame))).await;
                    break;
                }
                None => break,
            },
            _ = shutdown.changed() => break,
        }
    }
    st.hub.closed(id);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn uses_the_rightmost_forwarded_hop_and_ignores_spoofed_leading_entries() {
        let peer = ip("10.0.0.9");
        assert_eq!(
            client_ip(peer, "198.51.100.1, 203.0.113.1", true, 1),
            ip("203.0.113.1")
        );
        assert_eq!(
            client_ip(peer, "198.51.100.2, 203.0.113.1", true, 1),
            ip("203.0.113.1")
        );
        assert_eq!(
            client_ip(peer, "198.51.100.2, 203.0.113.2", true, 1),
            ip("203.0.113.2")
        );
    }

    #[test]
    fn uses_the_nth_hop_from_the_right() {
        let peer = ip("10.0.0.9");
        let xff = "198.51.100.1, 203.0.113.1, 10.0.0.1";
        assert_eq!(client_ip(peer, xff, true, 2), ip("203.0.113.1"));
        assert_eq!(client_ip(peer, xff, true, 3), ip("198.51.100.1"));
    }

    #[test]
    fn falls_back_to_the_peer_without_trust_or_usable_entries() {
        let peer = ip("10.0.0.9");
        assert_eq!(client_ip(peer, "203.0.113.1", false, 1), peer);
        assert_eq!(client_ip(peer, "", true, 1), peer);
        assert_eq!(client_ip(peer, "203.0.113.1", true, 2), peer);
        assert_eq!(client_ip(peer, "not-an-ip", true, 1), peer);
    }

    fn list(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn allows_all_origins_when_the_allowlist_is_unset() {
        assert!(origin_allowed(&[], Some("https://evil.example")));
    }

    #[test]
    fn allows_an_empty_origin_when_the_allowlist_is_set() {
        let allowed = list(&["https://app.example"]);
        assert!(origin_allowed(&allowed, None));
        assert!(origin_allowed(&allowed, Some("")));
    }

    #[test]
    fn checks_the_origin_against_the_allowlist() {
        let allowed = list(&["https://good.example", "https://other.example"]);
        assert!(origin_allowed(&allowed, Some("https://good.example")));
        assert!(!origin_allowed(&allowed, Some("https://evil.example")));
    }

    #[test]
    fn loopback_hosts_need_the_listener_port() {
        assert!(host_allowed("localhost:8080", 8080));
        assert!(host_allowed("LOCALHOST:8080", 8080));
        assert!(host_allowed("127.0.0.1:8080", 8080));
        assert!(host_allowed("[::1]:8080", 8080));
        assert!(!host_allowed("localhost:9999", 8080));
        assert!(!host_allowed("localhost", 8080));
        assert!(!host_allowed("evil.example:8080", 8080));
        assert!(!host_allowed("192.168.1.5:8080", 8080));
        assert!(!host_allowed("localhost.evil.example:8080", 8080));
    }

    #[test]
    fn room_ids_are_22_base64url_characters() {
        assert!(is_v3_room_id("AAAAAAAAAAAAAAAAAAAAAA"));
        assert!(!is_v3_room_id("short"));
        assert!(!is_v3_room_id("AAAAAAAAAAAAAAAAAAAAA+"));
    }
}
