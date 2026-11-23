//! Simulated peer connections and the DataChannel.

use super::*;

impl World {
    pub(super) fn dispatch_peer(&mut self, ev: PeerEv) {
        match ev {
            PeerEv::LocalDesc {
                actor,
                handle,
                sdp_type,
            } => self.on_local_desc(actor, handle, sdp_type),
            PeerEv::PcConnect {
                a,
                ha,
                b,
                hb,
                since,
            } => self.on_pc_connect(a, ha, b, hb, since),
            PeerEv::Data { to, handle, text } => self.on_data(to, handle, text),
            PeerEv::DataClosed { to, handle } => self.on_data_closed(to, handle),
            PeerEv::Watch => {
                self.on_watch();
                self.push(self.now + 1_000, 4, PeerEv::Watch);
            }
        }
    }

    /// Messages already in flight arrive even if the sender closed meanwhile.
    fn on_data(&mut self, to: usize, handle: u32, text: String) {
        let rx_ok = self.actors[to]
            .pcs
            .get(&handle)
            .is_some_and(|p| p.connected && !p.closed && !p.failed && self.reach(to, p.epoch));
        if !rx_ok {
            return;
        }
        if self.actors[to].raw.is_some() {
            self.raw_on_data(to, handle, &text);
            return;
        }
        let text = self.mitm_adjust(to, handle, text);
        self.actors[to].last_rx_data = self.now;
        let input = LinkInput::AttemptDataMessage {
            attempt: handle,
            text,
        };
        self.deliver(to, input);
    }

    fn on_data_closed(&mut self, to: usize, handle: u32) {
        let open = self.actors[to]
            .pcs
            .get(&handle)
            .is_some_and(|p| !p.closed && !p.failed);
        if open && self.actors[to].raw.is_none() {
            self.deliver(to, LinkInput::AttemptDataClosed { attempt: handle });
        }
    }

    pub fn new_pc(&mut self, a: usize, handle: u32) {
        self.fp_seq += 1;
        let fp = synthetic_fp(self.fp_seq);
        let fail_after = self.rng.range(FAIL_AFTER_MIN, FAIL_AFTER_MAX);
        let epoch = self.actors[a].path_epoch;
        self.actors[a].pcs.insert(
            handle,
            Pc {
                fp,
                dtls_fp: None,
                local: false,
                remote: false,
                peer: None,
                closed: false,
                connected: false,
                failed: false,
                connecting: false,
                epoch,
                unhealthy_since: None,
                fail_after,
            },
        );
    }

    pub fn sdp_for(&self, a: usize, fp: &str) -> String {
        let ac = &self.actors[a];
        let (monitor, talk) = (ac.sdp_dir.as_str(), ac.talk_dir.as_str());
        // The fingerprint line stays last: the core's extractor reads hex until the first
        // non-hex character.
        format!(
            "v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na={monitor}\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na={talk}\r\na=fingerprint:sha-256 {}\r\n",
            colon_upper(fp)
        )
    }

    pub(super) fn on_local_desc(&mut self, a: usize, handle: u32, sdp_type: &'static str) {
        let Some(pc) = self.actors[a].pcs.get_mut(&handle) else {
            return;
        };
        if pc.closed {
            return;
        }
        pc.local = true;
        let fp = pc.fp.clone();
        let sdp = self.sdp_for(a, &fp);
        self.deliver(
            a,
            LinkInput::AttemptLocalDescription {
                attempt: handle,
                sdp_type: sdp_type.into(),
                sdp,
            },
        );
        self.maybe_connect(a, handle);
    }

    pub fn set_remote(&mut self, a: usize, handle: u32, sdp: &str) {
        // Routing follows the lenient v2 reading: a relay that hides the real fingerprint in `s=`
        // still forwards the media path to the real peer, while DTLS itself is authenticated
        // against the real line (`dtls_fp`).
        let fp = extract_sha256_fingerprint_v2_legacy(sdp).unwrap_or_default();
        let mut peer = None;
        for (m, other) in self.actors.iter().enumerate() {
            if m == a {
                continue;
            }
            for (h, pc) in &other.pcs {
                if pc.fp == fp && !pc.closed {
                    peer = Some((m, *h));
                }
            }
        }
        if let Some(pc) = self.actors[a].pcs.get_mut(&handle) {
            pc.remote = true;
            pc.peer = peer;
            pc.dtls_fp = extract_sha256_fingerprint(sdp);
        }
        self.maybe_connect(a, handle);
    }

    pub fn maybe_connect(&mut self, a: usize, h: u32) {
        let ready =
            |p: &Pc| p.local && p.remote && !p.closed && !p.failed && !p.connecting && !p.connected;
        let Some(pc) = self.actors[a].pcs.get(&h) else {
            return;
        };
        if !ready(pc) {
            return;
        }
        let Some((b, hb)) = pc.peer else {
            return;
        };
        let Some(other) = self.actors[b].pcs.get(&hb) else {
            return;
        };
        if !ready(other) || other.peer != Some((a, h)) {
            return;
        }
        self.actors[a].pcs.get_mut(&h).expect("pc").connecting = true;
        self.actors[b].pcs.get_mut(&hb).expect("pc").connecting = true;
        let d = self.rng.range(250, 1_200);
        let since = self.now;
        self.push(
            self.now + d,
            2,
            PeerEv::PcConnect {
                a,
                ha: h,
                b,
                hb,
                since,
            },
        );
    }

    pub(super) fn pair_reachable(&self, a: usize, h: u32, pa: &Pc) -> bool {
        let Some((b, hb)) = pa.peer else {
            return false;
        };
        let Some(pb) = self.actors[b].pcs.get(&hb) else {
            return false;
        };
        !pb.closed
            && !pb.failed
            && self.reach(a, pa.epoch)
            && self.reach(b, pb.epoch)
            && pb.peer == Some((a, h))
    }

    /// The pc exists, is open and both ends can reach each other right now.
    pub fn pc_ok(&self, a: usize, h: u32) -> bool {
        let Some(pc) = self.actors[a].pcs.get(&h) else {
            return false;
        };
        pc.connected && !pc.closed && !pc.failed && self.pair_reachable(a, h, pc)
    }

    pub(super) fn on_pc_connect(&mut self, a: usize, ha: u32, b: usize, hb: u32, since: Time) {
        let alive = |w: &World, x: usize, h: u32| {
            w.actors[x]
                .pcs
                .get(&h)
                .is_some_and(|p| !p.closed && !p.failed)
        };
        if !alive(self, a, ha) || !alive(self, b, hb) {
            return;
        }
        let reachable = {
            let pa = &self.actors[a].pcs[&ha];
            let pb = &self.actors[b].pcs[&hb];
            self.reach(a, pa.epoch) && self.reach(b, pb.epoch)
        };
        if reachable {
            for (x, hx) in [(a, ha), (b, hb)] {
                self.actors[x].pcs.get_mut(&hx).expect("pc").connected = true;
            }
            for (x, hx) in [(a, ha), (b, hb)] {
                if self.actors[x].raw.is_some() {
                    self.raw_on_open(x, hx);
                } else {
                    self.deliver(
                        x,
                        LinkInput::AttemptPcState {
                            attempt: hx,
                            state: "connected".into(),
                        },
                    );
                    self.deliver(x, LinkInput::AttemptDataOpen { attempt: hx });
                    if let Some(fingerprint) = self.actors[x].pcs[&hx].dtls_fp.clone() {
                        self.deliver(
                            x,
                            LinkInput::AttemptDtlsFingerprint {
                                attempt: hx,
                                fingerprint,
                            },
                        );
                    }
                }
            }
        } else if self.now.saturating_sub(since) < CONNECT_GIVE_UP_MS {
            self.push(
                self.now + 500,
                2,
                PeerEv::PcConnect {
                    a,
                    ha,
                    b,
                    hb,
                    since,
                },
            );
        } else {
            for (x, hx) in [(a, ha), (b, hb)] {
                self.fail_pc(x, hx);
            }
        }
    }

    pub(super) fn fail_pc(&mut self, a: usize, h: u32) {
        let Some(pc) = self.actors[a].pcs.get_mut(&h) else {
            return;
        };
        if pc.failed || pc.closed {
            return;
        }
        pc.failed = true;
        if self.actors[a].raw.is_none() {
            self.deliver(
                a,
                LinkInput::AttemptPcState {
                    attempt: h,
                    state: "failed".into(),
                },
            );
        }
    }

    /// ICE consent: a connected pair that stays unreachable for ~30 s becomes `failed`.
    pub(super) fn on_watch(&mut self) {
        let now = self.now;
        let mut to_fail = Vec::new();
        for a in 0..self.actors.len() {
            let handles: Vec<u32> = self.actors[a]
                .pcs
                .iter()
                .filter(|(_, p)| p.connected && !p.closed && !p.failed)
                .map(|(h, _)| *h)
                .collect();
            for h in handles {
                let ok = self.pc_ok(a, h);
                let pc = self.actors[a].pcs.get_mut(&h).expect("pc");
                if ok {
                    pc.unhealthy_since = None;
                } else {
                    let since = *pc.unhealthy_since.get_or_insert(now);
                    if now.saturating_sub(since) >= pc.fail_after {
                        to_fail.push((a, h));
                    }
                }
            }
        }
        for (a, h) in to_fail {
            self.fail_pc(a, h);
        }
    }

    pub(super) fn close_attempt(&mut self, a: usize, h: u32) {
        let Some(pc) = self.actors[a].pcs.get(&h) else {
            return;
        };
        if pc.closed {
            return;
        }
        let notify = pc.connected && self.pair_reachable(a, h, pc);
        let peer = pc.peer;
        self.actors[a].pcs.get_mut(&h).expect("pc").closed = true;
        if let (true, Some((b, hb))) = (notify, peer) {
            let lat = self.rng.range(10, 60);
            let t = self.fifo_time(2, b, hb, self.now + lat, false);
            self.push(t, 2, PeerEv::DataClosed { to: b, handle: hb });
        }
    }

    pub fn send_data(&mut self, a: usize, h: u32, text: String) {
        if !self.pc_ok(a, h) {
            return;
        }
        let Some((b, hb)) = self.actors[a].pcs[&h].peer else {
            return;
        };
        for (extra, bypass) in self.plan(a, Via::Data, &text) {
            let lat = self.rng.range(10, 60) + extra;
            let t = self.fifo_time(2, b, hb, self.now + lat, bypass);
            self.push(
                t,
                2,
                PeerEv::Data {
                    to: b,
                    handle: hb,
                    text: text.clone(),
                },
            );
        }
    }

    pub(crate) fn pc_close(&mut self, a: usize, h: u32) {
        self.close_attempt(a, h);
    }
}
