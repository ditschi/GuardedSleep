//! The log canary of a run (AGENTS.md section 7.6).

use super::*;

impl World {
    /// Turns the log canary on (AGENTS.md section 7.6): from now on everything a platform could
    /// log is scanned for the PSKs of the run (known), the seeds, the keys derived from them, the
    /// room ids, plaintext SDP and, once the first pairing has drawn it, the parent secret.
    ///
    /// "Log" here is the `{:?}` text of every input the core receives, of every action and event
    /// it returns and of every config: the worst a careless driver could print. Wire payloads (a
    /// mailbox frame, an SDP in transit) may carry the room id and SDP by design, so for those
    /// lines only the secrets are checked. Call before [`World::run`].
    pub fn enable_log_canary(&mut self) {
        let mut set = CanarySet::new();
        set.add_pairing("", &synthetic_psk("A"));
        set.add_pairing("_other", &synthetic_psk("B"));
        for ac in &self.actors {
            // The two synthetic PSKs are in already; a scenario may name another one.
            if let Some(psk) = decode_psk(&ac.cfg.psk_b64url)
                && psk != synthetic_psk("A")
                && psk != synthetic_psk("B")
            {
                set.add_pairing(&format!("_{}", ac.name), &psk);
            }
            set.add_seed(&format!("seed_{}_run0", ac.name), &ac.cfg.seed);
        }
        set.add_sdp_markers();
        self.canary = Some(CanaryRun {
            set,
            reported: BTreeSet::new(),
            parent_secret: None,
            lines: 0,
        });
        let configs: Vec<(usize, String)> = self
            .actors
            .iter()
            .enumerate()
            .map(|(i, ac)| (i, format!("{:?}", ac.cfg)))
            .collect();
        for (i, text) in configs {
            self.log_line(i, "config", &text, false);
        }
    }

    /// The canary list of the run (names only matter to callers and tests).
    pub fn canary_set(&self) -> Option<&CanarySet> {
        self.canary.as_ref().map(|c| &c.set)
    }

    /// The parent secret of the first pairing, once the run has drawn it.
    pub fn canary_parent_secret(&self) -> Option<&[u8]> {
        self.canary
            .as_ref()
            .and_then(|c| c.parent_secret.as_deref())
    }

    /// How many log lines the canary has scanned.
    pub fn canary_lines(&self) -> u64 {
        self.canary.as_ref().map_or(0, |c| c.lines)
    }

    /// One log line of actor `a`; a leak becomes a violation (the canary name and encoding, never
    /// the value). Public so a test can plant a leak through the very same path.
    pub fn log_line(&mut self, a: usize, kind: &str, text: &str, wire: bool) {
        let name = self
            .actors
            .get(a)
            .map_or("?", |ac| ac.name.as_str())
            .to_owned();
        let now = self.now;
        let Some(c) = self.canary.as_mut() else {
            return;
        };
        c.lines += 1;
        let place = format!("sim log of {name} at t={now} ({kind})");
        let hits = c.set.scan(&place, text.as_bytes(), wire);
        for h in hits {
            let key = (h.canary.clone(), h.encoding.clone(), kind.to_owned());
            if c.reported.insert(key) {
                self.violations.push(format!(
                    "log canary: {} ({}) in {place}",
                    h.canary, h.encoding
                ));
            }
        }
    }

    pub(super) fn log_actions(&mut self, a: usize, actions: &[LinkAction]) {
        // Learn the run-time values first: the parent secret comes out in `ParentPinned`, before
        // the action that sends it, in the very same list.
        if let Some(c) = self.canary.as_mut() {
            for act in actions {
                match act {
                    LinkAction::Emit {
                        event: LinkEvent::ParentPinned { secret, .. },
                    } if c.parent_secret.is_none() => {
                        c.parent_secret = Some(secret.bytes.clone());
                        c.set.add_parent_secret(&secret.bytes);
                    }
                    LinkAction::SendData { text, .. } if carries_parent_pin(text) => {
                        c.set.add_pin_message(text);
                    }
                    LinkAction::CreateAttempt { ice_servers, .. } => {
                        for cred in ice_servers.iter().filter_map(|s| s.credential.as_deref()) {
                            c.set.add_turn_password(cred);
                        }
                    }
                    _ => {}
                }
            }
        }
        for act in actions {
            let wire = matches!(
                act,
                LinkAction::MailboxSend { .. }
                    | LinkAction::SendData { .. }
                    | LinkAction::SetRemoteDescription { .. }
                    | LinkAction::AddIceCandidate { .. }
            );
            self.log_line(a, "action", &format!("{act:?}"), wire);
        }
    }
}
