# Spec Delta

## ADDED Requirements

### Requirement: libwebrtc patch policy

A critical or exploited CVE in a used WebRTC path (audio codecs, DTLS/SRTP, SCTP, ICE/STUN, SDP) that the vendor has not patched 30 days after the Chrome fix, or six months without a vendor release, SHALL trigger a change that switches the vendor, builds a patched version or disables the feature.

#### Scenario: Unpatched SRTP CVE

- **WHEN** a critical SRTP CVE is still unpatched by the vendor 30 days after Chrome fixed it
- **THEN** a change is opened to switch vendor, patch-build or disable the affected feature

### Requirement: libwebrtc vendor isolation

Only files in the engine package (`org.guardedsleep.app.media.libwebrtc`) SHALL import `org.webrtc`; the rest of the Android app talks to the `MediaEngine` interface.

#### Scenario: Vendor import outside the engine package

- **WHEN** a file outside `media/libwebrtc` imports `org.webrtc`
- **THEN** the Android unit tests fail

### Requirement: libwebrtc sourcing

The libwebrtc artifact SHALL resolve from Maven Central only (Gradle `exclusiveContent` on `io.github.webrtc-sdk`), and Google Maven SHALL serve only Google and AndroidX groups.

#### Scenario: Same group published on another repository

- **WHEN** another repository serves a component in group `io.github.webrtc-sdk`
- **THEN** Gradle ignores it and resolves from Maven Central

## MODIFIED Requirements

### Requirement: Supply chain

`cargo-deny` SHALL check Rust advisories, AGPL-compatible licences, crates.io as the only source and wildcard versions, and SHALL ban tokio, mio, getrandom and socket crates from the core graph. A guard SHALL fail when the core's dependency count changes without a reviewed bless. `pnpm audit` and `cargo deny` SHALL also run daily, and Renovate SHALL propose updates weekly with a minimum release age of 3 days; libwebrtc and Gradle plugins SHALL use a 7-day minimum release age, and libwebrtc updates SHALL come in their own PR and never be automerged.

#### Scenario: Async runtime pulled into the core

- **WHEN** a change adds a dependency that brings tokio into the core's graph
- **THEN** `just deny` fails

#### Scenario: libwebrtc update

- **WHEN** Renovate proposes a new `io.github.webrtc-sdk:android` version
- **THEN** it is its own PR, at least 7 days old, and needs manual review
