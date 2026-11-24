//! Simulated peer connections: creation, descriptions, pairing up and the DataChannel.

use super::*;

impl World {
    pub fn new_pc(&mut self, a: impl Into<ActorIx>, handle: impl Into<PcHandle>) {
        let (a, handle) = (a.into(), handle.into());
        self.fp_seq += 1;
        let fp = synthetic_fp(self.fp_seq);
        let fail_after = self.rng.range(FAIL_AFTER_MIN, FAIL_AFTER_MAX);
        let epoch = self.actors[a].path_epoch;
        self.actors[a]
            .pcs
            .insert(handle.raw(), Pc::fresh(fp, epoch, fail_after));
    }

    pub fn sdp_for(&self, a: impl Into<ActorIx>, fp: &str) -> String {
        let ac = &self.actors[a.into()];
        let (monitor, talk) = (ac.sdp_dir.as_str(), ac.talk_dir.as_str());
        // The fingerprint line stays last: the core's extractor reads hex until the first
        // non-hex character.
        format!(
            "v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na={monitor}\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na={talk}\r\na=fingerprint:sha-256 {}\r\n",
            colon_upper(fp)
        )
    }

    pub(super) fn on_local_desc(&mut self, me: PcRef, sdp_type: &'static str) {
        let Some(pc) = self.pc_mut(me) else {
            return;
        };
        if pc.closed {
            return;
        }
        pc.local = true;
        let fp = pc.fp.clone();
        let sdp = self.sdp_for(me.actor, &fp);
        let attempt = me.handle.raw();
        let input = LinkInput::AttemptLocalDescription {
            attempt,
            sdp_type: sdp_type.into(),
            sdp,
        };
        self.deliver(me.actor, input);
        self.maybe_connect(me.actor, me.handle);
    }

    pub fn set_remote(&mut self, a: impl Into<ActorIx>, handle: impl Into<PcHandle>, sdp: &str) {
        let (a, handle) = (a.into(), handle.into());
        // Routing follows the lenient v2 reading: a relay that hides the real fingerprint in `s=`
        // still forwards the media path to the real peer, while DTLS itself is authenticated
        // against the real line (`dtls_fp`).
        let fp = extract_sha256_fingerprint_v2_legacy(sdp).unwrap_or_default();
        let peer = self.find_pc_by_fp(a, &fp);
        if let Some(pc) = self.actors[a].pc_mut(handle) {
            pc.remote = true;
            pc.peer = peer.map(PcRef::raw);
            pc.dtls_fp = extract_sha256_fingerprint(sdp);
        }
        self.maybe_connect(a, handle);
    }

    /// The open connection of another actor that announced the fingerprint `fp` (the last one, if
    /// several did).
    fn find_pc_by_fp(&self, a: ActorIx, fp: &str) -> Option<PcRef> {
        self.actors
            .iter()
            .enumerate()
            .filter(|(m, _)| *m != a.index())
            .flat_map(|(m, other)| {
                other
                    .pcs
                    .iter()
                    .filter(|(_, pc)| pc.fp == fp && !pc.closed)
                    .map(move |(h, _)| PcRef::from((m, *h)))
            })
            .last()
    }

    pub fn maybe_connect(&mut self, a: impl Into<ActorIx>, h: impl Into<PcHandle>) {
        let me = PcRef::new(a.into(), h.into());
        let Some(ends) = self.connectable_pair(me) else {
            return;
        };
        for end in ends {
            if let Some(pc) = self.pc_mut(end) {
                pc.connecting = true;
            }
        }
        let d = self.rng.range(250, 1_200);
        let since = self.now;
        self.push(self.now + d, 2, PeerEv::PcConnect { ends, since });
    }

    /// `me` and its peer, when both are ready and point at each other.
    fn connectable_pair(&self, me: PcRef) -> Option<[PcRef; 2]> {
        let pc = self.pc(me)?;
        if !pc.ready_to_connect() {
            return None;
        }
        let other = pc.peer_ref()?;
        let other_pc = self.pc(other)?;
        let mutual = other_pc.peer_ref() == Some(me);
        (other_pc.ready_to_connect() && mutual).then_some([me, other])
    }

    /// Whether the peer of `pa` (the state of `me`) is open and both ends reach each other now.
    fn pair_reachable(&self, me: PcRef, pa: &Pc) -> bool {
        let Some(other) = pa.peer_ref() else {
            return false;
        };
        let Some(pb) = self.pc(other) else {
            return false;
        };
        pb.is_live()
            && self.reach(me.actor, pa.epoch)
            && self.reach(other.actor, pb.epoch)
            && pb.peer_ref() == Some(me)
    }

    /// The pc exists, is open and both ends can reach each other right now.
    pub fn pc_ok(&self, a: impl Into<ActorIx>, h: impl Into<PcHandle>) -> bool {
        let me = PcRef::new(a.into(), h.into());
        self.pc(me)
            .is_some_and(|pc| pc.is_up() && self.pair_reachable(me, pc))
    }

    pub(super) fn fail_pc(&mut self, me: PcRef) {
        let Some(pc) = self.pc_mut(me) else {
            return;
        };
        if !pc.is_live() {
            return;
        }
        pc.failed = true;
        if self.actors[me.actor].raw.is_none() {
            let input = LinkInput::AttemptPcState {
                attempt: me.handle.raw(),
                state: "failed".into(),
            };
            self.deliver(me.actor, input);
        }
    }

    pub(super) fn close_attempt(&mut self, a: ActorIx, h: PcHandle) {
        let me = PcRef::new(a, h);
        let Some(pc) = self.pc(me) else {
            return;
        };
        if pc.closed {
            return;
        }
        let notify = pc.connected && self.pair_reachable(me, pc);
        let peer = pc.peer_ref();
        if let Some(pc) = self.pc_mut(me) {
            pc.closed = true;
        }
        if let (true, Some(to)) = (notify, peer) {
            let lat = self.rng.range(10, 60);
            self.push_flow(Flow::Data(to), lat, PeerEv::DataClosed { to });
        }
    }

    pub fn send_data(&mut self, a: impl Into<ActorIx>, h: impl Into<PcHandle>, text: String) {
        let me = PcRef::new(a.into(), h.into());
        if !self.pc_ok(me.actor, me.handle) {
            return;
        }
        let Some(to) = self.pc(me).and_then(Pc::peer_ref) else {
            return;
        };
        for delivery in self.plan(me.actor, Via::Data, &text) {
            let lat = self.rng.range(10, 60) + delivery.extra;
            let t = self.fifo_time(Flow::Data(to), self.now + lat, delivery.bypass);
            self.push(
                t,
                2,
                PeerEv::Data {
                    to,
                    text: text.clone(),
                },
            );
        }
    }
}
