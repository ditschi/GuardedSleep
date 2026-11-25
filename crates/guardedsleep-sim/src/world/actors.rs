//! Actors: process lifecycle, pacing and the simulated wall clock.

use super::*;

impl World {
    pub(super) fn dispatch_actor(&mut self, ev: ActorEv) {
        match ev {
            ActorEv::Tick { actor, chain } => self.on_tick(actor, chain),
            ActorEv::Input { actor, input } => self.deliver(actor, input),
            ActorEv::PathBack { actor } => {
                self.actors[actor].net_up = true;
                self.flush_after_up(actor, true);
            }
            ActorEv::Restart { actor } => self.restart_actor(actor),
            ActorEv::RawTick { actor } => self.raw_tick(actor.index()),
            ActorEv::RawAct { actor } => self.raw_act(actor.index()),
        }
    }

    pub(super) fn start_actor(&mut self, a: ActorIx) {
        let entropy = self.rng.next_u64() & 0xFFFF_FFFF;
        self.actors[a].chain += 1;
        let chain = self.actors[a].chain;
        self.actors[a].last_wall_ms = None;
        self.deliver(a, LinkInput::Start { entropy });
        self.report_wall_clock(a);
        self.push(self.now + 250, 3, ActorEv::Tick { actor: a, chain });
    }

    /// Reports the simulated wall clock at start and then every 60 s of virtual time, like a driver
    /// does. Synthetic (no real date) and not an "input" for the tick pacing.
    pub(super) fn report_wall_clock(&mut self, a: ActorIx) {
        let due = self.actors[a]
            .last_wall_ms
            .is_none_or(|t| self.now.saturating_sub(t) >= WALL_CLOCK_EVERY_MS);
        if !due {
            return;
        }
        self.actors[a].last_wall_ms = Some(self.now);
        let true_s = i64::try_from(self.wall_base + self.now / 1_000).unwrap_or(i64::MAX);
        let unix_s = u64::try_from(true_s + self.actors[a].wall_offset_s).unwrap_or(0);
        self.deliver(a, LinkInput::WallClock { unix_s });
    }

    /// A tick of an earlier run of the process (or of a dead one) does nothing.
    fn tick_is_stale(&self, a: ActorIx, chain: u32) -> bool {
        let ac = &self.actors[a];
        ac.chain != chain || !ac.alive || ac.link.is_none()
    }

    /// The core's own state, once it has stopped for good no tick is due any more.
    fn link_has_ended(&self, a: ActorIx) -> bool {
        let state = self.link_state(a);
        matches!(state, Some(LinkState::Stopped | LinkState::FailedNeedsUser))
    }

    fn link_state(&self, a: ActorIx) -> Option<LinkState> {
        let link = self.actors[a].link.as_ref()?;
        link.status().map(|s| s.state)
    }

    pub(super) fn on_tick(&mut self, a: ActorIx, chain: u32) {
        if self.tick_is_stale(a, chain) {
            return;
        }
        self.deliver(a, LinkInput::Tick);
        // After the tick: the first input of a moment takes the timers that are due.
        self.report_wall_clock(a);
        if self.link_has_ended(a) {
            return;
        }
        // Tick often while something happened recently, otherwise at the 500 ms contract rate.
        let recent = self.now.saturating_sub(self.actors[a].last_input_at) < 2_000;
        let period = if recent { 250 } else { 500 };
        self.push(self.now + period, 3, ActorEv::Tick { actor: a, chain });
    }

    pub(super) fn finish_rec(&mut self, a: ActorIx) {
        if let Some(r) = self.actors[a].rec.take() {
            self.recs[r].final_state = self.link_state(a);
        }
    }

    /// Process death: sockets and peer connections vanish, nobody is told.
    pub(super) fn kill_actor(&mut self, a: ActorIx) {
        self.finish_rec(a);
        let reachable = self.actors[a].net_up;
        let socks = std::mem::take(&mut self.actors[a].socks);
        for s in socks.into_values().filter(|_| reachable) {
            self.queue_server_close(SockIx::from(s));
        }
        let ac = &mut self.actors[a];
        ac.alive = false;
        ac.link = None;
        ac.chain += 1;
        ac.pcs.clear();
        ac.held_up.clear();
        ac.held_down.clear();
    }

    pub(super) fn restart_actor(&mut self, a: ActorIx) {
        if self.actors[a].raw.is_some() {
            return;
        }
        let mut cfg = self.actors[a].cfg.clone();
        cfg.paired_before = true;
        // A restart is a new run: a new seed, as the platforms draw one per run.
        let run = self.actors[a].instance + 1;
        cfg.seed = link_seed(self.seed, &self.actors[a].name, run);
        self.note_run_seed(a, run, &cfg.seed);
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

    /// The canary learns the seed of the new run.
    fn note_run_seed(&mut self, a: ActorIx, run: u32, seed: &[u8]) {
        let name = format!("seed_{}_run{run}", self.actors[a].name);
        if let Some(c) = self.canary.as_mut() {
            c.set.add_seed(&name, seed);
        }
    }
}
