//! Counters per relay, percentiles and the go/no-go verdict of `alpha-connection-model` design
//! section 9: at least 2 of 3 relays without data loss and a setup p95 below 3 s.

use serde_json::{Value, json};

/// Setup p95 limit of the go/no-go gate.
pub const SETUP_P95_LIMIT_MS: u64 = 3_000;
/// Relays that must be free of loss (of the three tested).
pub const CLEAN_RELAYS_NEEDED: usize = 2;
/// A relay needs this many delivered-or-lost events before "no loss" means anything.
pub const MIN_ELIGIBLE_EVENTS: u64 = 5;
/// Recoveries needed before a setup percentile means anything.
pub const MIN_RECOVERIES: usize = 5;

/// Nearest-rank percentile (`p` in 0..=100) of `values`; `None` when empty.
pub fn percentile(values: &[u64], p: u32) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let rank = (u64::from(p) * sorted.len() as u64).div_ceil(100).max(1) as usize;
    sorted.get(rank - 1).copied()
}

/// Everything counted for one relay URL.
#[derive(Debug, Default, Clone)]
pub struct RelayStats {
    pub url: String,
    /// Sockets the core asked for.
    pub opens: u64,
    pub open_failures: u64,
    /// Failure classes of the opens (`dns`, `tcp`, `tls`, ...), by count.
    pub open_failure_classes: Vec<(String, u64)>,
    pub connect_ms: Vec<u64>,
    /// The core closed the socket (a verified link, parking, a re-subscription).
    pub core_closes: u64,
    /// The relay or the network ended an open socket.
    pub unexpected_closes: u64,
    pub events_sent: u64,
    pub ok_true: u64,
    pub refused: u64,
    pub refused_clock: u64,
    pub refused_park: u64,
    pub refused_down: u64,
    pub refused_other: u64,
    /// Distinct refusal texts (at most 5, cut to 100 characters).
    pub refusal_samples: Vec<String>,
    pub notices: u64,
    pub notice_refusals: u64,
    pub closed_frames: u64,
    pub closed_refusals: u64,
    pub auth_requests: u64,
    pub unparsed_frames: u64,
    pub received_events: u64,
    /// Events that came back to their own sender (tag-only filter).
    pub echoes: u64,
    /// Events whose peer had a subscribed socket on this relay when they were sent.
    pub eligible: u64,
    pub delivered: u64,
    /// Eligible, accepted (or unanswered) and never delivered although the peer's socket stayed up.
    pub lost: u64,
    /// Eligible events not counted either way because the peer's socket changed meanwhile.
    pub inconclusive: u64,
    /// Events without an `OK` within the delivery window.
    pub no_ok: u64,
    pub latency_ms: Vec<u64>,
}

impl RelayStats {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_owned(),
            ..Self::default()
        }
    }

    pub fn open_failure(&mut self, class: &str) {
        self.open_failures += 1;
        match self
            .open_failure_classes
            .iter_mut()
            .find(|(c, _)| c == class)
        {
            Some((_, n)) => *n += 1,
            None => self.open_failure_classes.push((class.to_owned(), 1)),
        }
    }

    pub fn refusal_sample(&mut self, message: &str) {
        let cut: String = message.chars().take(100).collect();
        if self.refusal_samples.len() < 5 && !self.refusal_samples.contains(&cut) {
            self.refusal_samples.push(cut);
        }
    }

    /// Events were lost, or the relay never forwarded a single one although the peer listened (a
    /// relay that answers `REQ` and drops every `EVENT`, for example for an unlisted kind: every
    /// such event ends up "inconclusive" or "lost" depending on timing, so the whole run is judged).
    pub fn lossy(&self) -> bool {
        self.lost > 0 || (self.eligible >= MIN_ELIGIBLE_EVENTS && self.delivered == 0)
    }

    /// No loss, and enough evidence for that.
    pub fn clean(&self) -> bool {
        !self.lossy() && self.eligible >= MIN_ELIGIBLE_EVENTS
    }

    pub fn to_json(&self) -> Value {
        json!({
            "url": self.url,
            "opens": self.opens,
            "open_failures": self.open_failures,
            "open_failure_classes": self.open_failure_classes.iter()
                .map(|(c, n)| json!({"class": c, "count": n})).collect::<Vec<_>>(),
            "connect_ms": {
                "n": self.connect_ms.len(),
                "p50": percentile(&self.connect_ms, 50),
                "p95": percentile(&self.connect_ms, 95),
            },
            "core_closes": self.core_closes,
            "unexpected_closes": self.unexpected_closes,
            "events_sent": self.events_sent,
            "ok_true": self.ok_true,
            "refused": self.refused,
            "refused_created_at": self.refused_clock,
            "refused_park": self.refused_park,
            "refused_error": self.refused_down,
            "refused_other": self.refused_other,
            "refusal_samples": self.refusal_samples,
            "notices": self.notices,
            "notice_refusals": self.notice_refusals,
            "closed_frames": self.closed_frames,
            "closed_refusals": self.closed_refusals,
            "auth_requests": self.auth_requests,
            "unparsed_frames": self.unparsed_frames,
            "received_events": self.received_events,
            "echoes": self.echoes,
            "eligible": self.eligible,
            "delivered": self.delivered,
            "lost": self.lost,
            "never_delivered": self.eligible >= MIN_ELIGIBLE_EVENTS && self.delivered == 0,
            "inconclusive": self.inconclusive,
            "no_ok": self.no_ok,
            "delivery_ms": {
                "n": self.latency_ms.len(),
                "p50": percentile(&self.latency_ms, 50),
                "p95": percentile(&self.latency_ms, 95),
                "max": self.latency_ms.iter().max(),
            },
            "clean": self.clean(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Go,
    NoGo,
    Inconclusive,
}

impl Outcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Outcome::Go => "GO",
            Outcome::NoGo => "NO-GO",
            Outcome::Inconclusive => "INCONCLUSIVE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub outcome: Outcome,
    pub reasons: Vec<String>,
}

/// Design section 9. `recoveries_ms` holds every recovery of the run, a recovery that did not
/// finish counted with the time it had run.
pub fn verdict(relays: &[RelayStats], recoveries_ms: &[u64]) -> Verdict {
    let mut reasons = Vec::new();
    let total = relays.len();
    let clean = relays.iter().filter(|r| r.clean()).count();
    let lossy = relays.iter().filter(|r| r.lossy()).count();
    let p95 = percentile(recoveries_ms, 95);
    reasons.push(format!(
        "{clean} of {total} relays without loss ({lossy} with loss), setup p95 {}",
        p95.map_or("n/a".to_owned(), |v| format!("{v} ms"))
    ));
    let loss_decided = total >= 3 && total - lossy < CLEAN_RELAYS_NEEDED;
    let timing_decided = recoveries_ms.len() >= MIN_RECOVERIES;
    let timing_bad = timing_decided && p95.is_some_and(|v| v >= SETUP_P95_LIMIT_MS);
    if loss_decided {
        reasons.push(format!(
            "fewer than {CLEAN_RELAYS_NEEDED} relays can still be free of loss"
        ));
    }
    if timing_bad {
        reasons.push(format!(
            "setup p95 is not below {SETUP_P95_LIMIT_MS} ms over {} recoveries",
            recoveries_ms.len()
        ));
    }
    if loss_decided || timing_bad {
        return Verdict {
            outcome: Outcome::NoGo,
            reasons,
        };
    }
    if total < 3 {
        reasons.push("fewer than 3 relays tested: the gate needs 3".to_owned());
    }
    if !timing_decided {
        reasons.push(format!(
            "only {} recoveries (at least {MIN_RECOVERIES} needed)",
            recoveries_ms.len()
        ));
    }
    if clean < CLEAN_RELAYS_NEEDED {
        reasons.push(format!(
            "fewer than {CLEAN_RELAYS_NEEDED} relays proved loss-free (at least {MIN_ELIGIBLE_EVENTS} deliverable events each needed)"
        ));
    }
    let go = total >= 3 && timing_decided && clean >= CLEAN_RELAYS_NEEDED;
    Verdict {
        outcome: if go {
            Outcome::Go
        } else {
            Outcome::Inconclusive
        },
        reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn relay(url: &str, eligible: u64, lost: u64) -> RelayStats {
        RelayStats {
            url: url.into(),
            eligible,
            lost,
            delivered: eligible.saturating_sub(lost),
            ..RelayStats::default()
        }
    }

    #[test]
    fn percentile_is_nearest_rank() {
        assert_eq!(percentile(&[], 50), None);
        let v: Vec<u64> = (1..=20).collect();
        assert_eq!(percentile(&v, 50), Some(10));
        assert_eq!(percentile(&v, 95), Some(19));
        assert_eq!(percentile(&v, 100), Some(20));
        assert_eq!(percentile(&[7], 95), Some(7));
    }

    #[test]
    fn go_needs_two_clean_relays_and_fast_setup() {
        let relays = [relay("a", 40, 0), relay("b", 40, 0), relay("c", 40, 3)];
        let v = verdict(&relays, &[900, 1200, 1500, 1100, 2000, 1800]);
        assert_eq!(v.outcome, Outcome::Go);
    }

    #[test]
    fn two_lossy_relays_are_no_go() {
        let relays = [relay("a", 40, 2), relay("b", 40, 1), relay("c", 40, 0)];
        let v = verdict(&relays, &[900; 10]);
        assert_eq!(v.outcome, Outcome::NoGo);
    }

    #[test]
    fn slow_setup_is_no_go() {
        let relays = [relay("a", 40, 0), relay("b", 40, 0), relay("c", 40, 0)];
        let mut recoveries = vec![1_000; 18];
        recoveries.extend([3_500, 4_000]);
        let v = verdict(&relays, &recoveries);
        assert_eq!(v.outcome, Outcome::NoGo);
    }

    #[test]
    fn a_relay_that_never_forwards_counts_as_lossy() {
        let mut silent = relay("a", 40, 0);
        silent.delivered = 0;
        assert!(silent.lossy() && !silent.clean());
        let relays = [silent.clone(), silent, relay("c", 40, 0)];
        assert_eq!(verdict(&relays, &[900; 10]).outcome, Outcome::NoGo);
    }

    #[test]
    fn too_little_data_is_inconclusive() {
        let relays = [relay("a", 40, 0), relay("b", 3, 0), relay("c", 40, 0)];
        assert_eq!(verdict(&relays, &[900; 3]).outcome, Outcome::Inconclusive);
        // Two clean relays but only two relays tested.
        let two = [relay("a", 40, 0), relay("b", 40, 0)];
        assert_eq!(verdict(&two, &[900; 10]).outcome, Outcome::Inconclusive);
    }
}
