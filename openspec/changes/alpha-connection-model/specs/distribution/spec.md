# Spec Delta

## MODIFIED Requirements

### Requirement: Container images

The project SHALL publish `guardedsleep-signaling` and `guardedsleep-web` to GHCR, and a non-release `guardedsleep-web-staging` for the owner's test deployment: the same web client built with Vite mode `staging` and the official-deployment flag, so that new invites carry the project server. Each image SHALL be built natively on amd64 and arm64 runners and merged by digest into one multi-architecture manifest, with base images pinned by digest and a health check. The staging image SHALL be published only from `main` and manual runs (`:dev`, `:edge`, `:sha-<short>`), never for pull requests and never on a release tag, and SHALL NOT enable loopback mailboxes.

#### Scenario: Raspberry Pi self-hoster

- **WHEN** a self-hoster pulls `guardedsleep-signaling` on an arm64 machine
- **THEN** the manifest provides a native arm64 image

#### Scenario: Staging image and release tag

- **WHEN** a maintainer pushes the tag `web-v0.1.0` and later a commit lands on `main`
- **THEN** no `guardedsleep-web-staging` image is published for the tag, and the commit publishes it as `:dev`, `:edge` and `:sha-<short>`
