//! The curated lists (`data/nostr-relays.json`, `data/stun-servers.json`): typed loader, entry
//! validation and the build check (alpha-connection-model design section 6, spec
//! `signaling-mailboxes` "Curated public services").

use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;

/// Fewest usable relays a build needs ...
pub const MIN_RELAYS: usize = 6;
/// ... from at least this many operators.
pub const MIN_RELAY_OPERATORS: usize = 4;
/// Fewest STUN operators a build needs (the ICE configuration uses two).
pub const MIN_STUN_OPERATORS: usize = 2;
/// STUN operators the default ICE configuration uses.
pub const ICE_STUN_OPERATORS: usize = 2;
/// An entry last vetted longer ago than this many months raises a warning (never a failure).
pub const STALE_AFTER_MONTHS: i32 = 12;

/// EU member states, the EEA states outside the EU, and Switzerland (ISO 3166-1 alpha-2).
const ALLOWED_COUNTRIES: &[&str] = &[
    // EU
    "AT", "BE", "BG", "HR", "CY", "CZ", "DK", "EE", "FI", "FR", "DE", "GR", "HU", "IE", "IT", "LV",
    "LT", "LU", "MT", "NL", "PL", "PT", "RO", "SK", "SI", "ES", "SE", // EEA
    "IS", "LI", "NO", // Switzerland
    "CH",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Shipped: in the generated lists and the web CSP.
    Active,
    /// Kept in the file for history and notes, but neither shipped nor counted.
    Suspended,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relay {
    /// DNS name; the relay is always `wss://<host>`.
    pub host: String,
    pub operator: String,
    /// Who runs the machine. `None` when unknown or a private host; a pick avoids two relays at
    /// one hoster when it can.
    pub hoster: Option<String>,
    pub country: String,
    pub software: String,
    // Vetting data for people reading the file (strfry keeps ephemeral events for ~5 min).
    #[allow(dead_code)]
    pub keeps_ephemeral: bool,
    pub privacy_policy: Option<String>,
    pub last_vetted: String,
    pub status: Status,
    // Vetting notes for people reading the file; not part of any output.
    #[allow(dead_code)]
    pub notes: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stun {
    pub host: String,
    pub port: u16,
    pub operator: String,
    pub country: String,
    pub privacy_policy: Option<String>,
    pub last_vetted: String,
    pub status: Status,
    // Vetting notes for people reading the file; not part of any output.
    #[allow(dead_code)]
    pub notes: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RelayFile {
    relays: Vec<Relay>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StunFile {
    servers: Vec<Stun>,
}

#[derive(Debug, Clone, Default)]
pub struct Lists {
    pub relays: Vec<Relay>,
    pub stun: Vec<Stun>,
}

impl Relay {
    pub fn url(&self) -> String {
        format!("wss://{}", self.host)
    }
}

impl Stun {
    pub fn url(&self) -> String {
        format!("stun:{}:{}", self.host, self.port)
    }
}

impl Lists {
    pub fn parse(relays_json: &str, stun_json: &str) -> Result<Self, String> {
        let relays: RelayFile =
            serde_json::from_str(relays_json).map_err(|e| format!("nostr-relays.json: {e}"))?;
        let stun: StunFile =
            serde_json::from_str(stun_json).map_err(|e| format!("stun-servers.json: {e}"))?;
        Ok(Self {
            relays: relays.relays,
            stun: stun.servers,
        })
    }

    pub fn load(data_dir: &Path) -> Result<Self, String> {
        let read = |name: &str| {
            std::fs::read_to_string(data_dir.join(name))
                .map_err(|e| format!("{}: {e}", data_dir.join(name).display()))
        };
        Self::parse(&read("nostr-relays.json")?, &read("stun-servers.json")?)
    }

    /// Entries that ship, in file order.
    pub fn active_relays(&self) -> impl Iterator<Item = &Relay> {
        self.relays.iter().filter(|r| r.status == Status::Active)
    }

    pub fn active_stun(&self) -> impl Iterator<Item = &Stun> {
        self.stun.iter().filter(|s| s.status == Status::Active)
    }

    /// The STUN servers of the default ICE configuration: every entry of the first
    /// [`ICE_STUN_OPERATORS`] distinct operators, in file order (design section 7: Nextcloud on 443
    /// and 3478, then Telekom).
    pub fn default_ice_stun(&self) -> Vec<&Stun> {
        let mut operators: Vec<&str> = Vec::new();
        for s in self.active_stun() {
            if !operators.contains(&s.operator.as_str()) && operators.len() < ICE_STUN_OPERATORS {
                operators.push(&s.operator);
            }
        }
        self.active_stun()
            .filter(|s| operators.contains(&s.operator.as_str()))
            .collect()
    }
}

// ---- dates -------------------------------------------------------------------------------------

/// A calendar date; ordering is chronological.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Date {
    /// `YYYY-MM-DD`, a real calendar day.
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split('-');
        let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
        if parts.next().is_some() || y.len() != 4 || m.len() != 2 || d.len() != 2 {
            return None;
        }
        let date = Self {
            year: y.parse().ok()?,
            month: m.parse().ok()?,
            day: d.parse().ok()?,
        };
        let leap = date.year % 4 == 0 && (date.year % 100 != 0 || date.year % 400 == 0);
        let days = match date.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            2 => 28,
            _ => return None,
        };
        (1..=days).contains(&date.day).then_some(date)
    }

    /// The date from Unix seconds (civil-from-days, proleptic Gregorian).
    pub fn from_unix_seconds(secs: u64) -> Self {
        let z = (secs / 86_400) as i64 + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = (yoe + era * 400) as i32 + i32::from(month <= 2);
        Self { year, month, day }
    }

    /// True when `self` is more than `months` months before `today`.
    pub fn older_than_months(self, months: i32, today: Date) -> bool {
        let total = self.year * 12 + self.month as i32 - 1 + months;
        let limit = Date {
            year: total.div_euclid(12),
            month: total.rem_euclid(12) as u32 + 1,
            day: self.day,
        };
        limit < today
    }
}

// ---- validation --------------------------------------------------------------------------------

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// A DNS host name: lower-case labels of letters, digits and `-`, at least one dot. IP literals,
/// schemes, ports and paths are not host names here (the value ends up in a URL, a CSP header and
/// generated source).
pub fn valid_host(host: &str) -> bool {
    host.len() <= 253
        && host.contains('.')
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
        && !host
            .split('.')
            .all(|l| l.bytes().all(|b| b.is_ascii_digit()))
}

/// Display text that ends up in generated source and the UI: printable ASCII without quote,
/// backslash, dollar or angle brackets.
fn plain_text(text: &str) -> bool {
    !text.trim().is_empty()
        && text.trim() == text
        && text
            .bytes()
            .all(|b| (0x20..0x7f).contains(&b) && !b"\"\\$<>`".contains(&b))
}

fn valid_policy(link: &str) -> bool {
    link.strip_prefix("https://").is_some_and(|rest| {
        rest.contains('.')
            && rest
                .bytes()
                .all(|b| b.is_ascii_graphic() && !b"\"\\$<>`'".contains(&b))
    })
}

struct Common<'a> {
    what: &'a str,
    operator: &'a str,
    country: &'a str,
    policy: Option<&'a str>,
    last_vetted: &'a str,
}

fn check_common(c: &Common<'_>, today: Date, report: &mut Report) {
    let what = c.what;
    if !plain_text(c.operator) {
        report
            .errors
            .push(format!("{what}: operator must be plain printable text"));
    }
    if !ALLOWED_COUNTRIES.contains(&c.country) {
        report.errors.push(format!(
            "{what}: country {:?} is not an EU, EEA or CH ISO 3166-1 alpha-2 code",
            c.country
        ));
    }
    if let Some(link) = c.policy
        && !valid_policy(link)
    {
        report
            .errors
            .push(format!("{what}: privacy_policy must be an https:// link"));
    }
    match Date::parse(c.last_vetted) {
        None => report.errors.push(format!(
            "{what}: last_vetted {:?} is not YYYY-MM-DD",
            c.last_vetted
        )),
        Some(d) if d > today => report.errors.push(format!(
            "{what}: last_vetted {} is in the future",
            c.last_vetted
        )),
        Some(d) if d.older_than_months(STALE_AFTER_MONTHS, today) => report.warnings.push(format!(
            "{what}: last vetted {} is more than {STALE_AFTER_MONTHS} months ago, re-vet it",
            c.last_vetted
        )),
        Some(_) => {}
    }
}

/// Entry rules plus the build minimums. `errors` fail the build, `warnings` do not.
pub fn validate(lists: &Lists, today: Date) -> Report {
    let mut report = Report::default();

    let mut hosts = BTreeSet::new();
    let mut operators = BTreeSet::new();
    for r in &lists.relays {
        let what = format!("relay {}", r.host);
        if !valid_host(&r.host) {
            report
                .errors
                .push(format!("{what}: host must be a lower-case DNS name without scheme, port or path (wss:// is implied)"));
        }
        if !hosts.insert(r.host.clone()) {
            report.errors.push(format!("{what}: listed twice"));
        }
        if let Some(h) = &r.hoster
            && !plain_text(h)
        {
            report.errors.push(format!(
                "{what}: hoster must be plain printable text or null"
            ));
        }
        if !plain_text(&r.software) {
            report
                .errors
                .push(format!("{what}: software must be plain printable text"));
        }
        check_common(
            &Common {
                what: &what,
                operator: &r.operator,
                country: &r.country,
                policy: r.privacy_policy.as_deref(),
                last_vetted: &r.last_vetted,
            },
            today,
            &mut report,
        );
        if r.status == Status::Active && !operators.insert(r.operator.to_lowercase()) {
            report.errors.push(format!(
                "{what}: operator {:?} already has an active relay (one entry per operator)",
                r.operator
            ));
        }
    }

    let mut endpoints = BTreeSet::new();
    for s in &lists.stun {
        let what = format!("stun {}:{}", s.host, s.port);
        if !valid_host(&s.host) {
            report
                .errors
                .push(format!("{what}: host must be a lower-case DNS name"));
        }
        if s.port == 0 {
            report.errors.push(format!("{what}: port must be 1-65535"));
        }
        if !endpoints.insert((s.host.clone(), s.port)) {
            report.errors.push(format!("{what}: listed twice"));
        }
        check_common(
            &Common {
                what: &what,
                operator: &s.operator,
                country: &s.country,
                policy: s.privacy_policy.as_deref(),
                last_vetted: &s.last_vetted,
            },
            today,
            &mut report,
        );
    }

    let relays = lists.active_relays().count();
    let relay_operators: BTreeSet<_> = lists
        .active_relays()
        .map(|r| r.operator.to_lowercase())
        .collect();
    if relays < MIN_RELAYS || relay_operators.len() < MIN_RELAY_OPERATORS {
        report.errors.push(format!(
            "relay list too small: {relays} usable relays from {} operators, need at least {MIN_RELAYS} from {MIN_RELAY_OPERATORS}",
            relay_operators.len()
        ));
    }
    let stun_operators: BTreeSet<_> = lists
        .active_stun()
        .map(|s| s.operator.to_lowercase())
        .collect();
    if stun_operators.len() < MIN_STUN_OPERATORS {
        report.errors.push(format!(
            "STUN list too small: {} usable operators, need at least {MIN_STUN_OPERATORS} distinct operators",
            stun_operators.len()
        ));
    }
    report
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn relay(host: &str, operator: &str) -> Relay {
        Relay {
            host: host.into(),
            operator: operator.into(),
            hoster: None,
            country: "DE".into(),
            software: "strfry".into(),
            keeps_ephemeral: true,
            privacy_policy: None,
            last_vetted: "2026-10-23".into(),
            status: Status::Active,
            notes: String::new(),
        }
    }

    pub fn stun(host: &str, port: u16, operator: &str) -> Stun {
        Stun {
            host: host.into(),
            port,
            operator: operator.into(),
            country: "DE".into(),
            privacy_policy: None,
            last_vetted: "2026-10-23".into(),
            status: Status::Active,
            notes: String::new(),
        }
    }

    pub fn today() -> Date {
        Date::parse("2026-10-25").unwrap()
    }

    pub fn good() -> Lists {
        Lists {
            relays: (1..=6)
                .map(|i| relay(&format!("r{i}.example.net"), &format!("op{i}")))
                .collect(),
            stun: vec![
                stun("a.example.net", 3478, "A"),
                stun("a.example.net", 443, "A"),
                stun("b.example.net", 3478, "B"),
                stun("c.example.net", 3478, "C"),
            ],
        }
    }

    #[test]
    fn a_good_list_passes() {
        assert_eq!(validate(&good(), today()), Report::default());
    }

    #[test]
    fn the_shipped_files_pass_the_build_check() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let lists = Lists::load(&dir).unwrap();
        let report = validate(&lists, today());
        assert_eq!(report.errors, Vec::<String>::new());
        assert!(lists.active_relays().count() >= MIN_RELAYS);
    }

    #[test]
    fn fewer_than_six_relays_fail_and_name_the_shortfall() {
        let mut l = good();
        l.relays.truncate(5);
        let r = validate(&l, today());
        assert!(
            r.errors.iter().any(|e| e.contains("5 usable relays")),
            "{r:?}"
        );
    }

    #[test]
    fn six_relays_from_three_operators_fail() {
        let mut l = good();
        for (i, r) in l.relays.iter_mut().enumerate() {
            r.operator = format!("op{}", i % 3);
            // One entry per operator would also trip; this checks the operator count message.
        }
        let r = validate(&l, today());
        assert!(
            r.errors.iter().any(|e| e.contains("from 3 operators")),
            "{r:?}"
        );
    }

    #[test]
    fn a_suspended_relay_does_not_count() {
        let mut l = good();
        l.relays[0].status = Status::Suspended;
        let r = validate(&l, today());
        assert!(
            r.errors.iter().any(|e| e.contains("5 usable relays")),
            "{r:?}"
        );
    }

    #[test]
    fn two_active_relays_of_one_operator_fail() {
        let mut l = good();
        l.relays[1].operator = "OP1".into();
        let r = validate(&l, today());
        assert!(
            r.errors
                .iter()
                .any(|e| e.contains("one entry per operator")),
            "{r:?}"
        );
    }

    #[test]
    fn stun_needs_two_distinct_operators() {
        let mut l = good();
        l.stun = vec![
            stun("a.example.net", 3478, "A"),
            stun("a.example.net", 443, "A"),
        ];
        let r = validate(&l, today());
        assert!(
            r.errors.iter().any(|e| e.contains("STUN list too small")),
            "{r:?}"
        );
    }

    #[test]
    fn entry_rules() {
        type Mutation = Box<dyn Fn(&mut Lists)>;
        let cases: Vec<(&str, Mutation)> = vec![
            (
                "host",
                Box::new(|l| l.relays[0].host = "wss://x.example.net".into()),
            ),
            (
                "host",
                Box::new(|l| l.relays[0].host = "Relay.Example.net".into()),
            ),
            ("host", Box::new(|l| l.relays[0].host = "10.0.0.1".into())),
            ("host", Box::new(|l| l.relays[0].host = "localhost".into())),
            ("country", Box::new(|l| l.relays[0].country = "US".into())),
            ("country", Box::new(|l| l.stun[0].country = "GB".into())),
            (
                "privacy_policy",
                Box::new(|l| l.relays[0].privacy_policy = Some("http://x.example/p".into())),
            ),
            (
                "last_vetted",
                Box::new(|l| l.relays[0].last_vetted = "2026-13-01".into()),
            ),
            (
                "future",
                Box::new(|l| l.relays[0].last_vetted = "2026-11-01".into()),
            ),
            (
                "listed twice",
                Box::new(|l| l.relays[1].host = l.relays[0].host.clone()),
            ),
            ("port", Box::new(|l| l.stun[0].port = 0)),
            (
                "listed twice",
                Box::new(|l| l.stun[1].port = l.stun[0].port),
            ),
            (
                "operator",
                Box::new(|l| l.relays[0].operator = "a\"b".into()),
            ),
        ];
        for (needle, mutate) in cases {
            let mut l = good();
            mutate(&mut l);
            let r = validate(&l, today());
            assert!(
                r.errors.iter().any(|e| e.contains(needle)),
                "{needle}: {r:?}"
            );
        }
    }

    #[test]
    fn a_stale_entry_warns_but_does_not_fail() {
        let mut l = good();
        l.relays[0].last_vetted = "2025-10-24".into();
        l.stun[0].last_vetted = "2024-01-01".into();
        let r = validate(&l, today());
        assert_eq!(r.errors, Vec::<String>::new());
        assert_eq!(r.warnings.len(), 2, "{r:?}");
        // Exactly twelve months is still fresh.
        l.relays[0].last_vetted = "2025-10-25".into();
        l.stun[0].last_vetted = "2025-10-25".into();
        assert_eq!(validate(&l, today()).warnings, Vec::<String>::new());
    }

    #[test]
    fn unknown_fields_are_refused() {
        let bad = r#"{"relays":[{"host":"a.b","extra":1}]}"#;
        assert!(Lists::parse(bad, r#"{"servers":[]}"#).is_err());
    }

    #[test]
    fn dates() {
        assert_eq!(Date::parse("2024-02-29").map(|d| d.day), Some(29));
        assert_eq!(Date::parse("2025-02-29"), None);
        assert_eq!(Date::parse("2025-2-9"), None);
        assert_eq!(
            Date::from_unix_seconds(1_792_886_400),
            Date::parse("2026-10-25").unwrap()
        );
        assert_eq!(
            Date::from_unix_seconds(0),
            Date::parse("1970-01-01").unwrap()
        );
    }

    #[test]
    fn the_default_ice_set_is_every_entry_of_the_first_two_operators() {
        let l = good();
        let urls: Vec<String> = l.default_ice_stun().iter().map(|s| s.url()).collect();
        assert_eq!(
            urls,
            [
                "stun:a.example.net:3478",
                "stun:a.example.net:443",
                "stun:b.example.net:3478"
            ]
        );
    }
}
