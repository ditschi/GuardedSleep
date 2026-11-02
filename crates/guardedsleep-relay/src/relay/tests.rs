//! Behaviour of the retired Node server (its tests: `git show ae71762d:apps/signaling-server`) plus admission.

use std::collections::HashMap;

use guardedsleep_core::pairing::{admission_mac, derive_admission_key};

use super::*;

const ROOM_V3: &str = "AAAAAAAAAAAAAAAAAAAAAA";

struct H {
    relay: Relay<DeterministicEntropy>,
    inbox: HashMap<u64, Vec<Value>>,
    closed: HashMap<u64, CloseReason>,
    pings: HashMap<u64, usize>,
    logs: Vec<LogEvent>,
    now: Millis,
}

impl H {
    fn new(cfg: RelayConfig) -> Self {
        Self {
            relay: Relay::new(cfg, DeterministicEntropy::new(7)),
            inbox: HashMap::new(),
            closed: HashMap::new(),
            pings: HashMap::new(),
            logs: Vec::new(),
            now: 0,
        }
    }

    /// Limits off, like the Node tests' `startServer`.
    fn open() -> Self {
        Self::new(RelayConfig {
            hello_rate_per_min: 0,
            new_room_rate_per_min: 0,
            ..RelayConfig::default()
        })
    }

    fn apply(&mut self, effects: Vec<RelayEffect>) {
        for e in effects {
            match e {
                RelayEffect::Send { conn, text } => self
                    .inbox
                    .entry(conn.0)
                    .or_default()
                    .push(serde_json::from_str(&text).unwrap()),
                RelayEffect::Ping { conn } => *self.pings.entry(conn.0).or_default() += 1,
                RelayEffect::Close { conn, reason } => {
                    self.closed.insert(conn.0, reason);
                }
                RelayEffect::Log { event } => self.logs.push(event),
            }
        }
    }

    fn connect_from(&mut self, id: u64, client: u128) {
        let fx = self
            .relay
            .on_connect(ConnId(id), ClientKey(client), self.now);
        self.apply(fx);
    }

    fn connect(&mut self, id: u64) {
        self.connect_from(id, u128::from(id));
    }

    fn send(&mut self, id: u64, msg: &str) {
        let fx = self.relay.on_frame(ConnId(id), msg.as_bytes(), self.now);
        self.apply(fx);
    }

    fn hello(&mut self, id: u64, role: &str, room: &str, version: u32) {
        self.send(
            id,
            &json!({"type":"hello","role":role,"roomId":room,"protocolVersion":version})
                .to_string(),
        );
    }

    fn tick(&mut self, ms: Millis) {
        self.now += ms;
        let fx = self.relay.on_tick(self.now);
        self.apply(fx);
    }

    fn close(&mut self, id: u64) {
        let fx = self.relay.on_close(ConnId(id));
        self.apply(fx);
    }

    fn types(&self, id: u64) -> Vec<String> {
        self.inbox
            .get(&id)
            .map(|v| {
                v.iter()
                    .map(|m| m["type"].as_str().unwrap().to_owned())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn last(&self, id: u64) -> &Value {
        self.inbox.get(&id).and_then(|v| v.last()).unwrap()
    }

    fn first(&self, id: u64) -> &Value {
        self.inbox.get(&id).and_then(|v| v.first()).unwrap()
    }

    fn joined(&self, id: u64) -> bool {
        self.types(id).iter().any(|t| t == "welcome")
    }

    fn client_id(&self, id: u64) -> String {
        self.inbox[&id]
            .iter()
            .find(|m| m["type"] == "welcome")
            .unwrap()["clientId"]
            .as_str()
            .unwrap()
            .to_owned()
    }
}

#[test]
fn the_defaults_are_the_numbers_of_the_signaling_relay_spec() {
    let c = RelayConfig::default();
    assert_eq!(c.hello_timeout_ms, 30_000);
    assert_eq!(c.ping_interval_ms, 10_000);
    assert_eq!(c.admission_timeout_ms, 5_000);
    assert_eq!(c.max_frame_bytes, 64 * 1024);
    assert_eq!(c.hello_rate_per_min, 20);
    assert_eq!(c.new_room_rate_per_min, 10);
    assert_eq!(c.max_parents_per_room, 1);
    assert_eq!(c.max_connections_per_client, 16);
    assert_eq!(c.frame_rate_unjoined_per_min, 120);
    assert_eq!(c.frame_rate_per_min, 600);
}

#[test]
fn answers_ping_with_pong() {
    let mut h = H::open();
    h.connect(1);
    h.send(1, r#"{"type":"ping"}"#);
    assert_eq!(h.types(1), ["pong"]);
}

#[test]
fn rejects_garbage_with_bad_json_and_keeps_the_connection() {
    let mut h = H::open();
    h.connect(1);
    h.send(1, "not json");
    h.send(
        1,
        r#"{"type":"hello","role":"admin","roomId":"r","protocolVersion":2}"#,
    );
    assert_eq!(
        h.inbox[&1]
            .iter()
            .filter(|m| m["code"] == "BAD_JSON")
            .count(),
        2
    );
    assert!(h.closed.is_empty());
}

#[test]
fn invalid_utf8_is_bad_json() {
    let mut h = H::open();
    h.connect(1);
    let fx = h.relay.on_frame(ConnId(1), &[0xff, 0xfe], 0);
    h.apply(fx);
    assert_eq!(h.first(1)["code"], "BAD_JSON");
}

#[test]
fn rejects_second_hello_on_same_connection() {
    let mut h = H::open();
    h.connect(1);
    h.hello(1, "baby", "r1", 2);
    assert_eq!(h.types(1), ["welcome", "members", "peer"]);
    h.hello(1, "parent", "r1", 2);
    assert_eq!(h.last(1)["type"], "error");
    assert_eq!(h.last(1)["code"], "INVALID_MESSAGE");
    assert!(h.closed.is_empty());
}

#[test]
fn rejects_a_second_baby_in_the_same_room() {
    let mut h = H::open();
    h.connect(1);
    h.connect(2);
    h.hello(1, "baby", "r-baby-twice", 2);
    h.hello(2, "baby", "r-baby-twice", 2);
    assert_eq!(h.first(2)["code"], "BABY_TAKEN");
    assert_eq!(h.closed[&2], CloseReason::BabyTaken);
    assert_eq!(h.closed[&2].code(), 1008);
    assert_eq!(h.relay.room_count(), 1);
}

#[test]
fn requires_signal_to_when_more_than_two_peers() {
    let mut h = H::new(RelayConfig {
        max_parents_per_room: 2,
        hello_rate_per_min: 0,
        new_room_rate_per_min: 0,
        ..RelayConfig::default()
    });
    let room = "r-three";
    for id in 1..=3 {
        h.connect(id);
    }
    h.hello(1, "baby", room, 2);
    assert_eq!(h.types(1), ["welcome", "members", "peer"]);
    h.hello(2, "parent", room, 2);
    assert_eq!(h.types(2), ["welcome", "members", "peer"]);
    assert_eq!(h.types(1)[3..], ["members", "peer"]);
    h.hello(3, "parent", room, 2);
    assert_eq!(h.types(3), ["welcome", "members", "peer"]);
    assert_eq!(h.types(1)[5..], ["members", "peer"]);
    assert_eq!(h.types(2)[3..], ["members", "peer"]);

    h.send(
        1,
        &json!({"type":"signal","roomId":room,"payload":{"n":1}}).to_string(),
    );
    assert_eq!(h.last(1)["code"], "SIGNAL_TARGET_REQUIRED");

    let p1 = h.client_id(2);
    h.send(
        1,
        &json!({"type":"signal","roomId":room,"to":p1,"payload":{"n":2}}).to_string(),
    );
    assert_eq!(h.last(2)["type"], "signal");
    assert_eq!(h.last(2)["payload"]["n"], 2);
    assert_eq!(h.last(2)["from"], h.client_id(1));
}

#[test]
fn relays_signals_between_two_peers_without_a_target() {
    let mut h = H::open();
    h.connect(1);
    h.connect(2);
    h.hello(1, "baby", "r", 2);
    h.hello(2, "parent", "r", 2);
    h.send(2, r#"{"type":"signal","roomId":"r","payload":{"sdp":"x"}}"#);
    assert_eq!(h.last(1)["type"], "signal");
    assert_eq!(h.last(1)["payload"]["sdp"], "x");
    assert_eq!(h.last(1)["from"], h.client_id(2));
}

#[test]
fn a_sealed_envelope_payload_is_forwarded_untouched_up_to_the_largest_class() {
    // Since the alpha connection model every payload is `{"env": base64url(envelope)}`. The
    // largest class (8 KiB bucket) is 8,235 bytes, 10,980 characters of base64url. The relay does
    // not parse, resize or log it; only the frame cap applies.
    let mut h = H::open();
    h.connect(1);
    h.connect(2);
    h.hello(1, "baby", "r", 2);
    h.hello(2, "parent", "r", 2);
    let env: String = (0..10_980)
        .map(|i| ["A", "b", "-", "_", "9", "Z"][i % 6])
        .collect();
    let payload = json!({ "env": env });
    h.send(
        2,
        &json!({"type":"signal","roomId":"r","payload":payload}).to_string(),
    );
    assert_eq!(h.last(1)["payload"], payload);
    assert_eq!(h.last(1)["payload"]["env"].as_str().unwrap().len(), 10_980);
}

#[test]
fn a_claimed_sender_in_a_signal_is_replaced_by_the_real_client_id() {
    let mut h = H::open();
    h.connect(1);
    h.connect(2);
    h.hello(1, "baby", "r", 2);
    h.hello(2, "parent", "r", 2);
    h.send(
        2,
        r#"{"type":"signal","roomId":"r","from":"forged","payload":{"sdp":"x"}}"#,
    );
    assert_eq!(h.last(1)["from"], h.client_id(2));
    assert_ne!(h.last(1)["from"], "forged");
}

#[test]
fn signal_errors() {
    let mut h = H::open();
    h.connect(1);
    h.connect(2);
    h.send(1, r#"{"type":"signal","roomId":"r","payload":1}"#);
    assert_eq!(h.last(1)["code"], "NOT_JOINED");
    h.hello(1, "baby", "r", 2);
    h.hello(2, "parent", "r", 2);
    h.send(1, r#"{"type":"signal","roomId":"other","payload":1}"#);
    assert_eq!(h.last(1)["message"], "room mismatch");
    h.send(
        1,
        r#"{"type":"signal","roomId":"r","to":"nobody","payload":1}"#,
    );
    assert_eq!(h.last(1)["message"], "unknown signal target");
    let own = h.client_id(1);
    h.send(
        1,
        &json!({"type":"signal","roomId":"r","to":own,"payload":1}).to_string(),
    );
    assert_eq!(h.last(1)["message"], "unknown signal target");
}

#[test]
fn signal_to_an_empty_room_peer_is_dropped_silently() {
    let mut h = H::open();
    h.connect(1);
    h.hello(1, "baby", "r", 2);
    let before = h.types(1).len();
    h.send(1, r#"{"type":"signal","roomId":"r","payload":1}"#);
    assert_eq!(h.types(1).len(), before);
}

#[test]
fn terminates_a_baby_that_stops_answering_pings_and_frees_the_slot() {
    let mut h = H::new(RelayConfig {
        ping_interval_ms: 100,
        hello_rate_per_min: 0,
        new_room_rate_per_min: 0,
        ..RelayConfig::default()
    });
    h.connect(1);
    h.hello(1, "baby", "r-stale", 2);
    assert!(h.joined(1));
    h.tick(100);
    assert_eq!(h.pings[&1], 1);
    h.tick(100);
    assert_eq!(h.closed[&1], CloseReason::Stale);
    assert_eq!(h.relay.room_count(), 0);

    h.connect(2);
    h.hello(2, "baby", "r-stale", 2);
    assert_eq!(h.first(2)["type"], "welcome");
}

#[test]
fn keeps_healthy_sockets_and_broadcasts_members_when_a_stale_parent_is_dropped() {
    let mut h = H::new(RelayConfig {
        ping_interval_ms: 100,
        hello_rate_per_min: 0,
        new_room_rate_per_min: 0,
        ..RelayConfig::default()
    });
    h.connect(1);
    h.hello(1, "baby", "r-live", 2);
    h.connect(2);
    h.hello(2, "parent", "r-live", 2);
    h.tick(100);
    h.relay.on_pong(ConnId(1));
    h.tick(100);
    assert_eq!(h.closed.get(&1), None);
    assert_eq!(h.closed[&2], CloseReason::Stale);
    let members = h.inbox[&1]
        .iter()
        .rev()
        .find(|m| m["type"] == "members")
        .unwrap();
    assert_eq!(members["peers"].as_array().unwrap().len(), 1);
    assert_eq!(members["peers"][0]["role"], "baby");
}

#[test]
fn any_frame_counts_as_a_pong() {
    let mut h = H::new(RelayConfig {
        ping_interval_ms: 100,
        ..RelayConfig::default()
    });
    h.connect(1);
    h.tick(100);
    h.send(1, r#"{"type":"ping"}"#);
    h.tick(100);
    assert!(h.closed.is_empty());
}

#[test]
fn terminates_sockets_that_never_joined() {
    let mut h = H::new(RelayConfig {
        ping_interval_ms: 100,
        ..RelayConfig::default()
    });
    h.connect(1);
    h.tick(100);
    h.tick(100);
    assert_eq!(h.closed[&1], CloseReason::Stale);
    assert_eq!(h.relay.conn_count(), 0);
}

#[test]
fn closes_sockets_without_hello_after_the_hello_timeout() {
    let mut h = H::open();
    h.connect(1);
    h.tick(29_999);
    assert!(h.closed.is_empty());
    h.relay.on_pong(ConnId(1));
    h.tick(1);
    assert_eq!(h.closed[&1], CloseReason::HelloTimeout);
    assert_eq!(h.closed[&1].code(), 1008);
}

#[test]
fn a_joined_socket_is_not_subject_to_the_hello_timeout() {
    let mut h = H::open();
    h.connect(1);
    h.hello(1, "baby", "r", 2);
    for _ in 0..10 {
        h.tick(10_000);
        h.relay.on_pong(ConnId(1));
    }
    assert!(h.closed.is_empty());
}

#[test]
fn accepts_v2_and_v3_and_pins_a_room_to_the_first_members_version() {
    let mut h = H::open();
    for id in 1..=4 {
        h.connect(id);
    }
    h.hello(1, "baby", ROOM_V3, 3);
    assert!(h.joined(1));
    h.hello(2, "parent", ROOM_V3, 2);
    assert_eq!(h.first(2)["code"], "UNSUPPORTED_PROTOCOL");
    assert_eq!(h.closed[&2], CloseReason::VersionMismatch);
    h.connect(5);
    h.hello(5, "parent", ROOM_V3, 3);
    assert!(h.joined(5));
    h.hello(4, "baby", "r-v4", 4);
    assert_eq!(h.first(4)["code"], "UNSUPPORTED_PROTOCOL");
}

#[test]
fn a_room_is_unpinned_when_it_empties() {
    let mut h = H::open();
    h.connect(1);
    h.hello(1, "baby", ROOM_V3, 2);
    h.close(1);
    h.connect(2);
    h.hello(2, "baby", ROOM_V3, 3);
    assert!(h.joined(2));
}

#[test]
fn requires_22_base64url_chars_for_v3_room_ids() {
    let mut h = H::open();
    h.connect(1);
    h.hello(1, "baby", "short-room", 3);
    assert_eq!(h.first(1)["code"], "INVALID_MESSAGE");
    assert_eq!(h.closed[&1], CloseReason::InvalidRoomId);
    h.connect(2);
    h.hello(2, "baby", "AAAAAAAAAAAAAAAAAAAA+A", 3);
    assert_eq!(h.first(2)["code"], "INVALID_MESSAGE");
}

#[test]
fn defaults_to_one_parent_per_room() {
    let mut h = H::open();
    h.connect(1);
    h.connect(2);
    h.hello(1, "parent", "r-cap", 2);
    h.hello(2, "parent", "r-cap", 2);
    assert_eq!(h.first(2)["code"], "PARENT_CAP");
    assert_eq!(h.closed[&2], CloseReason::ParentCap);
}

#[test]
fn allows_one_baby_and_up_to_the_parent_cap() {
    let mut h = H::new(RelayConfig {
        max_parents_per_room: 2,
        hello_rate_per_min: 0,
        new_room_rate_per_min: 0,
        ..RelayConfig::default()
    });
    for (id, role) in [(1, "baby"), (2, "parent"), (3, "parent"), (4, "parent")] {
        h.connect(id);
        h.hello(id, role, "r", 2);
    }
    assert!(h.joined(1) && h.joined(2) && h.joined(3));
    assert_eq!(h.first(4)["code"], "PARENT_CAP");
}

#[test]
fn rate_limits_hello_attempts_per_client() {
    let mut h = H::new(RelayConfig {
        hello_rate_per_min: 2,
        new_room_rate_per_min: 0,
        ..RelayConfig::default()
    });
    for i in 0..2u64 {
        h.connect_from(10 + i, 99);
        h.hello(10 + i, "baby", &format!("r-hello-{i}"), 2);
        assert_eq!(h.first(10 + i)["type"], "welcome");
        h.close(10 + i);
    }
    h.connect_from(20, 99);
    h.hello(20, "baby", "r-hello-3", 2);
    assert_eq!(h.first(20)["code"], "RATE_LIMITED");
    assert_eq!(h.closed[&20], CloseReason::RateLimited);
    h.connect_from(21, 100);
    h.hello(21, "baby", "r-hello-4", 2);
    assert_eq!(
        h.first(21)["type"],
        "welcome",
        "another client key has its own budget"
    );
}

#[test]
fn the_eleventh_new_room_of_one_client_within_a_minute_is_rate_limited_by_default() {
    let mut h = H::new(RelayConfig {
        hello_rate_per_min: 0,
        ..RelayConfig::default()
    });
    for i in 0..10u64 {
        h.connect_from(100 + i, 7);
        h.hello(100 + i, "baby", &format!("r-stuff-{i}"), 2);
        assert_eq!(h.first(100 + i)["type"], "welcome");
    }
    h.connect_from(200, 7);
    h.hello(200, "baby", "r-stuff-10", 2);
    assert_eq!(h.first(200)["code"], "RATE_LIMITED");
    assert_eq!(h.closed[&200], CloseReason::RateLimited);
}

#[test]
fn rate_limits_new_rooms_but_not_joins_of_existing_rooms() {
    let mut h = H::new(RelayConfig {
        hello_rate_per_min: 0,
        new_room_rate_per_min: 1,
        ..RelayConfig::default()
    });
    h.connect_from(1, 5);
    h.hello(1, "baby", "r-new-1", 2);
    h.connect_from(2, 5);
    h.hello(2, "parent", "r-new-1", 2);
    assert!(h.joined(1) && h.joined(2));
    h.connect_from(3, 5);
    h.hello(3, "baby", "r-new-2", 2);
    assert_eq!(h.first(3)["code"], "RATE_LIMITED");
    assert_eq!(h.first(3)["message"], "too many rooms");
}

#[test]
fn rate_limit_budgets_refill_with_time() {
    let mut h = H::new(RelayConfig {
        hello_rate_per_min: 1,
        new_room_rate_per_min: 0,
        ..RelayConfig::default()
    });
    h.connect_from(1, 5);
    h.hello(1, "baby", "r1", 2);
    h.close(1);
    h.connect_from(2, 5);
    h.hello(2, "baby", "r1", 2);
    assert_eq!(h.first(2)["code"], "RATE_LIMITED");
    h.now += 60_000;
    h.connect_from(3, 5);
    h.hello(3, "baby", "r1", 2);
    assert_eq!(h.first(3)["type"], "welcome");
}

#[test]
fn empty_rooms_are_deleted() {
    let mut h = H::open();
    h.connect(1);
    h.hello(1, "baby", "r", 2);
    assert_eq!(h.relay.room_count(), 1);
    h.close(1);
    assert_eq!(h.relay.room_count(), 0);
    assert_eq!(h.relay.conn_count(), 0);
}

#[test]
fn leaving_broadcasts_the_remaining_members() {
    let mut h = H::open();
    h.connect(1);
    h.connect(2);
    h.hello(1, "baby", "r", 2);
    h.hello(2, "parent", "r", 2);
    h.close(2);
    let m = h.last(1);
    assert_eq!(m["type"], "members");
    assert_eq!(m["peers"].as_array().unwrap().len(), 1);
}

#[test]
fn welcome_peer_shapes_match_the_node_server() {
    let mut h = H::open();
    h.connect(1);
    h.connect(2);
    h.hello(1, "baby", "r", 2);
    assert_eq!(h.inbox[&1][2], json!({"type":"peer","peerId":null}));
    h.hello(2, "parent", "r", 2);
    let baby_id = h.client_id(1);
    let parent_id = h.client_id(2);
    assert_eq!(h.inbox[&2][2], json!({"type":"peer","peerId":baby_id}));
    assert_eq!(h.last(1), &json!({"type":"peer","peerId":parent_id}));
    assert_eq!(
        h.inbox[&2][1],
        json!({"type":"members","peers":[
            {"clientId":baby_id,"role":"baby"},{"clientId":parent_id,"role":"parent"}]})
    );
    let id = h.client_id(1);
    assert_eq!(id.len(), 36);
    assert_eq!(id.as_bytes()[14], b'4');
}

#[test]
fn oversized_frames_close_the_connection() {
    let mut h = H::new(RelayConfig {
        max_frame_bytes: 32,
        ..RelayConfig::default()
    });
    h.connect(1);
    h.send(1, &"x".repeat(33));
    assert_eq!(h.closed[&1], CloseReason::TooLarge);
    assert_eq!(h.closed[&1].code(), 1009);
    h.connect(2);
    h.send(2, r#"{"type":"ping"}"#);
    assert_eq!(h.types(2), ["pong"]);
}

#[test]
fn connections_are_capped() {
    let mut h = H::new(RelayConfig {
        max_connections: 2,
        ..RelayConfig::default()
    });
    h.connect(1);
    h.connect(2);
    h.connect(3);
    assert_eq!(h.closed[&3], CloseReason::Capacity);
    assert_eq!(h.relay.conn_count(), 2);
}

#[test]
fn one_client_cannot_hold_more_than_its_share_of_connections() {
    let mut h = H::new(RelayConfig {
        max_connections_per_client: 3,
        ..RelayConfig::default()
    });
    for id in 1..=3 {
        h.connect_from(id, 7);
    }
    h.connect_from(4, 7);
    assert_eq!(h.closed[&4], CloseReason::Capacity);
    assert_eq!(h.relay.conn_count(), 3);
    // Another client is unaffected.
    h.connect_from(5, 8);
    assert!(!h.closed.contains_key(&5));
    // A freed slot is usable again, and the refused connection did not count.
    h.close(1);
    h.connect_from(6, 7);
    assert!(!h.closed.contains_key(&6));
    h.connect_from(7, 7);
    assert_eq!(h.closed[&7], CloseReason::Capacity);
}

#[test]
fn connections_dropped_by_the_relay_free_the_client_slot() {
    let mut h = H::new(RelayConfig {
        max_connections_per_client: 1,
        ..RelayConfig::default()
    });
    h.connect_from(1, 7);
    h.tick(30_000);
    assert_eq!(h.closed[&1], CloseReason::HelloTimeout);
    h.connect_from(2, 7);
    assert!(!h.closed.contains_key(&2));
}

#[test]
fn the_connection_cap_per_client_can_be_disabled() {
    let mut h = H::new(RelayConfig {
        max_connections_per_client: 0,
        ..RelayConfig::default()
    });
    for id in 1..=50 {
        h.connect_from(id, 7);
    }
    assert_eq!(h.relay.conn_count(), 50);
}

#[test]
fn a_connection_that_floods_before_joining_is_closed() {
    let mut h = H::new(RelayConfig {
        frame_rate_unjoined_per_min: 5,
        ..RelayConfig::default()
    });
    h.connect(1);
    for _ in 0..5 {
        h.send(1, r#"{"type":"ping"}"#);
    }
    assert!(!h.closed.contains_key(&1));
    h.send(1, r#"{"type":"ping"}"#);
    assert_eq!(h.last(1)["code"], "RATE_LIMITED");
    assert_eq!(h.closed[&1], CloseReason::RateLimited);
    assert_eq!(h.relay.conn_count(), 0);
}

#[test]
fn a_joined_connection_has_its_own_larger_frame_budget_that_refills() {
    let mut h = H::new(RelayConfig {
        frame_rate_unjoined_per_min: 2,
        frame_rate_per_min: 6,
        hello_rate_per_min: 0,
        new_room_rate_per_min: 0,
        ping_interval_ms: 3_600_000,
        ..RelayConfig::default()
    });
    h.connect(1);
    h.hello(1, "baby", ROOM_V3, 3);
    assert!(h.joined(1));
    for _ in 0..6 {
        h.send(1, r#"{"type":"ping"}"#);
    }
    assert!(!h.closed.contains_key(&1), "joined budget is separate");
    h.tick(10_000);
    h.send(1, r#"{"type":"ping"}"#);
    assert!(!h.closed.contains_key(&1), "one token refilled in 10 s");
    h.send(1, r#"{"type":"ping"}"#);
    assert_eq!(h.closed[&1], CloseReason::RateLimited);
    assert_eq!(h.relay.room_count(), 0);
}

#[test]
fn one_flooding_connection_does_not_limit_another() {
    let mut h = H::new(RelayConfig {
        frame_rate_unjoined_per_min: 1,
        ..RelayConfig::default()
    });
    h.connect(1);
    h.connect(2);
    h.send(1, r#"{"type":"ping"}"#);
    h.send(1, r#"{"type":"ping"}"#);
    assert_eq!(h.closed[&1], CloseReason::RateLimited);
    h.send(2, r#"{"type":"ping"}"#);
    assert_eq!(h.types(2), ["pong"]);
}

#[test]
fn a_silent_baby_is_stale_within_the_spec_bound_with_the_default_config() {
    // Spec: terminated within 20 s; the server ticks every 500 ms.
    let mut h = H::new(RelayConfig::default());
    h.connect(1);
    h.hello(1, "baby", ROOM_V3, 3);
    assert!(h.joined(1));
    let mut elapsed = 0;
    while !h.closed.contains_key(&1) && elapsed <= 30_000 {
        h.tick(500);
        elapsed += 500;
    }
    assert_eq!(h.closed[&1], CloseReason::Stale);
    assert!(elapsed <= 20_500, "closed after {elapsed} ms");
    assert!(h.pings[&1] >= 1);
}

#[test]
fn frames_for_unknown_connections_are_ignored() {
    let mut h = H::open();
    h.send(9, r#"{"type":"ping"}"#);
    h.close(9);
    h.relay.on_pong(ConnId(9));
    assert!(h.inbox.is_empty());
}

#[test]
fn logs_carry_no_identifiers() {
    let mut h = H::open();
    h.connect(1);
    h.hello(1, "baby", ROOM_V3, 3);
    h.close(1);
    let text: String = h
        .logs
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("|");
    assert_eq!(text, "connected|joined as baby|left");
}

// Admission (ADR-021 section 1).

const PSK: [u8; 32] = [9; 32];

fn required() -> H {
    let mut h = H::new(RelayConfig {
        admission: Admission::Required,
        hello_rate_per_min: 0,
        new_room_rate_per_min: 0,
        ..RelayConfig::default()
    });
    assert!(
        h.relay
            .set_admission_key(ROOM_V3, derive_admission_key(&PSK))
    );
    h
}

fn nonce_of(h: &H, id: u64) -> String {
    let c = h.first(id);
    assert_eq!(c["type"], "challenge");
    c["nonce"].as_str().unwrap().to_owned()
}

fn admit(h: &mut H, id: u64, role: Role, room: &str, key_psk: &[u8]) {
    let nonce = nonce_of(h, id);
    let mac = admission_mac(&derive_admission_key(key_psk), &nonce, role, room).unwrap();
    h.send(
        id,
        &json!({"type":"admit","role":role.as_str(),"roomId":room,"protocolVersion":3,"mac":mac})
            .to_string(),
    );
}

#[test]
fn open_relays_do_not_send_a_challenge_and_refuse_admit() {
    let mut h = H::open();
    h.connect(1);
    assert!(h.inbox.is_empty());
    h.send(
        1,
        &json!({"type":"admit","role":"baby","roomId":ROOM_V3,"protocolVersion":3,"mac":"0".repeat(64)})
            .to_string(),
    );
    assert_eq!(h.first(1)["code"], "INVALID_MESSAGE");
    assert!(h.closed.is_empty());
}

#[test]
fn required_relays_challenge_every_joiner_with_a_fresh_nonce() {
    let mut h = required();
    h.connect(1);
    h.connect(2);
    assert_ne!(nonce_of(&h, 1), nonce_of(&h, 2));
    assert!(guardedsleep_core::protocol::valid_nonce(&nonce_of(&h, 1)));
}

#[test]
fn a_valid_answer_takes_the_slot_only_after_verification() {
    let mut h = required();
    h.connect(1);
    assert_eq!(h.relay.room_count(), 0, "connecting alone takes no slot");
    admit(&mut h, 1, Role::Parent, ROOM_V3, &PSK);
    assert_eq!(h.types(1), ["challenge", "welcome", "members", "peer"]);
    assert!(h.logs.contains(&LogEvent::Admitted));
}

#[test]
fn a_wrong_key_fails_and_closes() {
    let mut h = required();
    h.connect(1);
    admit(&mut h, 1, Role::Parent, ROOM_V3, &[1; 32]);
    assert_eq!(h.last(1)["code"], "ADMISSION_FAILED");
    assert_eq!(h.closed[&1], CloseReason::AdmissionFailed);
    assert_eq!(h.relay.room_count(), 0);
}

#[test]
fn a_replayed_first_message_fails_and_cannot_take_the_parent_slot() {
    let mut h = required();
    let key = derive_admission_key(&PSK);
    h.connect(0);
    let nonce = nonce_of(&h, 0);
    let mac = admission_mac(&key, &nonce, Role::Baby, ROOM_V3).unwrap();
    h.send(
        0,
        &json!({"type":"admit","role":"baby","roomId":ROOM_V3,"protocolVersion":3,"mac":mac})
            .to_string(),
    );
    assert!(h.joined(0));

    h.connect(1);
    let nonce = nonce_of(&h, 1);
    let mac = admission_mac(&key, &nonce, Role::Parent, ROOM_V3).unwrap();
    let first =
        json!({"type":"admit","role":"parent","roomId":ROOM_V3,"protocolVersion":3,"mac":mac})
            .to_string();
    h.send(1, &first);
    assert!(h.joined(1));
    h.close(1);
    assert_eq!(h.relay.room_count(), 1, "the baby is still in the room");

    // An eavesdropper replays the captured message on its own connection.
    h.connect(2);
    h.send(2, &first);
    assert_eq!(h.last(2)["code"], "ADMISSION_FAILED");
    assert_eq!(h.closed[&2], CloseReason::AdmissionFailed);
    let members = h.last(0);
    assert_eq!(members["type"], "members");
    assert_eq!(members["peers"].as_array().unwrap().len(), 1, "baby only");
    assert_eq!(h.relay.room_count(), 1);
}

#[test]
fn a_second_admit_on_the_same_connection_is_refused() {
    let mut h = required();
    h.connect(1);
    let nonce = nonce_of(&h, 1);
    let key = derive_admission_key(&PSK);
    let mac = admission_mac(&key, &nonce, Role::Baby, ROOM_V3).unwrap();
    // Wrong role first: the nonce is spent, so the right answer afterwards cannot rescue it.
    let wrong = admission_mac(&key, &nonce, Role::Parent, ROOM_V3).unwrap();
    h.send(
        1,
        &json!({"type":"admit","role":"baby","roomId":ROOM_V3,"protocolVersion":3,"mac":wrong})
            .to_string(),
    );
    assert_eq!(h.closed[&1], CloseReason::AdmissionFailed);
    h.send(
        1,
        &json!({"type":"admit","role":"baby","roomId":ROOM_V3,"protocolVersion":3,"mac":mac})
            .to_string(),
    );
    assert!(!h.joined(1));
}

#[test]
fn the_answer_is_bound_to_role_and_room() {
    let mut h = required();
    h.connect(1);
    let nonce = nonce_of(&h, 1);
    let key = derive_admission_key(&PSK);
    let mac = admission_mac(&key, &nonce, Role::Parent, ROOM_V3).unwrap();
    h.send(
        1,
        &json!({"type":"admit","role":"baby","roomId":ROOM_V3,"protocolVersion":3,"mac":mac})
            .to_string(),
    );
    assert_eq!(h.last(1)["code"], "ADMISSION_FAILED");
}

#[test]
fn rooms_without_a_registered_key_admit_nobody() {
    let mut h = required();
    h.connect(1);
    admit(&mut h, 1, Role::Parent, "BBBBBBBBBBBBBBBBBBBBBB", &PSK);
    assert_eq!(h.last(1)["code"], "ADMISSION_FAILED");
}

#[test]
fn hello_is_refused_when_admission_is_required() {
    let mut h = required();
    h.connect(1);
    h.hello(1, "baby", ROOM_V3, 3);
    assert_eq!(h.last(1)["code"], "ADMISSION_REQUIRED");
    assert_eq!(h.closed[&1], CloseReason::AdmissionRequired);
    assert_eq!(h.relay.room_count(), 0);
}

#[test]
fn unauthenticated_connections_are_closed_after_five_seconds() {
    let mut h = required();
    h.connect(1);
    h.tick(4_999);
    assert!(h.closed.is_empty());
    h.tick(1);
    assert_eq!(h.closed[&1], CloseReason::AdmissionTimeout);
    assert_eq!(h.relay.conn_count(), 0);
}

#[test]
fn admitted_connections_survive_the_admission_deadline() {
    let mut h = required();
    h.connect(1);
    admit(&mut h, 1, Role::Baby, ROOM_V3, &PSK);
    h.tick(5_000);
    h.relay.on_pong(ConnId(1));
    assert!(h.closed.is_empty());
}

#[test]
fn unauthenticated_sockets_cannot_signal() {
    let mut h = required();
    h.connect(1);
    h.send(
        1,
        &json!({"type":"signal","roomId":ROOM_V3,"payload":1}).to_string(),
    );
    assert_eq!(h.last(1)["code"], "NOT_JOINED");
}

#[test]
fn the_admission_key_table_is_bounded() {
    let mut h = required();
    for i in 0..15 {
        assert!(
            h.relay
                .set_admission_key(&format!("room-{i}"), derive_admission_key(&PSK))
        );
    }
    assert!(
        !h.relay
            .set_admission_key("one-too-many", derive_admission_key(&PSK))
    );
    assert!(
        h.relay
            .set_admission_key(ROOM_V3, derive_admission_key(&[2; 32])),
        "replacing is fine"
    );
    h.relay.remove_admission_key(ROOM_V3);
    assert!(
        h.relay
            .set_admission_key("one-too-many", derive_admission_key(&PSK))
    );
}

#[test]
fn deterministic_entropy_differs_per_call() {
    let mut e = DeterministicEntropy::new(1);
    let (mut a, mut b) = ([0u8; 16], [0u8; 16]);
    e.fill(&mut a);
    e.fill(&mut b);
    assert_ne!(a, b);
}
