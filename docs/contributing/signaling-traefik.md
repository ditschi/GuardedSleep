# Signaling server behind Traefik (TLS, domain, WebSocket)

This complements [signaling-server-docker.md](signaling-server-docker.md). Use it when you run **Traefik** as a reverse proxy in front of the signaling container so clients use **`wss://`** on your own domain.

## What you need to decide

1. **DNS** — an `A` or `AAAA` record pointing your hostname (e.g. `signal.example.com`) to the host that runs Traefik (or its load balancer).
2. **TLS** — Traefik terminates TLS (e.g. Let’s Encrypt via an ACME `certificatesResolver`). The signaling container stays **HTTP-only** on port `8787`; Traefik speaks **HTTPS/WSS** on 443.
3. **App URL** — the mobile app must be configured with the **public WebSocket URL**, e.g. `wss://signal.example.com` (path `/` unless you add a path prefix; default server has no path).
4. **`SIGNALING_ALLOWED_ORIGINS`** — if you set this env var on the signaling server, it must list the **Origin** values your clients send on the WebSocket handshake:
   - **Capacitor / WebView** often sends no `Origin` or a custom scheme — those are still **allowed** when the allowlist is set (same as our default policy).
   - **Browser-based** testing might send `Origin: https://app.example.com` — add that exact origin string.
5. **Long-lived connections** — WebSockets are long-lived. If Traefik or an upstream proxy closes idle connections too aggressively, raise idle/read timeouts for the signaling route (Traefik v2/v3: transport responding timeouts, or service-specific settings).
6. **Health checks** — Docker/Kubernetes health checks should hit `http://127.0.0.1:8787/` **inside** the container (plain HTTP), not through Traefik, unless you probe the public URL intentionally.

## Compose: Traefik labels (example)

Traefik discovers services via **labels** on the same Docker network as Traefik. Names (`certresolver`, `entrypoints`, network name) must match **your** Traefik static/dynamic config.

Example labels for a `signaling` service (adjust domain, network, resolver, entrypoint):

```yaml
services:
  signaling:
    image: guardedsleep-signaling:local # or GHCR image
    environment:
      PORT: "8787"
      # Optional: lock browser Origins (exact strings). Native clients without Origin still work.
      # SIGNALING_ALLOWED_ORIGINS: "https://parent.example.com"
    networks:
      - proxy
    labels:
      - "traefik.enable=true"
      - "traefik.docker.network=proxy"
      - "traefik.http.routers.guardedsleep-signaling.rule=Host(`signal.example.com`)"
      - "traefik.http.routers.guardedsleep-signaling.entrypoints=websecure"
      - "traefik.http.routers.guardedsleep-signaling.tls=true"
      - "traefik.http.routers.guardedsleep-signaling.tls.certresolver=letsencrypt"
      - "traefik.http.services.guardedsleep-signaling.loadbalancer.server.port=8787"

networks:
  proxy:
    external: true # or attach to the network your Traefik container uses
```

Notes:

- Replace `signal.example.com` with your domain (backticks in `Host(\`...\`)` are required by Traefik’s rule syntax).
- Replace `websecure` / `letsencrypt` / `proxy` with your entrypoint, certificate resolver, and network names.
- **WebSocket**: Traefik forwards the upgrade automatically for HTTP/1.1; no extra label is required for a plain upgrade to the same backend port.
- If you use a **path prefix** (e.g. `/signaling`), you need matching router rules **and** the app must use `wss://signal.example.com/signaling` — the stock server is mounted at `/`, so path prefixes are an advanced setup.

## Merging with this repo’s Compose files

You can:

- **Copy** the `signaling` service (and env) from [compose.build.yaml](../../docker/signaling-server/compose.build.yaml) into your stack file and **add** the Traefik labels and `networks:`; or
- Use **Compose `extends`** / multiple compose files (`docker compose -f traefik.yml -f docker/signaling-server/compose.build.yaml`) — order and overrides are your responsibility.

A minimal **label-only** example lives at [compose.traefik.example.yaml](../../docker/signaling-server/compose.traefik.example.yaml).

## Full stack: static PWA + signaling

To expose both the **browser app** (HTTPS) and **signaling** (`wss://`) on separate hostnames, see [compose.web-and-signaling.traefik.example.yaml](../../docker/stack/compose.web-and-signaling.traefik.example.yaml) and [mobile-web-docker.md](mobile-web-docker.md). Build the **`guardedsleep-web:local`** image with `--build-arg VITE_SIGNALING_URL=wss://<your-signal-host>` so the default client points at your public signaling URL; users can still override the URL in the app UI.

## Checklist before going live

- [ ] `wss://` URL reachable from a client on the internet (firewall, DNS, TLS).
- [ ] App configured to that `wss://` URL (QR / settings).
- [ ] If using `SIGNALING_ALLOWED_ORIGINS`, tested with real client Origins (browser + native).
- [ ] Monitoring: Traefik access logs will show connection metadata to the signaling host (expected).
