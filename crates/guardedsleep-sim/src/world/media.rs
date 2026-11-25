//! The simulated media layer in motion: connecting pairs, the DataChannel and the consent watch.

use super::*;

impl World {
    pub(super) fn dispatch_peer(&mut self, ev: PeerEv) {
        match ev {
            PeerEv::LocalDesc { pc, kind } => self.on_local_desc(pc, kind),
            PeerEv::PcConnect { ends, since } => self.on_pc_connect(ends, since),
            PeerEv::Data { to, text } => self.on_data(to, text),
            PeerEv::DataClosed { to } => self.on_data_closed(to),
            PeerEv::Watch => {
                self.on_watch();
                self.push(self.now + 1_000, 4, PeerEv::Watch);
            }
        }
    }

    /// Messages already in flight arrive even if the sender closed meanwhile.
    fn on_data(&mut self, to: PcRef, text: String) {
        let a = to.actor;
        let rx_ok = self
            .pc(to)
            .is_some_and(|p| p.is_up() && self.reach(a, p.epoch));
        if !rx_ok {
            return;
        }
        if self.actors[a].raw.is_some() {
            self.raw_on_data(a.index(), to.handle.raw(), &text);
            return;
        }
        let text = self.mitm_adjust(to, text);
        self.actors[a].last_rx_data = self.now;
        let attempt = to.handle.raw();
        self.deliver(a, LinkInput::AttemptDataMessage { attempt, text });
    }

    fn on_data_closed(&mut self, to: PcRef) {
        let open = self.pc(to).is_some_and(Pc::is_live);
        if open && self.actors[to.actor].raw.is_none() {
            let attempt = to.handle.raw();
            self.deliver(to.actor, LinkInput::AttemptDataClosed { attempt });
        }
    }

    fn pc_is_live(&self, end: PcRef) -> bool {
        self.pc(end).is_some_and(Pc::is_live)
    }

    /// Both ends of a pair are up for the first time, or keep trying, or give up.
    pub(super) fn on_pc_connect(&mut self, ends: [PcRef; 2], since: Time) {
        if !ends.iter().all(|end| self.pc_is_live(*end)) {
            return;
        }
        if self.ends_reach_each_other(ends) {
            self.open_pair(ends);
        } else if self.now.saturating_sub(since) < CONNECT_GIVE_UP_MS {
            self.push(self.now + 500, 2, PeerEv::PcConnect { ends, since });
        } else {
            for end in ends {
                self.fail_pc(end);
            }
        }
    }

    fn ends_reach_each_other(&self, ends: [PcRef; 2]) -> bool {
        ends.iter().all(|end| {
            self.pc(*end)
                .is_some_and(|pc| self.reach(end.actor, pc.epoch))
        })
    }

    fn open_pair(&mut self, ends: [PcRef; 2]) {
        for end in ends {
            if let Some(pc) = self.pc_mut(end) {
                pc.connected = true;
            }
        }
        for end in ends {
            self.announce_open(end);
        }
    }

    /// Tells one end that its connection is up, its DataChannel open and what DTLS authenticated.
    fn announce_open(&mut self, end: PcRef) {
        let (a, attempt) = (end.actor, end.handle.raw());
        if self.actors[a].raw.is_some() {
            self.raw_on_open(a.index(), attempt);
            return;
        }
        let state = "connected".into();
        self.deliver(a, LinkInput::AttemptPcState { attempt, state });
        self.deliver(a, LinkInput::AttemptDataOpen { attempt });
        if let Some(fingerprint) = self.pc(end).and_then(|pc| pc.dtls_fp.clone()) {
            let input = LinkInput::AttemptDtlsFingerprint {
                attempt,
                fingerprint,
            };
            self.deliver(a, input);
        }
    }

    /// The connections that are up, per actor in handle order.
    pub(super) fn up_pcs(&self) -> Vec<PcRef> {
        self.actors
            .iter()
            .enumerate()
            .flat_map(|(a, ac)| {
                ac.pcs
                    .iter()
                    .filter(|(_, pc)| pc.is_up())
                    .map(move |(h, _)| PcRef::from((a, *h)))
            })
            .collect()
    }

    /// ICE consent: a connected pair that stays unreachable for ~30 s becomes `failed`.
    pub(super) fn on_watch(&mut self) {
        let expired: Vec<PcRef> = self
            .up_pcs()
            .into_iter()
            .filter(|pc| self.watch_expired(*pc))
            .collect();
        for pc in expired {
            self.fail_pc(pc);
        }
    }

    /// Updates the health clock of one connection; true once it was unhealthy for its limit.
    fn watch_expired(&mut self, me: PcRef) -> bool {
        let ok = self.pc_ok(me.actor, me.handle);
        let now = self.now;
        let Some(pc) = self.pc_mut(me) else {
            return false;
        };
        if ok {
            pc.unhealthy_since = None;
            return false;
        }
        let since = *pc.unhealthy_since.get_or_insert(now);
        now.saturating_sub(since) >= pc.fail_after
    }
}
