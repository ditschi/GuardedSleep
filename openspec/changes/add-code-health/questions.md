# Open questions

Owner questions for `add-code-health`. Answered items stay with the answer and date, so the record is complete.

## Open

1. **Rust test modules inside `src` and Primitive Obsession in tests** (owner decision 2026-11-24: Primitive Obsession off in test code). ai-health has one config per file and only exclude globs without negation. Options: (a) keep `score_test_modules = true` (answer to the former question 5), so Primitive Obsession counts in inline tests unless a file-wide suppression with a reason is set; (b) switch to `false`, so inline tests are not scored at all. Recommendation: (a).
2. **Tests in `apps/web/src` (`*.test.ts`).** A test-only profile cannot be expressed with exclude globs. Proposal: leave them unscored for now, give `apps/e2e` and `crates/*/tests` a test profile (`primitive_obsession = 0`), and ask upstream for an include option. Recommendation: yes.

## Answered

Owner, in this session, 2026-11-24 ("all recommendations"):

- Gate strictness: a new file ≥ 9.5 (goal 10); existing files must not get worse (delta tolerance 0); component scores must not drop.
- `link.rs` and `LinkController.kt` may be split into modules, and large inline Rust test modules may move into their own files; one file per pull request, `link.rs` last.
- Pull requests: first the tooling pull request (with `world.rs`), then refactoring pull requests per component.
- Refactor sweep: after the tooling lands, fix the scheduled task's commands, add the score as target metric and re-enable it.
- Inline Rust test modules are scored (`score_test_modules = true`); see open question 1 for its interaction with the newer Primitive Obsession decision.
- Primitive Obsession stays at 30 % in production code, fixed file by file with real types (never by removing type annotations).
- Skills: the recommended selection is installed project-locally (systematic-debugging, verification-before-completion, property-based-testing, cargo-fuzz, svelte-core-bestpractices, compose-ui-testing-patterns, android-intent-security, android-permissions-security).
- Kotlin pin: wait for an upstream ai-health release with the Kotlin adapter; no fork pin.
- Push, pull request and merge of `feat/code-health` after review and green CI.

Earlier:

- ai-health instead of the own structure pass and jscpd, for Rust, TypeScript and later Kotlin; profile "strict"; delta tolerance 0 plus a component score comparison base → head (≤ 0.01 drop); Svelte as an ESLint report without score; detekt at most as a transitional linter (owner, 2026-11-23, relayed by the coordination session).
- Install ai-health and start with `crates/guardedsleep-sim/src/world.rs` (owner, in this session, 2026-11-23).
- ESLint despite ADR-020 "No ESLint at T0": only for the Svelte report, no style rules (follows from the Svelte decision).
- Per-component scores instead of one repo score (owner, 2026-11-23).
