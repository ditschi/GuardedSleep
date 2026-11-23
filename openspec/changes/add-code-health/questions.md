# Open questions

Owner questions for `add-code-health`. Answered items stay with the answer and date, so the record is complete.

## Open

1. **Gate strictness for existing files.** Proposal: a new file must score ≥ 9.5 (goal 10); a file already below that must not get worse (ai-health `delta` with tolerance 0), and its component score must not drop. Alternative: every touched file must reach 9.5 at once, which blocks small changes to hotspots such as `link.rs` until they are refactored. Recommendation: the proposal.
2. **Splitting the large safety files.** May `crates/guardedsleep-core/src/link.rs` (4 503 code lines, score 1.02) and `apps/android/platform/…/link/LinkController.kt` be split into modules, and may large inline Rust test modules move into their own files? Vectors, traces and screenshot baselines stay unchanged. Recommendation: yes, one file per pull request, `link.rs` last.
3. **Pull request split.** First one tooling pull request (ai-health pin, profiles, `just code-health`, gate, docs, `world.rs` as the first refactoring), then refactoring pull requests per component. Recommendation: yes.
4. **Weekly refactor sweep.** After the tooling lands on `main`: fix the stale `pnpm run lint` / `pnpm test` in the scheduled task, add the score as target metric (design §6) and re-enable the task (disabled since 2026-11-09)? Recommendation: yes.
5. **Inline Rust test modules in the score.** ai-health leaves `#[cfg(test)] mod tests` out by default; the profile scores them (`score_test_modules = true`), following the standard that tests count. Rust: 4.04 with them, 4.23 without. Keep them in? Recommendation: yes.
6. **Primitive Obsession at 30 %.** Almost every Rust file triggers it (indices, handles, epochs as `usize`/`u32`/`u64`). Fixing it means newtypes across the crates. Keep 30 % as the target, or treat it separately from the other smells while the rest is cleaned up? Recommendation: keep it, fix it file by file together with the other refactorings.
7. **Skills.** Which skills from the shortlist (and the extended review, in progress) to install project-locally?
8. **Kotlin pin.** When the Kotlin adapter exists in `ditschi/ai-health`: pin the fork commit until the upstream merge, or wait for an upstream release?

## Answered

- ai-health instead of the own structure pass and jscpd, for Rust, TypeScript and later Kotlin; profile "strict"; delta tolerance 0 plus a component score comparison base → head (≤ 0.01 drop); Svelte as an ESLint report without score; detekt at most as a transitional linter (owner, 2026-11-23, relayed by the coordination session).
- Install ai-health and start with `crates/guardedsleep-sim/src/world.rs` (owner, in this session, 2026-11-23).
- ESLint despite ADR-020 "No ESLint at T0": only for the Svelte report, no style rules (follows from the Svelte decision).
- Per-component scores instead of one repo score (owner, 2026-11-23).
