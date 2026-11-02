# Design

## Context

Sources: ADR-020 §3 (fuzzing), §4 (Roborazzi goldens), §7 (design tokens), §11 (log canary, secrecy, size budgets, SECURITY.md, AGPL §13 link), AGENTS.md §7.6 (log canary must pass once added).

## Decisions

- **11 goldens instead of a full matrix**: 5 critical screens (connected, alert, reconnecting, link lost, mic denied) × {en 100 %, de 200 % font on a small phone} + 1 light-mode monitor screen.
- **Status never by colour alone** is enforced by tokens: every status token has an icon id and a message key.
- **Each gate gets a row in `docs/ci-gates.md`** and counts against the 12 h/month budget.
- **Discarded alternatives:** a full screenshot matrix (maintenance cost); Style Dictionary now (T2); OSS-Fuzz now (T2).

## Risks / Trade-offs

- Gate cost: the six-month no-catch rule removes gates that do not pay off.
