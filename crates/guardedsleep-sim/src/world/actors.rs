//! Actors: process lifecycle, driving a `Link`, executing its actions, the client side of signaling.

use super::*;

impl World {
    pub(super) fn start_actor(&mut self, a: usize) {
        let entropy = self.rng.next_u64() & 0xFFFF_FFFF;
        self.actors[a].chain += 1;
        let chain = self.actors[a].chain;
        self.actors[a].last_wall_ms = None;
        self.deliver(a, LinkInput::Start { entropy });
        self.report_wall_clock(a);
        self.push(self.now + 250, 3, Ev::Tick { actor: a, chain });
    }

    /// Reports the simulated wall clock at start and then every 60 s of virtual time, like a driver
    /// does. Synthetic (no real date) and not an "input" for the tick pacing.
    pub(super) fn report_wall_clock(&mut self, a: usize) {
        let due = self.actors[a]
            .last_wall_ms
            .is_none_or(|t| self.now.saturating_sub(t) >= WALL_CLOCK_EVERY_MS);
        if due {
            self.actors[a].last_wall_ms = Some(self.now);
            let true_s = i64::try_from(self.wall_base + self.now / 1_000).unwrap_or(i64::MAX);
            let unix_s = u64::try_from(true_s + self.actors[a].wall_offset_s).unwrap_or(0);
            self.deliver(a, LinkInput::WallClock { unix_s });
        }
    }

    pub(super) fn on_tick(&mut self, a: usize, chain: u32) {
        let ac = &self.actors[a];
        if ac.chain != chain || !ac.alive || ac.link.is_none() {
            return;
        }
        self.deliver(a, LinkInput::Tick);
        // After the tick: the first input of a moment takes the timers that are due.
        self.report_wall_clock(a);
        let ac = &self.actors[a];
        let state = ac.link.as_ref().and_then(|l| l.status()).map(|s| s.state);
        if matches!(state, Some(LinkState::Stopped | LinkState::FailedNeedsUser)) {
            return;
        }
        // Tick often while something happened recently, otherwise at the 500 ms contract rate.
        let period = if self.now.saturating_sub(ac.last_input_at) < 2_000 {
            250
        } else {
            500
        };
        self.push(self.now + period, 3, Ev::Tick { actor: a, chain });
    }

    pub(super) fn finish_rec(&mut self, a: usize) {
        if let Some(r) = self.actors[a].rec.take() {
            self.recs[r].final_state = self.actors[a]
                .link
                .as_ref()
                .and_then(|l| l.status())
                .map(|s| s.state);
        }
    }

    /// Process death: sockets and peer connections vanish, nobody is told.
    pub(super) fn kill_actor(&mut self, a: usize) {
        self.finish_rec(a);
        let reachable = {
            let ac = &self.actors[a];
            ac.net_up
        };
        let socks = std::mem::take(&mut self.actors[a].socks);
        for s in socks.into_values() {
            if reachable {
                let t = self.fifo_time(1, s, 0, self.now + 20, false);
                self.push(t, 2, Ev::ServerClose { sock: s });
            }
        }
        let ac = &mut self.actors[a];
        ac.alive = false;
        ac.link = None;
        ac.chain += 1;
        ac.pcs.clear();
        ac.held_up.clear();
        ac.held_down.clear();
    }

    pub(super) fn restart_actor(&mut self, a: usize) {
        if self.actors[a].raw.is_some() {
            return;
        }
        let mut cfg = self.actors[a].cfg.clone();
        cfg.paired_before = true;
        // A restart is a new run: a new seed, as the platforms draw one per run.
        cfg.seed = link_seed(self.seed, &self.actors[a].name, self.actors[a].instance + 1);
        if let Some(c) = self.canary.as_mut() {
            c.set.add_seed(
                &format!(
                    "seed_{}_run{}",
                    self.actors[a].name,
                    self.actors[a].instance + 1
                ),
                &cfg.seed,
            );
        }
        if let Some(m) = self.actors[a].pending_mailboxes.take() {
            cfg.mailboxes = m;
        }
        let Ok(link) = Link::new(cfg.clone()) else {
            return;
        };
        let ac = &mut self.actors[a];
        ac.cfg = cfg;
        ac.link = Some(link);
        ac.alive = true;
        ac.instance += 1;
        ac.rec = None;
        self.start_actor(a);
    }

    pub(super) fn deliver(&mut self, a: usize, input: LinkInput) {
        if !self.actors[a].alive {
            return;
        }
        let now = self.now;
        // The wall clock, like a tick, says nothing about activity.
        let is_tick = matches!(input, LinkInput::Tick | LinkInput::WallClock { .. });
        if !is_tick {
            self.actors[a].last_input_at = now;
        }
        if self.record && self.actors[a].rec.is_none() && self.actors[a].link.is_some() {
            let ac = &self.actors[a];
            self.recs
                .push(TraceRec::new(&ac.name, ac.instance, ac.cfg.clone()));
            let ix = self.recs.len() - 1;
            self.actors[a].rec = Some(ix);
        }
        let recorded = self.actors[a].rec.map(|_| input.clone());
        if self.canary.is_some() && !is_tick {
            let wire = matches!(
                input,
                LinkInput::MailboxFrame { .. }
                    | LinkInput::AttemptLocalDescription { .. }
                    | LinkInput::AttemptLocalIce { .. }
                    | LinkInput::AttemptDataMessage { .. }
            );
            self.log_line(a, "input", &format!("{input:?}"), wire);
        }
        let Some(link) = self.actors[a].link.as_mut() else {
            return;
        };
        let actions = link.handle(now, input);
        if self.canary.is_some() {
            self.log_actions(a, &actions);
        }
        if let (Some(r), Some(inp)) = (self.actors[a].rec, recorded) {
            self.recs[r].push(now, inp, &actions);
        }
        self.exec(a, actions);
    }

    pub(super) fn exec(&mut self, a: usize, actions: Vec<LinkAction>) {
        for act in actions {
            match act {
                LinkAction::OpenMailbox { id, .. } => self.open_mailbox(a, id),
                LinkAction::CloseMailbox { id } => {
                    if let Some(s) = self.actors[a].socks.remove(&id) {
                        let epoch = self.socks[&s].1;
                        if self.reach(a, epoch) {
                            let t = self.fifo_time(1, s, 0, self.now + 20, false);
                            self.push(t, 2, Ev::ServerClose { sock: s });
                        }
                    }
                }
                LinkAction::MailboxSend { id, frame } => self.client_send_on(a, id, frame, None),
                LinkAction::CreateAttempt {
                    attempt,
                    ice_servers,
                    ..
                } => {
                    self.attempts.push((a, self.now));
                    self.attempt_ice.push((a, self.now, ice_servers));
                    if self.actors[a].pcs.values().any(|p| !p.closed) {
                        self.violations.push(format!(
                            "{}: create_attempt {attempt} while another attempt is live",
                            self.actors[a].name
                        ));
                    }
                    self.new_pc(a, attempt);
                }
                LinkAction::CloseAttempt { attempt } => self.close_attempt(a, attempt),
                LinkAction::CreateOffer { attempt } => {
                    let d = self.rng.range(5, 30);
                    self.push(
                        self.now + d,
                        2,
                        Ev::LocalDesc {
                            actor: a,
                            handle: attempt,
                            sdp_type: "offer",
                        },
                    );
                }
                LinkAction::CreateAnswer { attempt } => {
                    let d = self.rng.range(5, 30);
                    self.push(
                        self.now + d,
                        2,
                        Ev::LocalDesc {
                            actor: a,
                            handle: attempt,
                            sdp_type: "answer",
                        },
                    );
                }
                LinkAction::SetRemoteDescription { attempt, sdp, .. } => {
                    self.set_remote(a, attempt, &sdp);
                }
                LinkAction::SendData { attempt, text } => self.send_data(a, attempt, text),
                // The fake platform of talk-back: a healthy one. The chime takes 600 ms, the
                // microphone attaches 30 ms after it was asked to; what the baby's microphone
                // hears (`levels`, `talk_levels` verbs) is up to the scenario.
                LinkAction::PlayTalkChime { attempt, .. } => self.push(
                    self.now + 600,
                    2,
                    Ev::Input {
                        actor: a,
                        input: LinkInput::TalkChimeDone { attempt },
                    },
                ),
                LinkAction::SetTalkMic { enabled: true, .. } => self.push(
                    self.now + 30,
                    2,
                    Ev::Input {
                        actor: a,
                        input: LinkInput::TalkMicAttached,
                    },
                ),
                LinkAction::SetTalkMic { .. }
                | LinkAction::SetPlaybackDucked { .. }
                | LinkAction::SetTalkPlayback { .. }
                | LinkAction::AddIceCandidate { .. }
                | LinkAction::SetMicEnabled { .. }
                | LinkAction::ApplyQuiet
                | LinkAction::SendProbeTone { .. } => {}
                LinkAction::Emit { event } => self.record_event(a, event),
            }
        }
    }

    pub(super) fn record_event(&mut self, a: usize, event: LinkEvent) {
        let now = self.now;
        let ac = &mut self.actors[a];
        if let LinkEvent::Status { status } = &event {
            ac.statuses.push((now, status.clone()));
        }
        // Like a platform: persist the pinned parent secret, so that a restarted process
        // (`restart_process`) is configured with it.
        if let LinkEvent::ParentPinned {
            secret,
            window_until_s,
        } = &event
        {
            ac.cfg.pinned_parent = Some(secret.clone());
            ac.cfg.pin_window_until_s = *window_until_s;
        }
        let since = if ac.last_rx_data == 0 {
            now
        } else {
            now.saturating_sub(ac.last_rx_data)
        };
        ac.events.push(EvRec {
            t: now,
            ev: serde_json::to_value(&event).unwrap_or(Value::Null),
            since_contact: since,
        });
    }

    pub(super) fn reach(&self, a: usize, epoch: u32) -> bool {
        let ac = &self.actors[a];
        ac.alive && ac.net_up && ac.path_epoch == epoch
    }

    /// The path would work again once the network returns.
    pub(super) fn viable(&self, a: usize, epoch: u32) -> bool {
        let ac = &self.actors[a];
        ac.alive && ac.path_epoch == epoch
    }

    pub(super) fn responsive(&self, c: &Conn) -> bool {
        c.actor.is_some_and(|a| self.reach(a, c.epoch))
    }

    pub(super) fn flush_after_up(&mut self, a: usize, emit_available: bool) {
        let ups = std::mem::take(&mut self.actors[a].held_up);
        for (sock, text) in ups {
            let Some(&(_, epoch)) = self.socks.get(&sock) else {
                continue;
            };
            if self.viable(a, epoch) {
                let lat = self.rng.range(5, 40);
                let t = self.fifo_time(1, sock, 0, self.now + lat, false);
                self.push(
                    t,
                    2,
                    Ev::SigToServer {
                        sock,
                        text,
                        spoof: None,
                    },
                );
            }
        }
        let downs = std::mem::take(&mut self.actors[a].held_down);
        for (sock, msg) in downs {
            self.select_sock(sock);
            if self.server.conns.contains_key(&sock) {
                let lat = self.rng.range(5, 40);
                let t = self.fifo_time(0, sock, 0, self.now + lat, false);
                self.push(
                    t,
                    2,
                    Ev::SigToClient {
                        actor: a,
                        sock,
                        msg,
                    },
                );
            }
        }
        if emit_available && self.actors[a].link.is_some() {
            let d = self.rng.range(50, 400);
            self.push(
                self.now + d,
                2,
                Ev::Input {
                    actor: a,
                    input: LinkInput::NetworkAvailable,
                },
            );
        }
    }

    pub fn open_signaling(&mut self, a: usize) {
        self.open_mailbox(a, 0);
    }

    /// Opens mailbox `id` of actor `a`: a socket to simulated relay `id`.
    pub fn open_mailbox(&mut self, a: usize, id: MailboxId) {
        let sock = self.next_sock;
        self.next_sock += 1;
        let epoch = self.actors[a].path_epoch;
        self.socks.insert(sock, (a, epoch));
        self.sock_server.insert(sock, id as usize);
        self.actors[a].socks.insert(id, sock);
        self.select_server(id as usize);
        let (up, reach) = (self.server.up, self.reach(a, epoch));
        if !reach {
            let d = 3_000 + self.rng.range(0, 500);
            self.push(
                self.now + d,
                2,
                Ev::SigToClient {
                    actor: a,
                    sock,
                    msg: ToClient::Closed,
                },
            );
        } else if !up {
            let d = self.rng.range(20, 80);
            self.push(
                self.now + d,
                2,
                Ev::SigToClient {
                    actor: a,
                    sock,
                    msg: ToClient::Closed,
                },
            );
        } else {
            let d = self.rng.range(5, 40);
            self.push(self.now + d, 2, Ev::Accept { sock });
        }
    }

    pub fn client_send(&mut self, a: usize, text: String, spoof: Option<String>) {
        self.client_send_on(a, 0, text, spoof);
    }

    pub fn client_send_on(&mut self, a: usize, id: MailboxId, text: String, spoof: Option<String>) {
        let Some(sock) = self.actors[a].socks.get(&id).copied() else {
            return;
        };
        if let Some(t) = self.signal_type_for(a, &text) {
            let now = self.now;
            self.actors[a].sent_signals.push((now, t));
        }
        let epoch = self.socks[&sock].1;
        for (extra, bypass) in self.plan(a, Via::Signaling, &text) {
            if self.reach(a, epoch) {
                let lat = self.rng.range(5, 40) + extra;
                let t = self.fifo_time(1, sock, 0, self.now + lat, bypass);
                self.push(
                    t,
                    2,
                    Ev::SigToServer {
                        sock,
                        text: text.clone(),
                        spoof: spoof.clone(),
                    },
                );
            } else if self.viable(a, epoch) {
                self.actors[a].held_up.push((sock, text.clone()));
            }
        }
    }

    pub(super) fn on_client_rx(&mut self, a: usize, sock: usize, msg: ToClient) {
        let id = self.sock_server.get(&sock).copied().unwrap_or(0) as MailboxId;
        if self.actors[a].socks.get(&id) != Some(&sock) || !self.actors[a].alive {
            return;
        }
        let epoch = self.socks[&sock].1;
        if !matches!(msg, ToClient::Closed) && !self.reach(a, epoch) {
            if self.viable(a, epoch) {
                self.actors[a].held_down.push((sock, msg));
            }
            return;
        }
        if let ToClient::Text(t) = &msg
            && serde_json::from_str::<Value>(t)
                .ok()
                .and_then(|v| v["type"].as_str().map(|s| s == "welcome"))
                == Some(true)
        {
            let now = self.now;
            self.actors[a].welcomes.push(now);
        }
        if matches!(msg, ToClient::Closed) {
            self.actors[a].socks.remove(&id);
        }
        if self.actors[a].raw.is_some() {
            self.raw_on_signaling(a, msg);
            return;
        }
        let input = match msg {
            ToClient::Opened => LinkInput::MailboxOpened { id },
            ToClient::Text(bytes) => LinkInput::MailboxFrame { id, bytes },
            ToClient::Closed => LinkInput::MailboxClosed {
                id,
                reason: "closed".into(),
            },
        };
        self.deliver(a, input);
    }

    pub(crate) fn schedule_raw_tick(&mut self, actor: usize, delay: u64) {
        self.push(self.now + delay, 3, Ev::RawTick { actor });
    }

    pub(crate) fn schedule_raw_act(&mut self, actor: usize, delay: u64) {
        self.push(self.now + delay, 3, Ev::RawAct { actor });
    }

    pub(crate) fn fresh_fp(&mut self) -> String {
        self.fp_seq += 1;
        synthetic_fp(self.fp_seq)
    }

    pub(crate) fn add_raw_actor(&mut self, name: &str, role: Role, raw: RawState) -> usize {
        let seed = link_seed(self.seed, name, 0);
        let cfg = LinkConfig::with_defaults(role, encode_psk(&synthetic_psk("A")), &seed);
        let room = raw.room.clone();
        let mut a = Actor::new(name, role, cfg, room);
        a.raw = Some(raw);
        self.actors.push(a);
        self.actors.len() - 1
    }

    pub(super) fn actor_psk(&self, a: usize) -> Option<Vec<u8>> {
        decode_psk(&self.actors[a].cfg.psk_b64url)
    }

    /// What a frame `a` sent is, for the "sent_signal" assertions: the frame type, or for a relay
    /// `signal` the type of the message inside (read with the sender's PSK when it is sealed).
    pub(super) fn signal_type_for(&self, a: usize, text: &str) -> Option<String> {
        let v: Value = serde_json::from_str(text).ok()?;
        let t = v["type"].as_str()?;
        if t != "signal" {
            return Some(t.to_owned());
        }
        let payload = &v["payload"];
        if payload.get("env").is_some() {
            let psk = self.actor_psk(a)?;
            return decode_message(&psk, payload)?["type"]
                .as_str()
                .map(str::to_owned);
        }
        payload["type"].as_str().map(str::to_owned)
    }
}
