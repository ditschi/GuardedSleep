//! Session descriptions of the simulated peer connections.

use super::*;

impl World {
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

    pub(super) fn on_local_desc(&mut self, me: PcRef, kind: SdpKind) {
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
            sdp_type: kind.as_str().into(),
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
}
