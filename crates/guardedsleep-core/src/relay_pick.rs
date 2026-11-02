//! Deterministic choice of public Nostr relays from a shipped list (`alpha-connection-model`,
//! design sections 5 and 6).
//!
//! The core carries no relay data: a platform passes its shipped list (generated from
//! `data/nostr-relays.json`) as [`RelayCandidate`]s. Two devices with the same PSK and the same
//! list pick the same relays, and devices whose lists differ a little still pick mostly the same,
//! because the choice is rendezvous hashing: every candidate scores `HMAC-SHA256(K, host)` with
//! `K = HKDF(PSK, "guardedsleep relay pick v1")` and the lowest scores win. Adding or removing one
//! list entry moves at most one pick.
//!
//! A pick never takes two relays of one operator, and avoids two relays at one hoster while the
//! list allows it. Relays the caller reports as parked are skipped. Sans-IO: pure functions of
//! their arguments.

use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::envelope::{Purpose, derive_key};
use crate::invite::InviteMailbox;
use crate::mailbox::MailboxKind;

/// Relays the baby adds to an invite.
pub const PICK_COUNT: usize = 3;
/// Relays a device may add from its own list on top of the invite's.
pub const EXTRA_COUNT: usize = 2;

/// One entry of a shipped relay list, as far as the choice needs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct RelayCandidate {
    /// DNS name; the relay is `wss://<host>`.
    pub host: String,
    pub operator: String,
    /// Who runs the machine; `None` when unknown or private (no hoster constraint).
    pub hoster: Option<String>,
}

fn score(key: &[u8; 32], host: &str) -> Option<[u8; 32]> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).ok()?;
    mac.update(host.to_ascii_lowercase().as_bytes());
    Some(mac.finalize().into_bytes().into())
}

/// The candidates best first (lowest score, host as tie-break). Empty for an empty PSK.
fn ranked<'a>(psk: &[u8], candidates: &'a [RelayCandidate]) -> Vec<&'a RelayCandidate> {
    let Some(key) = derive_key(psk, Purpose::RelayPick) else {
        return Vec::new();
    };
    let mut scored: Vec<([u8; 32], &RelayCandidate)> = candidates
        .iter()
        .filter(|c| !c.host.is_empty())
        .filter_map(|c| Some((score(key.expose(), &c.host)?, c)))
        .collect();
    scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.host.cmp(&b.1.host)));
    scored.into_iter().map(|(_, c)| c).collect()
}

fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Greedy walk down `order`: skip parked relays, `blocked` hosts and any operator already taken;
/// the first pass also skips a hoster already taken, the second pass only fills what is left.
fn choose(
    order: &[&RelayCandidate],
    parked: &[String],
    blocked: &[String],
    mut operators: Vec<String>,
    mut hosters: Vec<String>,
    count: usize,
) -> Vec<InviteMailbox> {
    let mut out: Vec<&RelayCandidate> = Vec::new();
    for strict in [true, false] {
        for c in order {
            if out.len() >= count {
                break;
            }
            let hoster_taken = c
                .hoster
                .as_deref()
                .is_some_and(|h| hosters.iter().any(|t| same(t, h)));
            let skip = out.iter().any(|o| same(&o.host, &c.host))
                || parked.iter().chain(blocked).any(|p| same(p, &c.host))
                || operators.iter().any(|o| same(o, &c.operator))
                || (strict && hoster_taken);
            if skip {
                continue;
            }
            operators.push(c.operator.clone());
            if let Some(h) = &c.hoster {
                hosters.push(h.clone());
            }
            out.push(c);
        }
    }
    out.into_iter()
        .map(|c| InviteMailbox {
            kind: MailboxKind::Nostr,
            url: format!("wss://{}", c.host.to_ascii_lowercase()),
        })
        .collect()
}

/// Up to `count` public relays for `psk` from distinct operators (and hosters where possible), as
/// `nostr` mailboxes. `parked` lists hosts to skip. Fewer than `count` come back when the list has
/// too few operators; none for an empty PSK.
pub fn pick_relays(
    psk: &[u8],
    candidates: &[RelayCandidate],
    parked: &[String],
    count: usize,
) -> Vec<InviteMailbox> {
    let order = ranked(psk, candidates);
    choose(&order, parked, &[], Vec::new(), Vec::new(), count)
}

/// Up to `max` more relays from this device's own list, on top of the `current` mailbox hosts
/// (the invite's). Operators and hosters already in `current` (looked up in `candidates`) are
/// avoided, and `current` hosts are never returned. Both devices run this over their own list
/// with the same PSK, so overlapping lists still share relays.
pub fn extra_relays(
    psk: &[u8],
    candidates: &[RelayCandidate],
    parked: &[String],
    current: &[String],
    max: usize,
) -> Vec<InviteMailbox> {
    let order = ranked(psk, candidates);
    let known: Vec<&RelayCandidate> = candidates
        .iter()
        .filter(|c| current.iter().any(|h| same(h, &c.host)))
        .collect();
    let operators = known.iter().map(|c| c.operator.clone()).collect();
    let hosters = known.iter().filter_map(|c| c.hoster.clone()).collect();
    choose(&order, parked, current, operators, hosters, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(host: &str, operator: &str, hoster: Option<&str>) -> RelayCandidate {
        RelayCandidate {
            host: host.into(),
            operator: operator.into(),
            hoster: hoster.map(Into::into),
        }
    }

    fn list() -> Vec<RelayCandidate> {
        vec![
            cand("a.example.net", "A", None),
            cand("b.example.net", "B", Some("Contabo")),
            cand("c.example.net", "C", Some("Hetzner")),
            cand("d.example.net", "D", Some("Hetzner")),
            cand("e.example.net", "E", Some("Contabo")),
            cand("f.example.net", "F", Some("Init7")),
            cand("g.example.net", "G", Some("Init7")),
            cand("h.example.net", "H", Some("OVH")),
        ]
    }

    fn hosts(m: &[InviteMailbox]) -> Vec<String> {
        m.iter()
            .map(|m| m.url.trim_start_matches("wss://").to_owned())
            .collect()
    }

    fn psk(n: u8) -> Vec<u8> {
        vec![n; 32]
    }

    #[test]
    fn the_pick_is_deterministic_and_nostr_wss() {
        let a = pick_relays(&psk(1), &list(), &[], PICK_COUNT);
        let b = pick_relays(&psk(1), &list(), &[], PICK_COUNT);
        assert_eq!(a, b);
        assert_eq!(a.len(), 3);
        assert!(
            a.iter()
                .all(|m| m.kind == MailboxKind::Nostr && m.url.starts_with("wss://"))
        );
    }

    #[test]
    fn the_order_of_the_list_does_not_matter() {
        let mut reversed = list();
        reversed.reverse();
        assert_eq!(
            pick_relays(&psk(2), &list(), &[], 3),
            pick_relays(&psk(2), &reversed, &[], 3)
        );
    }

    #[test]
    fn different_keys_spread_over_the_list() {
        let mut seen = std::collections::BTreeSet::new();
        for n in 0..40u8 {
            seen.extend(hosts(&pick_relays(&psk(n), &list(), &[], 3)));
        }
        assert_eq!(
            seen.len(),
            list().len(),
            "every relay is picked by some key"
        );
    }

    #[test]
    fn operators_are_distinct_and_hosters_too_while_the_list_allows() {
        let l = list();
        for n in 0..60u8 {
            let picked = pick_relays(&psk(n), &l, &[], 3);
            let chosen: Vec<&RelayCandidate> = hosts(&picked)
                .iter()
                .map(|h| l.iter().find(|c| &c.host == h).unwrap())
                .collect();
            let mut ops: Vec<_> = chosen.iter().map(|c| &c.operator).collect();
            ops.sort();
            ops.dedup();
            assert_eq!(ops.len(), 3);
            let mut hos: Vec<_> = chosen.iter().filter_map(|c| c.hoster.as_ref()).collect();
            let before = hos.len();
            hos.sort();
            hos.dedup();
            assert_eq!(hos.len(), before, "key {n}: {chosen:?}");
        }
    }

    #[test]
    fn one_operator_never_gets_two_slots_even_when_the_list_is_short() {
        let l = vec![
            cand("a.example.net", "Same", Some("X")),
            cand("b.example.net", "Same", Some("Y")),
            cand("c.example.net", "Other", Some("X")),
        ];
        let picked = pick_relays(&psk(3), &l, &[], 3);
        assert_eq!(picked.len(), 2);
        // The hoster constraint gives way when it would leave a slot empty.
        assert!(hosts(&picked).contains(&"c.example.net".to_owned()));
    }

    #[test]
    fn parked_relays_are_skipped_and_the_next_in_rank_takes_over() {
        let first = hosts(&pick_relays(&psk(4), &list(), &[], 3));
        let parked = vec![first[0].clone()];
        let after = hosts(&pick_relays(&psk(4), &list(), &parked, 3));
        assert!(!after.contains(&first[0]));
        assert_eq!(after.len(), 3);
        // The other two stay unless the replacement displaced them by operator or hoster.
        assert!(after.iter().filter(|h| first.contains(h)).count() >= 1);
    }

    #[test]
    fn a_changed_list_moves_at_most_the_picks_that_touch_the_change() {
        let full = list();
        let base = hosts(&pick_relays(&psk(5), &full, &[], 3));
        // Removing a relay that is not picked changes nothing.
        let unpicked = full.iter().find(|c| !base.contains(&c.host)).unwrap();
        let without: Vec<_> = full
            .iter()
            .filter(|c| c.host != unpicked.host)
            .cloned()
            .collect();
        assert_eq!(hosts(&pick_relays(&psk(5), &without, &[], 3)), base);
        // Removing a picked relay keeps the other two (up to a displaced hoster conflict).
        let without: Vec<_> = full.iter().filter(|c| c.host != base[0]).cloned().collect();
        let changed = hosts(&pick_relays(&psk(5), &without, &[], 3));
        assert!(changed.iter().filter(|h| base.contains(h)).count() >= 2);
    }

    #[test]
    fn an_empty_psk_or_list_picks_nothing() {
        assert!(pick_relays(&[], &list(), &[], 3).is_empty());
        assert!(pick_relays(&psk(1), &[], &[], 3).is_empty());
        assert!(pick_relays(&psk(1), &list(), &[], 0).is_empty());
    }

    #[test]
    fn host_case_is_ignored() {
        let upper: Vec<_> = list()
            .into_iter()
            .map(|mut c| {
                c.host = c.host.to_ascii_uppercase();
                c
            })
            .collect();
        assert_eq!(
            pick_relays(&psk(6), &list(), &[], 3),
            pick_relays(&psk(6), &upper, &[], 3)
        );
    }

    #[test]
    fn extras_avoid_the_invite_hosts_operators_and_hosters() {
        let l = list();
        for n in 0..40u8 {
            let invite = hosts(&pick_relays(&psk(n), &l, &[], 3));
            let extras = hosts(&extra_relays(&psk(n), &l, &[], &invite, EXTRA_COUNT));
            assert!(!extras.is_empty() && extras.len() <= EXTRA_COUNT);
            for e in &extras {
                assert!(!invite.contains(e), "key {n}: {e} is in the invite");
            }
            let op = |h: &String| l.iter().find(|c| &c.host == h).unwrap().operator.clone();
            let used: Vec<String> = invite.iter().map(op).collect();
            for e in &extras {
                assert!(!used.contains(&op(e)));
            }
        }
    }

    #[test]
    fn devices_with_overlapping_lists_share_their_relays() {
        let a = list();
        let b: Vec<_> = list()
            .into_iter()
            .filter(|c| c.host != "h.example.net")
            .collect();
        for n in 0..40u8 {
            let pa: Vec<_> = hosts(&pick_relays(&psk(n), &a, &[], 5));
            let pb: Vec<_> = hosts(&pick_relays(&psk(n), &b, &[], 5));
            assert!(pa.iter().filter(|h| pb.contains(h)).count() >= 3, "key {n}");
        }
    }

    #[test]
    fn the_scores_use_the_relay_pick_key() {
        // Pins the key schedule: HKDF(PSK, "guardedsleep relay pick v1"), then HMAC over the host.
        let key = derive_key(&psk(7), Purpose::RelayPick).unwrap();
        let mut mac = Hmac::<Sha256>::new_from_slice(key.expose()).unwrap();
        mac.update(b"a.example.net");
        let expect: [u8; 32] = mac.finalize().into_bytes().into();
        assert_eq!(score(key.expose(), "A.Example.NET"), Some(expect));
    }
}
