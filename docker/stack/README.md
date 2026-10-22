# Stack: web (PWA) + signaling

Two ways to run the full GuardedSleep stack: **build from source** (this repo) or **pull pre-built images from GHCR** (no clone needed).

| Mode                                 | Compose file                                                                                    | When to use                                                                                                                                                                                                            |
| ------------------------------------ | ----------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **A. Build from source** (this repo) | [`compose.yaml`](compose.yaml) (+ optional [`.env.example`](.env.example))                      | Development, customising `VITE_SIGNALING_URL` at build time, testing local code changes.                                                                                                                               |
| **B. Pre-built images (GHCR)**       | [`compose.ghcr.yaml`](compose.ghcr.yaml) (+ [`.env.ghcr.example`](.env.ghcr.example))           | Deployments / self-hosting. **No repo clone required** — download just the two files. Tag strategy: see [ADR-008](../../openspec/changes/archive/2026-05-13-adr-008-container-image-build-and-distribution/design.md). |
| **C. Pre-built images + Traefik**    | [`compose.traefik.yaml`](compose.traefik.yaml) (+ [`traefik.env.example`](traefik.env.example)) | Behind an existing Traefik on 443. One domain: app at `/`, signaling at `/signaling`. Configure via `.env` only — see [signaling-traefik.md](../../docs/contributing/signaling-traefik.md).                            |

## Requirements

- Docker with Compose v2

For **Mode A** also run commands from the **repository root** (so build contexts resolve correctly).

### Phone / tablet on the same Wi‑Fi (recommended)

`127.0.0.1` in the default URL is the **phone itself**, not your PC — WebSocket signaling fails from a second device. Until a public EU community signaling server is configured, use the **local Docker stack with your host’s LAN IP**:

```bash
just stack-lan
```

This runs [`scripts/stack-lan-env.sh`](../../scripts/stack-lan-env.sh) (detects LAN IPv4, writes `docker/stack/.env`) and starts `docker compose … up --build`. The PWA and Android `cap sync` build use `VITE_SIGNALING_URL=wss://<your-LAN-IP>:8443/signaling`.

On the phone (same Wi‑Fi): open **`https://<LAN-IP>:8443`** (accept the self-signed certificate warning).

Regenerate `.env` only: `bash scripts/stack-lan-env.sh` · Start stack with existing `.env`: `just stack-up`

## Mode A — build from source (current repo)

**Defaults only (no env file):** every variable in `compose.yaml` uses `${VAR:-default}`, so this is enough:

```bash
docker compose -f docker/stack/compose.yaml up --build
```

**Custom ports / LAN / signaling URL:** copy [`.env.example`](.env.example) to `docker/stack/.env`, edit, then:

```bash
docker compose -f docker/stack/compose.yaml --env-file docker/stack/.env up --build
```

(`docker/stack/.env` is **not** auto-loaded unless you pass `--env-file`; Compose only loads a `.env` from your **current working directory** by default, usually the repo root.)

## Mode B — pre-built images from GHCR (no clone required)

For end-user self-hosting. Works without cloning this repository — just two files.

1. Download `compose.ghcr.yaml` and `.env.ghcr.example` from the repo into an empty directory on the host.
2. `cp .env.ghcr.example .env` and edit at least:
   - `GUARDEDSLEEP_OWNER` — the GitHub owner/org publishing the images (lowercase).
   - `SIGNALING_ALLOWED_ORIGINS` — the **exact** browser origins of your PWA (e.g. `https://baby.example.com` or `http://192.168.1.10:8080`).
   - `SIGNALING_TAG` / `WEB_TAG` — `latest` (stable), `dev` (bleeding edge), or `vX.Y.Z` (pinned). See the Tag strategy section below.
3. Run:

   ```bash
   docker compose -f compose.ghcr.yaml --env-file .env pull
   docker compose -f compose.ghcr.yaml --env-file .env up -d
   ```

Open `http://<host>:${WEB_PORT}/` (default `:8080`) or `https://<host>:${WEB_HTTPS_PORT}/` (default `:8443`, self-signed warning expected). The PWA's signaling URL can be set in-app (Settings) — the published `:latest` image is built without a baked-in `VITE_SIGNALING_URL`.

### Tag strategy (per [ADR-008](../../openspec/changes/archive/2026-05-13-adr-008-container-image-build-and-distribution/design.md))

| Tag                    | Pushed when                                      | Use for                           |
| ---------------------- | ------------------------------------------------ | --------------------------------- |
| `:latest`              | A `signaling-server-v*` / `mobile-v*` tag exists | Production self-hosting           |
| `:vX.Y.Z`              | Same release tag                                 | Reproducible / pinned deployments |
| `:vX.Y`                | Same release tag                                 | Auto-patch updates within a minor |
| `:vX` (post-1.0 only)  | Same release tag                                 | Auto-minor updates within a major |
| `:dev` (alias `:edge`) | Every push to `main`                             | Testers / bleeding edge           |
| `:sha-<short>`         | Every push                                       | Debug / exact commit reproduction |

**Important:** until GuardedSleep cuts its first release, `:latest` does **not** exist. Use `SIGNALING_TAG=dev` and `WEB_TAG=dev` during the pre-release / PoC phase. ([ADR-001](../../openspec/changes/archive/2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/design.md) defines the PoC gate; [ADR-006](../../openspec/changes/archive/2026-05-13-adr-006-release-and-changelog-automation/design.md) governs when tags are created.)

- **PWA / UI (HTTP):** `http://127.0.0.1:8080/` (or your `WEB_PORT`)
- **PWA / UI (HTTPS, self-signed):** `https://127.0.0.1:8443/` (or your `WEB_HTTPS_PORT`; browser warning is normal)
- **Signaling (health):** `http://127.0.0.1:8787/`
- Default **`VITE_SIGNALING_URL`** is `wss://127.0.0.1:8443/signaling` (TLS on the web port; nginx proxies `/signaling` to the signaling container). For HTTP-only on port 8080, set `VITE_SIGNALING_URL=ws://127.0.0.1:8787` and rebuild **web**.

Adjust **`SIGNALING_ALLOWED_ORIGINS`** in `docker/stack/.env` so it matches every origin users type in the address bar (e.g. add `http://192.168.1.10:8080` for LAN testing). Rebuild is not required for that env change on **signaling**; **`web`** only needs rebuild when you change `VITE_SIGNALING_URL` or app sources.

### HTTPS on a phone (connection failed / WebSocket)

The default bundle uses **`wss://127.0.0.1:8443/signaling`**. On a **phone**, `127.0.0.1` is the phone, not your PC — the app cannot reach signaling → “connection failed”.

1. Find your PC’s LAN IP (e.g. `192.168.1.42`).
2. In `docker/stack/.env` set `VITE_SIGNALING_URL=wss://192.168.1.42:8443/signaling` and add `https://192.168.1.42:8443` (and `http://192.168.1.42:8080` if needed) to **`SIGNALING_ALLOWED_ORIGINS`**.
3. `docker compose -f docker/stack/compose.yaml --env-file docker/stack/.env up --build`
4. On the phone open **`https://192.168.1.42:8443`** (not `127.0.0.1`). Accept the self-signed certificate warning.

**Without rebuilding:** open the app, go to **Settings**, and set the signaling URL to `wss://<PC-IP>:8443/signaling` (must match how you open the page). Ensure the signaling allowlist includes your `https://<PC-IP>:8443` origin (edit `.env`, restart **signaling** only).

### HTTPS certificate (self-signed)

The **web** image creates a **self-signed** TLS certificate at build time (`openssl` in `docker/mobile-web/Dockerfile`). It is **not** from a public CA. By default it is valid for **`localhost`** and **`127.0.0.1`** only. Opening **`https://<your-LAN-IP>:8443`** still works, but the browser may warn about name mismatch until you proceed.

Optional: set **`TLS_EXTRA_SAN=IP:192.168.1.42`** (your PC’s IP) in `docker/stack/.env` and **`up --build`** so the cert includes that IP and warnings are reduced.

If you **cannot load the UI at all** from the phone, check: same Wi‑Fi, use **`https://<PC-LAN-IP>:8443`** (not `127.0.0.1`), host firewall allows TCP **8443**, and `docker compose ps` shows ports published (`0.0.0.0:8443->443`).

## Files

| File                                                         | Role                                                                 |
| ------------------------------------------------------------ | -------------------------------------------------------------------- |
| [compose.yaml](compose.yaml)                                 | **Mode A** — build both images from this repo (local development)    |
| [.env.example](.env.example)                                 | Mode A defaults — ports, origins, `VITE_SIGNALING_URL`               |
| [`scripts/stack-lan-env.sh`](../../scripts/stack-lan-env.sh) | Auto-write `.env` with host LAN IP (`bash scripts/stack-lan-env.sh`) |
| [compose.ghcr.yaml](compose.ghcr.yaml)                       | **Mode B** — pull pre-built images from GHCR (no repo clone needed)  |
| [.env.ghcr.example](.env.ghcr.example)                       | Mode B defaults — owner, tags, origins                               |
| [compose.traefik.yaml](compose.traefik.yaml)                 | **Mode C** — GHCR images behind an existing Traefik (one domain)     |
| [traefik.env.example](traefik.env.example)                   | Mode C defaults — domain, Traefik network/entrypoint/resolver        |

More detail: [docs/contributing/mobile-web-docker.md](../../docs/contributing/mobile-web-docker.md). Tag strategy: [ADR-008](../../openspec/changes/archive/2026-05-13-adr-008-container-image-build-and-distribution/design.md).
