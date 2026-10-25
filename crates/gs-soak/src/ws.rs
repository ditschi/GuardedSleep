//! One WebSocket per thread: the platform side of a mailbox (`signaling-mailboxes`). The core
//! decides when to open, send and close; this module only owns the socket and reports what
//! happened. Reads time out every 100 ms so one loop serves both directions, and a half-open
//! socket never blocks the thread forever (the core's own subscription timers notice silence).

use std::io::ErrorKind;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use rustls::{ClientConfig, RootCertStore};
use tungstenite::client::IntoClientRequest;
use tungstenite::client_tls_with_config;
use tungstenite::{Connector, HandshakeError, Message};

const TCP_CONNECT: Duration = Duration::from_secs(10);
const HANDSHAKE_BUDGET: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(100);

/// What the main loop sends to a socket thread.
pub enum Out {
    Text(String),
    Close,
}

/// What a socket thread reports.
#[derive(Debug)]
pub enum Ev {
    Opened,
    Text(String),
    /// The socket is gone (`reason` is diagnostic: an error class, never frame content).
    Closed(String),
}

/// An event of the socket `(actor, id)` generation `gen`; a late event of an older generation is
/// discarded by the receiver, as the core's platform contract demands.
#[derive(Debug)]
pub struct Msg {
    pub actor: usize,
    pub id: u32,
    pub gen_no: u64,
    pub ev: Ev,
}

fn tls_config() -> Result<Arc<ClientConfig>, String> {
    static CONFIG: OnceLock<Result<Arc<ClientConfig>, String>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let mut roots = RootCertStore::empty();
            let (added, _) =
                roots.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);
            if added == 0 {
                return Err("tls: no trusted root certificates on this system".to_owned());
            }
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let config = ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .map_err(|e| format!("tls: {e}"))?
                .with_root_certificates(roots)
                .with_no_client_auth();
            Ok(Arc::new(config))
        })
        .clone()
}

/// `host` and `port` of a `wss://host[:port][/path]` URL.
pub fn split_url(url: &str) -> Option<(String, u16)> {
    let rest = url.strip_prefix("wss://")?;
    let authority = rest.split('/').next()?;
    if authority.is_empty() {
        return None;
    }
    match authority.rsplit_once(':') {
        Some((host, port)) => Some((host.to_owned(), port.parse().ok()?)),
        None => Some((authority.to_owned(), 443)),
    }
}

fn is_would_block(e: &std::io::Error) -> bool {
    matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
}

/// Opens the socket on its own thread. The returned sender feeds frames out and closes it.
pub fn spawn(url: String, actor: usize, id: u32, gen_no: u64, events: Sender<Msg>) -> Sender<Out> {
    let (tx, rx) = channel::<Out>();
    std::thread::spawn(move || {
        let report = |ev: Ev| {
            let _ = events.send(Msg {
                actor,
                id,
                gen_no,
                ev,
            });
        };
        match run(&url, &rx, &report) {
            Ok(()) => {}
            Err(reason) => report(Ev::Closed(reason)),
        }
    });
    tx
}

fn run(url: &str, rx: &Receiver<Out>, report: &dyn Fn(Ev)) -> Result<(), String> {
    let (host, port) = split_url(url).ok_or("bad url")?;
    let addrs = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|_| "dns".to_owned())?;
    let mut stream = None;
    for addr in addrs {
        if let Ok(s) = TcpStream::connect_timeout(&addr, TCP_CONNECT) {
            stream = Some(s);
            break;
        }
    }
    let stream = stream.ok_or("tcp")?;
    let _ = stream.set_nodelay(true);
    stream
        .set_read_timeout(Some(POLL))
        .map_err(|_| "socket".to_owned())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|_| "socket".to_owned())?;
    let request = url
        .into_client_request()
        .map_err(|_| "request".to_owned())?;
    let connector = Connector::Rustls(tls_config()?);
    let deadline = Instant::now() + HANDSHAKE_BUDGET;
    let mut attempt = client_tls_with_config(request, stream, None, Some(connector));
    let mut ws = loop {
        match attempt {
            Ok((ws, _)) => break ws,
            Err(HandshakeError::Interrupted(mid)) => {
                if Instant::now() > deadline {
                    return Err("handshake timeout".into());
                }
                if matches!(rx.try_recv(), Ok(Out::Close)) {
                    return Ok(());
                }
                attempt = mid.handshake();
            }
            Err(HandshakeError::Failure(e)) => {
                return Err(format!("handshake: {}", error_class(&e)));
            }
        }
    };
    report(Ev::Opened);
    loop {
        loop {
            match rx.try_recv() {
                Ok(Out::Text(t)) => match ws.send(Message::text(t)) {
                    Ok(()) => {}
                    Err(tungstenite::Error::Io(e)) if is_would_block(&e) => {}
                    Err(e) => return Err(format!("send: {}", error_class(&e))),
                },
                Ok(Out::Close) | Err(TryRecvError::Disconnected) => {
                    let _ = ws.close(None);
                    let _ = ws.flush();
                    return Ok(());
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        match ws.read() {
            Ok(Message::Text(t)) => report(Ev::Text(t.as_str().to_owned())),
            Ok(Message::Close(_)) => return Err("closed by relay".into()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if is_would_block(&e) => {}
            Err(e) => return Err(format!("read: {}", error_class(&e))),
        }
        // A queued pong (answer to the relay's ping) leaves with the next flush.
        match ws.flush() {
            Ok(()) => {}
            Err(tungstenite::Error::Io(e)) if is_would_block(&e) => {}
            Err(e) => return Err(format!("flush: {}", error_class(&e))),
        }
    }
}

/// A short class of an error for the report; the text of an error can echo a server's words, so
/// only the variant is kept.
fn error_class(e: &tungstenite::Error) -> &'static str {
    match e {
        tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed => "closed",
        tungstenite::Error::Io(_) => "io",
        tungstenite::Error::Tls(_) => "tls",
        tungstenite::Error::Capacity(_) => "capacity",
        tungstenite::Error::Protocol(_) => "protocol",
        tungstenite::Error::Http(_) | tungstenite::Error::HttpFormat(_) => "http",
        tungstenite::Error::Url(_) => "url",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::split_url;

    #[test]
    fn splits_host_and_port() {
        assert_eq!(split_url("wss://nos.lol"), Some(("nos.lol".into(), 443)));
        assert_eq!(
            split_url("wss://relay.example:8443/path"),
            Some(("relay.example".into(), 8443))
        );
        assert_eq!(split_url("ws://relay.example"), None);
        assert_eq!(split_url("wss://"), None);
    }
}
