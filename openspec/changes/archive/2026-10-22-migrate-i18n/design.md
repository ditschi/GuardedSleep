# Design

## Context

Sources: ADR-020 §7, ADR-021 §2, `user-interface.md` (multi-language section), AGENTS.md §5.2a. As built: `locales/{en,de,fr,es}.json`, `crates/i18n-gen` (`catalogue.rs`: key, argument and plural parity; `android.rs`; `web.rs` with a pseudo-locale), `scripts/lint-ui-literals.sh`, `scripts/cspell-i18n.sh`, the language setting in `SettingsScreen.kt`.

## Goals / Non-Goals

**Goals:** spec the built catalogue and checks. **Non-Goals:** the ADR-021 locale policy, which is not built yet.

## Decisions

- **Built is stricter than decided:** today every locale must have every key. ADR-021's rule (only `critical` keys must be complete, others fall back to English) needs `critical` tags first. The prepared change `expand-locales` will MODIFY "Shipped locales are complete" when the ~20 locales arrive.
- **Not specified:** the claim in `link-supervision` that the i18n check verifies `message_keys()` against `en.json`; today a core unit test only checks that the key list is complete. Noted in the backlog for wave 3.
- Design tokens (ADR-020 §7) are wave-3 tooling and go into the prepared changes, not this spec.

## Risks / Trade-offs

- None beyond the usual drift; the generator enforces the spec.
