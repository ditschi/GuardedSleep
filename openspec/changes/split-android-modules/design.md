# Design

## Context

Source: ADR-020 §4 (build, formatting and lint, release build), AGENTS.md §5.1 (UI renders contract models only; in Android only `platform` may depend on `core-bindings`).

## Decisions

- **Module boundaries enforce the layers**: a UI import of an FFI type becomes a compile error instead of a review finding.
- **Manual constructor DI**, no Hilt.
- **Discarded alternatives:** one module with lint rules (violations found late); Hilt (annotation processing, harder for agents to follow).

## Risks / Trade-offs

- The ktfmt reformat touches about 72 files; land it as its own commit to keep blame readable.
