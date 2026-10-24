# Static web app (PWA) and full stack with Docker

The web client (`apps/web`, Vite 8 + Svelte 5) builds to a **static site** with a **service worker** and **web app manifest** (`vite-plugin-pwa`). You can serve it with any static file host; this repository includes an **nginx** image recipe and **Compose** examples that run **PWA + signaling** together. The nginx config sends the Content-Security-Policy that `apps/web/csp/csp.ts` defines (a Vitest test fails when they drift).

## Signaling URL

The PWA has no build-time signaling URL. A page without configured signaling URLs connects to the origin that served it (`apps/web/src/lib/platform/relay-stun.ts`); the image's nginx proxies WebSocket upgrades on any path to the signaling server. Set **`SIGNALING_ALLOWED_ORIGINS`** on the signaling server to the **exact browser Origin** of the PWA (e.g. `https://app.example.com` or `http://192.168.1.10:8080`). Native Android clients send **no** `Origin` and remain allowed when the allowlist is set.

## PWA installation and HTTPS

Browsers generally require **HTTPS** (or `localhost`) for service workers and for a reliable **“Install app”** / add-to-home-screen flow. For internet-facing deployments, terminate TLS with **Traefik** (or another reverse proxy) as in [signaling-traefik.md](signaling-traefik.md).

## Build the web image only

From the **repository root**:

```bash
docker build -f docker/web/Dockerfile -t guardedsleep-web:local .
```

The runtime stage serves the Vite output (`apps/web/dist`) from `/usr/share/nginx/html` with SPA fallback (`try_files … /index.html`).

## Compose: PWA + signaling (no Traefik)

For a quick **HTTP** lab setup (limited PWA install from other devices), use:

- [docker/stack/compose.yaml](../../docker/stack/compose.yaml) — builds **signaling** and **web** from the current repo
- Example variables: [docker/stack/.env.example](../../docker/stack/.env.example)
- Short overview: [docker/stack/README.md](../../docker/stack/README.md)

From the **repository root**:

```bash
docker compose -f docker/stack/compose.yaml up --build
```

Defaults are set in the Compose file (`${VAR:-…}`); no `.env` is required. For custom ports, or LAN origins, use `docker/stack/.env` and `--env-file docker/stack/.env` (see [.env.example](../../docker/stack/.env.example)).

Then open `http://127.0.0.1:8080/` (or your host LAN IP on port `8080`).

## Compose: PWA + signaling behind Traefik

Ready-to-use file, configured via `.env` only: [docker/stack/compose.traefik.yaml](../../docker/stack/compose.traefik.yaml) + [traefik.env.example](../../docker/stack/traefik.env.example). It serves the app at `https://<APP_DOMAIN>/` and signaling at `wss://<APP_DOMAIN>/signaling` (the PWA connects to its own origin).

Details, variables, and Caddy / nginx equivalents: [signaling-traefik.md](signaling-traefik.md).

## CI: publish to GHCR

The workflow [`.github/workflows/docker-web.yml`](../../.github/workflows/docker-web.yml) publishes `ghcr.io/<owner-lowercase>/guardedsleep-web` for the **`linux/amd64`** and **`linux/arm64`** platforms.

### Tag strategy ([ADR-008](../../openspec/changes/archive/2026-05-13-adr-008-container-image-build-and-distribution/design.md))

Identical to the signaling image:

| Tag                    | When                                             | Intended consumer                                 |
| ---------------------- | ------------------------------------------------ | ------------------------------------------------- |
| `:latest`              | A `web-v*` tag is pushed (manual during alpha) | Production self-hosters wanting "stable, current" |
| `:vX.Y.Z` / `:vX.Y`    | Same as `:latest`                                | Reproducible / auto-patch deploys                 |
| `:vX` (post-1.0 only)  | Same as `:latest`                                | Auto-minor within a major                         |
| `:dev` (alias `:edge`) | Every push to `main`                             | Testers — **use this until first release**        |
| `:sha-<short>`         | Every push                                       | Exact-commit debugging                            |

### Published image

The image contains no signaling URL and no per-deployment setting, so one published image works for everyone.

The build-time TLS certificate (self-signed) is also generated with `localhost` + `127.0.0.1` only in the published images; LAN-IP coverage requires a local rebuild with `TLS_EXTRA_SAN=IP:192.168.x.y`.

## End-user deployment from GHCR

Drop-in self-hosting of **web + signaling** together without cloning this repo: see [docker/stack/README.md](../../docker/stack/README.md) Mode B, which pulls both `guardedsleep-signaling` and `guardedsleep-web` from GHCR using a single compose file.

## Relation to signaling-only Compose

The signaling server alone is documented in [signaling-server-docker.md](signaling-server-docker.md). The stack under `docker/stack/` adds the **static web** service for self-hosted **browser** clients; **Android APK** builds are built with `just build-android` (the `android` job in `.github/workflows/ci.yml`).
