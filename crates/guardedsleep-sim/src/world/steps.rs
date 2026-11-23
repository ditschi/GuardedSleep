//! Timeline verbs that need more than a line: fault planning, time jumps, hostile-relay steps.

use super::*;

impl World {
    pub(super) fn run_step(&mut self, i: usize) {
        let st = self.timeline[i].clone();
        match st.verb.as_str() {
            "end" => {}
            "start" => {
                let targets = match &st.peer {
                    Some(_) => self.peer_ix(&st).into_iter().collect(),
                    None => self.link_actors(),
                };
                for a in targets {
                    self.start_actor(a);
                }
            }
            "stop" => {
                if let Some(a) = self.peer_ix(&st) {
                    self.deliver(a, LinkInput::Stop);
                }
            }
            "acknowledge" => {
                if let Some(a) = self.peer_ix(&st) {
                    self.deliver(a, LinkInput::AcknowledgeAlarm);
                }
            }
            "input" => {
                if let Some(a) = self.peer_ix(&st) {
                    match serde_json::from_value::<LinkInput>(st.input.clone().unwrap_or_default())
                    {
                        Ok(input) => self.deliver(a, input),
                        Err(e) => self.violations.push(format!("bad input: {e}")),
                    }
                }
            }
            // What the baby's microphone hears (`levels`) and the level of the incoming talk audio
            // (`talk_levels`): one sample every 100 ms for `for_ms`, like the platform's ~10 Hz.
            "levels" | "talk_levels" => {
                if let Some(a) = self.peer_ix(&st) {
                    let rms = st.rms.unwrap_or(0.0);
                    for i in 0..st.for_ms.unwrap_or(100) / 100 {
                        let input = if st.verb == "levels" {
                            LinkInput::AudioLevel { rms }
                        } else {
                            LinkInput::TalkLevel { rms }
                        };
                        self.push(self.now + i * 100, 2, Ev::Input { actor: a, input });
                    }
                }
            }
            "network_down" => {
                if let Some(a) = self.peer_ix(&st) {
                    self.actors[a].net_up = false;
                }
            }
            "network_up" => {
                if let Some(a) = self.peer_ix(&st) {
                    self.actors[a].net_up = true;
                    self.flush_after_up(a, st.emit_available.unwrap_or(true));
                }
            }
            "path_switch" => {
                if let Some(a) = self.peer_ix(&st) {
                    let gap = st.gap_ms.unwrap_or(1_500);
                    let ac = &mut self.actors[a];
                    ac.path_epoch += 1;
                    ac.net_up = false;
                    ac.held_up.clear();
                    ac.held_down.clear();
                    self.push(self.now + gap, 3, Ev::PathBack { actor: a });
                }
            }
            "signaling_down" => {
                for srv in self.targeted_servers(&st) {
                    self.select_server(srv);
                    self.signaling_down();
                }
            }
            "signaling_up" => {
                for srv in self.targeted_servers(&st) {
                    self.select_server(srv);
                    self.server.up = true;
                    self.server.sweep_gen += 1;
                    let r#gen = self.server.sweep_gen;
                    self.push(self.now + SWEEP_MS, 3, Ev::Sweep { r#gen, srv });
                }
            }
            // A relay that stays up, answers pings and takes frames, but delivers no `signal`.
            "relay_drop_signals" => {
                let on = st.mode.as_deref() != Some("off");
                for srv in self.targeted_servers(&st) {
                    self.select_server(srv);
                    self.server.drop_signals = on;
                }
            }
            "kill_process" => {
                if let Some(a) = self.peer_ix(&st) {
                    self.kill_actor(a);
                }
            }
            "restart_process" => {
                if let Some(a) = self.peer_ix(&st) {
                    self.kill_actor(a);
                    self.actors[a].pending_mailboxes = st.mailboxes.clone();
                    let after = st.after_ms.unwrap_or(0);
                    self.push(self.now + after, 2, Ev::Restart { actor: a });
                }
            }
            "drop_data" | "delay" | "duplicate" | "reorder" => self.add_fault(&st),
            "clear_faults" => self.faults.clear(),
            "time_jump" => self.time_jump(&st),
            "stranger_join" => self.stranger_join(&st),
            "stranger_leave" => {
                if let Some(a) = self.peer_ix(&st) {
                    self.kill_actor(a);
                }
            }
            "ghost_join" => self.ghost_join(&st),
            "replay_stop" => self.replay_stop(&st),
            // The relay turns hostile (or honest again) at run time: `mode` is an `sdp_rewrite`
            // value, absent for honest.
            "relay_rewrite" => self.relay.sdp_rewrite = st.mode.clone(),
            "reject_hello" => {
                let for_ms = st.for_ms.unwrap_or(u64::MAX - self.now);
                for srv in self.targeted_servers(&st) {
                    self.select_server(srv);
                    self.server.reject_hello_until = self.now.saturating_add(for_ms);
                }
            }
            other => self
                .violations
                .push(format!("unknown timeline verb `{other}`")),
        }
    }

    pub(super) fn add_fault(&mut self, st: &Step) {
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
                Ev::PcConnect {
                    a,
                    ha,
                    b,
                    hb,
                    since,
                } => {
                    let ev = Ev::PcConnect {
                        a,
                        ha,
                        b,
                        hb,
                        since: since + ms,
                    };
                    self.q.insert((t + ms, p, s), ev);
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
                    Ev::Data {
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
