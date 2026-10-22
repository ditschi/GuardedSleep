# Signaling server (Docker / Compose)

The signaling server is the Rust `guardedsleep-server` ([ADR-020](../../openspec/changes/archive/2026-10-20-adr-020-toolchain-and-development-setup/design.md) §6; `crates/guardedsleep-server` on the sans-IO `crates/guardedsleep-relay`). It replaced the Node `ws` server with the same environment variables, the same WebSocket protocol and the same `/healthz`. It can be run as a container built from this repository or from a **pre-built image on GitHub Container Registry (GHCR)**.

## Requirements

- Docker with BuildKit (default in current Docker Desktop / Engine)
- Docker Compose v2 (`docker compose`)

## Image layout

- **Dockerfile:** `docker/signaling-server/Dockerfile` (build context = **repository root**)
- **Build:** cargo-chef caches the dependency layer; the Rust channel comes from `rust-toolchain.toml`. Each platform (amd64, arm64) compiles natively.
- **Runtime:** `gcr.io/distroless/cc-debian13:nonroot` (about 30 MB, no shell), user `nonroot` (65532). It writes nothing, so the Compose files run it with `read_only: true`, `cap_drop: [ALL]` and `no-new-privileges`.
- **Ports:** `8787/tcp` (WebSocket signaling and `/healthz`) and `3478/udp` (STUN).
- **Health check:** the image has no `curl`, so `HEALTHCHECK` runs `guardedsleep-server --healthcheck`, which probes `GET /healthz` on loopback.

## Security (signaling)

The server applies baseline HTTP headers on the health endpoint (`nosniff`, `DENY` framing, `no-store`). WebSockets use a **64 KiB max message size**, **no per-message compression** (reduces compression-based abuse), a **`SIGNALING_ALLOWED_ORIGINS`** allowlist for browser `Origin` headers (connections **without** `Origin` stay allowed for native clients and tooling), and a **`SIGNALING_HELLO_TIMEOUT_MS`** window so connections cannot stay open indefinitely without completing `hello`. A second `hello` on the same socket is rejected. Liveness and abuse limits: the server pings every WebSocket every `SIGNALING_PING_INTERVAL_MS` (default `10000`, min `2000`) and terminates sockets that missed the previous ping, so ghost baby/parent sockets disappear within about two intervals. Per client IP, `SIGNALING_HELLO_RATE_PER_MIN` (default `20`) and `SIGNALING_NEW_ROOM_RATE_PER_MIN` (default `10`) cap hello attempts and room creation (`RATE_LIMITED`; `0` disables), and `SIGNALING_MAX_CONNECTIONS_PER_IP` (default `16`; `0` disables) caps open sockets so one address cannot take every slot (the extra socket is closed with `server full`). Per socket, `SIGNALING_FRAME_RATE_UNJOINED_PER_MIN` (default `120`) and `SIGNALING_FRAME_RATE_PER_MIN` (default `600`) cap frames before and after joining (`RATE_LIMITED`, then the socket is closed). Many families behind one carrier-grade NAT address share the connection cap; raise it if you serve such networks. Behind a reverse proxy these limits only work per client when `SIGNALING_TRUST_PROXY=1` is set (see [signaling-traefik.md](signaling-traefik.md)); `SIGNALING_TRUST_PROXY=1` takes the client IP from the **rightmost** `X-Forwarded-For` hop (the one your proxy appended; set `SIGNALING_TRUSTED_PROXY_HOPS=n` for n proxies, taking the n-th from the right; default 1). Leading hops are client-controlled and ignored. IPs and room ids are never logged.

**Built-in STUN** (UDP, default `3478`): the server answers STUN Binding requests only (RFC 5389) with the sender's address and ignores everything else. It is on by default. A response is at most 44 bytes and at most 2.2 times its request (a bare 20-byte request, which browsers and libwebrtc send, gets 32 bytes for IPv4 or 44 for IPv6), so the server is useless as a traffic amplifier. Each source address gets `STUN_RATE_PER_MIN` answers per minute (default `60`), and the table of tracked addresses is bounded. `STUN_PORT` moves the port, `STUN_DISABLED=1` turns the responder off. Senders are never logged.

**Static web UI (experimental; specified in `add-offline-lan`, task 1.3):** `--serve-web <dir>` (or `GS_SERVE_WEB`) serves a built web UI from the same port, with `/index.html` as the fallback for client routes. `/healthz` and WebSocket upgrades keep working.

**Local mode (experimental; specified in `add-offline-lan`, task 1.3; `--local`, [ADR-021](../../openspec/changes/archive/2026-10-20-adr-021-offline-lan-topologies-locales-and-distribution/design.md) §1):** for a PC that hosts the relay on a LAN without internet. The UI and the registration endpoint bind to `127.0.0.1` and `::1` only (`--ui-port`, default `8080`); every request there must carry a loopback `Host` with that port, and a present `Origin` must be a loopback page. Only the signaling port (`PORT`, bound to `GS_BIND`, default `0.0.0.0`) is on the LAN, and it serves nothing but signaling and `/healthz`. The relay requires admission: each joiner receives a nonce in a `challenge` message and answers with `admit`, whose `mac` is the lowercase hex of `HMAC-SHA256(admission key, "gs-relay-admit/1|" + nonce + "|" + role + "|" + room id)` (`role` is the joining role, `baby` or `parent`); a role slot is taken only after a valid answer, and sockets that do not authenticate within 5 seconds are closed. The local UI registers the room's admission key (`HKDF(PSK, "guardedsleep relay admission v1")`, 43 base64url characters) over loopback: `PUT /admission/<room id>` with the key as the body (`204`; the same key again is accepted, a different key for a known room gets `409` until `DELETE /admission/<room id>`). The relay never sees the PSK. The public server runs without admission (`hello`): it cannot know any PSK, and the end-to-end pairing proof plus DTLS protect the call as before.

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
- `STUN_PORT` — host UDP port published for STUN (default `3478`); `STUN_DISABLED=1` turns the responder off

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
docker run --rm --read-only --cap-drop ALL -p 8787:8787 -p 3478:3478/udp guardedsleep-signaling:local
```

For a development binary without Docker: `just dev` (server and Vite) or `scripts/cargo.sh run -p guardedsleep-server` on a host with cargo. `just smoke` runs the end-to-end smoke test against the built binary.

## CI: publish to GHCR

The workflow [`.github/workflows/docker-signaling.yml`](../../.github/workflows/docker-signaling.yml) publishes `ghcr.io/<owner-lowercase>/guardedsleep-signaling` for the **`linux/amd64`** and **`linux/arm64`** platforms.

### Tag strategy ([ADR-008](../../openspec/changes/archive/2026-05-13-adr-008-container-image-build-and-distribution/design.md))

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
