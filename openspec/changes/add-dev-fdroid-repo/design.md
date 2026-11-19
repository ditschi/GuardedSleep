# Design

## Context

`add-dev-apk-prerelease` publishes the gate-tested debug APK, re-signed with the dev key, to the rolling pre-release `android-dev` (`dev-apk` job in `android-native.yml`). The debug and release builds share `org.guardedsleep.app.alpha`, and `versionCode` comes only from `version.properties` (`AndroidVersion.versionCode`, 100101 for `0.1.0-alpha.1`). `scripts/gradle.sh` already passes the commit (`GS_GIT_COMMIT`, 8 hex digits) and a display time (`GS_GIT_COMMIT_TIME`) into the build. The dev APK is about 27 MB (arm64 only).

Everything here is build and CI tooling. No runtime layer (UI, State, Platform, Bindings, Core) changes, apart from resources in the debug source set. Privacy guardrails (AGENTS.md §7): GitHub Pages only serves APKs and an index, like GitHub Releases does today; nothing enters the media, alert or signaling path, and there is no telemetry. The repository key lives only in the `dev-repo` environment and the dev APK key only in `dev-apk` (§7.7).

## 1. Which builds get the dev identity

**Option A — every debug build is `org.guardedsleep.app.dev` (recommended).** An `androidComponents.onVariants` hook in `app/build.gradle.kts` sets the debug variant's `applicationId` and version; `src/debug/` overrides label and icon.

- Pros: the APK the gate tests is exactly the one that is published (re-signing stays as in `add-dev-apk-prerelease` §1); one rule, no CI-only switch; local debug builds also order by commit.
- Cons: a local debug build (local debug key) and the CI dev build (dev key) share an id, so switching between them on one phone means one uninstall. That is the same today with `.alpha`, so it is no regression.

**Option B — the dev identity only when CI sets a flag.**

- Pros: local builds stay apart from the CI dev build.
- Cons: the gate would test a different id than local runs; a second code path in the build; easy to publish the wrong variant.

**Option C — a `dev` product flavor.**

- Pros: explicit in Gradle.
- Cons: doubles every variant (`devDebug`, `devRelease`, …), all task names in the scripts and CI change; too heavy for one id and one version rule.

Recommendation: A. The debug manifest's action strings (`org.guardedsleep.app.alpha.debug.*`) are plain strings, not tied to the package, and stay as they are; only the package id in `scripts/poc/gate.sh`, `scripts/android-install.sh` and the Maestro flows changes.

## 2. Dev version

`versionName = <base> + sep + "dev." + yyyyMMddHHmmss(UTC committer time) + "+" + sha8`, where `sep` is `.` if the base already has a pre-release part (`0.1.0-alpha.1.dev.20261119143012+abc12345`) and `-` otherwise (`0.1.0-dev.…`). Both are valid SemVer; the timestamp is a numeric identifier without a leading zero.

`versionCode = committerEpoch − 1_767_225_600` (2026-01-01T00:00:00Z). That stays below the Play limit of 2 100 000 000 until about 2092; the raw Unix time would reach it in 2036.

- **Committer time, not author time:** "Rebase and merge" and "Squash and merge" set the committer time to the moment of the merge, so it rises along `main`. The author time of an old branch can be older than the previous build, and F-Droid clients would never offer it.
- `scripts/gradle.sh` passes a new `GS_GIT_COMMIT_EPOCH` (`git log -1 --format=%ct`); the display time and the version both come from the commit, so the build stays reproducible. Without the variable (plain `./gradlew` in an IDE) the debug build gets `versionCode` 1 and `versionName` `<base>…dev.0+unknown`.
- The pure function lives next to `AndroidVersion` in build-logic (`AndroidVersion.devVersion(base, epoch, sha)`) with unit tests; release builds keep `versionCode(versionName)` unchanged. It validates its inputs: the epoch must be digits only, and the commit is cut to its first 8 characters and must be lowercase hex (`git rev-parse --short=8` may return more when 8 are ambiguous, and callers can set `GS_GIT_COMMIT` freely); invalid input falls back like a missing one, with a Gradle warning.
- Two commits merged in the same second (a rebase-merge of several commits) share a timestamp, but only the tip of a push is built and published.

## 3. Repository hosting

**Option A — GitHub Pages through `actions/upload-pages-artifact` + `actions/deploy-pages` (recommended).**

- Pros: stable HTTPS URL; no branch whose history grows with every APK; Pages environments are restricted to `main` by default.
- Cons: each deployment replaces the whole site, so older APKs must be put back into each build (§4); a future stable own repository on Pages must be assembled into the same site.

**Option B — a `gh-pages` branch.**

- Pros: older APKs stay without extra work.
- Cons: about 27 MB per build in git history forever; needs a push token or `contents: write` on another branch.

**Option C — APKs as release assets, index somewhere else.**

- Pros: no APK on Pages.
- Cons: F-Droid clients expect the APKs under the repository URL; mirrors complicate the index for little gain.

Recommendation: A. Site layout: `/fdroid/dev/repo/` (index-v1, index-v2/entry, the APKs, icons, and the `index.html` with QR code that `fdroid update` writes itself). The `android-native` workflow **owns the whole Pages site**: every deploy replaces it. Any future Pages content (an own stable repository, a web demo) must be assembled into the same deploy; `docs/ci-gates.md` and AGENTS.md say so. IzzyOnDroid (planned for beta) needs nothing here.

## 4. Keeping the last three builds

APKs in `repo/` are named `org.guardedsleep.app.dev_<versionCode>.apk`, so builds never overwrite each other. Before `fdroid update`, the repository job:

1. downloads the live `entry.jar` and verifies it with the JDK tools, not with fdroidserver from the same venv: `jarsigner -verify` must pass with exactly one signer and no unsigned entries (without `-strict`: fdroid's self-signed `entry.jar` always trips its chain and expiry warnings, so trust comes from the pinned fingerprint instead), and the SHA-256 of its signer certificate (`keytool -printcert -jarfile`) must equal the pinned fingerprint `GS_DEV_REPO_FINGERPRINT` (a public value in the workflow `env:`, set by the owner from task 2.5); then `entry.json` inside it gives the SHA-256 of `index-v2.json`, which is downloaded and checked; on any failure the job starts fresh;
2. takes from that verified index the up to two newest APKs whose `versionCode` is **lower** than the new one (a re-run or dispatch of the same commit therefore never yields two entries with one code; fdroidserver does not deduplicate);
3. downloads them, checks the SHA-256 listed in the verified index and that `apksigner verify --print-certs` shows the dev APK certificate;
4. adds the new APK.

If the site does not exist yet or any step fails, the repository starts with only the new build and the job logs a warning, so a lost site heals itself. `archive_older: 0` turns the archive off: at most three APKs are fed in anyway, and an archive index would point to files that are not deployed. A key rotation means testers re-add the repository with the new fingerprint (new link), and the first run after it starts fresh. The Pages CDN may serve an index up to about 10 minutes old; harmless, because the workflow-level concurrency (`android-native-${{ github.ref }}`, no cancel) serialises `main` runs, so the previous deploy has finished, and at worst one older build drops out early.

Discarded: keeping old APKs as versioned assets of `android-dev` (clutters the release and the asset list that testers see); trusting the unsigned `index-v1.json` for the hashes (same origin as the APKs, proves nothing).

## 5. Repository key

A separate PKCS12 RSA 4096 key (fdroidserver cannot sign `index-v1.jar` with an EC key), secrets `GS_DEV_REPO_KEYSTORE_B64`, `GS_DEV_REPO_KEYSTORE_PASSWORD`, `GS_DEV_REPO_KEY_ALIAS` in a new environment `dev-repo` (deployments from `main` only), created by `scripts/dev-repo-key.sh` (same safeguards as `dev-apk-key.sh`: no overwrite without `GS_ROTATE_DEV_REPO_KEY=1`, Bitwarden item, no secret printed; it also sets a `keydname` and prints the public certificate fingerprint). PKCS12 has one password, so `keystorepass` and `keypass` are the same. The fingerprint goes into the docs, the `fdroidrepos://` link and the job (§4 step 1).

- Pros: the APK key and the index key rotate independently; a leaked index key cannot sign an APK and vice versa; it is clearly not a release key; the repository job never sees the APK key (§6).
- Cons: one more secret set and environment.

Discarded: reusing the dev APK key for the index (one secret less, but couples two rotations and puts the APK key into the job that runs fdroidserver).

## 6. Job shape

fdroidserver pulls in many Python packages (androguard, paramiko, GitPython, apache-libcloud, Pillow, …). It must not run where the dev APK key or a write token is available: a compromised release could otherwise sign and ship an auto-updating build to testers. Three jobs:

- **`dev-apk`** (as today, environment `dev-apk`, `contents: write`): re-sign; **delete `dev.p12` right after signing** (the `always()` step stays as a backstop); read `versionName` and `versionCode` with `aapt2 dump badging` (build-tools from `setup-android`); publish to `android-dev` with the `versionName` as title; upload the re-signed APK as a workflow artifact for the next job. The release asset becomes `guardedsleep-dev.apk` (the `alpha` in the old name is wrong after the id change). The repository secrets are not checked here, so a missing repository key never stops the pre-release.
- **`dev-repo`** (new, `needs: dev-apk`, environment `dev-repo`, `permissions: { contents: read }`, no `GH_TOKEN`): checks the three repository secrets (fails naming the missing one), installs fdroidserver from a **hash-locked requirements file** (`tools/fdroid/requirements.txt`, generated by `uv pip compile --generate-hashes` from `requirements.in` with `fdroidserver==2.4.5`, installed with `uv pip install --require-hashes` into a venv). Two dependencies, `args` and `clint`, exist only as source packages, so uv builds them; the build backend (`setuptools`, `wheel`) is pinned with hashes too, in `tools/fdroid/build-constraints.txt`, passed with `--build-constraints`, so nothing in the install is unhashed. Renovate's `pip-compile` manager updates both files (explicit `managerFilePatterns`; the uv header comment in each file must stay), writes `config.yml` (`repo_url`, `repo_name`, `repo_icon`, `repo_description`, `repo_keyalias`, `keystore`, `keystorepass`, `keypass`, `keydname`, `sdk_path`, `archive_older: 0`) and the keystore into `$RUNNER_TEMP`, runs §4, generates `metadata/org.guardedsleep.app.dev.yml` (name "GuardedSleep Dev", summary "Unreviewed test build from main, not a release", source link, licence AGPL-3.0-or-later), runs `fdroid update`, deletes the keystore, and uploads the **site root** (the directory that contains `fdroid/dev/repo/`, so the repository is served under that path, not at `/`) with `actions/upload-pages-artifact`. fdroidserver 2.4.x reads APKs with androguard (no `aapt`) and only warns about a debuggable APK.
- **`dev-pages`** (new, `needs: dev-repo`, environment `github-pages`, `permissions: { pages: write, id-token: write }`, `concurrency: { group: pages, cancel-in-progress: false }`): `actions/deploy-pages`. A job has one environment, and the deployment must run in `github-pages`.

A run that fails after `dev-apk` leaves the pre-release updated and the repository on the previous build; re-running the failed job fixes it (§4 handles the same `versionCode`). SHA-pinned actions; `docs/ci-gates.md` gets rows for `dev-repo` and `dev-pages` and an updated `dev-apk` row (§5.2: each job its own row, with runbook).

## 7. Clients

- **Neo Store / F-Droid:** add the repository via the `fdroidrepos://ditschi.github.io/GuardedSleep/fdroid/dev/repo?fingerprint=…` link or the QR code. Updates by `versionCode`.
- **Obtainium (recommended and documented):** source "F-Droid Third-Party Repo" with the same URL, so all clients share one update rule. Tracking the GitHub pre-release is not documented as a path: its tag stays `android-dev`, and whether Obtainium reads the version from the title depends on its settings.
- **Direct download:** the `android-dev` pre-release stays.

## 8. Migration and local builds

- Testers with the current `.alpha` dev build install the `.dev` app once and pair again, then remove the old app. Anyone who filtered the release asset by name (`guardedsleep-alpha-dev.apk`) updates the filter to `guardedsleep-dev.apk`. The docs and the first `android-dev` notes after the change say so.
- Local debug builds now order by commit, so installing a build of an older commit, or an IDE build (`versionCode` 1), over a newer one is a downgrade. `scripts/android-install.sh` installs with `adb install -r -d` (allowed because debug builds are debuggable) and says so in its help.
- The dev label is a non-critical en-only locale key; it is also generated into release resources, where it is unused and harmless.
- Settings → About shows `versionName` and the commit; the dev name is about 45 characters and repeats the sha. The layout is checked on a narrow screen (task 1.4).

## 9. Dev icon

Dev builds override `ic_launcher_background` with a clearly different colour, and add a small corner badge to the **monochrome** layer as well, because themed icons (Android 13+) ignore the background colour. With `use-brand-launcher-icon` the same overrides apply to the real logo layers.

## Risks

- **Pages quota:** three APKs of about 27 MB are far below the 1 GB site limit.
- **fdroidserver supply chain:** isolated in `dev-repo` without the APK key or a write token, installed from hashes including the build backend of the one source package; Renovate updates the locks in their own PR, reviewed like any runtime dependency (no automerge). **What the isolation does not cover:** a compromised dependency in `dev-repo` still holds the repository key. It could list an APK signed with a foreign key; testers with an installed dev build are protected, because Android rejects an update with another signer, but a tester installing for the first time would accept it. Mitigation: hash-locked, reviewed updates, and the first install can be checked against the SHA-256 in the `android-dev` notes, which the repository job cannot change.
- **Committer-time regressions** (a commit pushed with a wrong clock): the build would get a lower code and clients would not offer it; the next correct commit fixes it. Branch protection and GitHub-side merges make this unlikely. A build-time epoch before 2026-01-01 (a local machine with a bad clock) falls back to `versionCode` 1 with a Gradle warning instead of failing the build.
- **Merge order:** if the change merges before the owner created the `dev-repo` environment, `dev-repo` fails on `main`, while `dev-apk` still publishes. The owner steps are therefore preconditions of the merge (tasks §4).

## Critical review responses

APPROVED WITH CONCERNS; all IMPORTANT findings are answered in this design:

1. Same file name for every APK → versioned repository file names, renamed release asset (§4, §6, §8).
2. Duplicate `versionCode` on re-runs → only older codes are fetched (§4).
3. Unpinned fdroidserver next to secrets → separate `dev-repo` job without the APK key or write token, hash-locked install, APK key deleted right after signing (§6, Risks).
4. Proposal contradicted the design → proposal aligned.
5. Merge order → repository secrets checked only in `dev-repo`; owner steps before merge (§6, Risks, tasks §4).
6. Downgrades on local installs → `adb install -r -d` (§8).
7. Missed references → repository-wide grep in task 1.5, AGENTS.md §8 qualifier in task 3.3.

Second pass: all seven accepted. New finding A (unhashed build backend for the source-only `clint` and `args`) → hashed build constraints (§6, task 2.2). Suggestions B–E taken: the limit of the isolation (Risks, spec scenario), the verification chain and fingerprint location (§4), the site-root upload and `archive_older: 0` (§4, §6), the Renovate file patterns (§6, task 2.2).

First-pass suggestions taken: `sha8` and epoch validation (§2), fallback instead of failing on an early epoch (Risks), config keys (§6), fdroidserver's own `index.html` (§3), signed-index verification (§4), monochrome badge (§9), About layout check (§8), Obtainium via the F-Droid source only (§7), Pages ownership (§3).

## Implementation notes

Found while applying: the RSA repository key (§5), `jarsigner` without `-strict` (§4), `args` as a second source-only package (§6), and the new script and `tools/fdroid/**` added to the `android-native` path filters and `.github/path-filters.yml` (they must match, `affected.test.mjs`). The workflow artifact keeps its name `guardedsleep-alpha-debug-apk`; only the release asset is renamed.

## Discarded alternatives

- **Commit count as `versionCode`** (`git rev-list --count`): monotonic on a linear `main` and reproducible, but needs the full history in CI (shallow clones) and the owner preferred a readable timestamp version.
- **CI run number as `versionCode`:** not reproducible from the commit; re-runs change it.
- **Raw Unix time as `versionCode`:** reaches the Play limit in 2036.
- **Same application id for dev and alpha** (status quo of `add-dev-apk-prerelease`): signature conflict with the alpha release and loss of pairings.
- **Obtainium only, no repository:** no Pages work, but Neo Store and F-Droid cannot use it.
- **`gh-pages` branch, release assets for old APKs, trusting the unsigned index, reusing the APK key for the index:** see §3, §4, §5.
- **fdroidserver inside `dev-apk`** (first draft): one job less, but third-party Python code next to the APK key and a `contents: write` token.
