# Static web app (PWA) and full stack with Docker

The web client (`apps/web`, Vite 8 + Svelte 5) builds to a **static site** with a **service worker** and **web app manifest** (`vite-plugin-pwa`). You can serve it with any static file host; this repository includes an **nginx** image recipe and **Compose** examples that run **PWA + signaling** together. The nginx config sends the Content-Security-Policy that `apps/web/csp/csp.ts` defines (a Vitest test fails when they drift).

## Signaling URL

The PWA has no build-time signaling URL. A page without configured signaling URLs connects to the origin that served it (`apps/web/src/lib/platform/relay-stun.ts`); the image's nginx proxies WebSocket upgrades on any path to the signaling server. Set **`SIGNALING_ALLOWED_ORIGINS`** on the signaling server to the **exact browser Origin** of the PWA (e.g. `https://app.example.com` or `http://192.168.1.10:8080`). Native Android clients send **no** `Origin` and remain allowed when the allowlist is set.

## PWA installation and HTTPS

Browsers generally require **HTTPS** (or `localhost`) for service workers and for a reliable **“Install app”** / add-to-home-screen flow. For internet-facing deployments, terminate TLS with **Traefik** (or another reverse proxy) as in [docker/public-server/README.md](../../docker/public-server/README.md).

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

## Compose: signaling behind Traefik

The one Traefik example of the repository is [docker/public-server/](../../docker/public-server/README.md) (signaling and coturn behind an existing Traefik). Its optional `web` profile serves this PWA on its **own origin**, never on the signaling origin (README, "Self-hosting variants"); Caddy and nginx equivalents for the signaling route are in the same README.

## CI: publish to GHCR

The workflow [`.github/workflows/docker-web.yml`](../../.github/workflows/docker-web.yml) (triggers; the build is the reusable [`docker-image.yml`](../../.github/workflows/docker-image.yml)) publishes `ghcr.io/<owner-lowercase>/guardedsleep-web` for the **`linux/amd64`** and **`linux/arm64`** platforms.

### Tag strategy ([ADR-008](../../openspec/changes/archive/2026-05-13-adr-008-container-image-build-and-distribution/design.md))

Identical to the signaling image:

| Tag                    | When                                           | Intended consumer                                 |
| ---------------------- | ---------------------------------------------- | ------------------------------------------------- |
| `:latest`              | A `web-v*` tag is pushed (manual during alpha) | Production self-hosters wanting "stable, current" |
| `:vX.Y.Z` / `:vX.Y`    | Same as `:latest`                              | Reproducible / auto-patch deploys                 |
| `:vX` (post-1.0 only)  | Same as `:latest`                              | Auto-minor within a major                         |
| `:dev` (alias `:edge`) | Every push to `main`                           | Testers — **use this until first release**        |
| `:sha-<short>`         | Every push                                     | Exact-commit debugging                            |

### Published image

The image contains no signaling URL and no per-deployment setting, so one published image works for everyone.

The build-time TLS certificate (self-signed) is also generated with `localhost` + `127.0.0.1` only in the published images; LAN-IP coverage requires a local rebuild with `TLS_EXTRA_SAN=IP:192.168.x.y`.

## Staging image (test deployment)

`ghcr.io/<owner-lowercase>/guardedsleep-web-staging` is published by the second job of `docker-web.yml` from `main` (`:dev`, `:edge`, `:sha-<short>`), never for pull requests and never on release tags. It is the owner's test deployment of the official origin (`https://app.gs.mydata.rocks`), the web analogue of an Android debug build:

- built with `--build-arg WEB_BUILD_MODE=staging --build-arg VITE_GS_OFFICIAL_DEPLOYMENT=true` ([Vite modes](https://vite.dev/guide/env-and-mode#modes)), so new invites carry the project server and the CSP `connect-src` lists `wss://gs.mydata.rocks`;
- loopback mailboxes stay refused, as in the release image;
- the Dockerfile accepts only `production` (default) and `staging` as `WEB_BUILD_MODE`.

It is not a release: do not offer it to the public as the web client. The signaling server must list the web origin in `SIGNALING_ALLOWED_ORIGINS`, and `/signaling` must not be routed to this image on its host (the browser talks to `wss://gs.mydata.rocks/signaling` directly). Release builds (`guardedsleep-web`, mode `production`) are unchanged and keep the project server off until task 6.2 of `alpha-connection-model`.

## End-user deployment from GHCR

Drop-in self-hosting of **web + signaling** together without cloning this repo: see [docker/stack/README.md](../../docker/stack/README.md) Mode B, which pulls both `guardedsleep-signaling` and `guardedsleep-web` from GHCR using a single compose file.

## Relation to signaling-only Compose

The signaling server alone is documented in [signaling-server-docker.md](signaling-server-docker.md). The stack under `docker/stack/` adds the **static web** service for self-hosted **browser** clients; **Android APK** builds are built with `just build-android` (the `android` job in `.github/workflows/ci.yml`).
