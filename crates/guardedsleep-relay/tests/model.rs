//! Model-based property test: random event sequences against an independent model of rooms and
//! admission, in both admission modes.

use std::collections::{HashMap, HashSet};

use guardedsleep_core::pairing::{admission_mac, derive_admission_key};
use guardedsleep_core::protocol::Role;
use guardedsleep_relay::{
    Admission, ClientKey, ConnId, DeterministicEntropy, Millis, Relay, RelayConfig, RelayEffect,
};
use proptest::prelude::*;
use serde_json::{Value, json};

const ROOMS: [&str; 2] = ["AAAAAAAAAAAAAAAAAAAAAA", "BBBBBBBBBBBBBBBBBBBBBB"];
/// Only the first room has an admission key registered.
const PSK: [u8; 32] = [3; 32];
const CONNS: u8 = 6;

#[derive(Debug, Clone)]
enum Op {
    Connect(u8, u8),
    Hello(u8, bool, usize),
    Admit {
        conn: u8,
        baby: bool,
        room: usize,
        good: bool,
    },
    Signal(u8, usize),
    Garbage(u8),
    Close(u8),
    Pong(u8),
    Tick(u32),
}

fn op() -> impl Strategy<Value = Op> {
    let c = 0..CONNS;
    prop_oneof![
        (c.clone(), 0u8..3).prop_map(|(c, k)| Op::Connect(c, k)),
        (c.clone(), any::<bool>(), 0usize..2).prop_map(|(c, b, r)| Op::Hello(c, b, r)),
        (c.clone(), any::<bool>(), 0usize..2, any::<bool>()).prop_map(
            |(conn, baby, room, good)| Op::Admit {
                conn,
                baby,
                room,
                good
            }
        ),
        (c.clone(), 0usize..2).prop_map(|(c, r)| Op::Signal(c, r)),
        c.clone().prop_map(Op::Garbage),
        c.clone().prop_map(Op::Close),
        c.prop_map(Op::Pong),
        (0u32..20_000).prop_map(Op::Tick),
    ]
}

#[derive(Default)]
struct Model {
    live: HashSet<u64>,
    pending: HashMap<u64, (Role, usize)>,
    joined: HashMap<u64, (usize, Role, String)>,
    nonce: HashMap<u64, String>,
    valid_answers: HashSet<u64>,
    last_members: HashMap<u64, Vec<String>>,
}

struct Sut {
    relay: Relay<DeterministicEntropy>,
    model: Model,
    required: bool,
    now: Millis,
}

impl Sut {
    fn new(required: bool) -> Self {
        let mut relay = Relay::new(
            RelayConfig {
                admission: if required {
                    Admission::Required
                } else {
                    Admission::Open
                },
                hello_rate_per_min: 0,
                new_room_rate_per_min: 0,
                ..RelayConfig::default()
            },
            DeterministicEntropy::new(42),
        );
        assert!(relay.set_admission_key(ROOMS[0], derive_admission_key(&PSK)));
        Self {
            relay,
            model: Model::default(),
            required,
            now: 0,
        }
    }

    fn apply(&mut self, effects: Vec<RelayEffect>) {
        let live_at_start = self.model.live.clone();
        let mut closed = Vec::new();
        for e in effects {
            match e {
                RelayEffect::Send { conn, text } => {
                    assert!(
                        live_at_start.contains(&conn.0),
                        "send to a connection the relay forgot"
                    );
                    let v: Value = serde_json::from_str(&text).unwrap();
                    match v["type"].as_str().unwrap() {
                        "challenge" => {
                            self.model
                                .nonce
                                .insert(conn.0, v["nonce"].as_str().unwrap().into());
                        }
                        "welcome" => {
                            let (role, room) = self.model.pending[&conn.0];
                            assert_eq!(v["roomId"], ROOMS[room]);
                            if self.required {
                                assert!(
                                    self.model.valid_answers.contains(&conn.0),
                                    "joined without a valid admission answer"
                                );
                            }
                            let id = v["clientId"].as_str().unwrap().to_owned();
                            self.model.joined.insert(conn.0, (room, role, id));
                        }
                        "members" => {
                            let ids = v["peers"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|p| p["clientId"].as_str().unwrap().to_owned())
                                .collect();
                            self.model.last_members.insert(conn.0, ids);
                        }
                        _ => {}
                    }
                }
                RelayEffect::Close { conn, .. } => closed.push(conn.0),
                RelayEffect::Ping { .. }
                | RelayEffect::Log { .. }
                | RelayEffect::IssueIceServers { .. } => {}
            }
        }
        for c in closed {
            self.forget(c);
        }
    }

    fn forget(&mut self, c: u64) {
        self.model.live.remove(&c);
        self.model.joined.remove(&c);
        self.model.last_members.remove(&c);
        self.model.nonce.remove(&c);
        self.model.valid_answers.remove(&c);
    }

    fn step(&mut self, op: &Op) {
        match *op {
            Op::Connect(c, k) => {
                let c = u64::from(c);
                if self.model.live.contains(&c) {
                    return;
                }
                self.model.live.insert(c);
                self.model.pending.remove(&c);
                let fx = self
                    .relay
                    .on_connect(ConnId(c), ClientKey(u128::from(k)), self.now);
                self.apply(fx);
            }
            Op::Hello(c, baby, room) => {
                let role = if baby { Role::Baby } else { Role::Parent };
                let c = u64::from(c);
                self.model.pending.insert(c, (role, room));
                let msg = json!({"type":"hello","role":role.as_str(),"roomId":ROOMS[room],"protocolVersion":3});
                let fx = self
                    .relay
                    .on_frame(ConnId(c), msg.to_string().as_bytes(), self.now);
                self.apply(fx);
            }
            Op::Admit {
                conn,
                baby,
                room,
                good,
            } => {
                let role = if baby { Role::Baby } else { Role::Parent };
                let c = u64::from(conn);
                let mac = match self.model.nonce.get(&c) {
                    Some(n) => {
                        let psk = if good { PSK } else { [4; 32] };
                        admission_mac(&derive_admission_key(&psk), n, role, ROOMS[room]).unwrap()
                    }
                    None => "0".repeat(64),
                };
                if self.model.live.contains(&c) {
                    let first_answer = self.model.nonce.remove(&c).is_some();
                    if good && first_answer && room == 0 && !self.model.joined.contains_key(&c) {
                        self.model.valid_answers.insert(c);
                    }
                }
                self.model.pending.insert(c, (role, room));
                let msg = json!({"type":"admit","role":role.as_str(),"roomId":ROOMS[room],"protocolVersion":3,"mac":mac});
                let fx = self
                    .relay
                    .on_frame(ConnId(c), msg.to_string().as_bytes(), self.now);
                self.apply(fx);
            }
            Op::Signal(c, room) => {
                let msg = json!({"type":"signal","roomId":ROOMS[room],"payload":{"x":1}});
                let fx =
                    self.relay
                        .on_frame(ConnId(u64::from(c)), msg.to_string().as_bytes(), self.now);
                self.apply(fx);
            }
            Op::Garbage(c) => {
                let fx = self
                    .relay
                    .on_frame(ConnId(u64::from(c)), b"\xff{{", self.now);
                self.apply(fx);
            }
            Op::Close(c) => {
                let c = u64::from(c);
                let was_live = self.model.live.contains(&c);
                let fx = self.relay.on_close(ConnId(c));
                if !was_live {
                    assert!(fx.is_empty());
                }
                self.model.live.remove(&c);
                self.model.joined.remove(&c);
                self.apply(fx);
                self.forget(c);
            }
            Op::Pong(c) => self.relay.on_pong(ConnId(u64::from(c))),
            Op::Tick(ms) => {
                self.now += Millis::from(ms);
                let fx = self.relay.on_tick(self.now);
                self.apply(fx);
            }
        }
        self.check();
    }

    fn check(&self) {
        let mut rooms: HashMap<usize, Vec<(&Role, &String)>> = HashMap::new();
        for (room, role, id) in self.model.joined.values() {
            rooms.entry(*room).or_default().push((role, id));
        }
        for (room, members) in &rooms {
            let babies = members.iter().filter(|m| *m.0 == Role::Baby).count();
            let parents = members.len() - babies;
            assert!(babies <= 1, "room {room}: {babies} babies");
            assert!(parents <= 1, "room {room}: {parents} parents");
        }
        assert_eq!(self.relay.conn_count(), self.model.live.len());
        assert_eq!(self.relay.room_count(), rooms.len());
        for (c, (room, _, _)) in &self.model.joined {
            let expect: HashSet<&String> = rooms[room].iter().map(|m| m.1).collect();
            let got: HashSet<&String> = self.model.last_members[c].iter().collect();
            assert_eq!(got, expect, "stale member list for conn {c}");
        }
        if self.required {
            for c in self.model.joined.keys() {
                assert!(self.model.valid_answers.contains(c) || !self.model.live.contains(c));
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn rooms_and_admission_hold_their_invariants(
        required in any::<bool>(),
        ops in proptest::collection::vec(op(), 1..120),
    ) {
        let mut sut = Sut::new(required);
        for o in &ops {
            sut.step(o);
        }
        for c in 0..CONNS {
            let fx = sut.relay.on_close(ConnId(u64::from(c)));
            sut.apply(fx);
            sut.forget(u64::from(c));
        }
        prop_assert_eq!(sut.relay.conn_count(), 0);
        prop_assert_eq!(sut.relay.room_count(), 0);
    }

    #[test]
    fn arbitrary_bytes_never_panic_and_never_join(
        required in any::<bool>(),
        frames in proptest::collection::vec(proptest::collection::vec(any::<u8>(), 0..200), 1..30),
    ) {
        let mut sut = Sut::new(required);
        sut.step(&Op::Connect(0, 0));
        for f in &frames {
            let fx = sut.relay.on_frame(ConnId(0), f, sut.now);
            sut.apply(fx);
            prop_assert!(sut.model.joined.is_empty());
        }
    }
}
