//! `gs-scenario/1` (docs/technical/test-schemas.md section 2): peers, relay settings, a timeline of
//! fault verbs and assertions. Unknown fields are errors so typos cannot silently disable a check.

use crate::world::NostrRelaySpec;
use guardedsleep_core::invite::InviteMailbox;
use guardedsleep_core::protocol::Role;
use serde::Deserialize;
use serde_json::Value;

pub const SCHEMA: &str = "gs-scenario/1";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub schema: String,
    pub id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "one")]
    pub seed: u64,
    /// Upper bound on the seeds the suite runs for this scenario (default: all). For scenarios of
    /// many simulated hours, whose run time grows with the simulated time and not with the seed.
    #[serde(default)]
    pub max_seeds: Option<u64>,
    pub peers: Vec<PeerSpec>,
    #[serde(default)]
    pub relay: RelaySpec,
    /// The simulated public Nostr relays the peers' `n:` entries name.
    #[serde(default)]
    pub nostr: Vec<NostrRelaySpec>,
    /// True wall-clock time (Unix seconds) at virtual time 0; default a synthetic value.
    #[serde(default)]
    pub wall_start_s: Option<u64>,
    pub timeline: Vec<Step>,
    #[serde(default)]
    pub assert: Vec<Assertion>,
    /// A known failure of the core under test: the scenario must still fail, and the reason is
    /// reported. A scenario that starts passing fails the suite until this field is removed.
    #[serde(default)]
    pub expected_fail: Option<String>,
    /// Traces committed under `crates/guardedsleep-core/vectors/traces/` by `gs-sim bless-traces`.
    #[serde(default)]
    pub export_traces: Vec<TraceExport>,
}

fn one() -> u64 {
    1
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeerSpec {
    pub id: String,
    pub role: Role,
    /// Synthetic PSK name: `A` (default) or `B` (a different key).
    #[serde(default = "psk_a")]
    pub psk: String,
    #[serde(default)]
    pub paired_before: bool,
    pub heartbeat_interval_ms: Option<u64>,
    pub alarm_after_ms: Option<u64>,
    /// Direction attribute of the baby's offer (default `sendonly`).
    pub sdp_direction: Option<String>,
    /// Baby: the mailbox set it sends the parent after each verification (`LinkConfig::mailboxes`).
    #[serde(default)]
    pub mailboxes: Vec<InviteMailbox>,
    /// The first-pairing window (`LinkConfig::pin_window_until_s`) as seconds of simulated wall
    /// clock after the start of the run: the invite expiry of the pairing.
    pub pin_window_s: Option<u64>,
    /// Parent: the device name it sends the baby (`LinkConfig::device_name`).
    #[serde(default)]
    pub device_name: Option<String>,
    /// This device's wall clock is this many seconds off true time.
    #[serde(default)]
    pub wall_offset_s: i64,
    /// `LinkConfig::only_local`: "Only same Wi-Fi or my own server".
    #[serde(default)]
    pub only_local: bool,
}

fn psk_a() -> String {
    "A".into()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelaySpec {
    /// Test-only: every room id maps to one room, so peers with different PSKs (and therefore
    /// different derived rooms) end up next to each other.
    #[serde(default)]
    pub alias_rooms: bool,
    /// `SIGNALING_MAX_PARENTS` of the simulated server (ADR-017 section 10: default 1).
    #[serde(default = "one_usize")]
    pub parent_cap: usize,
    /// A compromised relay that rewrites every relayed SDP: `fingerprint_confusion` moves the real
    /// `a=fingerprint` value into an `s=` line and puts the relay's own certificate fingerprint on
    /// the real line (it terminates DTLS and forwards the DataChannel in both directions).
    #[serde(default)]
    pub sdp_rewrite: Option<String>,
    /// A relay that hands out the same client id to every connection of a role (`c-baby`,
    /// `c-parent`) instead of a fresh one, so MACs bound to a client id alone would replay.
    #[serde(default)]
    pub reuse_client_ids: bool,
}

impl Default for RelaySpec {
    fn default() -> Self {
        Self {
            alias_rooms: false,
            parent_cap: 1,
            sdp_rewrite: None,
            reuse_client_ids: false,
        }
    }
}

fn one_usize() -> usize {
    1
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceExport {
    pub peer: String,
    #[serde(default)]
    pub instance: u32,
    pub id: String,
}

/// One timeline entry. Which fields apply depends on `do`; see the README for the verb table.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub at: u64,
    #[serde(rename = "do")]
    pub verb: String,
    pub peer: Option<String>,
    /// Sender whose outgoing messages a fault affects (`baby`, `parent`, a peer id; default all).
    pub from: Option<String>,
    pub ms: Option<u64>,
    pub jitter_ms: Option<u64>,
    pub p: Option<f64>,
    pub for_ms: Option<u64>,
    /// `data` (DataChannel, default) or `signaling`.
    pub via: Option<String>,
    /// A fault only affects messages whose text contains this (e.g. `"parent_pin"`).
    pub message: Option<String>,
    pub gap_ms: Option<u64>,
    pub after_ms: Option<u64>,
    pub mode: Option<String>,
    pub role: Option<Role>,
    pub id: Option<String>,
    pub spoof_from: Option<bool>,
    pub act_after_ms: Option<u64>,
    /// `time_jump`: `kept` delivers one heartbeat per link at wake-up, `none` (default) delivers
    /// nothing, as if the peer had been lost during the gap.
    pub peer_traffic: Option<String>,
    pub emit_available: Option<bool>,
    /// `restart_process`: the mailbox set the restarted process is configured with (an app update
    /// that ships a new list).
    pub mailboxes: Option<Vec<InviteMailbox>>,
    /// `input`: any `LinkInput` in its serde form.
    pub input: Option<Value>,
}

/// One check. The discriminating field is `event`, `never`, `state`, `max_level`,
/// `reaches_level`, `sent_signal` or `joined`.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Assertion {
    pub peer: String,
    #[serde(default)]
    pub note: Option<String>,
    pub event: Option<String>,
    pub never: Option<String>,
    #[serde(rename = "where")]
    pub where_: Option<Value>,
    /// Window `[lo, hi]` in simulated milliseconds (default: the whole run).
    pub within_ms: Option<[u64; 2]>,
    pub count: Option<usize>,
    pub count_min: Option<usize>,
    pub count_max: Option<usize>,
    /// `event` only: the matching event must happen `[lo, hi]` ms after the last DataChannel
    /// message the peer received (the core's `last_alive`).
    pub since_last_contact_ms: Option<[u64; 2]>,
    pub state: Option<String>,
    pub at_ms: Option<u64>,
    pub key: Option<String>,
    pub level: Option<u8>,
    pub max_level: Option<u8>,
    pub reaches_level: Option<u8>,
    pub sent_signal: Option<String>,
    pub joined: Option<bool>,
    /// The exact text of every frame this peer sent on a socket that never said `hello` (the
    /// reachability probes of a verified link): at least one, and no other text.
    pub preauth: Option<String>,
    /// At most this many `EVENT`s per Nostr relay sent by the peer within `within_ms` (and at least
    /// one in total, so the check cannot pass on a silent run).
    pub nostr_sent_max: Option<usize>,
    /// The reachability checks of a verified link reveal nothing of the pairing: every frame the
    /// peer sent on a Nostr socket while connected is the throwaway `REQ` or a `CLOSE`, without an
    /// author or any daily tag, and at least one such frame exists.
    pub nostr_probe: Option<bool>,
    /// The peer sent at least one frame to each of these Nostr relays (all of them).
    pub nostr_used: Option<Vec<String>>,
    /// The peer closed at most this many Nostr sockets within `within_ms` (a relay it parked or
    /// found unusable shows up as a close; 0 proves it never left a relay).
    pub nostr_closes_max: Option<usize>,
}

impl Scenario {
    pub fn parse(text: &str) -> Result<Scenario, String> {
        let s: Scenario = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if s.schema != SCHEMA {
            return Err(format!("unsupported scenario schema {}", s.schema));
        }
        if !s.timeline.iter().any(|t| t.verb == "end") {
            return Err("timeline needs an `end` step".into());
        }
        Ok(s)
    }

    pub fn end_ms(&self) -> u64 {
        self.timeline
            .iter()
            .filter(|t| t.verb == "end")
            .map(|t| t.at)
            .min()
            .unwrap_or(60_000)
    }
}
