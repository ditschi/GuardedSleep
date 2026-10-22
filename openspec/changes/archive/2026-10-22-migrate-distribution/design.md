# Design

## Context

Sources: ADR-005 (naming, ASO, distribution), ADR-006 (Release Please), ADR-008 (images, amended by ADR-020), ADR-020 §10, ADR-021 §3. As built: `.github/workflows/docker-{signaling,web}.yml`, `docker/*/Dockerfile`, `docker/stack/`, `apps/android/app/build.gradle.kts` (`applicationId`, versionCode 1).

## Goals / Non-Goals

**Goals:** spec what is published today. **Non-Goals:** the Android release process (not built; prepared change `add-android-release`); release automation (T1).

## Decisions

- **Release Please (ADR-006) is T1** per ADR-020 §10; its npm-centric components are legacy. ADR-006 is archived, the T1 setup is in the backlog.
- **The web image builds the legacy `apps/mobile`** today; its tag prefix `mobile-v*` is kept in the spec because it is what runs. The wave-3 web change will MODIFY the image source and tag prefix.
- **Distroless, non-root, read-only Rust signaling image** (ADR-020 §6) belongs to `add-rust-signaling-server`; today the Node image runs as `node`.
- Store ASO wording (ADR-005) applies once a store listing exists; recorded with `add-android-release`.

## Risks / Trade-offs

- None.
