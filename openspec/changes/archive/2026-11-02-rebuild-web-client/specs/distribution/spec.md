# Spec Delta

## MODIFIED Requirements

### Requirement: Image tags

A push to `main` SHALL publish `:dev` and `:sha-<short>`. A release tag (`signaling-server-v*` for signaling, `web-v*` for the web image built from `apps/web`) SHALL publish `:latest`, `:vX.Y.Z` and `:vX.Y`. Pull requests SHALL only build, with a read-only token and no registry login or push.

#### Scenario: Pull request from a fork

- **WHEN** a pull request changes the signaling Dockerfile
- **THEN** both architectures are built without pushing any image

#### Scenario: Web release tag

- **WHEN** a maintainer pushes the tag `web-v0.1.0`
- **THEN** the web image is published as `:latest`, `:v0.1.0` and `:v0.1`, and a `mobile-v*` tag publishes nothing
