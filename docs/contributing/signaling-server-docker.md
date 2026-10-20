# Signaling server (Docker / Compose)

The WebSocket signaling server (`apps/signaling-server`) can be run as a container built from this repository or from a **pre-built image on GitHub Container Registry (GHCR)**.

## Requirements

- Docker with BuildKit (default in current Docker Desktop / Engine)
- Docker Compose v2 (`docker compose`)

## Image layout

- **Dockerfile:** `docker/signaling-server/Dockerfile` (build context = **repository root**)
- **Production artifact:** `pnpm deploy` copies `@guardedsleep/signaling-server` with production dependencies into a small runtime layer.

## Security (signaling)

The server applies baseline HTTP headers on the health endpoint (`nosniff`, `DENY` framing, `no-store`). WebSockets use a **64 KiB max frame size**, **no `perMessageDeflate`** (reduces compression-based abuse), a **`SIGNALING_ALLOWED_ORIGINS`** allowlist for browser `Origin` headers (connections **without** `Origin` stay allowed for native clients and tooling), and a **`SIGNALING_HELLO_TIMEOUT_MS`** window so connections cannot stay open indefinitely without completing `hello`. A second `hello` on the same socket is rejected. Liveness and abuse limits: the server pings every WebSocket every `SIGNALING_PING_INTERVAL_MS` (default `10000`, min `2000`) and terminates sockets that missed the previous ping, so ghost baby/parent sockets disappear within about two intervals. Per client IP, `SIGNALING_HELLO_RATE_PER_MIN` (default `20`) and `SIGNALING_NEW_ROOM_RATE_PER_MIN` (default `10`) cap hello attempts and room creation (`RATE_LIMITED`; `0` disables); `SIGNALING_TRUST_PROXY=1` takes the client IP from the **rightmost** `X-Forwarded-For` hop (the one your proxy appended; set `SIGNALING_TRUSTED_PROXY_HOPS=n` for n proxies, taking the n-th from the right; default 1). Leading hops are client-controlled and ignored. IPs and room ids are never logged.

When **`NODE_ENV=production`**, **`SIGNALING_ALLOWED_ORIGINS` must be non-empty** or the process exits at startup. The Docker image and Compose files set a localhost-only default so local runs work; **you must override this** with your real app origins (e.g. `https://app.example`) for any internet-facing deployment.

**Room model (signaling protocol v2):** at most **one baby** and up to **`SIGNALING_MAX_PARENTS`** parent units per room (default `1` for the MVP 1:1 pairing, cap `256`). The **product** roadmap caps paired parents at **4** per baby in-app (Phase 2); raise `SIGNALING_MAX_PARENTS` in deployment when multi-parent lands. The server broadcasts a **`members`** roster after each join/leave. With **more than two peers**, each **`signal`** must include **`to`** (target `clientId`) so WebRTC payloads reach the correct parent/baby pair.

Use **TLS** (reverse proxy or `wss://`) in production; the container alone speaks plain HTTP/WebSocket.

**Reverse proxy (Traefik, Caddy, nginx):** see **[signaling-traefik.md](signaling-traefik.md)**. Ready-to-use Traefik file: [compose.traefik.yaml](../../docker/signaling-server/compose.traefik.yaml) + [traefik.env.example](../../docker/signaling-server/traefik.env.example) — only port 443 on the proxy, no published host port. Health endpoint: `GET /healthz`.

**Static PWA + signaling (full stack):** see **[mobile-web-docker.md](mobile-web-docker.md)** for the `docker/mobile-web` image and Compose examples under [docker/stack/](../../docker/stack/).

## Option A — build and run locally

From the **repository root**:

```bash
docker compose -f docker/signaling-server/compose.build.yaml build
docker compose -f docker/signaling-server/compose.build.yaml up
```

Foreground with rebuild after source changes:

```bash
docker compose -f docker/signaling-server/compose.build.yaml up --build
```

- Default URL: `http://127.0.0.1:8787/` (plain-text health response)
- WebSocket: same host and port as configured for the app (`PORT`, default `8787`)

### Ports and environment

Optional file `docker/signaling-server/.env` (see `docker/signaling-server/.env.example`):

- `SIGNALING_PORT` — port on your machine (default `8787`)
- `SIGNALING_INTERNAL_PORT` — port inside the container (default `8787`; must stay in sync with the port mapping)

## Option B — run a pre-built image from GHCR

Use this when CI (or you) has already pushed an image to GHCR.

1. Create `docker/signaling-server/.env` and set at least:

   ```bash
   SIGNALING_IMAGE=ghcr.io/<github-user-or-org>/guardedsleep-signaling:<tag>
   ```

   For private images, log in first:

   ```bash
   echo <GITHUB_TOKEN> | docker login ghcr.io -u <github-username> --password-stdin
   ```

2. From the **repository root**:

   ```bash
   docker compose -f docker/signaling-server/compose.ghcr.yaml pull
   docker compose -f docker/signaling-server/compose.ghcr.yaml up -d
   ```

`compose.ghcr.yaml` requires `SIGNALING_IMAGE` to be set (Compose will error if it is missing).

## Manual `docker build` (without Compose)

From the **repository root**:

```bash
docker build -f docker/signaling-server/Dockerfile -t guardedsleep-signaling:local .
docker run --rm -p 8787:8787 guardedsleep-signaling:local
```

## CI: publish to GHCR

The workflow [`.github/workflows/docker-signaling.yml`](../../.github/workflows/docker-signaling.yml) publishes `ghcr.io/<owner-lowercase>/guardedsleep-signaling` for the **`linux/amd64`** and **`linux/arm64`** platforms.

### Tag strategy ([ADR-008](../decisions/ADR-008-container-image-build-and-distribution.md))

| Tag                    | When                                                                                              | Intended consumer                                          |
| ---------------------- | ------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| `:latest`              | A `signaling-server-v*` git tag is pushed (created by [Release Please](release-and-changelog.md)) | Production self-hosters wanting "stable, current"          |
| `:vX.Y.Z`              | Same as `:latest`                                                                                 | Reproducible deploys                                       |
| `:vX.Y`                | Same as `:latest`                                                                                 | Auto-patch within a minor                                  |
| `:vX` (post-1.0 only)  | Same as `:latest`                                                                                 | Auto-minor within a major                                  |
| `:dev` (alias `:edge`) | Every push to `main`                                                                              | Testers / bleeding edge — **use this until first release** |
| `:sha-<short>`         | Every push (including PRs — PR builds are **not** pushed; the tag is for cache attribution only)  | Exact-commit debugging                                     |

Until GuardedSleep cuts its first release, `:latest` does **not** exist; use `:dev` in the meantime.

Forks should adjust image names or secrets as needed.

## End-user deployment from GHCR

Drop-in self-hosting without cloning this repo — see [docker/stack/README.md](../../docker/stack/README.md) Mode B. The [`docker/stack/compose.ghcr.yaml`](../../docker/stack/compose.ghcr.yaml) file pulls both the signaling and the web image; for **signaling only** use [`docker/signaling-server/compose.ghcr.yaml`](../../docker/signaling-server/compose.ghcr.yaml).
