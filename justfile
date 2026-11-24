# The single entry point for tasks (ADR-020 §1). Fixed vocabulary:
#   setup fmt lint test gen build e2e gate ci   (+ sub-recipes such as fmt-check, build-android)
# Install: `mise install` (pins in mise.toml). Recipes run with the mise-pinned tools and fall back to
# plain bash with a hint when mise is missing (scripts/lib/shell.sh).
#
# Tasks that need Docker, KVM or the Android SDK print "SKIPPED: <task> needs <x>" and exit 0.
# CI sets GS_STRICT=1, which turns every skip into a failure (scripts/lib/require.sh).
# just manual: https://just.systems/man/en/

set shell := ["./scripts/lib/shell.sh", "-c"]

[private]
default:
    @just --list

# One-time setup: pinned tools, JS dependencies, git hooks.
[script("bash")]
setup:
    set -euo pipefail
    if ! command -v mise >/dev/null 2>&1; then
      echo "mise is required: https://mise.jdx.dev/getting-started.html" >&2; exit 1
    fi
    mise install
    # rustup reads rust-toolchain.toml and installs the pinned toolchain, components and targets.
    if command -v rustup >/dev/null 2>&1; then rustup show >/dev/null; else echo "hint: install rustup (https://rustup.rs) for host cargo; Docker is the fallback."; fi
    mise exec -- pnpm install --frozen-lockfile
    # Clones that ran the old Husky setup still point core.hooksPath at .husky/_; lefthook refuses to
    # install over it, and the stale Husky hooks do nothing once .husky is gone.
    # The setting can live in the repository config (--local) or, in a linked worktree, in its own
    # config.worktree (--worktree); check and clear both scopes.
    for scope in local worktree; do
      hooks_path="$(git config "--$scope" --get core.hooksPath 2>/dev/null || true)"
      if [ "${hooks_path#*.husky}" != "$hooks_path" ]; then
        git config "--$scope" --unset-all core.hooksPath
        echo "Removed the stale Husky core.hooksPath ($hooks_path, $scope scope); lefthook replaces it."
      fi
    done
    mise exec -- lefthook install
    # ai-health venv and grammars for `just code-health` and the lint gate; skips without uv.
    just _code-health-setup
# Lists the prerequisites of the recipes (mise tools, rustup, Docker, uv, Playwright browser, KVM, Android SDK, git hooks) with a fix for each missing one. Informational, always exits 0.
doctor:
    scripts/doctor.sh

# Format everything that is formatted automatically (Prettier, rustfmt, taplo, ktfmt).
fmt:
    pnpm exec prettier --write .
    scripts/cargo.sh fmt --all
    taplo fmt
    scripts/lib/ktfmt.sh

# Same checks without writing; also the whitespace/JSON hygiene the old pre-commit config had.
fmt-check:
    pnpm exec prettier --check .
    scripts/cargo.sh fmt --all -- --check
    taplo fmt --check
    scripts/lib/ktfmt.sh --dry-run --set-exit-if-changed
    scripts/lib/hygiene.sh
    scripts/lib/workspace-lints.sh
    git ls-files -z 'scripts/*.sh' 'docker/*.sh' | xargs -0 shellcheck -x -P SCRIPTDIR

# ktfmt over apps/android (also part of fmt and fmt-check).
fmt-kotlin *args:
    scripts/lib/ktfmt.sh {{ args }}

# Everything static: text, workflows, then the code checks. CI runs the three parts as separate jobs.
lint: lint-text lint-workflows lint-code

# Text and repository hygiene for every file, docs included: Prettier, spelling, whitespace/JSON hygiene,
# REUSE, OpenSpec spec/change validation, brand icon layers vs. docs/brand/logo.svg and the secret scan over the full history (CI checks out
# with fetch-depth 0).
lint-text: _js-deps
    scripts/lib/hooks-check.sh
    pnpm exec prettier --check .
    openspec validate --all --strict --no-interactive
    typos --force-exclude
    scripts/lib/hygiene.sh
    scripts/lib/workspace-lints.sh
    scripts/lib/no-google-stun.sh
    node scripts/lib/brand-icons.mjs
    reuse lint
    scripts/lib/secrets.sh

# GitHub workflow linters.
lint-workflows:
    actionlint
    zizmor --offline --min-severity medium .github

# Code: Rust format and clippy (all features), iOS check, supply chain, API snapshots, generate-twice, i18n, curated public lists (minimums, unlisted STUN hosts), UI literals, types.
lint-code:
    scripts/lib/slot.sh medium -- just _lint-code

# `--config` is always passed to ai-health: without it ai-health silently reads <path>/.aihealth.toml or
# its defaults. Score per scanned path and profile: `crates` (rust, rust-tests), `apps` (web), `apps/e2e`
# (web-tests), `packages` (web); test profiles switch Primitive Obsession off.
# Code health (ai-health, docs/contributing/code-health.md); flags: `--component rust|web`, `--json` (one JSON object per path), `--changed [base]` (delta since the merge base, uncommitted files included; fails on degradation), `--staged`.
[script("bash")]
code-health *args:
    set -euo pipefail
    source scripts/lib/require.sh
    ah=target/code-health/venv/bin/aihealth
    [ -x "$ah" ] || gs_unmet code-health "the ai-health venv (run 'just setup')"
    set -- {{ args }}
    mode=scan fmt=text base=origin/main only=""
    while [ $# -gt 0 ]; do
      case "$1" in
        --json) fmt=json ;;
        --component)
          case "${2:-}" in rust | web) only="$2"; shift ;; *) echo "--component needs rust or web" >&2; exit 2 ;; esac ;;
        --changed) mode=changed; case "${2:-}" in "" | --*) ;; *) base="$2"; shift ;; esac ;;
        --staged) mode=staged ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
      esac
      shift
    done
    rc=0
    for pp in crates:rust crates:rust-tests apps:web apps/e2e:web-tests packages:web; do
      path="${pp%%:*}" profile="${pp##*:}"
      [ -z "$only" ] || [ "$only" = "${profile%-tests}" ] || continue
      cfg="tools/code-health/$profile.toml"
      [ "$fmt" = json ] || echo "== $path ($profile profile) =="
      case "$mode" in
        scan) "$ah" scan "$path" --config "$cfg" --format "$fmt" --top 5 ;;
        changed) "$ah" delta --base "$(git merge-base "$base" HEAD)" --config "$cfg" --format "$fmt" "$path" || rc=1 ;;
        staged) "$ah" delta --staged --config "$cfg" --format "$fmt" "$path" || rc=1 ;;
      esac
    done
    [ "$fmt" = json ] || [ -n "$only" ] || echo "android: not yet scored (Kotlin adapter pending)"
    exit "$rc"

# ai-health in target/code-health/venv: every PyPI dependency from the hash lock, ai-health itself from its
# commit pin without dependencies (a git requirement cannot carry a hash), then the tree-sitter grammars
# (downloaded once, SHA-256 checked, offline afterwards). All steps are idempotent and take about a second
# when nothing changed. https://docs.astral.sh/uv/pip/compile/#including-hashes
[private]
[script("bash")]
_code-health-setup:
    set -euo pipefail
    source scripts/lib/require.sh
    command -v uv >/dev/null 2>&1 || gs_unmet code-health uv
    py=target/code-health/venv/bin/python
    [ -x "$py" ] || uv venv --python 3.12 target/code-health/venv
    uv pip sync --python "$py" --require-hashes tools/code-health/aihealth.lock.txt
    uv pip install --python "$py" --no-deps -r tools/code-health/aihealth.in
    "$py" -c "from tree_sitter_language_pack import download; download(['rust', 'typescript', 'tsx', 'javascript'])"

# ai-health ignores a suppression without " -- <reason>" silently, so a bare marker would pass unnoticed.
# The syntax is documented in docs/ and openspec/, hence the excluded paths.
[private]
_code-health-suppressions:
    ! git grep -nP 'aihealth:[ \t]*disable=(?![\w, \t]+-- .{10})' -- . ':!openspec' ':!docs' ':!*.md' ':!.claude' ':!justfile'

# The gate of `just lint-code`: delta per scanned path against the merge base with origin/main, and the
# project score of each path must not fall more than 0.01 below the merge base (owner decision; `delta`
# reports file scores only, so the base tree is scanned from `git archive`).
[private]
[script("bash")]
_code-health-gate: _code-health-setup _code-health-suppressions
    set -euo pipefail
    source scripts/lib/require.sh
    ah=target/code-health/venv/bin/aihealth
    [ -x "$ah" ] || gs_unmet code-health "the ai-health venv (run 'just setup')"
    base="$(git merge-base origin/main HEAD)"
    rm -rf target/code-health/base && mkdir -p target/code-health/base
    trap 'rm -rf target/code-health/base' EXIT
    git archive "$base" crates apps/web apps/e2e packages | tar -x -C target/code-health/base
    rc=0
    for pp in crates:rust crates:rust-tests apps:web apps/e2e:web-tests packages:web; do
      path="${pp%%:*}" cfg="tools/code-health/${pp##*:}.toml"
      "$ah" delta --base "$base" --config "$cfg" "$path" || rc=1
      before="$("$ah" scan "target/code-health/base/$path" --config "$cfg" | sed -n 's/^project score: \([0-9.]*\).*/\1/p')"
      [ -n "$before" ] || { echo "FAILED: no project score for $path at the merge base" >&2; rc=1; continue; }
      min="$(awk -v b="$before" 'BEGIN { printf "%.2f", b - 0.01 }')"
      "$ah" scan "$path" --config "$cfg" --fail-under "$min" >/dev/null ||
        { echo "FAILED: $path scores below $min (merge base $before)" >&2; rc=1; }
    done
    exit "$rc"

[private]
_lint-code: _js-deps _js-gen check-ios check-determinism
    just _code-health-gate
    scripts/cargo.sh fmt --all -- --check
    taplo fmt --check
    scripts/lib/ktfmt.sh --dry-run --set-exit-if-changed
    scripts/cspell-i18n.sh
    scripts/cargo.sh clippy --workspace --all-targets --all-features -- -D warnings
    just deny
    just api-check
    pnpm run i18n:check
    pnpm run lists:check
    pnpm run lists:stun
    just lint-message-keys
    scripts/lint-ui-literals.sh
    pnpm -r --if-present run check

# Every `message_keys()` entry of the core exists in locales/en.json as `core.<camelCase>` (openspec i18n spec).
lint-message-keys:
    scripts/lib/slot.sh medium -- scripts/cargo.sh test -p guardedsleep-core --test message_keys_catalogue

# The core, the relay (shared with the Android LAN listener) and the UniFFI facade must stay buildable for iOS (ADR-020 §3); rust-toolchain.toml installs the target.
check-ios:
    scripts/cargo.sh check -p guardedsleep-core -p guardedsleep-relay -p guardedsleep-ffi --target aarch64-apple-ios

# Generate-twice check: the Kotlin bindings (from the host ffi library) and the wasm package are generated
# twice (the second time after a clean of the three workspace crates) and must hash identically. SKIPPED without Docker.
[script("bash")]
check-determinism:
    set -euo pipefail
    source scripts/lib/require.sh
    require check-determinism docker
    scripts/core-build.sh determinism

# API-surface snapshots of the binding facades (cargo public-api on the pinned nightly in tools/nightly, plus
# the generated .d.ts). Fails on a diff; `just api-snapshot` regenerates. SKIPPED without Docker.
api-check:
    scripts/lib/api-snapshot.sh check

# Regenerate crates/guardedsleep-{ffi,wasm}/api.txt and packages/core-wasm/api/core.d.ts after an intended change.
api-snapshot:
    scripts/lib/api-snapshot.sh update

# cargo-deny (advisories, licences, sources, sans-IO bans) plus the core dependency-count guard.
deny:
    scripts/lib/deny.sh

# Secret scan of the full history (Betterleaks, validation off).
secrets:
    scripts/lib/secrets.sh

# Host Rust, WASM + node conformance, pnpm tests, Android JVM tests. Builds no Android ABI: the JVM tests
# load the host build of the core.
test:
    scripts/lib/slot.sh medium -- just _test

[private]
_test: _js-deps _js-gen test-rust test-wasm test-web test-android test-tooling

# Unit tests of the repository scripts (delta-test planner and the like).
test-tooling:
    node --test scripts/lib/*.test.mjs

# Runs only the tests the changed paths need (committed since the merge base with <base>, uncommitted and
# untracked files), mapped by .github/path-filters.yml; global or unmapped paths run the full `just test`.
# The plan itself runs `_js-deps`/`_js-gen` where Vitest or WASM tests need them. `--dry-run` prints the
# plan only: `node scripts/lib/affected.mjs --dry-run [base]`.
test-affected base="origin/main":
    scripts/lib/slot.sh medium -- node scripts/lib/affected.mjs {{ base }}

# Cargo workspace tests; nextest when installed, doctests through cargo test.
test-rust:
    if command -v cargo-nextest >/dev/null 2>&1 && command -v rustup >/dev/null 2>&1; then cargo nextest run --workspace && cargo test --workspace --doc; else scripts/cargo.sh test --workspace; fi

# `_js-gen` has already built (or found up to date) the WASM core.
test-wasm: _js-gen
    node --test crates/guardedsleep-core/js/*.test.mjs

test-web:
    pnpm test

# Android JVM unit tests (core + app) against the host build of the core, no ABI cross-compile (medium slot).
test-android:
    scripts/lib/slot.sh medium -- just _test-android

[private]
[script("bash")]
_test-android:
    set -euo pipefail
    source scripts/lib/require.sh
    require test-android android-sdk
    scripts/core-build.sh host
    scripts/gradle.sh buildLogicTest :core-bindings:testDebugUnitTest :platform:testDebugUnitTest :app:testDebugUnitTest

# Generate the i18n output (Android resources, web JSON) and the curated public lists (web module, CSP origins). Nothing generated is committed.
gen:
    pnpm run i18n:gen
    pnpm --filter @guardedsleep/web run lists:gen

# Everything: web, Android debug + release APKs, Android Lint, JVM tests, Roborazzi, release-surface and
# core-size checks. Runs verify-android (not build-android) so `just ci` gives CI's android verdict.
build:
    scripts/lib/slot.sh medium -- just _build

[private]
_build: _js-deps build-web verify-android

build-web:
    pnpm run build

# Core for Android (cross-compiled ABIs), debug and release APKs and their checks. Extra Gradle tasks
# join the single Gradle run, e.g. `just build-android :app:verifyRoborazziDebug`.
build-android *gradle_tasks:
    scripts/lib/slot.sh heavy -- just _build-android {{ gradle_tasks }}

[private]
[script("bash")]
_build-android *gradle_tasks:
    set -euo pipefail
    source scripts/lib/require.sh
    require build-android android-sdk
    scripts/core-build.sh android
    scripts/gradle.sh {{ gradle_tasks }} :app:assembleDebug :app:assembleRelease
    scripts/poc/release-check.sh
    scripts/poc/size-check.sh

# Install the local debug APK on every connected phone (USB or Wi-Fi adb) and launch it. Options: --build, --no-launch; env GS_DEVICES, GS_APK (scripts/android-install.sh).
install-android *args:
    scripts/android-install.sh {{ args }}

# What the CI android job runs: build-android plus Android Lint (warnings are errors), the JVM tests and Roborazzi screenshot verification.
verify-android:
    just build-android lintDebug buildLogicTest :core-bindings:testDebugUnitTest :platform:testDebugUnitTest :app:testDebugUnitTest :app:verifyRoborazziDebug

# Browser E2E (Playwright, local dev stack). Prints SKIPPED without Playwright browsers unless GS_STRICT=1.
# Extra arguments go to Playwright, e.g. `just e2e --shard=2/3` (what CI runs per shard) or `just e2e --list`.
e2e *args:
    scripts/lib/slot.sh heavy -- just _e2e {{args}}

[private]
_e2e *args: _js-deps _js-gen
    scripts/lib/e2e.sh {{args}}

# Emulator gate AC-1..AC-4 (needs KVM; DOZE_MINUTES=2 just gate for a quick run). Run build-android first.
gate:
    scripts/lib/slot.sh heavy -- just _gate

[private]
[script("bash")]
_gate:
    set -euo pipefail
    source scripts/lib/require.sh
    require gate kvm docker
    # CI runs the TURN-lifetime probe as its own job in parallel (android-native.yml, `turn-lifetime`)
    # and sets GS_GATE_TURN_LIFETIME=0 here; locally the default runs it first, as before.
    if [ "${GS_GATE_TURN_LIFETIME:-1}" != "0" ]; then
      scripts/poc/turn-lifetime.sh
    fi
    scripts/poc/local-gate.sh

# TURN allocation lifetime spike (task 9.1): a 60 s REST credential keeps relaying for 11+ minutes on coturn. Needs Docker; takes ~12 minutes.
turn-lifetime:
    scripts/lib/slot.sh heavy -- scripts/poc/turn-lifetime.sh

# Public-server stack (task 10.1): no client address in any container log after a relayed test call, coturn's Binding size, check-turn.sh. Needs Docker; builds the signaling image on the first run.
public-server-logs:
    scripts/lib/slot.sh heavy -- scripts/poc/public-server-logs.sh

# turns: on 5349 (task 10.2): coturn without and with a TEST certificate (made at run time), the acme deploy hook, reload-turn-cert.sh (restart, then SIGUSR2), a TLS allocation with relayed data. Needs Docker.
public-server-turns:
    scripts/lib/slot.sh heavy -- scripts/poc/public-server-turns.sh

# turns: on 5349 terminated by Traefik (task 10.4, the default of docker/public-server): coturn without a certificate behind a Traefik with the owner's TCP router and readTimeout 0 (and one without, as control), a TLS allocation with relayed data, 75 s of keep-alive, no-SNI default certificate, TLS 1.1 refused, wrong credential rejected. Needs Docker.
public-server-turns-traefik:
    scripts/lib/slot.sh heavy -- scripts/poc/public-server-turns-traefik.sh

# The same verdict as CI (the PR title check and the emulator gate are CI-only): lint, test, build, e2e in order,
# then a list of every SKIPPED line of the run (a skip is not a pass), also when a step fails.
ci:
    scripts/lib/ci.sh {{ just_executable() }}

# Rust signaling server plus the web client, for local development.
dev:
    scripts/dev.sh

# Runs one cargo-fuzz target (fuzz/Cargo.toml) on the pinned nightly of tools/nightly; extra args go to libFuzzer,
# e.g. `just fuzz envelope_open -- -max_total_time=60`. SKIPPED without cargo-fuzz and rustup.
fuzz target="envelope_open" *args:
    scripts/lib/fuzz.sh {{target}} {{args}}

# Signaling server smoke: starts the built guardedsleep-server binary and relays hello + signal over a WebSocket.
smoke:
    scripts/cargo.sh test -p guardedsleep-server --test smoke

# Docker stack with the host LAN IP, for phone tests.
stack-lan:
    scripts/lib/slot.sh heavy -- just _stack-lan

[private]
[script("bash")]
_stack-lan:
    set -euo pipefail
    source scripts/lib/require.sh
    require stack-lan docker
    bash scripts/stack-lan-env.sh
    docker compose -f docker/stack/compose.yaml --env-file docker/stack/.env up --build

stack-up:
    scripts/lib/slot.sh heavy -- docker compose -f docker/stack/compose.yaml --env-file docker/stack/.env up --build

diagnose-lan:
    bash scripts/diagnose-lan-connect.sh

check-kvm:
    bash scripts/check-kvm.sh

audit:
    pnpm run audit

# Preview the Release Please output (maintainer; needs GH_TOKEN and GITHUB_REPOSITORY).
release-preview:
    npx --yes release-please release-pr --token=$GH_TOKEN --repo-url=$GITHUB_REPOSITORY --config-file=release-please-config.json --manifest-file=.release-please-manifest.json --dry-run --debug

# List leftover dev servers, stuck agent polls and old project containers; --stop ends them; --worktrees lists finished agent worktrees (AGENTS.md section 10.2).
reap *args:
    scripts/lib/reap.sh {{ args }}

# Generated, uncommitted packages that svelte-check and Vitest import (the WASM core); rebuilt only when its
# Rust inputs changed (scripts/lib/core-wasm-fresh.mjs). `pnpm --filter @guardedsleep/core-wasm build` forces it.
[private]
_js-gen:
    node scripts/lib/core-wasm-fresh.mjs ensure

[private]
_js-deps:
    pnpm install --frozen-lockfile
