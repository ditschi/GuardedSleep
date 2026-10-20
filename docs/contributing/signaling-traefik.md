# Signaling server behind a reverse proxy (Traefik, Caddy, nginx)

> **Superseded by [ADR-020](../decisions/ADR-020-toolchain-and-development-setup.md) §6:** the Node signaling image referenced here is replaced by the Rust axum server; the T1 plan adds a Caddy + signaling (+ TURN) compose bundle with a self-hosting guide. Replaced in toolchain realignment wave 2 and T1; this procedure stays valid until then.

This complements [signaling-server-docker.md](signaling-server-docker.md). Use it when you already run a **reverse proxy on port 443** and want clients to reach signaling via **`wss://` on your own domain**.

## How it works

- The signaling container speaks **plain HTTP on port `8787`** (internal only). A WebSocket is an HTTP request with an `Upgrade` header, so any reverse proxy that forwards upgrades can route it by **hostname and/or path** — no extra public port, no extra Traefik entrypoint.
- The server accepts WebSocket upgrades on **any path**. Routing `/signaling` (or any prefix) to it needs **no prefix stripping**.
- `GET /healthz` (also `<prefix>/healthz`) returns `200 ok` for health checks.
- TLS terminates at the proxy. The container never needs a certificate.

## Option 1: Traefik, ready-to-use Compose files (recommended)

No YAML editing: download a compose file plus its env template, fill in `.env`, start.

| File                                                                                                 | Routes                                                                                        |
| ---------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| [`docker/stack/compose.traefik.yaml`](../../docker/stack/compose.traefik.yaml)                       | **One domain:** `https://<APP_DOMAIN>/` → web app, `wss://<APP_DOMAIN>/signaling` → signaling |
| [`docker/signaling-server/compose.traefik.yaml`](../../docker/signaling-server/compose.traefik.yaml) | Signaling only: `wss://<SIGNALING_DOMAIN><SIGNALING_PATH_PREFIX>`                             |

```bash
cp traefik.env.example .env   # then set APP_DOMAIN (or SIGNALING_DOMAIN) and GUARDEDSLEEP_OWNER
docker compose -f compose.traefik.yaml up -d
```

Variables (see `traefik.env.example` next to each file):

| Variable                    | Default                | Purpose                                                               |
| --------------------------- | ---------------------- | --------------------------------------------------------------------- |
| `APP_DOMAIN`                | —                      | Web app hostname (stack); also derives the Origin allowlist           |
| `SIGNALING_DOMAIN`          | —                      | Signaling hostname (signaling-only file)                              |
| `SIGNALING_PATH_PREFIX`     | `/`                    | Optional path, e.g. `/signaling` (signaling-only file)                |
| `TRAEFIK_NETWORK`           | `proxy`                | External Docker network shared with Traefik                           |
| `TRAEFIK_ENTRYPOINT`        | `websecure`            | HTTPS entrypoint name from your Traefik static config                 |
| `TRAEFIK_CERTRESOLVER`      | `letsencrypt`          | ACME resolver name                                                    |
| `TRAEFIK_ROUTER_PREFIX`     | `guardedsleep`         | Router/service name prefix (Traefik names are global across stacks)   |
| `SIGNALING_ALLOWED_ORIGINS` | `https://<APP_DOMAIN>` | Browser Origin allowlist; native clients without `Origin` always pass |

**Why one domain is the easy path:** one DNS record, one certificate, and the browser Origin equals the app origin. The PWA infers `wss://<its-own-host>/signaling` from its page origin, so neither a rebuild with `VITE_SIGNALING_URL` nor a manual URL setting is needed. The signaling router's rule (`Host(...) && PathPrefix(/signaling)`) is longer than the web router's, so Traefik gives it priority.

## Option 2: Caddy

Caddy forwards WebSocket upgrades automatically and obtains certificates on its own.

```caddyfile
baby.example.com {
	reverse_proxy /signaling* signaling:8787
	reverse_proxy web:80
}
```

Signaling on its own hostname instead:

```caddyfile
signal.example.com {
	reverse_proxy signaling:8787
}
```

## Option 3: nginx

nginx needs the `Upgrade` / `Connection` headers and a long read timeout (WebSockets are long-lived):

```nginx
map $http_upgrade $connection_upgrade {
    default upgrade;
    ''      close;
}

server {
    listen 443 ssl;
    server_name baby.example.com;
    # ssl_certificate / ssl_certificate_key …

    location /signaling {
        proxy_pass http://signaling:8787;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection $connection_upgrade;
        proxy_set_header Host $host;
        proxy_read_timeout 3600s;
        proxy_send_timeout 3600s;
    }

    location / {
        proxy_pass http://web:80;
    }
}
```

The `guardedsleep-web` image ships the same `/signaling` proxy internally ([nginx.default.conf](../../docker/mobile-web/nginx.default.conf)), which is how the LAN stack works without an external proxy.

## Notes for all proxies

- **Long-lived connections:** if your proxy closes idle connections aggressively, raise idle/read timeouts for the signaling route (Traefik: `respondingTimeouts` on the entrypoint; nginx: `proxy_read_timeout`).
- **Idle WebSockets and ping/pong:** the server sends a WebSocket ping every 10 s and clients send an application `ping` every 15 s, but a quiet room can still be idle for more than 35 s at the application level. Proxies must keep idle WebSocket connections open for **more than 35 s** and forward WebSocket ping/pong frames (Traefik and nginx defaults are fine; with nginx keep `proxy_read_timeout` at 60 s or more, the example above uses 3600 s).
- **Per-IP rate limits:** the server limits hello attempts and new rooms per client IP. Behind a proxy set `SIGNALING_TRUST_PROXY=1` so the client IP is taken from `X-Forwarded-For`; otherwise all clients share the proxy's IP. Traefik appends the peer address as the **last** hop, and the server uses the rightmost hop (or the n-th from the right with `SIGNALING_TRUSTED_PROXY_HOPS=n` when n proxies are chained, e.g. Traefik behind a CDN). Client-supplied leading entries are ignored. Only enable it when every connection really passes through the trusted proxies.
- **Health checks** should hit `http://127.0.0.1:8787/healthz` **inside** the container (the image's `HEALTHCHECK` does this), not through the proxy.
- **Access logs:** the proxy logs connection metadata (IP, time) for the signaling host. That is expected; avoid shipping these logs to third parties (privacy guardrail 6).
- **Origins:** browser clients send `Origin: https://<app-domain>`; it must be in `SIGNALING_ALLOWED_ORIGINS`. Native apps send no Origin and are accepted.

## TURN is different (optional)

A self-hosted TURN server (coturn, see ADR-003) relays media and **cannot** go through an HTTP reverse proxy. It needs UDP `3478` plus a UDP relay port range opened in your firewall. Alternative: TURN over TLS can share port 443 with a **Traefik TCP router** (`HostSNI(\`turn.example.com\`)` with TLS passthrough). Signaling alone never needs more than 443.

## Checklist before going live

- [ ] DNS for the app/signaling hostname points at your proxy.
- [ ] `https://<domain>/signaling/healthz` (or `https://<signal-domain>/healthz`) returns `ok` through the proxy.
- [ ] App connects via `wss://` (browser + native).
- [ ] `SIGNALING_ALLOWED_ORIGINS` matches the browser origin(s) you actually use.
