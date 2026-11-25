//! Timeline verbs: one method per verb, found through [`VERBS`]; plus fault planning, time jumps
//! and the hostile-relay steps.

use super::*;

type VerbFn = fn(&mut World, &Step);

/// The verbs of a `gs-scenario/1` timeline. A table instead of a `match`: a new verb is one row and
/// one method.
const VERBS: &[(&str, VerbFn)] = &[
    ("end", |_, _| {}),
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
}
