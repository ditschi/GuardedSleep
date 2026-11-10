//! Token bucket per key with integer arithmetic; time comes in as a parameter.

use std::collections::HashMap;
use std::hash::Hash;

use crate::relay::Millis;

/// One token is `PER_TOKEN` units, so a refill of `elapsed_ms * per_minute` units never rounds.
const PER_TOKEN: u64 = 60_000;
/// Distinct keys tracked at once; beyond this, `take` prunes, and if the table is still full it evicts one entry
/// instead of refusing the newcomer (memory stays bounded, newcomers are never locked out).
const DEFAULT_MAX_KEYS: usize = 100_000;

#[derive(Debug)]
struct Bucket {
    units: u64,
    at: Millis,
}

/// Allows a burst of `per_minute` and refills that many tokens per minute. Keys are never logged.
#[derive(Debug)]
pub struct RateLimiter<K: Eq + Hash> {
    per_minute: u64,
    max_keys: usize,
    /// Time of the last prune forced by a full table, so a flood cannot make every call scan.
    last_full_prune: Option<Millis>,
    buckets: HashMap<K, Bucket>,
}

impl<K: Eq + Hash> RateLimiter<K> {
    /// `per_minute == 0` disables the limiter.
    pub fn new(per_minute: u32) -> Self {
        Self {
            per_minute: u64::from(per_minute),
            max_keys: DEFAULT_MAX_KEYS,
            last_full_prune: None,
            buckets: HashMap::new(),
        }
    }

    pub fn with_max_keys(mut self, max_keys: usize) -> Self {
        self.max_keys = max_keys;
        self
    }

    fn capacity(&self) -> u64 {
        self.per_minute.saturating_mul(PER_TOKEN)
    }

    fn refilled(&self, b: &Bucket, now: Millis) -> u64 {
        let add = now.saturating_sub(b.at).saturating_mul(self.per_minute);
        b.units.saturating_add(add).min(self.capacity())
    }

    /// Consumes one token; `false` when the key is over its limit.
    pub fn take(&mut self, key: K, now: Millis) -> bool {
        if self.per_minute == 0 {
            return true;
        }
        let capacity = self.capacity();
        if !self.buckets.contains_key(&key) && self.buckets.len() >= self.max_keys {
            self.make_room(now);
        }
        let units = match self.buckets.get(&key) {
            Some(b) => self.refilled(b, now),
            None => capacity,
        };
        let ok = units >= PER_TOKEN;
        self.buckets.insert(
            key,
            Bucket {
                units: if ok { units - PER_TOKEN } else { units },
                at: now,
            },
        );
        ok
    }

    /// Frees one slot for a new key: prune refilled buckets (at most once per millisecond), and if
    /// the table is still full evict an arbitrary entry. A newcomer must never be locked out.
    fn make_room(&mut self, now: Millis) {
        if self.last_full_prune != Some(now) {
            self.last_full_prune = Some(now);
            self.prune(now);
        }
        if self.buckets.len() >= self.max_keys {
            // No spare key handle without `Clone`, so drop the first entry through `retain`.
            let mut dropped = false;
            self.buckets.retain(|_, _| {
                if dropped {
                    true
                } else {
                    dropped = true;
                    false
                }
            });
        }
    }

    /// Drops buckets that have refilled completely so the map cannot grow without bound.
    pub fn prune(&mut self, now: Millis) {
        let capacity = self.capacity();
        let per_minute = self.per_minute;
        self.buckets.retain(|_, b| {
            let add = now.saturating_sub(b.at).saturating_mul(per_minute);
            b.units.saturating_add(add) < capacity
        });
    }

    /// Forgets a key, e.g. when the thing it identifies is gone.
    pub fn remove(&mut self, key: &K) {
        self.buckets.remove(key);
    }

    pub fn len(&self) -> usize {
        self.buckets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_a_burst_up_to_the_limit_then_refills_over_time() {
        let mut rl = RateLimiter::new(3);
        let got: Vec<bool> = (0..4).map(|_| rl.take("a", 0)).collect();
        assert_eq!(got, [true, true, true, false]);
        assert!(rl.take("b", 0));
        assert!(rl.take("a", 20_000));
        assert!(!rl.take("a", 20_000));
    }

    #[test]
    fn is_disabled_at_zero() {
        let mut rl = RateLimiter::new(0);
        for _ in 0..100 {
            assert!(rl.take("a", 0));
        }
        assert!(rl.is_empty());
    }

    #[test]
    fn prunes_refilled_buckets() {
        let mut rl = RateLimiter::new(2);
        rl.take("a", 0);
        assert_eq!(rl.len(), 1);
        rl.prune(1_000);
        assert_eq!(rl.len(), 1);
        rl.prune(60_000);
        assert!(rl.is_empty());
        assert!(rl.take("a", 60_000));
    }

    #[test]
    fn frequent_calls_still_refill() {
        let mut rl = RateLimiter::new(1);
        assert!(rl.take("a", 0));
        let mut allowed = false;
        for t in 1..=60_000u64 {
            if rl.take("a", t) {
                allowed = true;
                break;
            }
        }
        assert!(allowed, "one token per minute must come back");
    }

    #[test]
    fn full_table_of_live_keys_evicts_instead_of_refusing_or_growing() {
        let mut rl = RateLimiter::new(5).with_max_keys(2);
        assert!(rl.take(1, 0));
        assert!(rl.take(2, 0));
        assert!(rl.take(3, 0));
        assert_eq!(rl.len(), 2);
        for k in 4..100 {
            assert!(rl.take(k, 0));
            assert!(rl.len() <= 2);
        }
    }

    #[test]
    fn full_table_with_stale_entries_prunes_before_evicting() {
        let mut rl = RateLimiter::new(5).with_max_keys(2);
        assert!(rl.take(1, 0));
        assert!(rl.take(2, 0));
        // Both buckets are refilled after a minute; the live key 9 must survive the next newcomer.
        assert!(rl.take(9, 60_000));
        assert!(rl.take(10, 60_000));
        assert_eq!(rl.len(), 2);
        assert!(rl.take(9, 60_000), "live key kept its bucket");
    }
}
