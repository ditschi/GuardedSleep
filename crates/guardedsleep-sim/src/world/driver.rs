//! Driving a `Link`: feeding it inputs, recording the trace and performing the actions it returns
//! on the fake platform.

use super::*;

/// The wall clock, like a tick, says nothing about activity.
pub(super) fn is_tick_input(input: &LinkInput) -> bool {
    matches!(input, LinkInput::Tick | LinkInput::WallClock { .. })
}

impl World {
    pub(super) fn deliver(&mut self, a: ActorIx, input: LinkInput) {
        if !self.actors[a].alive {
            return;
        }
        let now = self.now;
        if !is_tick_input(&input) {
            self.actors[a].last_input_at = now;
        }
        self.start_recording(a);
        let recorded = self.actors[a].rec.map(|_| input.clone());
        self.log_input(a, &input);
        let Some(link) = self.actors[a].link.as_mut() else {
            return;
        };
        let actions = link.handle(now, input);
        self.log_actions(a, &actions);
        self.record_step(a, recorded, &actions);
        self.exec(a, actions);
    }

    /// The first input of a live `Link` opens its trace record.
    fn start_recording(&mut self, a: ActorIx) {
        if !self.should_record(a) {
            return;
        }
        let ac = &self.actors[a];
        self.recs
            .push(TraceRec::new(&ac.name, ac.instance, ac.cfg.clone()));
        self.actors[a].rec = Some(self.recs.len() - 1);
    }

    fn should_record(&self, a: ActorIx) -> bool {
        let ac = &self.actors[a];
        self.record && ac.rec.is_none() && ac.link.is_some()
    }

    fn record_step(&mut self, a: ActorIx, input: Option<LinkInput>, actions: &[LinkAction]) {
        if let (Some(r), Some(input)) = (self.actors[a].rec, input) {
            self.recs[r].push(self.now, input, actions);
        }
    }

    pub(super) fn exec(&mut self, a: ActorIx, actions: Vec<LinkAction>) {
        for act in actions {
            self.exec_one(a, act);
        }
    }

    /// Sorts one action into the part of the fake platform that performs it. Every variant is
    /// named, so a new core action does not compile until it is placed.
    fn exec_one(&mut self, a: ActorIx, act: LinkAction) {
        match act {
            LinkAction::Emit { event } => self.record_event(a, event),
            LinkAction::OpenMailbox { .. }
            | LinkAction::CloseMailbox { .. }
            | LinkAction::MailboxSend { .. } => self.exec_mailbox(a, act),
            LinkAction::CreateAttempt { .. }
            | LinkAction::CloseAttempt { .. }
            | LinkAction::CreateOffer { .. }
            | LinkAction::CreateAnswer { .. }
            | LinkAction::SetRemoteDescription { .. }
            | LinkAction::SendData { .. } => self.exec_attempt(a, act),
            LinkAction::PlayTalkChime { .. } | LinkAction::SetTalkMic { .. } => {
                self.exec_talk(a, act)
            }
            LinkAction::SetPlaybackDucked { .. }
            | LinkAction::SetTalkPlayback { .. }
            | LinkAction::AddIceCandidate { .. }
            | LinkAction::SetMicEnabled { .. }
            | LinkAction::ApplyQuiet
            | LinkAction::SendProbeTone { .. } => {}
        }
    }

    fn exec_mailbox(&mut self, a: ActorIx, act: LinkAction) {
        match act {
            LinkAction::OpenMailbox { id, .. } => self.open_mailbox(a, id),
            LinkAction::CloseMailbox { id } => self.close_mailbox(a, id),
            LinkAction::MailboxSend { id, frame } => self.client_send_on(a, id, frame, None),
            _ => {}
        }
    }

    fn close_mailbox(&mut self, a: ActorIx, id: MailboxId) {
        let Some(s) = self.actors[a].socks.remove(&id) else {
            return;
        };
        self.close_toward_server(a, SockIx::from(s));
    }

    fn exec_attempt(&mut self, a: ActorIx, act: LinkAction) {
        match act {
            LinkAction::CreateAttempt {
                attempt,
                ice_servers,
                ..
            } => self.create_attempt(PcRef::new(a, attempt.into()), ice_servers),
            LinkAction::CloseAttempt { attempt } => self.close_attempt(a, attempt.into()),
            LinkAction::CreateOffer { attempt } => {
                self.schedule_local_desc(PcRef::new(a, attempt.into()), SdpKind::Offer);
            }
            LinkAction::CreateAnswer { attempt } => {
                self.schedule_local_desc(PcRef::new(a, attempt.into()), SdpKind::Answer);
            }
            LinkAction::SetRemoteDescription { attempt, sdp, .. } => {
                self.set_remote(a, attempt, &sdp);
            }
            LinkAction::SendData { attempt, text } => self.send_data(a, attempt, text),
            _ => {}
        }
    }

    fn create_attempt(&mut self, pc: PcRef, ice_servers: Vec<IceServer>) {
        let a = pc.actor;
        self.attempts.push((a.index(), self.now));
        self.attempt_ice.push((a.index(), self.now, ice_servers));
        if self.actors[a].has_open_pc() {
            self.violations.push(format!(
                "{}: create_attempt {} while another attempt is live",
                self.actors[a].name,
                pc.handle.raw()
            ));
        }
        self.new_pc(a, pc.handle);
    }

    fn schedule_local_desc(&mut self, pc: PcRef, kind: SdpKind) {
        let d = self.rng.range(5, 30);
        self.push(self.now + d, 2, PeerEv::LocalDesc { pc, kind });
    }

    /// The fake platform of talk-back: a healthy one. The chime takes 600 ms, the microphone
    /// attaches 30 ms after it was asked to; what the baby's microphone hears (`levels`,
    /// `talk_levels` verbs) is up to the scenario.
    fn exec_talk(&mut self, a: ActorIx, act: LinkAction) {
        match act {
            LinkAction::PlayTalkChime { attempt, .. } => {
                self.push_input(a, 600, LinkInput::TalkChimeDone { attempt });
            }
            LinkAction::SetTalkMic { enabled: true, .. } => {
                self.push_input(a, 30, LinkInput::TalkMicAttached);
            }
            _ => {}
        }
    }

    /// Feeds `input` to actor `a` after `delay` ms.
    pub(super) fn push_input(&mut self, a: ActorIx, delay: u64, input: LinkInput) {
        self.push(self.now + delay, 2, ActorEv::Input { actor: a, input });
    }

    pub(super) fn record_event(&mut self, a: ActorIx, event: LinkEvent) {
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

    pub(crate) fn schedule_raw_tick(&mut self, actor: impl Into<ActorIx>, delay: u64) {
        let actor = actor.into();
        self.push(self.now + delay, 3, ActorEv::RawTick { actor });
    }

    pub(crate) fn schedule_raw_act(&mut self, actor: impl Into<ActorIx>, delay: u64) {
        let actor = actor.into();
        self.push(self.now + delay, 3, ActorEv::RawAct { actor });
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
}
