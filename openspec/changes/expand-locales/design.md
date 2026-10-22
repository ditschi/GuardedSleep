# Design

## Context

Sources: ADR-021 §2, ADR-020 §7, AGENTS.md §5.2a.

## Decisions

- **Reach over perfection** (owner D2): AI translation now, native-speaker review as an improvement path; Arabic proves RTL early.
- **Critical keys are complete or the locale does not ship**, so an alarm can never mix languages; non-critical keys may fall back.
- **Prompting rules**: keep placeholders and plural categories; keep alert copy short; the generator validates the output.
- **Discarded alternatives:** Fluent (second format next to ICU on Android); Android XML as the source (no web output); four locales only (smaller reach).

## Risks / Trade-offs

- Machine translation errors in safety copy: length budgets and `critical` tags keep alerts short and reviewable; German is owner-reviewed.
