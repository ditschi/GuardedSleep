# Contributing documentation

- [Development environment](development.md) — Dev Container, local setup, lint/format, hooks
- [Signaling server (Docker / Compose)](signaling-server-docker.md) — image build, local Compose, GHCR
- [Signaling behind a reverse proxy (Traefik, Caddy, nginx)](signaling-traefik.md) — `wss://` on 443, ready-to-use Traefik Compose, path routing, checklist
- [Own TURN server on a VPS (coturn, Compose)](turn-server-vps.md) — Oracle Cloud walk-through, firewall, verification, known app gaps
- [Static PWA + full stack (Docker)](web-docker.md) — nginx image, Compose with/without Traefik
- [Relay soak and go/no-go (public Nostr relays)](relay-soak.md) — `just soak`, `just relays-check`, how to read the verdict, operator mail template
- [Decisions, roles and documentation conventions](decisions-and-roles.md) — options checklists, role coordination, Critical Review Gate, status tags
- [Releases and changelog](release-and-changelog.md) — Keep a Changelog, Conventional PR titles, manual alpha tags

Repository entry point: [CONTRIBUTING.md](../../CONTRIBUTING.md).

## Open ideas

- `STATUS: IDEA` Add an `examples/` folder with minimal reproductions (for example a standalone WebRTC PoC harness) so the PoC gate from ADR-001 has a permanent home outside Playwright app tests.
