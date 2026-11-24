//! Fault injection: the faults a scenario switches on, and how they shape one message.

use super::*;

/// What the active faults add up to for one message.
struct Effect {
    extra: u64,
    copies: usize,
    bypass: bool,
}

impl World {
    pub(super) fn add_fault(&mut self, st: &Step) {
        let from = st
            .from
            .as_deref()
            .and_then(|n| if n == "all" { None } else { self.actor_ix(n) });
        let kind = match st.verb.as_str() {
            "drop_data" => FaultKind::Drop,
            "delay" => FaultKind::Delay,
            "duplicate" => FaultKind::Duplicate,
            _ => FaultKind::Reorder,
        };
        let via = if st.via.as_deref() == Some("signaling") {
            Via::Signaling
        } else {
            Via::Data
        };
        self.faults.push(Fault {
            kind,
            from,
            via,
            contains: st.message.clone(),
            start: self.now,
            until: st.for_ms.map_or(u64::MAX, |d| self.now + d),
            p: st.p.unwrap_or(1.0),
            ms: st.ms.unwrap_or(0),
            jitter: st.jitter_ms.unwrap_or(0),
        });
    }

    /// The faults that apply to `text` sent by `from` over `via` right now.
    fn active_faults(&self, from: ActorIx, via: Via, text: &str) -> Vec<Fault> {
        let now = self.now;
        let applies = |f: &Fault| {
            let in_window = f.start <= now && now < f.until;
            let sender = f.from.is_none_or(|x| x == from.index());
            let matching = f.contains.as_deref().is_none_or(|c| text.contains(c));
            f.via == via && in_window && sender && matching
        };
        self.faults.iter().filter(|f| applies(f)).cloned().collect()
    }

    /// How a message sent by `from` over `via` travels: one entry per copy that arrives, none if
    /// it is dropped.
    pub(super) fn plan(&mut self, from: ActorIx, via: Via, text: &str) -> Vec<Delivery> {
        let mut effect = Effect {
            extra: 0,
            copies: 1,
            bypass: false,
        };
        for f in self.active_faults(from, via, text) {
            if !self.apply_fault(&f, &mut effect) {
                return Vec::new();
            }
        }
        (0..effect.copies)
            .map(|i| Delivery {
                extra: effect.extra + if i > 0 { self.rng.range(1, 30) } else { 0 },
                bypass: effect.bypass,
            })
            .collect()
    }

    /// Applies one fault to the effect; false when the message is dropped.
    fn apply_fault(&mut self, f: &Fault, effect: &mut Effect) -> bool {
        match f.kind {
            FaultKind::Drop => !self.rng.chance(f.p),
            FaultKind::Delay => {
                effect.extra += f.ms + self.rng.range(0, f.jitter);
                true
            }
            FaultKind::Duplicate => {
                if self.rng.chance(f.p) {
                    effect.copies = 2;
                }
                true
            }
            FaultKind::Reorder => {
                if self.rng.chance(f.p) {
                    effect.extra += self.rng.range(0, f.ms);
                    effect.bypass = true;
                }
                true
            }
        }
    }
}
