# Tasks

## 1. Spec

- [x] 1.1 Write the `pairing` delta spec from the sources listed in design.md
- [x] 1.2 Check every requirement against the core and Android code

## 2. Sources

- [x] 2.1 Add a "Migrated to OpenSpec" note to ADR-003 (§3, §4), ADR-004 (all except §4) and ADR-017 (§1)
- [x] 2.2 Remove the migrated pairing sections from `connectivity-pairing.md` and link the spec
- [x] 2.3 Move the pairing `IDEA`/`DEFERRED` items and the follow-ups from design.md to `docs/backlog.md`
- [x] 2.4 Archive ADR-001 verbatim as `openspec/changes/archive/2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/` with a `proposal.md`, and update `docs/decisions/README.md`

## 3. Verify

- [x] 3.1 `openspec validate --all --strict` and `openspec validate --archived`
- [x] 3.2 Prettier, typos and REUSE on the changed files
