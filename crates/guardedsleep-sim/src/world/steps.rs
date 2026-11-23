//! Timeline verbs: one method per verb, found through [`VERBS`]; plus fault planning, time jumps
//! and the hostile-relay steps.

use super::*;

type VerbFn = fn(&mut World, &Step);

/// The verbs of a `gs-scenario/1` timeline. A table instead of a `match`: a new verb is one row and
/// one method.
const VERBS: &[(&str, VerbFn)] = &[
    ("end", World::verb_end),
    ("start", World::verb_start),
    ("stop", World::verb_stop),
    ("acknowledge", World::verb_acknowledge),
    ("input", World::verb_input),
    ("levels", World::verb_levels),
    ("talk_levels", World::verb_talk_levels),
    ("network_down", World::verb_network_down),
    ("network_up", World::verb_network_up),
    ("path_switch", World::verb_path_switch),
    ("signaling_down", World::verb_signaling_down),
    ("signaling_up", World::verb_signaling_up),
    ("relay_drop_signals", World::verb_relay_drop_signals),
    ("kill_process", World::verb_kill_process),
    ("restart_process", World::verb_restart_process),
    ("drop_data", World::add_fault),
    ("delay", World::add_fault),
    ("duplicate", World::add_fault),
    ("reorder", World::add_fault),
    ("clear_faults", World::verb_clear_faults),
    ("time_jump", World::time_jump),
    ("stranger_join", World::stranger_join),
    ("stranger_leave", World::verb_kill_process),
    ("ghost_join", World::ghost_join),
    ("replay_stop", World::replay_stop),
    ("relay_rewrite", World::verb_relay_rewrite),
    ("reject_hello", World::verb_reject_hello),
];

impl World {
    pub(super) fn run_step(&mut self, i: usize) {
        let st = self.timeline[i].clone();
        match VERBS.iter().find(|(verb, _)| *verb == st.verb) {
            Some((_, run)) => run(self, &st),
            None => self
                .violations
                .push(format!("unknown timeline verb `{}`", st.verb)),
        }
    }

    fn verb_end(&mut self, _st: &Step) {}

    fn verb_start(&mut self, st: &Step) {
        let targets: Vec<usize> = match &st.peer {
            Some(_) => self.peer_ix(st).into_iter().collect(),
            None => self.link_actors(),
        };
        for a in targets {
            self.start_actor(a);
        }
    }

    fn deliver_to_peer(&mut self, st: &Step, input: LinkInput) {
        if let Some(a) = self.peer_ix(st) {
            self.deliver(a, input);
        }
    }

    fn verb_stop(&mut self, st: &Step) {
        self.deliver_to_peer(st, LinkInput::Stop);
    }

    fn verb_acknowledge(&mut self, st: &Step) {
        self.deliver_to_peer(st, LinkInput::AcknowledgeAlarm);
    }

    fn verb_input(&mut self, st: &Step) {
        let Some(a) = self.peer_ix(st) else {
            return;
        };
        match serde_json::from_value::<LinkInput>(st.input.clone().unwrap_or_default()) {
            Ok(input) => self.deliver(a, input),
            Err(e) => self.violations.push(format!("bad input: {e}")),
        }
    }

    /// What the baby's microphone hears: one sample every 100 ms for `for_ms`, like the platform's
    /// ~10 Hz.
    fn verb_levels(&mut self, st: &Step) {
        self.feed_levels(st, |rms| LinkInput::AudioLevel { rms });
    }

    /// The level of the incoming talk audio, sampled like [`World::verb_levels`].
    fn verb_talk_levels(&mut self, st: &Step) {
        self.feed_levels(st, |rms| LinkInput::TalkLevel { rms });
    }

    fn feed_levels(&mut self, st: &Step, sample: fn(f64) -> LinkInput) {
        let Some(a) = self.peer_ix(st) else {
            return;
        };
        let rms = st.rms.unwrap_or(0.0);
        for i in 0..st.for_ms.unwrap_or(100) / 100 {
            let input = sample(rms);
            self.push(self.now + i * 100, 2, ActorEv::Input { actor: a, input });
        }
    }

    fn verb_network_down(&mut self, st: &Step) {
        if let Some(a) = self.peer_ix(st) {
            self.actors[a].net_up = false;
        }
    }

    fn verb_network_up(&mut self, st: &Step) {
        if let Some(a) = self.peer_ix(st) {
            self.actors[a].net_up = true;
            self.flush_after_up(a, st.emit_available.unwrap_or(true));
        }
    }

    fn verb_path_switch(&mut self, st: &Step) {
        let Some(a) = self.peer_ix(st) else {
            return;
        };
        let gap = st.gap_ms.unwrap_or(1_500);
        let ac = &mut self.actors[a];
        ac.path_epoch += 1;
        ac.net_up = false;
        ac.held_up.clear();
        ac.held_down.clear();
        self.push(self.now + gap, 3, ActorEv::PathBack { actor: a });
    }

    /// Runs `apply` once per relay the step addresses, with that relay selected.
    fn on_targeted_servers(&mut self, st: &Step, mut apply: impl FnMut(&mut World)) {
        for srv in self.targeted_servers(st) {
            self.select_server(srv);
            apply(self);
        }
    }

    fn verb_signaling_down(&mut self, st: &Step) {
        self.on_targeted_servers(st, World::signaling_down);
    }

    fn verb_signaling_up(&mut self, st: &Step) {
        self.on_targeted_servers(st, |w| {
            w.server.up = true;
            w.server.sweep_gen += 1;
            let (r#gen, srv) = (w.server.sweep_gen, w.cur_server);
            w.push(w.now + SWEEP_MS, 3, RelayEv::Sweep { r#gen, srv });
        });
    }

    /// A relay that stays up, answers pings and takes frames, but delivers no `signal`.
    fn verb_relay_drop_signals(&mut self, st: &Step) {
        let on = st.mode.as_deref() != Some("off");
        self.on_targeted_servers(st, |w| w.server.drop_signals = on);
    }

    /// `kill_process` and `stranger_leave`: the process dies, nobody is told.
    fn verb_kill_process(&mut self, st: &Step) {
        if let Some(a) = self.peer_ix(st) {
            self.kill_actor(a);
        }
    }

    fn verb_restart_process(&mut self, st: &Step) {
        let Some(a) = self.peer_ix(st) else {
            return;
        };
        self.kill_actor(a);
        self.actors[a].pending_mailboxes = st.mailboxes.clone();
        let after = st.after_ms.unwrap_or(0);
        self.push(self.now + after, 2, ActorEv::Restart { actor: a });
    }

    fn verb_clear_faults(&mut self, _st: &Step) {
        self.faults.clear();
    }

    /// The relay turns hostile (or honest again) at run time: `mode` is an `sdp_rewrite` value,
    /// absent for honest.
    fn verb_relay_rewrite(&mut self, st: &Step) {
        self.relay.sdp_rewrite = st.mode.clone();
    }

    fn verb_reject_hello(&mut self, st: &Step) {
        let for_ms = st.for_ms.unwrap_or(u64::MAX - self.now);
        let until = self.now.saturating_add(for_ms);
        self.on_targeted_servers(st, |w| w.server.reject_hello_until = until);
    }

    fn add_fault(&mut self, st: &Step) {
        let from = st
            .from
            .as_deref()
            .and_then(|n| if n == "all" { None } else { self.actor_ix(n) });
        let kind = match st.verb.as_str() {
            "drop_data" => FaultKind::Drop,
            "delay" => FaultKind::Delay,
            "duplicate" => FaultKind::Duplicate,
            _ => FaultKind::Reorder,
        };
        let via = if st.via.as_deref() == Some("signaling") {
            Via::Signaling
        } else {
            Via::Data
        };
        self.faults.push(Fault {
            kind,
            from,
            via,
            contains: st.message.clone(),
            start: self.now,
            until: st.for_ms.map_or(u64::MAX, |d| self.now + d),
            p: st.p.unwrap_or(1.0),
            ms: st.ms.unwrap_or(0),
            jitter: st.jitter_ms.unwrap_or(0),
        });
    }

    pub(super) fn plan(&mut self, from: usize, via: Via, text: &str) -> Vec<(u64, bool)> {
        let now = self.now;
        let faults: Vec<Fault> = self
            .faults
            .iter()
            .filter(|f| {
                f.via == via
                    && f.start <= now
                    && now < f.until
                    && f.from.is_none_or(|x| x == from)
                    && f.contains.as_deref().is_none_or(|c| text.contains(c))
            })
            .cloned()
            .collect();
        let (mut extra, mut copies, mut bypass) = (0u64, 1usize, false);
        for f in faults {
            match f.kind {
                FaultKind::Drop => {
                    if self.rng.chance(f.p) {
                        return Vec::new();
                    }
                }
                FaultKind::Delay => extra += f.ms + self.rng.range(0, f.jitter),
                FaultKind::Duplicate => {
                    if self.rng.chance(f.p) {
                        copies = 2;
                    }
                }
                FaultKind::Reorder => {
                    if self.rng.chance(f.p) {
                        extra += self.rng.range(0, f.ms);
                        bypass = true;
                    }
                }
            }
        }
        (0..copies)
            .map(|i| {
                (
                    extra + if i > 0 { self.rng.range(1, 30) } else { 0 },
                    bypass,
                )
            })
            .collect()
    }

    pub(super) fn time_jump(&mut self, st: &Step) {
        let ms = st.ms.unwrap_or(0);
        let old = std::mem::take(&mut self.q);
        for ((t, p, s), ev) in old {
            match ev {
                Ev::Timeline(_) => {
                    self.q.insert((t, p, s), ev);
                }
                Ev::Peer(PeerEv::PcConnect {
                    a,
                    ha,
                    b,
                    hb,
                    since,
                }) => {
                    let ev = PeerEv::PcConnect {
                        a,
                        ha,
                        b,
                        hb,
                        since: since + ms,
                    };
                    self.q.insert((t + ms, p, s), ev.into());
                }
                other => {
                    self.q.insert((t + ms, p, s), other);
                }
            }
        }
        for v in self.fifo.values_mut() {
            *v += ms;
        }
        for a in &mut self.actors {
            for pc in a.pcs.values_mut() {
                if let Some(s) = pc.unhealthy_since.as_mut() {
                    *s += ms;
                }
            }
        }
        self.now += ms;
        if st.peer_traffic.as_deref() == Some("kept") {
            let mut deliveries = Vec::new();
            for (i, a) in self.actors.iter().enumerate() {
                if a.link.is_none() {
                    continue;
                }
                for (h, pc) in &a.pcs {
                    if pc.connected && !pc.closed && !pc.failed {
                        deliveries.push((i, *h));
                    }
                }
            }
            for (i, h) in deliveries {
                let text = json!({"type":"heartbeat","v":3,"t":0,"seq":1_000_000}).to_string();
                self.push(
                    self.now,
                    0,
                    PeerEv::Data {
                        to: i,
                        handle: h,
                        text,
                    },
                );
            }
        }
    }

    /// A hostile relay re-delivers the last recorded `stop_notice` to `peer`, as the original
    /// sender, on whatever connection `peer` has now.
    pub(super) fn replay_stop(&mut self, st: &Step) {
        let Some(a) = self.peer_ix(st) else {
            return;
        };
        self.select_server(st.relay.unwrap_or(0));
        let Some((from, payload)) = self.server.last_stop.clone() else {
            self.violations
                .push("replay_stop: no stop notice was recorded".into());
            return;
        };
        let sock = self
            .server
            .conns
            .iter()
            .find(|(_, c)| c.actor == Some(a) && c.joined)
            .map(|(s, _)| *s);
        if let Some(sock) = sock {
            self.server_send(sock, json!({"type":"signal","from":from,"payload":payload}));
        }
    }

    /// A socket whose client died silently: holds its room slot until the liveness sweep.
    pub(super) fn ghost_join(&mut self, st: &Step) {
        let role = st.role.unwrap_or(Role::Parent);
        let requested = self.target_room();
        let key = if self.relay.alias_rooms {
            "alias".to_owned()
        } else {
            requested.clone()
        };
        let (babies, parents) = {
            let m = self.room_members(&key);
            (
                m.iter().filter(|(_, c)| c.role == Role::Baby).count(),
                m.iter().filter(|(_, c)| c.role == Role::Parent).count(),
            )
        };
        if (role == Role::Baby && babies > 0)
            || (role == Role::Parent && parents >= self.relay.parent_cap)
        {
            return;
        }
        self.select_server(st.relay.unwrap_or(0));
        let sock = self.next_sock;
        self.next_sock += 1;
        self.sock_server.insert(sock, self.cur_server);
        self.server.next_client += 1;
        self.server.next_join += 1;
        let client_id = format!("c{}", self.server.next_client);
        let join_seq = self.server.next_join;
        self.server.conns.insert(
            sock,
            Conn {
                actor: None,
                epoch: 0,
                role,
                client_id,
                requested,
                key: key.clone(),
                joined: true,
                alive: true,
                join_seq,
            },
        );
        self.broadcast_members(&key);
    }
}
