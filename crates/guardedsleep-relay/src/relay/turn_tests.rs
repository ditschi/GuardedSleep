//! When the relay asks the platform for TURN credentials (`signaling-relay` "ICE servers from the
//! relay", "TURN credential caps"): the gate, the ordering before the room-completing update, the
//! half-lifetime renewal, one credential per member and the per-address cap.

use super::*;

const TTL: Millis = 600_000;
const ROOM_A: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const ROOM_B: &str = "BBBBBBBBBBBBBBBBBBBBBB";

/// Effects in the order the relay produced them, as `<conn>:<what>`.
struct T {
    relay: Relay<DeterministicEntropy>,
    trace: Vec<String>,
    welcomes: Vec<Value>,
    now: Millis,
}

impl T {
    fn with(max_live: usize, ttl: Option<Millis>) -> Self {
        let cfg = RelayConfig {
            hello_rate_per_min: 0,
            new_room_rate_per_min: 0,
            max_connections_per_client: 0,
            // The tests jump over whole credential lifetimes without answering pings.
            ping_interval_ms: Millis::MAX / 2,
            turn_credential_ttl_ms: ttl,
            turn_max_live_per_addr: max_live,
            ..RelayConfig::default()
        };
        Self {
            relay: Relay::new(cfg, DeterministicEntropy::new(3)),
            trace: Vec::new(),
            welcomes: Vec::new(),
            now: 0,
        }
    }

    fn new(max_live: usize) -> Self {
        Self::with(max_live, Some(TTL))
    }

    fn apply(&mut self, fx: Vec<RelayEffect>) {
        for e in fx {
            match e {
                RelayEffect::Send { conn, text } => {
                    let v: Value = serde_json::from_str(&text).unwrap();
                    if v["type"] == "welcome" {
                        self.welcomes.push(v.clone());
                    }
                    self.trace
                        .push(format!("{}:{}", conn.0, v["type"].as_str().unwrap()));
                }
                RelayEffect::IssueIceServers { conn } => self.trace.push(format!("{}:ICE", conn.0)),
                RelayEffect::Close { .. } | RelayEffect::Ping { .. } | RelayEffect::Log { .. } => {}
            }
        }
    }

    /// Connects `id` from address group `net` and joins it.
    fn join(&mut self, id: u64, net: u128, role: &str, room: &str, version: u32) {
        let fx = self.relay.on_connect_in_net(
            ConnId(id),
            ClientKey(u128::from(id)),
            ClientKey(net),
            self.now,
        );
        self.apply(fx);
        let hello = json!({"type":"hello","role":role,"roomId":room,"protocolVersion":version});
        let fx = self
            .relay
            .on_frame(ConnId(id), hello.to_string().as_bytes(), self.now);
        self.apply(fx);
    }

    fn leave(&mut self, id: u64) {
        let fx = self.relay.on_close(ConnId(id));
        self.apply(fx);
    }

    fn tick_to(&mut self, now: Millis) {
        self.now = now;
        let fx = self.relay.on_tick(now);
        self.apply(fx);
    }

    fn issued_to(&self, id: u64) -> usize {
        let want = format!("{id}:ICE");
        self.trace.iter().filter(|t| **t == want).count()
    }

    fn clear(&mut self) {
        self.trace.clear();
    }
}

#[test]
fn never_into_a_room_with_one_role() {
    let mut t = T::new(8);
    t.join(1, 1, "baby", ROOM_A, 3);
    t.tick_to(TTL * 3);
    assert_eq!(t.trace, ["1:welcome", "1:members", "1:peer"]);

    let mut t = T::new(8);
    t.join(1, 1, "parent", ROOM_A, 3);
    t.tick_to(TTL * 3);
    assert!(!t.trace.iter().any(|x| x.ends_with(":ICE")));
}

#[test]
fn a_complete_room_issues_to_each_member_before_the_completing_update() {
    let mut t = T::new(8);
    t.join(1, 1, "baby", ROOM_A, 3);
    t.clear();
    t.join(2, 2, "parent", ROOM_A, 3);
    assert_eq!(
        t.trace,
        [
            "2:welcome",
            "1:ICE",
            "2:ICE",
            "1:members",
            "2:members",
            "1:peer",
            "2:peer"
        ]
    );
}

#[test]
fn the_order_holds_when_the_baby_joins_last() {
    let mut t = T::new(8);
    t.join(1, 1, "parent", ROOM_A, 3);
    t.clear();
    t.join(2, 2, "baby", ROOM_A, 3);
    let ice = t.trace.iter().position(|x| x == "1:ICE").unwrap();
    let members = t
        .trace
        .iter()
        .position(|x| x.ends_with(":members"))
        .unwrap();
    assert!(ice < members);
    assert_eq!(t.issued_to(2), 1);
}

#[test]
fn legacy_rooms_and_unconfigured_relays_issue_nothing() {
    let mut t = T::new(8);
    t.join(1, 1, "baby", "legacy-room", 2);
    t.join(2, 2, "parent", "legacy-room", 2);
    t.tick_to(TTL * 2);
    assert!(!t.trace.iter().any(|x| x.ends_with(":ICE")));

    let mut t = T::with(8, None);
    t.join(1, 1, "baby", ROOM_A, 3);
    t.join(2, 2, "parent", ROOM_A, 3);
    t.tick_to(TTL * 2);
    assert!(!t.trace.iter().any(|x| x.ends_with(":ICE")));
}

#[test]
fn both_members_get_a_fresh_credential_at_half_the_lifetime() {
    let mut t = T::new(8);
    t.join(1, 1, "baby", ROOM_A, 3);
    t.join(2, 2, "parent", ROOM_A, 3);
    t.clear();
    t.tick_to(TTL / 2 - 1);
    assert!(t.trace.is_empty());
    t.tick_to(TTL / 2);
    assert_eq!(t.trace, ["1:ICE", "2:ICE"]);
    t.clear();
    t.tick_to(TTL / 2 + 1_000);
    assert!(t.trace.is_empty(), "one renewal per half lifetime");
    t.tick_to(TTL);
    assert_eq!(t.trace, ["1:ICE", "2:ICE"]);
}

#[test]
fn no_renewal_after_a_role_leaves_and_a_new_completion_issues_to_both() {
    let mut t = T::new(8);
    t.join(1, 1, "baby", ROOM_A, 3);
    t.join(2, 2, "parent", ROOM_A, 3);
    t.leave(2);
    t.clear();
    t.tick_to(TTL / 2);
    t.tick_to(TTL * 2);
    assert!(
        !t.trace.iter().any(|x| x.ends_with(":ICE")),
        "the baby waits alone"
    );
    t.join(3, 3, "parent", ROOM_A, 3);
    let ice: Vec<&String> = t.trace.iter().filter(|x| x.ends_with(":ICE")).collect();
    assert_eq!(ice, ["1:ICE", "3:ICE"]);
}

#[test]
fn a_renewal_supersedes_the_previous_credential_in_the_count() {
    // Cap 2 = this room's two members. If renewals added to the count they would be refused.
    let mut t = T::new(2);
    t.join(1, 7, "baby", ROOM_A, 3);
    t.join(2, 7, "parent", ROOM_A, 3);
    for k in 1..=4 {
        t.tick_to(k * (TTL / 2));
    }
    assert_eq!(t.issued_to(1), 5);
    assert_eq!(t.issued_to(2), 5);
}

#[test]
fn the_per_address_cap_refuses_further_credentials_but_the_room_still_works() {
    let mut t = T::new(2);
    t.join(1, 7, "baby", ROOM_A, 3);
    t.join(2, 7, "parent", ROOM_A, 3);
    t.clear();
    t.join(3, 7, "baby", ROOM_B, 3);
    t.join(4, 7, "parent", ROOM_B, 3);
    assert!(!t.trace.iter().any(|x| x.ends_with(":ICE")));
    assert!(t.trace.contains(&"3:members".to_owned()));
    // Another address is not affected.
    t.join(5, 8, "baby", "CCCCCCCCCCCCCCCCCCCCCC", 3);
    t.join(6, 8, "parent", "CCCCCCCCCCCCCCCCCCCCCC", 3);
    assert_eq!(t.issued_to(5), 1);
    assert_eq!(t.issued_to(6), 1);
}

#[test]
fn a_departed_members_credential_counts_until_it_expires_then_the_capped_room_is_retried() {
    let mut t = T::new(2);
    t.join(1, 7, "baby", ROOM_A, 3);
    t.join(2, 7, "parent", ROOM_A, 3);
    t.join(3, 7, "baby", ROOM_B, 3);
    t.join(4, 7, "parent", ROOM_B, 3);
    assert_eq!(t.issued_to(3), 0);
    t.leave(1);
    t.leave(2);
    t.clear();
    // The first retry (half lifetime) still finds the departed credentials live.
    t.tick_to(TTL / 2);
    assert_eq!(t.issued_to(3), 0);
    // They expire at the full lifetime; the next retry gets through.
    t.tick_to(TTL);
    assert_eq!(t.issued_to(3), 1);
    assert_eq!(t.issued_to(4), 1);
}

#[test]
fn a_cap_of_zero_issues_nothing() {
    let mut t = T::new(0);
    t.join(1, 1, "baby", ROOM_A, 3);
    t.join(2, 2, "parent", ROOM_A, 3);
    assert!(!t.trace.iter().any(|x| x.ends_with(":ICE")));
}

#[test]
fn a_second_parent_joining_a_complete_room_gets_its_own_credential_only() {
    let cfg = RelayConfig {
        max_parents_per_room: 2,
        ping_interval_ms: Millis::MAX / 2,
        hello_rate_per_min: 0,
        new_room_rate_per_min: 0,
        turn_credential_ttl_ms: Some(TTL),
        ..RelayConfig::default()
    };
    let mut t = T::new(8);
    t.relay = Relay::new(cfg, DeterministicEntropy::new(3));
    t.join(1, 1, "baby", ROOM_A, 3);
    t.join(2, 2, "parent", ROOM_A, 3);
    t.clear();
    t.join(3, 3, "parent", ROOM_A, 3);
    let ice: Vec<&String> = t.trace.iter().filter(|x| x.ends_with(":ICE")).collect();
    assert_eq!(ice, ["3:ICE"]);
}

#[test]
fn welcome_carries_the_configured_stun_urls_and_nothing_secret() {
    let cfg = RelayConfig {
        hello_rate_per_min: 0,
        new_room_rate_per_min: 0,
        ping_interval_ms: Millis::MAX / 2,
        stun_urls: vec!["stun:gs.example:3478".to_owned()],
        ..RelayConfig::default()
    };
    let mut t = T::new(8);
    t.relay = Relay::new(cfg, DeterministicEntropy::new(3));
    t.join(1, 1, "baby", ROOM_A, 3);
    let w = t.welcomes.first().unwrap();
    assert_eq!(w["iceServers"], json!(["stun:gs.example:3478"]));
    assert!(w.get("credential").is_none() && w.get("username").is_none());

    let mut plain = T::new(8);
    plain.join(1, 1, "baby", ROOM_A, 3);
    assert!(plain.welcomes.first().unwrap().get("iceServers").is_none());
}
