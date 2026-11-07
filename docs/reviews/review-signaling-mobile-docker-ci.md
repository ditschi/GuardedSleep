# Review: signaling server, mobile WebRTC, Docker, CI (rolling)

**Scope:** `apps/signaling-server`, the legacy mobile web client (signaling + WebRTC), `docker/signaling-server`, `.github/workflows` (incl. `ci.yml`, `docker-signaling.yml`), root `package.json` (`pnpm.overrides`, `audit`), tests, smoke scripts.

**Purpose:** Structured checklist so each **role** from [docs/roles/](../roles/README.md) can review the same areas consistently. Fill findings under each section; link PRs or commits when closing items.

---

## Technical Architect ([role](../roles/technical-architect.md))

- [ ] Monorepo builds remain reproducible: `pnpm install`, `pnpm test`, `pnpm run build`, signaling `smoke`.
- [ ] Docker image builds from repo root; `pnpm deploy` output is minimal and runnable.
- [ ] WebRTC path (STUN defaults, signaling URL) is configurable enough for future coturn / self-hosted STUN.
- [ ] CI jobs (pre-commit, audit, format, test, mobile build, signaling build, smoke, docker-signaling) are ordered sensibly and fail fast where intended.
- [ ] No unnecessary coupling between mobile and server packages beyond `@guardedsleep/protocol`.

### Findings

_(Add notes, risks, follow-ups.)_

---

## Security & Privacy Architect ([role](../roles/security-privacy-architect.md))

- [ ] Signaling: `maxPayload`, `perMessageDeflate`, `SIGNALING_ALLOWED_ORIGINS`, hello timeout, duplicate `hello` rejection align with threat model.
- [ ] HTTP headers on health endpoint; production assumes TLS in front (Traefik / reverse proxy).
- [ ] `pnpm audit` / `cookie` override: dependency risk tracked; no new high-severity issues ignored.
- [ ] STUN defaults (EU public servers): metadata exposure documented; self-hosted coturn recommended for production (see connectivity docs).
- [ ] WebRTC: signaling payloads validated before use (`peer-session`); no cleartext media through signaling server.

### Findings

_(Add notes.)_

---

## Critical Reviewer ([role](../roles/critical-reviewer.md))

- [ ] Docs (Docker, Traefik, security) match actual env vars and compose files.
- [ ] Contradictions between “no Google STUN” in code, feature docs, and privacy notes are resolved.
- [ ] Edge cases: WebSocket behind proxy (timeouts, `wss://`), Origin allowlist vs. native clients, empty origins.
- [ ] CI: `pnpm audit --audit-level=high` may miss low issues — acceptable trade-off documented.

### Findings

_(Add notes.)_

---

## UX / Usability Expert ([role](../roles/ux-usability-expert.md))

- [ ] Error paths from signaling (connection failed, protocol errors) surface in UI where applicable (or stubbed clearly for MVP).
- [ ] Pairing / settings copy will eventually reflect real `wss://` URL and STUN/TURN policy (no misleading “always Google”).

### Findings

_(Add notes.)_

---

## Product Owner ([role](../roles/product-owner.md))

- [ ] MVP scope (signaling + WebRTC scaffold) vs. deferred items (rate limiting, `STATUS: IDEA`) is clear.
- [ ] Self-hosting story (Docker + optional Traefik) is understandable for target users.

### Findings

_(Add notes.)_

---

## Maintainer checklist (CI / Docker)

- [ ] `docker-signaling.yml` pushes to GHCR with lowercase owner; tags sensible for deploy.
- [ ] `compose.build.yaml` / `compose.ghcr.yaml` pass through env vars needed for security tuning.
- [ ] Traefik example compose (`docker/public-server/compose.yaml`) matches its [README](../../docker/public-server/README.md).

### Findings

_(Add notes.)_
