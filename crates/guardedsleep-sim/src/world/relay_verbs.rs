//! Timeline verbs that act on the simulated relays.

use super::*;

impl World {
    /// Runs `apply` once per relay the step addresses, with that relay selected.
    fn on_targeted_servers(&mut self, st: &Step, mut apply: impl FnMut(&mut World)) {
        for srv in self.targeted_servers(st) {
            self.select_server(srv);
            apply(self);
        }
    }

    pub(super) fn verb_signaling_down(&mut self, st: &Step) {
        self.on_targeted_servers(st, World::signaling_down);
    }

    pub(super) fn verb_signaling_up(&mut self, st: &Step) {
        self.on_targeted_servers(st, |w| {
            w.server.up = true;
            w.server.sweep_gen += 1;
            let (r#gen, srv) = (w.server.sweep_gen, w.cur_server);
            w.push(w.now + SWEEP_MS, 3, RelayEv::Sweep { r#gen, srv });
        });
    }

    /// A relay that stays up, answers pings and takes frames, but delivers no `signal`.
    pub(super) fn verb_relay_drop_signals(&mut self, st: &Step) {
        let on = st.mode.as_deref() != Some("off");
        self.on_targeted_servers(st, |w| w.server.drop_signals = on);
    }

    pub(super) fn verb_reject_hello(&mut self, st: &Step) {
        let for_ms = st.for_ms.unwrap_or(u64::MAX - self.now);
        let until = self.now.saturating_add(for_ms);
        self.on_targeted_servers(st, |w| w.server.reject_hello_until = until);
    }
}
