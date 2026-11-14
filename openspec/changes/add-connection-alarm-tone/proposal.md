# Proposal

## Why

The parent device sounds a fixed, code-generated tone when the connection to the baby device is lost: a soft tone every 10 s from escalation level 2 and a looping two-tone alarm from level 3 (`link-supervision` "Escalation feedback per device"; Android `TonePlayer`/`PcmSynth`, web `AlertPlayer`). Parents cannot change it. The owner asked on 2026-11-14 for a choice of alarm tone in the settings. Some parents find the current tone too shrill or too easy to sleep through, and a tone that matches the parent's ears is a cheap way to make sure a connection alarm actually wakes them.

Noise alerts are not affected: the child's own audio is the noise alert by design (`parent-alerts` "Noise alert chain"), and this change keeps it that way.

The change touches no architecture, security model or core concept. It adds a parent-device setting and moves the tone recipes into the core so both platforms sound the same and the "never silent" rule lives in one place. Tier **T0** (alpha).

## What Changes

- **Connection alarm tone setting** on the parent device (Android and browser): Settings › Alerts › "Connection alarm tone", with **five** built-in, code-generated tones. Each tone is a pair: a soft reminder for the "no connection" stage (level 2, every 10 s; also the single tone when the baby device was stopped on purpose) and a loud alarm for the "alarm" stage (level 3, looping). Today's tone stays the default.
- **Preview and test:** each tone can be previewed (soft, then loud, about 3 s); "Test connection alarm" plays the selected pair on the alarm stream through the chosen alert output for about 8 s, so the parent hears it at the real alarm volume.
- **Never silent:** there is no "None" option and no system or user ringtone. An unknown or unreadable stored value resolves to the default tone. Every tone meets a minimum loudness and pitch range, checked by unit tests in the core.
- **Tone recipes in the core:** the five recipes (sine segments with frequency, duration, amplitude and gaps) and the id → recipe resolution with fallback live in `guardedsleep-core`, exposed through `ffi` and `wasm`. Platforms only synthesize the samples and play them.
- **Copy** for the setting, tone names and the explanation that noise alerts still play the child's sound, in all locales.
- **Out of scope:** a separate noise-alert ringtone (contradicts `parent-alerts`), system or custom ringtones, per-alert-type choice of sound, vibration patterns and alert cool-downs (all stay `IDEA` in `docs/backlog.md`), any sound on the baby device, the broader settings redesign (separate change after the design review).

Persona impact (`docs/concept/personas.md`): serves Persona B ("Alltags-Eltern"), who hear the connection alarm most often and want a tone they wake up to without being startled, and Persona C on old devices with weak speakers, who can pick the tone that carries best. Persona A is unaffected (default tone). Trade-off: one more setting; it sits behind the Alerts group and the default needs no action.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `parent-alerts`: new requirements for the connection alarm tone: choice and storage, never silent, preview and test.
- `link-supervision`: the level 2 and level 3 feedback uses the chosen connection alarm tone.

## Impact

`crates/guardedsleep-core` (tone recipes, resolution, loudness tests), `crates/guardedsleep-{ffi,wasm}` (API snapshots), `apps/android` (`TonePlayer` synthesizes from recipes, `SettingsRepository`, settings row and picker, preview and test), `apps/web` (`AlertPlayer` synthesizes from recipes, settings storage next to the theme setting, settings row and picker), `apps/e2e`, `locales/*.json`, `docs/backlog.md` (narrow the "configurable alert behaviour" idea).
