//! A wall-clock jump of the device (suspend, doze): everything queued moves along.

use super::*;

impl World {
    /// A wall-clock jump of `ms` (the device slept): queued events move along, the health clocks
    /// of connections too, and the time stands `ms` later.
    pub(super) fn time_jump(&mut self, st: &Step) {
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
