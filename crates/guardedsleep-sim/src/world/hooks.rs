//! What `raw.rs` and the tests use of the world, and small lookups on an actor's key material.

use super::*;

impl World {
    pub(crate) fn schedule_raw_tick(&mut self, actor: impl Into<ActorIx>, delay: u64) {
        let actor = actor.into();
        self.push(self.now + delay, 3, ActorEv::RawTick { actor });
    }

    pub(crate) fn schedule_raw_act(&mut self, actor: impl Into<ActorIx>, delay: u64) {
        let actor = actor.into();
        self.push(self.now + delay, 3, ActorEv::RawAct { actor });
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

    pub(super) fn actor_psk(&self, a: ActorIx) -> Option<Vec<u8>> {
        decode_psk(&self.actors[a].cfg.psk_b64url)
    }

    /// What a frame `a` sent is, for the "sent_signal" assertions: the frame type, or for a relay
    /// `signal` the type of the message inside (read with the sender's PSK when it is sealed).
    pub(super) fn signal_type_for(&self, a: ActorIx, text: &str) -> Option<String> {
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

    pub(crate) fn pc_close(&mut self, a: usize, h: u32) {
        self.close_attempt(ActorIx::from(a), PcHandle::from(h));
    }
}
