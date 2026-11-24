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
        let targets: Vec<ActorIx> = match &st.peer {
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

    /// A wall-clock jump of `ms` (the device slept): queued events move along, the health clocks
    /// of connections too, and the time stands `ms` later.
    fn time_jump(&mut self, st: &Step) {
        let ms = st.ms.unwrap_or(0);
        self.shift_queue(ms);
        self.shift_clocks(ms);
        self.now += ms;
        if st.peer_traffic.as_deref() == Some("kept") {
            self.wake_heartbeats();
        }
    }

    fn shift_queue(&mut self, ms: u64) {
        for ((t, p, s), ev) in std::mem::take(&mut self.q) {
            let (delay, ev) = ev.shifted(ms);
            self.q.insert((t + delay, p, s), ev);
        }
    }

    fn shift_clocks(&mut self, ms: u64) {
        for v in self.fifo.values_mut() {
            *v += ms;
        }
        let pcs = self.actors.iter_mut().flat_map(|a| a.pcs.values_mut());
        for since in pcs.filter_map(|pc| pc.unhealthy_since.as_mut()) {
            *since += ms;
        }
    }

    /// At wake-up one heartbeat reaches every link that is up (`peer_traffic: kept`).
    fn wake_heartbeats(&mut self) {
        let linked = |a: &Actor| a.link.is_some();
        let live: Vec<PcRef> = self
            .up_pcs()
            .into_iter()
            .filter(|pc| linked(&self.actors[pc.actor]))
            .collect();
        for to in live {
            let text = json!({"type":"heartbeat","v":3,"t":0,"seq":1_000_000}).to_string();
            self.push(self.now, 0, PeerEv::Data { to, text });
        }
    }
}
