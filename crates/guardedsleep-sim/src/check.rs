//! Assertion evaluation over a finished run.

use serde_json::Value;

use crate::scenario::{Assertion, Scenario};
use crate::world::{Actor, World};

pub struct Check {
    pub ok: bool,
    pub text: String,
}

fn subset(want: &Value, have: &Value) -> bool {
    match (want, have) {
        (Value::Object(w), Value::Object(h)) => w
            .iter()
            .all(|(k, v)| h.get(k).is_some_and(|hv| subset(v, hv))),
        _ => want == have,
    }
}

fn describe(a: &Assertion) -> String {
    if let Some(n) = &a.note {
        return format!("{}: {n}", a.peer);
    }
    let mut parts = vec![a.peer.clone()];
    for (k, v) in [
        ("event", a.event.as_deref()),
        ("never", a.never.as_deref()),
        ("state", a.state.as_deref()),
        ("sent_signal", a.sent_signal.as_deref()),
    ] {
        if let Some(v) = v {
            parts.push(format!("{k}={v}"));
        }
    }
    if let Some(l) = a.max_level {
        parts.push(format!("max_level<={l}"));
    }
    if let Some(l) = a.reaches_level {
        parts.push(format!("reaches_level>={l}"));
    }
    if let Some(w) = &a.where_ {
        parts.push(format!("where={w}"));
    }
    if let Some(w) = a.within_ms {
        parts.push(format!("within={w:?}"));
    }
    if let Some(t) = a.at_ms {
        parts.push(format!("at={t}"));
    }
    parts.join(" ")
}

/// The reachability checks of a verified link send nothing of the pairing (design section 1).
fn check_nostr_probe(world: &World, ix: usize) -> (bool, String) {
    use guardedsleep_core::nostr::NostrKeys;
    let psk = crate::world::synthetic_psk("A");
    let keys = NostrKeys::derive(&psk);
    let frames: Vec<_> = world
        .nostr
        .sent
        .iter()
        .filter(|s| s.actor == ix && s.probe)
        .collect();
    let mut bad = Vec::new();
    for f in &frames {
        // Not even the event kind of the app: a check asks for a random event id only.
        let revealing = f.text.contains("\"authors\"")
            || f.text.contains("24861")
            || f.text.contains("\"#t\"")
            || f.text.starts_with("[\"EVENT\"")
            || keys
                .as_ref()
                .is_some_and(|k| probe_mentions_pairing(k, world, &f.text));
        if revealing {
            bad.push(f.text.clone());
        }
    }
    let reqs = frames
        .iter()
        .filter(|f| f.text.starts_with("[\"REQ\""))
        .count();
    (
        bad.is_empty() && reqs > 0,
        format!(
            "{reqs} throwaway REQ(s) among {} probe frame(s); revealing: {bad:?}",
            frames.len()
        ),
    )
}

/// Whether a frame names the tag or a public key of the pairing for any day near the simulated
/// clock.
fn probe_mentions_pairing(
    keys: &guardedsleep_core::nostr::NostrKeys,
    world: &World,
    text: &str,
) -> bool {
    use guardedsleep_core::protocol::Role;
    let around = guardedsleep_core::nostr::epoch_of(world.wall_base + world.end / 1_000);
    (around.saturating_sub(2)..=around + 2).any(|e| {
        text.contains(&keys.tag(e))
            || [Role::Baby, Role::Parent]
                .into_iter()
                .filter_map(|r| keys.public_key_hex(r, e))
                .any(|pk| text.contains(&pk))
    })
}

pub fn evaluate(world: &World, sc: &Scenario) -> Vec<Check> {
    sc.assert
        .iter()
        .map(|a| {
            let (ok, detail) = check_one(world, a);
            Check {
                ok,
                text: format!("{} -> {detail}", describe(a)),
            }
        })
        .collect()
}

fn count_ok(a: &Assertion, n: usize) -> bool {
    if let Some(c) = a.count {
        return n == c;
    }
    n >= a.count_min.unwrap_or(usize::from(a.count_max.is_none()))
        && a.count_max.is_none_or(|m| n <= m)
}

fn level_at(actor: &Actor, t: u64) -> u8 {
    actor
        .statuses
        .iter()
        .take_while(|(st, _)| *st <= t)
        .last()
        .map_or(0, |(_, s)| s.level)
}

fn check_one(world: &World, a: &Assertion) -> (bool, String) {
    let Some(ix) = world.actor_ix(&a.peer) else {
        return (false, format!("unknown peer {}", a.peer));
    };
    let actor = &world.actors[ix];
    let [lo, hi] = a.within_ms.unwrap_or([0, world.end]);
    let in_window = |t: u64| t >= lo && t <= hi;

    let matching = |name: &str| -> Vec<(u64, u64)> {
        actor
            .events
            .iter()
            .filter(|e| {
                in_window(e.t)
                    && e.ev["type"] == name
                    && a.where_.as_ref().is_none_or(|w| subset(w, &e.ev))
                    && a.since_last_contact_ms
                        .is_none_or(|[l, h]| e.since_contact >= l && e.since_contact <= h)
            })
            .map(|e| (e.t, e.since_contact))
            .collect()
    };

    if let Some(name) = &a.event {
        let m = matching(name);
        let ok = count_ok(a, m.len());
        return (ok, format!("{} match(es) {:?}", m.len(), m));
    }
    if let Some(name) = &a.never {
        let m = matching(name);
        return (m.is_empty(), format!("{} match(es) {:?}", m.len(), m));
    }
    if let Some(want) = &a.state {
        let Some(at) = a.at_ms else {
            return (false, "`state` needs `at_ms`".into());
        };
        let last = actor.statuses.iter().take_while(|(t, _)| *t <= at).last();
        let Some((t, s)) = last else {
            return (false, format!("no status before {at}"));
        };
        let got = serde_json::to_value(s.state).unwrap_or_default();
        let mut ok = got.as_str() == Some(want.as_str());
        if let Some(k) = &a.key {
            ok &= s.key.as_deref() == Some(k.as_str());
        }
        if let Some(l) = a.level {
            ok &= s.level == l;
        }
        return (
            ok,
            format!("state {got} level {} key {:?} (since {t})", s.level, s.key),
        );
    }
    if let Some(max) = a.max_level {
        let top = actor
            .statuses
            .iter()
            .filter(|(t, _)| in_window(*t))
            .map(|(_, s)| s.level)
            .chain(std::iter::once(level_at(actor, lo)))
            .max()
            .unwrap_or(0);
        return (top <= max, format!("max level {top}"));
    }
    if let Some(min) = a.reaches_level {
        let top = actor
            .statuses
            .iter()
            .filter(|(t, _)| in_window(*t))
            .map(|(_, s)| s.level)
            .chain(std::iter::once(level_at(actor, lo)))
            .max()
            .unwrap_or(0);
        return (top >= min, format!("max level {top}"));
    }
    if let Some(kind) = &a.sent_signal {
        let n = actor
            .sent_signals
            .iter()
            .filter(|(t, k)| in_window(*t) && k == kind)
            .count();
        let ok = count_ok(a, n);
        return (ok, format!("{n} message(s)"));
    }
    if let Some(want) = &a.preauth {
        let frames: Vec<&str> = world
            .preauth
            .iter()
            .filter(|(i, _)| *i == ix)
            .map(|(_, t)| t.as_str())
            .collect();
        let ok = !frames.is_empty() && frames.iter().all(|f| f == want);
        return (
            ok,
            format!("{} frame(s) before hello: {frames:?}", frames.len()),
        );
    }
    if let Some(max) = a.nostr_sent_max {
        let mut per_relay: std::collections::BTreeMap<&str, usize> = Default::default();
        for s in world
            .nostr
            .sent
            .iter()
            .filter(|s| s.actor == ix && in_window(s.t) && s.text.starts_with("[\"EVENT\""))
        {
            *per_relay.entry(s.url.as_str()).or_default() += 1;
        }
        let busiest = per_relay.values().copied().max().unwrap_or(0);
        return (
            busiest > 0 && busiest <= max,
            format!("most events to one relay: {busiest} (limit {max}); {per_relay:?}"),
        );
    }
    if let Some(max) = a.nostr_closes_max {
        let n = world
            .nostr
            .closes
            .iter()
            .filter(|(actor, _, t)| *actor == ix && in_window(*t))
            .count();
        return (
            n <= max,
            format!("{n} Nostr socket(s) closed by the peer (limit {max})"),
        );
    }
    if let Some(used) = &a.nostr_used {
        let missing: Vec<&String> = used
            .iter()
            .filter(|u| {
                !world
                    .nostr
                    .sent
                    .iter()
                    .any(|s| s.actor == ix && &s.url == *u && in_window(s.t))
            })
            .collect();
        return (missing.is_empty(), format!("never used: {missing:?}"));
    }
    if a.nostr_probe == Some(true) {
        return check_nostr_probe(world, ix);
    }
    if let Some(want) = a.joined {
        let n = actor.welcomes.iter().filter(|t| in_window(**t)).count();
        return ((n > 0) == want, format!("{n} welcome(s)"));
    }
    (false, "assertion has no discriminating field".into())
}
