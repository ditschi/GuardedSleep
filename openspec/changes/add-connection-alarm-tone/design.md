# Design

Roles applied: Product Owner, Technical Architect, UX, Security/Privacy (no findings: a local, non-secret preference). The Critical Reviewer gate does not apply (no architecture, scope, security-model or core-concept change); a short critical pass is in §6.

## Context

- **Android:** `TonePlayer` (`apps/android/platform/.../alerts/TonePlayer.kt`) plays `PcmSynth.softTone` (660 Hz, 450 ms, amplitude 0.35) once and loops `PcmSynth.alarmCycle` (880 Hz / 660 Hz, 400 ms each, 200 ms gaps, amplitude 0.8) as `AudioTrack` on `USAGE_ALARM`. The soft tone also sounds once on `peer_stopped`.
- **Web:** `AlertPlayer` (`apps/web/src/lib/platform/audio.ts`) plays the same tones with an `OscillatorNode` behind the sound gate; `tonePlanForLink` maps the link snapshot to "soft every 10 s" or "alarm loop".
- The recipes are duplicated in Kotlin and TypeScript today and only agree by hand.
- Parent settings live in `SettingsRepository` (Android) and, for the theme, in `localStorage` (web). Neither is pairing material.

## Decisions

### 1. Where the tone recipes live

- **Option A — recipes and resolution in the core (recommended).** A pure-data table `alarm_tones()` and `resolve_alarm_tone(id: &str) -> AlarmTone` (unknown → default) in `guardedsleep-core`; a recipe is a list of segments `{ freq_hz, ms, amplitude, fade_ms }` and gaps, separately for `soft` and `alarm`.
  - Pros: one source for both platforms (no drift); the "never silent" fallback and the loudness floor are tested once; fits "safety logic lives once in the core"; pure data, so sans-IO and cargo-deny are unaffected.
  - Cons: grows the binding surface (two functions, two records); the API snapshots change.
- **Option B — duplicate recipes in Kotlin and TypeScript with a shared golden JSON test.**
  - Pros: no binding change.
  - Cons: two implementations of the fallback rule; drift is caught only if the golden test is kept in sync.
- **Option C — `data/alarm-tones.json` and a generator like `public-lists`.**
  - Pros: data editable without Rust.
  - Cons: a new generator and lint for five constant records; the fallback rule still sits in two platforms.
- **Recommendation:** A.

### 2. The five tones

Stable ids (stored), localized names (copy). Each has a `soft` (≤ 0.40 peak, once, ≤ 800 ms) and an `alarm` cycle (≥ 0.70 peak, looping, 0.8–1.6 s per cycle, at least 40 % of the cycle sounding). Fundamentals stay between 400 and 1 200 Hz, which carry on small speakers and stay audible with age-related high-frequency loss.

| id                  | Name (en) | Soft                                           | Alarm cycle                                                          |
| ------------------- | --------- | ---------------------------------------------- | -------------------------------------------------------------------- |
| `classic` (default) | Classic   | 660 Hz, 450 ms                                 | 880 / 660 Hz alternating, 400 ms each, 200 ms gaps (today's alarm)   |
| `beacon`            | Beacon    | 523 Hz, 600 ms, long fade                      | three 1 000 Hz pulses of 150 ms, 100 ms apart, then 500 ms pause     |
| `rising`            | Rising    | 440 → 660 Hz, 2 × 250 ms                       | 523 / 659 / 784 / 1 047 Hz, 200 ms each, 400 ms pause                |
| `chime`             | Chime     | 784 Hz + 1 175 Hz (soft partial), 700 ms decay | 784 / 988 / 1 175 Hz bell-like, 250 ms each with decay, 450 ms pause |
| `pulse`             | Pulse     | 392 Hz double pulse, 2 × 150 ms                | 740 Hz, 6 × 100 ms on / 60 ms off, 400 ms pause                      |

Exact values may be tuned during implementation within the stated limits; the limits are the test.

### 3. Setting, preview and test (UX)

- Settings › Alerts › "Connection alarm tone" shows the current tone name. The picker lists the five tones as radio rows with a play button each, and states above the list: "For a lost connection. Noise alerts always play your child's own sound."
- Preview: soft once, then one alarm cycle twice, on the alarm stream through the chosen alert output. Only one preview at a time; leaving the screen stops it.
- "Test connection alarm" plays the soft tone, then the alarm for about 8 s; a Stop button ends it. A real alert stops any preview or test at once.
- Available only on the parent role. The browser parent needs the sound gate (one tap) as for every tone.
- While monitoring, the setting is reachable from the monitor via Settings; a change takes effect at the next tone (a running alarm switches at its next cycle).

### 4. Storage

Parent-device preference `connection_alarm_tone` (string id): Android `SettingsRepository`, web `localStorage` next to the theme setting (not pairing material, so `localStorage` is allowed by AGENTS.md §7.2). The stored value is passed through `resolve_alarm_tone` on every read.

### 5. Testing

- Core: every tone resolves; unknown, empty and overlong ids resolve to `classic`; each recipe meets the loudness, duty and frequency limits; ids are unique and stable (snapshot).
- Android JVM: `TonePlayer` renders a recipe into the expected sample count; the chosen id reaches the player; preview stops on dispose.
- Web Vitest: `AlertPlayer` schedules the oscillator segments of the chosen recipe; fallback on a garbage stored value.
- E2E (Playwright): the picker changes the stored id and plays a preview after the sound gate.
- Copy: names and explanation in all locales; the explanation and the setting label are not critical keys (alerts themselves are unchanged).

### 6. Critical pass

- _Could a tone be too quiet to wake someone?_ The loudness floor is enforced in the core tests, and the alarm volume of the device stays the ceiling as today (`parent-alerts` "Alert output and gain"). The readiness check already warns when the alarm volume is low.
- _Could a broken setting silence the alarm?_ No: there is no silent id and resolution always yields a tone.
- _Does a choice add confusion with the noise alert?_ The picker states that noise alerts play the child's sound; the setting name says "connection".

## Discarded alternatives

- **System ringtone picker (`RingtoneManager`).** A ringtone can be deleted, be silent, point to an unavailable file or play on the wrong stream; that would make a silent alarm possible, and the browser has no equivalent.
- **"None" option.** Would allow a silent connection alarm; the vibration and full-screen alert alone are not enough at level 3 (`failure-model`).
- **Bundled audio files.** Larger APK and web bundle, licensing of sound assets, and no benefit over synthesized tones for short alarm patterns.
