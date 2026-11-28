---
name: refactor-sweep
description: Periodic improvement sweep for GuardedSleep, steered by the code-health score (`just code-health`). The refactor-scout agent finds refactoring/unification/simplification candidates, hotspots first, the implementer agent implements each in its own worktree branch, the planner verifies and opens one draft PR per finding.
---

# Refactor sweep

You are the **planner** (AGENTS.md §10, model routing). Scouting runs in `refactor-scout`, implementation in `implementer`; you triage, verify, commit, and open PRs. Never touch the user's main checkout's working tree or branch; all work happens in separate worktrees based on `origin/main`.

Let `REPO` be the repository root and `SWEEP=$(date +%Y%m%d)`.

**Target metric:** the ai-health code-health score per component (`docs/contributing/code-health.md`; goal 10, floor 9.5). Every finding names the score change it is expected to bring, and no finding may lower any file or component score.

## 1. Prepare

1. `git -C $REPO fetch origin --prune`.
2. Ensure the label exists: `gh label create refactor-sweep --color 0E8A16 --description "Automated refactor sweep" 2>/dev/null || true`.
3. Open sweep PRs: `gh pr list --label refactor-sweep --state open --json number,title`. If **3 or more** are open, stop and report "backlog not reviewed, sweep skipped" — do not pile up PRs.
4. Last sweep date: creation date of the newest PR with label `refactor-sweep` (`gh pr list --label refactor-sweep --state all --limit 1 --json createdAt`), else 14 days ago.
5. Scout worktree: `git -C $REPO worktree add --detach $REPO/../.gs-sweep-$SWEEP-scout origin/main`.
6. Scores: in the scout worktree run `just _code-health-setup` and `just code-health --json > $REPO/../.gs-sweep-$SWEEP-health.json` (one JSON object per scanned path). Note each component's project score and the top 10 hotspots, ranked by `(10 − file score) × (commits touching the file in 90 days + 1)` (`git log --since=90.days --format= --name-only -- <file> | wc -l`). Compare with the scores in the last sweep PR's body, if any, for the trend.

## 2. Scout

Spawn `refactor-scout` (foreground) with: the scout worktree path, the last sweep date, the open PR titles, and the hotspot list with file scores and findings. Ask for at least 2 of the up to 5 findings on hotspot files. Wait for the ranked list.

## 3. Triage

Pick at most `3 − (open sweep PRs)` findings. Prefer findings with the largest expected score gain on hotspots. Drop any finding that, after your own quick look at the cited code, is wrong, behavior-changing, needs an OpenSpec change, touches §9 features, or would only dodge a check (AGENTS.md §5.5: no dropped types, cosmetic aliases or splits only to get under a minimum). Keep the dropped ones and the scout's "Needs decision" list for the report.

## 4. Implement each finding (sequentially)

For each picked finding with slug `<slug>`:

1. `BR=refactor/sweep-$SWEEP-<slug>`; `WT=$REPO/../.gs-sweep-$SWEEP-<slug>`; `git -C $REPO worktree add -b $BR $WT origin/main`; `just setup` in `$WT` (until wave 1 lands: `pnpm --dir $WT install --frozen-lockfile`).
2. Spawn `implementer` (foreground) with: the absolute path `$WT` (it must work only there), the finding's Brief, Files, and Acceptance, and "do not commit".
3. Verify yourself in `$WT`: read `git diff`, confirm it matches the brief and is behavior-preserving; run `just lint` (it includes the code-health gate) and `just test` and the finding's acceptance commands; a SKIPPED task is not a pass. Run `just code-health --changed` and re-scan the touched files: a finding that degrades any file, or misses its named score target by a wide margin, goes back to the implementer once, then is discarded.
4. On failure: give the implementer **one** follow-up with the failing output. If it still fails, discard (`git -C $REPO worktree remove --force $WT && git -C $REPO branch -D $BR`) and record it in the report.
5. On success: commit in `$WT` with the finding's Conventional Commit title (hooks must pass; never `--no-verify`), body with the "Why", and the commit attribution trailer. Push `git -C $WT push -u origin $BR`. Open a draft PR: `gh pr create --draft --base main --head $BR --label refactor-sweep --title "<title>" --body ...` following `.github/pull_request_template.md`, including Why, Files, the score before → after for each touched file and its component, checks run, "Roles applied: Technical Architect (planner), Critical Reviewer (diff review)", and the PR attribution line.
6. Remove the worktree (`git -C $REPO worktree remove $WT`); the branch stays on the remote.

## 5. Clean up and report

Remove the scout worktree and the health JSON. Report briefly: the component scores and their trend since the last sweep, PRs opened (links) with their score gains, findings dropped or failed with reason, and the "Needs decision" items formatted as an options checklist (`docs/contributing/decisions-and-roles.md`, "Presenting options") where a choice is involved. Never merge PRs and never push to `main`.
