//! Connection alarm tones of the parent device (`parent-alerts` "Connection alarm tone").
//!
//! Pure data: each tone is a recipe of sine segments that the platforms synthesize and play
//! (Android `PcmSynth`, web `AlertPlayer`). Recipes and the id resolution live here so both
//! platforms sound the same and the "never silent" rule is stated and tested once: there is no
//! silent id, and any unknown stored value resolves to the default tone.

use serde::{Deserialize, Serialize};

/// Stable, stored ids. The string form is persisted by the platforms: never rename one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlarmToneId {
    Classic,
    Low,
    Beacon,
    Rising,
    Chime,
}

impl AlarmToneId {
    /// The default tone: today's sound, and the fallback for any unknown stored value.
    pub const DEFAULT: AlarmToneId = AlarmToneId::Classic;

    /// Picker order (design §3).
    pub const ALL: [AlarmToneId; 5] = [
        AlarmToneId::Classic,
        AlarmToneId::Low,
        AlarmToneId::Beacon,
        AlarmToneId::Rising,
        AlarmToneId::Chime,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            AlarmToneId::Classic => "classic",
            AlarmToneId::Low => "low",
            AlarmToneId::Beacon => "beacon",
            AlarmToneId::Rising => "rising",
            AlarmToneId::Chime => "chime",
        }
    }

    pub fn parse(id: &str) -> Option<AlarmToneId> {
        AlarmToneId::ALL.into_iter().find(|t| t.as_str() == id)
    }
}

/// One sine component of a segment.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct TonePartial {
    pub freq_hz: f64,
    /// Linear amplitude, 0..1 of full scale. The peak of a segment is the sum of its partials.
    pub amplitude: f64,
}

/// A stretch of sound, or of silence when `partials` is empty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct ToneSegment {
    pub partials: Vec<TonePartial>,
    pub ms: u32,
    /// Fade in and fade out, so a tone never clicks.
    pub fade_ms: u32,
}

impl ToneSegment {
    /// Sum of the partial amplitudes; 0 for a gap.
    pub fn peak(&self) -> f64 {
        self.partials.iter().map(|p| p.amplitude).sum()
    }
}

/// A tone: a soft reminder (level 2, every 10 s, and a baby device stopped on purpose) and one
/// cycle of the looping alarm (level 3). The alarm cycle includes its trailing pause.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "tsify", derive(tsify::Tsify))]
pub struct AlarmTone {
    /// One of the stable ids (`AlarmToneId::as_str`).
    pub id: String,
    pub soft: Vec<ToneSegment>,
    pub alarm: Vec<ToneSegment>,
}

impl AlarmTone {
    /// Total length of the segments in ms.
    pub fn soft_ms(&self) -> u32 {
        self.soft.iter().map(|s| s.ms).sum()
    }

    /// Length of one alarm cycle in ms.
    pub fn alarm_cycle_ms(&self) -> u32 {
        self.alarm.iter().map(|s| s.ms).sum()
    }
}

fn note(freq_hz: f64, amplitude: f64, ms: u32, fade_ms: u32) -> ToneSegment {
    ToneSegment {
        partials: vec![TonePartial { freq_hz, amplitude }],
        ms,
        fade_ms,
    }
}

/// A note with a quieter overtone; `main` and `over` are `(freq_hz, amplitude)`.
fn note2(main: (f64, f64), over: (f64, f64), ms: u32, fade_ms: u32) -> ToneSegment {
    ToneSegment {
        partials: vec![
            TonePartial {
                freq_hz: main.0,
                amplitude: main.1,
            },
            TonePartial {
                freq_hz: over.0,
                amplitude: over.1,
            },
        ],
        ms,
        fade_ms,
    }
}

fn gap(ms: u32) -> ToneSegment {
    ToneSegment {
        partials: Vec::new(),
        ms,
        fade_ms: 0,
    }
}

/// The recipe of one tone. Limits (design §2, enforced by the tests below): soft peak <= 0.40 and
/// <= 800 ms; alarm peak >= 0.70, cycle 0.8 to 1.6 s with at least 40 % sounding; every frequency
/// between 400 and 1 200 Hz.
fn recipe(id: AlarmToneId) -> AlarmTone {
    let (soft, alarm) = match id {
        // Today's tones, sample for sample (Android PcmSynth, web AlertPlayer before this change).
        AlarmToneId::Classic => (
            vec![note(660.0, 0.35, 450, 80)],
            vec![
                note(880.0, 0.8, 400, 20),
                gap(200),
                note(660.0, 0.8, 400, 20),
                gap(200),
            ],
        ),
        AlarmToneId::Low => (
            vec![
                note2((440.0, 0.28), (880.0, 0.10), 200, 40),
                gap(100),
                note2((440.0, 0.28), (880.0, 0.10), 200, 40),
            ],
            vec![
                note2((440.0, 0.55), (880.0, 0.25), 300, 30),
                gap(150),
                note2((523.0, 0.55), (1046.0, 0.25), 300, 30),
                gap(150),
            ],
        ),
        AlarmToneId::Beacon => (
            vec![note(523.0, 0.35, 600, 200)],
            vec![
                note(1000.0, 0.8, 150, 15),
                gap(100),
                note(1000.0, 0.8, 150, 15),
                gap(100),
                note(1000.0, 0.8, 150, 15),
                gap(400),
            ],
        ),
        // Steps through notes, never glides.
        AlarmToneId::Rising => (
            vec![note(440.0, 0.35, 250, 40), note(660.0, 0.35, 250, 40)],
            vec![
                note(523.0, 0.8, 200, 20),
                note(659.0, 0.8, 200, 20),
                note(784.0, 0.8, 200, 20),
                note(1047.0, 0.8, 200, 20),
                gap(400),
            ],
        ),
        // Long symmetric fades stand in for a bell's decay.
        AlarmToneId::Chime => (
            vec![note2((784.0, 0.28), (1175.0, 0.10), 700, 250)],
            vec![
                note2((784.0, 0.55), (1175.0, 0.25), 250, 100),
                note2((988.0, 0.55), (1175.0, 0.25), 250, 100),
                note2((1175.0, 0.55), (784.0, 0.25), 250, 100),
                gap(450),
            ],
        ),
    };
    AlarmTone {
        id: id.as_str().to_owned(),
        soft,
        alarm,
    }
}

/// All tones in picker order (Classic, Low, Beacon, Rising, Chime).
pub fn alarm_tones() -> Vec<AlarmTone> {
    AlarmToneId::ALL.into_iter().map(recipe).collect()
}

/// The tone for a stored id. An unknown, empty or overlong value gives the default tone, so a
/// broken setting can never silence the connection alarm.
pub fn resolve_alarm_tone(id: &str) -> AlarmTone {
    recipe(AlarmToneId::parse(id).unwrap_or(AlarmToneId::DEFAULT))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_id_resolves_to_its_own_tone() {
        for id in AlarmToneId::ALL {
            assert_eq!(resolve_alarm_tone(id.as_str()).id, id.as_str());
        }
    }

    #[test]
    fn unknown_values_resolve_to_classic() {
        let long = "x".repeat(1024);
        for bad in [
            "",
            "none",
            "Classic",
            " classic",
            "silent",
            "../x",
            long.as_str(),
        ] {
            assert_eq!(resolve_alarm_tone(bad).id, "classic", "{bad:?}");
        }
    }

    #[test]
    fn ids_are_unique_stable_and_in_picker_order() {
        let ids: Vec<String> = alarm_tones().into_iter().map(|t| t.id).collect();
        assert_eq!(ids, ["classic", "low", "beacon", "rising", "chime"]);
    }

    #[test]
    fn soft_tones_are_gentle_and_short() {
        for t in alarm_tones() {
            let peak = t.soft.iter().map(ToneSegment::peak).fold(0.0, f64::max);
            assert!(
                peak > 0.0 && peak <= 0.40 + 1e-9,
                "{} soft peak {peak}",
                t.id
            );
            assert!(t.soft_ms() <= 800, "{} soft {} ms", t.id, t.soft_ms());
        }
    }

    #[test]
    fn alarms_are_loud_with_a_short_cycle_that_mostly_sounds() {
        for t in alarm_tones() {
            let peak = t.alarm.iter().map(ToneSegment::peak).fold(0.0, f64::max);
            assert!((0.70..=1.0).contains(&peak), "{} alarm peak {peak}", t.id);
            let cycle = t.alarm_cycle_ms();
            assert!((800..=1600).contains(&cycle), "{} cycle {cycle} ms", t.id);
            let sounding: u32 = t
                .alarm
                .iter()
                .filter(|s| !s.partials.is_empty())
                .map(|s| s.ms)
                .sum();
            assert!(
                sounding * 100 >= cycle * 40,
                "{} sounds {sounding}/{cycle}",
                t.id
            );
        }
    }

    #[test]
    fn frequencies_stay_in_the_audible_band() {
        for t in alarm_tones() {
            for s in t.soft.iter().chain(&t.alarm) {
                assert!(
                    s.fade_ms * 2 <= s.ms.max(1),
                    "{} fade longer than segment",
                    t.id
                );
                for p in &s.partials {
                    assert!(
                        (400.0..=1200.0).contains(&p.freq_hz),
                        "{} {} Hz",
                        t.id,
                        p.freq_hz
                    );
                    assert!(p.amplitude > 0.0 && p.amplitude <= 1.0);
                }
            }
        }
    }

    #[test]
    fn classic_is_todays_tone() {
        let t = resolve_alarm_tone("classic");
        assert_eq!(t.soft, vec![note(660.0, 0.35, 450, 80)]);
        assert_eq!(t.alarm_cycle_ms(), 1200);
        assert_eq!(t.alarm[0].partials[0].freq_hz, 880.0);
        assert_eq!(t.alarm[2].partials[0].freq_hz, 660.0);
    }

    #[test]
    fn gaps_have_no_partials_and_every_tone_has_a_gap_in_its_alarm() {
        for t in alarm_tones() {
            assert!(t.alarm.iter().any(|s| s.partials.is_empty()), "{}", t.id);
        }
    }

    #[test]
    fn table_snapshot() {
        let json = serde_json::to_string_pretty(&alarm_tones()).expect("serializable");
        insta::assert_snapshot!(json);
    }
}
