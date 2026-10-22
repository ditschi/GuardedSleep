# Proposal

## Why

ADR-021 §2 (owner D2) ships about 20 AI-translated locales for reach, with the rule that an alarm is never in mixed languages. Today four locales ship with full key parity, there are no `critical` tags, no review markers and no RTL. Tier T0, wave 3.

## What Changes

- Locales: en (master), de, fr, es, it, pt, nl, pl, sv, da, cs, ro, zh-Hans, ja, ko, hi, ar, tr, uk, id.
- `critical` tags on alert, link-loss, connection-state and safety keys; a locale ships only when all critical keys are present, other keys fall back to English.
- `machineTranslated` / `reviewed` markers; the owner reviews German; review never gates shipping.
- Generator: length budgets on critical keys, `androidResources.localeFilters` with exactly the generated locales, an RTL pseudo-locale; screenshot and overflow tests for de (long) and ar (RTL).
- A check that every core `message_keys()` entry exists in `locales/en.json` (moved here from the backlog).

## Capabilities

### New Capabilities

### Modified Capabilities

- `i18n`: critical-key completeness replaces full parity; markers, RTL, locale filter, length budgets, message-key check.

## Impact

`locales/*.json`, `crates/i18n-gen`, Android Gradle config, Roborazzi and Playwright tests, `docs/backlog.md` (remove the message-key item).
