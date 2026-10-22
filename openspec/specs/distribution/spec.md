# distribution Specification

## Purpose

Give GuardedSleep one identity everywhere and publish what self-hosters need, the signaling server and the web client, as reproducible multi-architecture container images. Migrated from ADR-005 (naming), ADR-008 and ADR-020 §6/§9, as built.

## Requirements

### Requirement: Product identity

The product SHALL be named "GuardedSleep – Privacy Baby Monitor" (shortened to "GuardedSleep Baby Monitor" where a store limits the title). Engineering names SHALL be `GuardedSleep`, the npm scope `@guardedsleep`, Docker images `guardedsleep-*`, and the Android application id `org.guardedsleep.app`, with `org.guardedsleep.app.alpha` for alpha builds.

#### Scenario: Alpha build installed next to a later release

- **WHEN** an alpha build and a later release build are installed on one phone
- **THEN** they do not replace each other, because the alpha uses its own application id

### Requirement: Container images

The project SHALL publish `guardedsleep-signaling` and `guardedsleep-web` to GHCR. Each image SHALL be built natively on amd64 and arm64 runners and merged by digest into one multi-architecture manifest, with base images pinned by digest and a health check.

#### Scenario: Raspberry Pi self-hoster

- **WHEN** a self-hoster pulls `guardedsleep-signaling` on an arm64 machine
- **THEN** the manifest provides a native arm64 image

### Requirement: Image tags

A push to `main` SHALL publish `:dev` and `:sha-<short>`. A release tag (`signaling-server-v*` for signaling, `mobile-v*` for web) SHALL publish `:latest`, `:vX.Y.Z` and `:vX.Y`. Pull requests SHALL only build, with a read-only token and no registry login or push.

#### Scenario: Pull request from a fork

- **WHEN** a pull request changes the signaling Dockerfile
- **THEN** both architectures are built without pushing any image

### Requirement: Self-hosting stacks

The repository SHALL provide Compose stacks to run web and signaling together: built from source, from the GHCR images without a clone, and behind an existing Traefik with the app at `/` and signaling at `/signaling`.

#### Scenario: Self-hosting without cloning

- **WHEN** an operator downloads only the GHCR Compose file and its example environment
- **THEN** they can start web and signaling from the published images

### Requirement: No test hooks in release builds

Release APKs SHALL NOT contain the debug launch activity, the audio-injection receiver or its intent action, and debug-only test hooks (autostart and configuration extras, PoC logs) SHALL exist only in debug builds. An inspection of the release APK SHALL fail the build otherwise.

#### Scenario: Injection receiver left in a release build

- **WHEN** the release APK's manifest or dex still contains the audio-injection receiver
- **THEN** the release-surface check fails

### Requirement: Core size budget

The stripped Rust core library SHALL add at most 15 MB to the APK, summed over every packaged ABI.

#### Scenario: Core grows past the budget

- **WHEN** a change makes the packaged core libraries larger than 15 MB in total
- **THEN** the size check fails

### Requirement: Hardened signaling image

The signaling image SHALL run as a non-root user on a distroless base with a read-only root filesystem.

#### Scenario: Container compromise

- **WHEN** the server process is compromised
- **THEN** it cannot write to the image's file system or find a shell
