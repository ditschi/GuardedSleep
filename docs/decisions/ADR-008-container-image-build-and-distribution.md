# ADR-008: Container Image Build and Distribution Strategy

- Status: DECIDED
- Date: 2026-05-13
- Deciders: User decision after option analysis (Technical Architect + Critical Reviewer perspectives synthesized)
- Related: [ADR-002](ADR-002-client-platform-and-repository-structure.md) (signaling stack), [ADR-005](ADR-005-parent-alerts-product-naming-and-distribution.md) (distribution), [ADR-006](ADR-006-release-and-changelog-automation.md) (release automation)

## Context

GuardedSleep ships two server-side container images: the **WebSocket signaling server** (`apps/signaling-server`) and the **static PWA host** (nginx serving `apps/mobile` build output). Self-hosters — especially privacy-minded users with home labs — need:

1. A predictable place to **pull** these images (no requirement to clone the repo).
2. A predictable **tag scheme** that supports both "give me the latest stable" and "give me bleeding edge".
3. Confidence that the published images run on their hardware (x86 home servers **and** ARM single-board computers like Raspberry Pi).
4. A drop-in **`compose.yaml`** that wires both services into a working stack.

The repo today only builds and pushes the signaling image with a misused `:latest` (= "latest commit on `main`", not "latest release") and no Web image automation. This ADR fixes both.

## Options Considered

### Tag semantics for `:latest`

- **Option A — `:latest` = latest stable release.** Aligns with Docker Hub / nginx / postgres / traefik convention. Bleeding edge moves to `:dev` (or `:edge`).
- **Option B — `:latest` = latest commit on `main` (current behaviour).** Faster feedback for users tracking unreleased code, but surprises anyone who expects "stable".

### Distribution registry

- **Option A — GHCR (`ghcr.io/<owner>/...`).** Free for public open-source projects, integrates with the GitHub repo's permissions model, requires no extra account for users to pull.
- **Option B — Docker Hub.** Best discoverability but rate-limited for anonymous pulls and adds a separate account to maintain.
- **Option C — Both (mirrored).** Maximal reach, doubles workflow complexity.

### Target architectures

- **Option A — `linux/amd64` only.** Smallest CI cost; sufficient for typical x86 servers.
- **Option B — `linux/amd64` + `linux/arm64`.** Adds 2-3 minutes per workflow run (with cache) but covers Raspberry Pi 4/5, ARM servers, Apple Silicon dev machines. Privacy-baby-monitor + Pi homelab is a natural pairing.
- **Option C — Plus `linux/arm/v7`.** Pi Zero 2 / Pi 3 32-bit support. Significantly slower builds; user base is shrinking.

### Tag-generation tooling

- **Option A — `docker/metadata-action@v5`.** Standard, declarative, handles SemVer + branch + PR tags.
- **Option B — Hand-rolled bash/shell.** What the current `docker-signaling.yml` does. Fragile and harder to maintain.

### Web image `VITE_SIGNALING_URL` at build time

- **Option A — Build with EMPTY default.** The app's in-settings override / localStorage takes precedence; users without a reverse-proxy at the same origin override the URL in the app UI on first launch.
- **Option B — Build per-deployment from source.** Users who want a baked-in URL keep using `docker/stack/compose.yaml` (local build).
- **Option C — Publish multiple variants.** `:latest`, `:latest-tls`, `:latest-no-tls` etc. — combinatorial explosion.

## Decision

| Concern                          | Choice                                                                                                               |
| -------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Registry                         | **GHCR** (Option A). Path: `ghcr.io/<repo-owner-lowercase>/guardedsleep-{signaling,web}`.                            |
| Architectures                    | **`linux/amd64` + `linux/arm64`** (Option B). 32-bit ARM may be added later if demand appears.                       |
| `:latest` semantics              | **Latest stable release only** (Option A). Triggered by Release Please tags `<component>-v*`.                        |
| `:dev` (alias `:edge`)           | **Every push to `main`.** Replaces today's "main HEAD = `:latest`" behaviour.                                        |
| `:vX.Y.Z`, `:vX.Y`, `:vX`        | On each Release Please tag, computed by `docker/metadata-action`. `:vX` only post-1.0.                               |
| `:sha-<short>`                   | Every push to `main` and every PR (PR builds are validate-only, **not** pushed).                                     |
| `:pr-<n>` (PR previews)          | **Not enabled** in MVP — keeps the registry tidy.                                                                    |
| Tag generation tooling           | **`docker/metadata-action@v5`** (Option A). Hand-rolled scripts removed.                                             |
| Build tooling                    | **`docker/build-push-action@v6` + `docker/setup-buildx-action@v3`** with `cache-from/to=type=gha`.                   |
| Web `VITE_SIGNALING_URL` default | **Empty in published `:latest`/`:dev`** (Option A). Users who want a baked URL build locally.                        |
| Workflow split                   | **Two separate workflow files** (`docker-signaling.yml`, `docker-web.yml`) — different path triggers and build args. |
| Consumer compose for full stack  | New `docker/stack/compose.ghcr.yaml` pulling both images. Local-build `compose.yaml` stays.                          |
| Image label compliance           | OCI labels via `metadata-action`: `org.opencontainers.image.{source,description,licenses,…}`.                        |

## Consequences

### Positive

- **Self-hosters get a one-file deployment**: download `docker/stack/compose.ghcr.yaml` + a `.env`, `docker compose up -d`, done.
- **`:latest` becomes trustworthy** — it always points at a real release with a CHANGELOG entry and a Release Please-tagged commit.
- **`:dev` is honest about being unstable**, so people who want it can opt in.
- **ARM coverage** unlocks the most likely self-hoster hardware (Raspberry Pi).
- **Tag-source-of-truth** is the Release Please tag — no second version system to maintain.

### Negative

- **CI runtime increases** ~2-3 minutes per push (ARM emulation under QEMU). Mitigated by `cache-from/to=type=gha`.
- **`:latest` does not exist until the first release.** During the concept/PoC phase, users must use `:dev` if they want pre-built images. Documented prominently in [docs/contributing/](../contributing/signaling-server-docker.md).
- **Removing the "every push → `:latest`" behaviour is a breaking change for any existing consumer.** Mitigated by communicating in CHANGELOG and by the fact that **no GuardedSleep release has shipped yet**, so the user base affected is minimal.

### Risks

- **GHCR outage** → users can't pull. **Mitigation:** the `docker/stack/compose.yaml` local-build path is always available as a fallback.
- **ARM build flakes under QEMU** → workflow may sporadically fail. **Mitigation:** retry job, monitor flake rate; if it becomes routine, drop to amd64-only or switch to ARM-native runners (available on GitHub Actions when budget allows).
- **Image-tag-Release-Please drift** → if `metadata-action`'s regex for `signaling-server-v*` / `mobile-v*` fails to capture the version, no SemVer tags are produced. **Mitigation:** explicit smoke check in the workflow that prints the tag list and fails if empty on a tag push.

## Discarded alternatives

- **Docker Hub** — secondary distribution channel; can be added later if demand appears (mirror via `docker/build-push-action` `tags:` with two registries). For MVP, GHCR alone keeps things simple.
- **`linux/arm/v7`** — Pi Zero 2 / Pi 3 32-bit. Build slowdown not justified by audience size; revisit on demand.
- **Per-deployment `VITE_SIGNALING_URL` build variants** — combinatorial; users can either build locally or use the in-app override.
- **PR-preview image tags (`:pr-<n>`)** — useful for reviewers but adds registry-lifecycle work (cleanup); revisit if reviewers ask.
- **Single workflow with matrix for both images** — simpler in theory but different path-filter triggers (a signaling change shouldn't rebuild the web image and vice versa) made the two-file split cleaner.
- **Keep `:latest` = main HEAD** — community-incompatible; the broader Docker ecosystem expects `:latest` to be "current stable".
