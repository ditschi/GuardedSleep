# Tasks

Branch `feat/connection-alarm-tone` from `origin/main`. Commit after each task; `just test-affected` after each task. Every implementation task is a brief for the `implementer` sub-agent; layer rules of AGENTS.md §5.1 apply (recipes and fallback in the core, synthesis and playback in the platform, UI renders contract models only, no literals in UI code).

## 1. Core (`crates/guardedsleep-core`, façades)

- [ ] 1.1 Module `alarm_tone`: `AlarmToneId` (stable string ids `classic`, `beacon`, `rising`, `chime`, `low`), `ToneSegment { partials: Vec<(freq_hz, amplitude)>, ms, fade_ms }` (empty partials = gap; peak = sum of partial amplitudes), `AlarmTone { id, soft: Vec<ToneSegment>, alarm: Vec<ToneSegment> }`, `alarm_tones()` and `resolve_alarm_tone(&str) -> AlarmTone` (unknown → `classic`), recipes per design §2; `classic` matches today's Android/web tones exactly. Optional `tsify` derive under the existing feature only. Verify: unit tests for resolution (each id, unknown, empty, 1 kB string), the limits of design §2 per recipe (soft peak ≤ 0.40 and ≤ 800 ms; alarm peak ≥ 0.70, cycle 0.8–1.6 s, ≥ 40 % sounding; 400–1 200 Hz fundamentals), unique ids, an insta snapshot of the table.
- [ ] 1.2 Façades: export `alarm_tones` and `resolve_alarm_tone` from `guardedsleep-ffi` and `guardedsleep-wasm`; `just api-snapshot`, review the diffs. Verify: `just api-check`.

## 2. Android (`apps/android`)

- [ ] 2.1 `PcmSynth.render(segments, rate)` replaces `softTone`/`alarmCycle`; `TonePlayer` takes the resolved `AlarmTone` (soft once, alarm loop) and adds `preview(tone)` and `test(tone)` with a stop; a running alarm switches recipe at the next cycle. Verify: JVM tests (sample counts per recipe, `classic` renders the same samples as the old functions, preview stops on `stop()`, an alert stops a preview).
- [ ] 2.2 `SettingsRepository.connectionAlarmTone` (string id, read through `resolve_alarm_tone`), contract field on the parent settings model and the monitor's tone source. Verify: JVM test with an unknown stored value.
- [ ] 2.3 UI: Settings › Alerts row `settings_alarm_tone` with the current name; picker screen `alarm_tone_picker` (radio rows `alarm_tone_<id>`, play buttons `alarm_tone_preview_<id>`, explanation, `alarm_tone_test` / stop). Parent role only. Verify: Robolectric tests; Roborazzi baselines recorded in the pinned image (report `SKIPPED` without it).

## 3. Web (`apps/web`)

- [ ] 3.1 `AlertPlayer` plays segments of the resolved recipe (soft every 10 s, alarm loop, peer-stopped soft tone, preview, test); the stored id lives in `localStorage` next to the theme setting and is read through `resolve_alarm_tone`. Verify: Vitest (segments scheduled per recipe, garbage stored value → `classic`, nothing sounds before the sound gate).
- [ ] 3.2 Settings row and picker with the same test ids as Android. Verify: `svelte-check --fail-on-warnings`, Vitest; Playwright spec selecting a tone and running a preview after the gate (`just e2e`).

## 4. Copy and docs

- [ ] 4.1 Copy keys (setting label, five tone names, explanation, preview, test, stop) in `locales/en.json` and all locales with `machineTranslated` markers. Verify: `just gen`, `just lint-text`, literal lint.
- [ ] 4.2 `docs/backlog.md`: note in the "configurable alert behaviour" idea that the connection alarm tone choice is built. Verify: `just lint-text`.

## 5. Verification

- [ ] 5.1 Full `just lint` and `just test`; `just e2e`; manual check on one Android device that each tone is clearly audible at mid alarm volume on the speaker. Verify: logs; SKIPPED tasks listed in the PR.
