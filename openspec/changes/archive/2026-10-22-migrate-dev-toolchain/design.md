# Design

## Context

Sources: ADR-020 §1–§5, §8, §9, §11, §12 (wave 1 as built), ADR-016 (test layers), ADR-007 (browser E2E), ADR-019 (language by layer, Rust as protocol source), ADR-015 (native clients on a shared Rust core), ADR-014 (licence), ADR-002 (superseded). As built: `justfile`, `scripts/lib/require.sh`, `mise.toml`/`mise.lock`, `lefthook.yml`, `.github/workflows/ci.yml`, `renovate.json`, `deny.toml`/`deny.core.toml`, `REUSE.toml`, `.betterleaks.toml`, `docs/ci-gates.md`.

## Goals / Non-Goals

**Goals:** spec the wave-1 toolchain and close the ADR folder except for the sections that become prepared changes.

**Non-Goals:** wave 2/3 tooling (façades, API snapshots, generate-twice hash, fuzzing, log canary, size budgets, Gradle modules, ktfmt in `fmt`, design tokens, Dev Container).

## Decisions

- **Architecture rules stay in AGENTS.md §5.1**, not in a spec: layers, "safety logic lives once in the core", "UI renders contract models only" are rules for contributors, not observable behaviour. ADR-015 and ADR-019 are archived; AGENTS.md keeps pointing at their archive entries.
- **Licence** (ADR-014): AGPL-3.0-or-later and the trademark policy live in `LICENSE` and `docs/legal/trademark.md`; the spec only covers the REUSE check.
- **ADR-002** is fully superseded (ADR-015, ADR-019, ADR-020) and archived as history.
- **ADR-012** (connection model) is archived with the prepared change `add-offline-lan`, and ADR-018, ADR-020 and ADR-021 with the prepared wave changes, because their remaining sections are decided but not built.

## Risks / Trade-offs

- Specifying CI details can make small CI changes look like spec changes. Mitigation: the spec states guarantees (one required check, pins, strict mode), not job names or step order.
