# Proposal

## Why

All user-visible copy, including alarms, must come from one catalogue so that no screen shows hard-coded or mixed-language text. ADR-020 §7 and ADR-021 §2 decide the target (about 20 AI-translated locales with critical-key completeness), but today four complete locales are built. This change records the built behaviour as the `i18n` spec (tier T0, docs only).

## What Changes

- New capability spec `i18n`: one catalogue with English as master, ICU subset, full key parity across the four shipped locales, generated Android and web output, the literal lint, spellchecking and the in-app language choice.
- Decided but not built (about 20 locales, `critical` tags, review markers, RTL and pseudo-RTL, `localeFilters`, length budgets, design tokens) becomes part of the prepared wave-3 change `expand-locales`; Weblate is `PLANNED` (T1).
- ADR-020 and ADR-021 are archived after `dev-toolchain` and the prepared changes cover their remaining sections.

## Capabilities

### New Capabilities

- `i18n`: where user-visible copy lives, which languages ship and how the catalogue is checked and turned into platform resources.

### Modified Capabilities

## Impact

Docs only: `openspec/specs/i18n/`, notes in ADR-020 and ADR-021, `user-interface.md`, `docs/backlog.md`.
