# Tasks

Branch `ci/dev-apk-prerelease`. Tooling only (`skip_specs: true`).

## 1. Workflow

- [ ] 1.1 `dev-apk` job in `.github/workflows/android-native.yml` as design §5: download `guardedsleep-alpha-debug-apk`, re-sign with the dev key (design §1, §3), verify, write notes with commit, date, test-build warning and SHA-256, publish to the rolling `android-dev` pre-release (design §2), delete the key in an `always()` step. SHA-pinned actions, `permissions` on the job only, secrets passed through `env:` (never interpolated into `run:`). Verify: `just lint-workflows`.
- [ ] 1.2 Fail with a clear message naming the missing secret when the `dev-apk` environment is not set up. Verify: read the step; `just lint-workflows`.

## 2. Docs

- [ ] 2.1 `docs/ci-gates.md`: one row for `android-native` / `dev-apk` (protects: testers get the gate-tested build at a stable link that updates in place; runbook: missing secret, re-run from the `gate` artifact, key rotation means one uninstall). Verify: `just lint-text`.
- [ ] 2.2 `docs/contributing/` (the page that covers Android installs): link to the `android-dev` pre-release, install steps, the dev-versus-release key note, and the owner steps to create the keystore and the `dev-apk` environment. Verify: `just lint-text`.

## 3. Owner

- [ ] 3.1 Owner: create the keystore offline and the `dev-apk` environment (branch `main`) with the three secrets of design §3.
- [ ] 3.2 After merge: the first `main` push touching Android paths (or a dispatch) publishes `android-dev`; install it on a device, then install the next one over it and check the pairing survives.
