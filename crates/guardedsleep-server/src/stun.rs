//! Minimal STUN Binding responder (RFC 5389 sections 6, 7.3 and 15.2).
//!
//! It answers Binding requests and nothing else: every other message type is dropped without a
//! reply. The response carries one XOR-MAPPED-ADDRESS (32 bytes for IPv4, 44 for IPv6) and is
//! bounded: at most [`MAX_RESPONSE_LEN`] bytes and at most 2.2 times the request, so a bare 20-byte
//! request from a browser or libwebrtc is answered while the server stays useless as an amplifier.
//! The per-address rate limit in [`serve`] is part of that bound. The decoder never panics: it reads
//! through `get`, and the unit tests plus the property test feed it arbitrary bytes.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Instant;

use guardedsleep_relay::RateLimiter;
use tokio::net::UdpSocket;
use tokio::sync::watch;

const HEADER_LEN: usize = 20;
const MAGIC_COOKIE: u32 = 0x2112_A442;
const BINDING_REQUEST: u16 = 0x0001;
const BINDING_SUCCESS: u16 = 0x0101;
const XOR_MAPPED_ADDRESS: u16 = 0x0020;
/// Datagrams larger than this are not STUN requests worth parsing.
const MAX_REQUEST_LEN: usize = 548;

/// The largest response: header plus an IPv6 XOR-MAPPED-ADDRESS.
pub const MAX_RESPONSE_LEN: usize = 44;
/// Amplification bound as a ratio: the response is at most 22/10 = 2.2 times the request.
const AMPLIFICATION_NUM: usize = 22;
const AMPLIFICATION_DEN: usize = 10;
/// Distinct client addresses tracked by the rate limiter; spoofed sources cannot grow it further.
const MAX_TRACKED_ADDRESSES: usize = 65_536;
/// Answers per minute across all clients (20 000 per second), a cap on the total STUN output.
const GLOBAL_RATE_PER_MIN: u32 = 20_000 * 60;

/// `Some(response)` for a well-formed Binding request that may be answered within the
/// amplification bound, else `None`.
pub fn respond(request: &[u8], src: SocketAddr) -> Option<Vec<u8>> {
    let txid = decode_binding_request(request)?;
    let response = encode_success(txid, src);
    within_bound(request.len(), response.len()).then_some(response)
}

fn within_bound(request_len: usize, response_len: usize) -> bool {
    response_len <= MAX_RESPONSE_LEN
        && response_len.saturating_mul(AMPLIFICATION_DEN)
            <= request_len.saturating_mul(AMPLIFICATION_NUM)
}

/// The 12-byte transaction id of a Binding request.
fn decode_binding_request(buf: &[u8]) -> Option<[u8; 12]> {
    if buf.len() < HEADER_LEN || buf.len() > MAX_REQUEST_LEN {
        return None;
    }
    let msg_type = u16::from_be_bytes([*buf.first()?, *buf.get(1)?]);
    let msg_len = usize::from(u16::from_be_bytes([*buf.get(2)?, *buf.get(3)?]));
    let cookie = u32::from_be_bytes([*buf.get(4)?, *buf.get(5)?, *buf.get(6)?, *buf.get(7)?]);
    // The two top bits of a STUN message are zero; this also keeps RTP/DTLS off the port.
    if msg_type != BINDING_REQUEST || cookie != MAGIC_COOKIE || msg_len % 4 != 0 {
        return None;
    }
    if buf.len() != HEADER_LEN + msg_len {
        return None;
    }
    let txid: [u8; 12] = buf.get(8..HEADER_LEN)?.try_into().ok()?;
    attributes_well_formed(buf.get(HEADER_LEN..)?).then_some(txid)
}

/// Type-length-value structure only; the content of attributes is not interpreted.
fn attributes_well_formed(mut attrs: &[u8]) -> bool {
    while !attrs.is_empty() {
        let Some(&[hi, lo]) = attrs.get(2..4).and_then(|b| <&[u8; 2]>::try_from(b).ok()) else {
            return false;
        };
        let len = usize::from(u16::from_be_bytes([hi, lo]));
        let padded = (len + 3) & !3;
        let Some(rest) = attrs.get(4 + padded..) else {
            return false;
        };
        attrs = rest;
    }
    true
}

fn encode_success(txid: [u8; 12], src: SocketAddr) -> Vec<u8> {
    let cookie = MAGIC_COOKIE.to_be_bytes();
    let port = src.port() ^ (MAGIC_COOKIE >> 16) as u16;
    let ip = src.ip().to_canonical();
    let mut value = vec![0u8, if ip.is_ipv4() { 0x01 } else { 0x02 }];
    value.extend_from_slice(&port.to_be_bytes());
    match ip {
        IpAddr::V4(a) => {
            value.extend(a.octets().iter().zip(cookie).map(|(b, k)| b ^ k));
        }
        IpAddr::V6(a) => {
            let mask: Vec<u8> = cookie.iter().chain(txid.iter()).copied().collect();
            value.extend(a.octets().iter().zip(mask).map(|(b, k)| b ^ k));
        }
    }
    let mut out = Vec::with_capacity(HEADER_LEN + 4 + value.len());
    out.extend_from_slice(&BINDING_SUCCESS.to_be_bytes());
    out.extend_from_slice(&((4 + value.len()) as u16).to_be_bytes());
    out.extend_from_slice(&cookie);
    out.extend_from_slice(&txid);
    out.extend_from_slice(&XOR_MAPPED_ADDRESS.to_be_bytes());
    out.extend_from_slice(&(value.len() as u16).to_be_bytes());
    out.extend_from_slice(&value);
    out
}

/// Admission of one request to the answer budget: a per-address bucket plus a global one. Unknown
/// addresses beyond the tracking cap are pruned first, so spoofed sources cannot lock real
/// clients out of the table.
pub struct AnswerBudget {
    per_address: RateLimiter<IpAddr>,
    max_addresses: usize,
    global: RateLimiter<()>,
}

impl AnswerBudget {
    pub fn new(rate_per_min: u32) -> Self {
        Self::with_capacity(rate_per_min, MAX_TRACKED_ADDRESSES, GLOBAL_RATE_PER_MIN)
    }

    pub fn with_capacity(rate_per_min: u32, max_addresses: usize, global_per_min: u32) -> Self {
        Self {
            per_address: RateLimiter::new(rate_per_min).with_max_keys(max_addresses),
            max_addresses,
            global: RateLimiter::new(global_per_min),
        }
    }

    pub fn tracked(&self) -> usize {
        self.per_address.len()
    }

    pub fn prune(&mut self, now: u64) {
        self.per_address.prune(now);
    }

    /// `true` when the address may be answered now.
    pub fn take(&mut self, ip: IpAddr, now: u64) -> bool {
        if self.per_address.len() >= self.max_addresses {
            self.per_address.prune(now);
        }
        self.per_address.take(ip, now) && self.global.take((), now)
    }
}

/// Serves the socket until `shutdown` flips. Requests above the per-address budget are dropped
/// silently. Nothing about senders is logged.
pub async fn serve(socket: Arc<UdpSocket>, rate_per_min: u32, mut shutdown: watch::Receiver<bool>) {
    let mut budget = AnswerBudget::new(rate_per_min);
    let start = Instant::now();
    let mut last_prune = 0u64;
    let mut buf = vec![0u8; MAX_REQUEST_LEN + 1];
    loop {
        let (n, src) = tokio::select! {
            r = socket.recv_from(&mut buf) => match r {
                Ok(v) => v,
                Err(_) => continue,
            },
            _ = shutdown.changed() => return,
        };
        let now = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        if now.saturating_sub(last_prune) >= 10_000 {
            budget.prune(now);
            last_prune = now;
        }
        let Some(request) = buf.get(..n) else {
            continue;
        };
        // Parse before spending a token: junk from spoofed sources must not drain a victim's budget.
        let Some(response) = respond(request, src) else {
            continue;
        };
        if !budget.take(src.ip().to_canonical(), now) {
            continue;
        }
        let _ = socket.send_to(&response, src).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(attrs: &[u8]) -> Vec<u8> {
        let mut b = vec![0x00, 0x01];
        b.extend_from_slice(&(attrs.len() as u16).to_be_bytes());
        b.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        b.extend_from_slice(&[
            0xb7, 0xe7, 0xa7, 0x01, 0xbc, 0x34, 0xd6, 0x86, 0xfa, 0x87, 0xdf, 0xae,
        ]);
        b.extend_from_slice(attrs);
        b
    }

    fn v4() -> SocketAddr {
        "192.0.2.1:32853".parse().unwrap()
    }

    fn padding(len: usize) -> Vec<u8> {
        // A SOFTWARE-like attribute of `len` value bytes (multiple of 4).
        let mut a = vec![0x80, 0x22];
        a.extend_from_slice(&(len as u16).to_be_bytes());
        a.extend(std::iter::repeat_n(b'x', len));
        a
    }

    #[test]
    fn answers_with_the_rfc_5769_mapped_address() {
        let req = request(&padding(16));
        let resp = respond(&req, v4()).unwrap();
        assert!(resp.len() <= req.len());
        assert_eq!(&resp[0..2], &[0x01, 0x01]);
        assert_eq!(&resp[2..4], &[0x00, 0x0c]);
        assert_eq!(&resp[4..8], &MAGIC_COOKIE.to_be_bytes());
        assert_eq!(&resp[8..20], &req[8..20], "transaction id echoed");
        // XOR-MAPPED-ADDRESS, IPv4, port 0xA147, address e1 12 a6 43 (RFC 5769 section 2.2).
        assert_eq!(
            &resp[20..],
            &[
                0x00, 0x20, 0x00, 0x08, 0x00, 0x01, 0xa1, 0x47, 0xe1, 0x12, 0xa6, 0x43
            ]
        );
    }

    #[test]
    fn answers_ipv6_with_the_transaction_id_in_the_mask() {
        let src: SocketAddr = "[2001:db8:1234:5678:11:2233:4455:6677]:32853"
            .parse()
            .unwrap();
        let req = request(&padding(32));
        let resp = respond(&req, src).unwrap();
        assert_eq!(resp.len(), 44);
        assert!(resp.len() <= req.len());
        assert_eq!(&resp[20..26], &[0x00, 0x20, 0x00, 0x14, 0x00, 0x02]);
        let mask: Vec<u8> = MAGIC_COOKIE
            .to_be_bytes()
            .iter()
            .chain(&req[8..20])
            .copied()
            .collect();
        let plain: Vec<u8> = resp[28..].iter().zip(&mask).map(|(a, b)| a ^ b).collect();
        let want: std::net::Ipv6Addr = "2001:db8:1234:5678:11:2233:4455:6677".parse().unwrap();
        assert_eq!(plain, want.octets());
    }

    #[test]
    fn a_bare_20_byte_request_is_answered_within_the_amplification_bound() {
        let bare = request(&[]);
        assert_eq!(bare.len(), 20);
        let resp = respond(&bare, v4()).expect("a bare request is answered");
        assert_eq!(resp.len(), 32);
        let v6: SocketAddr = "[2001:db8::1]:1".parse().unwrap();
        let resp = respond(&bare, v6).expect("a bare request over IPv6 is answered");
        assert_eq!(resp.len(), MAX_RESPONSE_LEN);
        assert!(resp.len() * 10 <= bare.len() * 22, "at most 2.2 times");
    }

    #[test]
    fn a_response_beyond_the_bound_is_not_sent() {
        // 44 bytes is the ceiling and exactly 2.2 times the smallest request.
        assert!(within_bound(20, 44));
        assert!(within_bound(20, 32));
        assert!(!within_bound(20, 45), "above the byte ceiling");
        assert!(!within_bound(19, 44), "above 2.2 times the request");
        assert!(
            !within_bound(100, 45),
            "the ceiling holds for large requests"
        );
        assert!(within_bound(100, 44));
    }

    #[test]
    fn only_binding_requests_are_answered() {
        for msg_type in [0x0101u16, 0x0111, 0x0003, 0x0000, 0x4001] {
            let mut r = request(&[]);
            r[0..2].copy_from_slice(&msg_type.to_be_bytes());
            assert!(respond(&r, v4()).is_none(), "type {msg_type:#06x}");
        }
        let mut bad_cookie = request(&[]);
        bad_cookie[4] ^= 1;
        assert!(respond(&bad_cookie, v4()).is_none());
    }

    #[test]
    fn malformed_lengths_and_attributes_are_dropped() {
        let mut short = request(&padding(8));
        short.truncate(25);
        assert!(respond(&short, v4()).is_none());
        let mut unaligned = request(&padding(8));
        unaligned[3] = 11;
        assert!(respond(&unaligned, v4()).is_none());
        // Attribute claims more bytes than the message holds.
        let mut overlong = request(&[0x80, 0x22, 0x00, 0x40, 1, 2, 3, 4]);
        assert!(respond(&overlong, v4()).is_none());
        overlong.extend(std::iter::repeat_n(0, 600));
        assert!(respond(&overlong, v4()).is_none());
        assert!(respond(&[], v4()).is_none());
        assert!(respond(&[0u8; 19], v4()).is_none());
        // Zero-length attributes are fine.
        assert!(respond(&request(&[0x80, 0x22, 0x00, 0x00]), v4()).is_some());
        assert!(
            respond(
                &request(&[0x80, 0x22, 0x00, 0x00, 0x80, 0x22, 0x00, 0x00]),
                v4()
            )
            .is_some()
        );
    }

    #[test]
    fn arbitrary_bytes_never_panic() {
        let mut x = 0x1234_5678_9abc_def0u64;
        for len in 0..120usize {
            for _ in 0..200 {
                let mut buf: Vec<u8> = (0..len)
                    .map(|_| {
                        x ^= x << 13;
                        x ^= x >> 7;
                        x ^= x << 17;
                        x as u8
                    })
                    .collect();
                if len >= 8 && x.is_multiple_of(2) {
                    buf[0] = 0;
                    buf[1] = 1;
                    buf[4..8].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
                }
                let _ = respond(&buf, v4());
            }
        }
    }

    #[test]
    fn the_per_address_limit_stops_answers() {
        let mut b = AnswerBudget::new(2);
        let ip: IpAddr = "192.0.2.1".parse().unwrap();
        assert!(b.take(ip, 0));
        assert!(b.take(ip, 0));
        assert!(!b.take(ip, 0));
        assert!(b.take("192.0.2.2".parse().unwrap(), 0));
    }

    #[test]
    fn a_full_address_table_is_pruned_instead_of_locking_new_clients_out() {
        let mut b = AnswerBudget::with_capacity(1, 2, u32::MAX);
        let ips: Vec<IpAddr> = (1..=3)
            .map(|i| format!("192.0.2.{i}").parse().unwrap())
            .collect();
        assert!(b.take(ips[0], 0));
        assert!(b.take(ips[1], 0));
        assert_eq!(b.tracked(), 2);
        assert!(
            !b.take(ips[2], 0),
            "table full and nothing has refilled yet"
        );
        assert_eq!(b.tracked(), 2, "the table stays bounded");
        // A minute later the old buckets are full again; the third address is answered at once,
        // without waiting for the periodic prune.
        assert!(b.take(ips[2], 60_000));
        assert_eq!(b.tracked(), 1);
    }

    #[test]
    fn the_global_limit_caps_total_output() {
        let mut b = AnswerBudget::with_capacity(60, 100, 3);
        let answered = (1..=10)
            .filter(|i| b.take(format!("192.0.2.{i}").parse().unwrap(), 0))
            .count();
        assert_eq!(answered, 3);
    }
}
