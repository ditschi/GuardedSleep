# GuardedSleep public server (signaling and TURN)

The deployment of the project server `gs.mydata.rocks`, and the working example for anyone who runs their own: the Rust signaling server behind an existing Traefik, plus coturn for STUN and TURN on host networking. Design and rationale: [`design.md` section 9](../../openspec/changes/alpha-connection-model/design.md) and section 7 of the change `alpha-connection-model`. Files:

| File                             | Purpose                                                                            |
| -------------------------------- | ---------------------------------------------------------------------------------- |
| [`compose.yaml`](compose.yaml)   | `signaling`, `coturn` and the optional `web` profile (off)                         |
| [`.env.template`](.env.template) | every variable with its comment; copy to `.env` (git-ignored, holds `TURN_SECRET`) |
| [`check-turn.sh`](check-turn.sh) | the TURN allocation check for monitoring                                           |

## What this stack assumes

- **Traefik runs outside this repository** (the owner's homelab repository, `compose/contabo/traefik`): a Docker network named `traefik`, an HTTPS entrypoint (its name goes into `TRAEFIK_ENTRYPOINT`), the certificate resolver `letsencrypt`, the file middleware `secure-headers@file`, **access log off**, and idle timeouts of at least 35 s for WebSockets.
- **Real client addresses at Traefik.** The signaling server takes the client address from the last `X-Forwarded-For` hop (`SIGNALING_TRUST_PROXY=1`, one hop). Traefik must therefore see real IPv6 client addresses (an IPv6-enabled `traefik` network or host-mode ports); behind docker-proxy all IPv6 clients would share one address and every per-address limit would be wrong.
- **The host has its public IPv4 and IPv6 on the interface** (no 1:1 NAT, as on Contabo), so coturn listens and relays on them directly and no `--external-ip` mapping is needed. On a provider with NAT, see [Self-hosting variants](#self-hosting-variants).
- **DNS:** `gs.mydata.rocks` points at the host, DNS only (no proxying CDN, which would see the traffic).

## First deployment

1. On the host: `git clone` the repository (a read-only deploy key while the repository is private) and `cd docker/public-server`.
2. `cp .env.template .env` and edit it. Set `TURN_SECRET` to the output of `openssl rand -hex 32`, check `TRAEFIK_ENTRYPOINT` against your Traefik, and check `TURN_IPV4` and `TURN_IPV6` against `ip -br addr`.
3. Set `SIGNALING_TAG` to a `signaling-server-vX.Y.Z` release, without the prefix (`vX.Y.Z`). **No such release exists until the owner cuts the first one after group 9 of `alpha-connection-model`**; the template's `vX.Y.Z` fails to pull on purpose, and the compose file never accepts `latest` or `dev`. The images must be readable by the host: a public GHCR package, or `docker login ghcr.io` with a read-only token.
4. Open the firewall (below).
5. `docker compose up -d`, then run the checks of [Monitoring](#monitoring) once by hand.

## Update

```bash
git pull && docker compose up -d
```

Change `SIGNALING_TAG` (and `COTURN_TAG`, kept equal to `COTURN_IMAGE` in `tools/poc-images.env`) in `.env` to move to a new release; compose recreates only what changed. There is no automatic image update; the owner pulls.

## Firewall

Inbound, IPv4 and IPv6 alike, everything else dropped:

| Port        | Protocol | Why                                                                  |
| ----------- | -------- | -------------------------------------------------------------------- |
| 22          | tcp      | SSH                                                                  |
| 80, 443     | tcp      | Traefik (ACME HTTP-01 and the WebSocket)                             |
| 3478        | udp, tcp | STUN and TURN                                                        |
| 5349        | tcp      | `turns:`, once task 10.2 lands                                       |
| 49160-49359 | udp      | relay ports (`TURN_MIN_PORT` to `TURN_MAX_PORT`; one per allocation) |

```bash
ufw default deny incoming
ufw allow 22/tcp && ufw allow 80/tcp && ufw allow 443/tcp
ufw allow 3478 && ufw allow 49160:49359/udp
ufw logging off   # the default writes blocked source addresses to the kernel log
ufw enable
```

**Docker's published ports bypass `ufw`.** This stack publishes none: `signaling` joins only the `traefik` network, and `coturn` uses host networking, so its sockets are reachable exactly where `ufw` allows. Keep it that way (a `ports:` entry would open the port regardless of the rules above). Keep `TURN_TOTAL_QUOTA` equal to the relay range (200 ports).

## Logs and IP addresses

The project server logs no client IP addresses (AGENTS.md section 7, guardrails 1 and 6). What each part does:

- **Traefik:** access log off, its own log at `ERROR` (homelab repository).
- **signaling:** logs no addresses or room ids; the per-address limits live in memory only.
- **coturn: logs nothing** (`--log-file=/dev/null --no-stdout-log`). coturn writes client and peer addresses at several levels, and a denied peer is an `ERROR` line with the peer's address (for example `A peer IP 10.x.x.x denied in the range ...`), and `--log-min-level` cannot go above `error`, so no level keeps addresses out; discarding the log does. Restarts and crash loops stay visible through `docker compose ps` and the monitoring below. To debug, put `TURN_LOG_FILE=stdout` into `.env`, run `docker compose up -d`, reproduce, then **remove it and run `docker compose up -d` again**: while it is set the logs contain addresses.
- **Docker:** `json-file` logs capped at 10 MB times 3 files per container.
- **Host:** `ufw logging off`.

**Check after a relayed test call** (do it on the live host after the first deployment, owner task 10.3), and look for the test client's address everywhere:

```bash
docker compose logs --no-color | grep -c '<client address>'      # expect 0
docker logs --since 1h <traefik container> 2>&1 | grep -c '<client address>'   # expect 0
journalctl --since '-1h' | grep -c '<client address>'            # expect 0 (kernel and system)
```

The same search runs locally against this compose file on every change, with a positive control that proves the search can find an address when coturn logs verbosely: `just public-server-logs` (needs Docker; a rootless bridge stands in for host networking).

## Monitoring

Uptime Kuma on the owner's Oracle host. Nothing about users is collected.

1. **Signaling:** an HTTP(s) monitor with a keyword on `https://gs.mydata.rocks/signaling/healthz`. The server answers `200` with the body `ok` under any path prefix; set the keyword to `ok` and the expected status to 200.
2. **TURN, with an authenticated allocation** (a STUN or TCP check on 3478 would not notice a `TURN_SECRET` mismatch that breaks every relayed call). [`check-turn.sh`](check-turn.sh) runs one allocation with `turnutils_uclient` inside the coturn container, with a credential signed by the secret **the signaling container really runs with**, and fails when coturn refuses it. In Uptime Kuma add a **Push** monitor, copy its push URL into `KUMA_PUSH_URL` in `.env`, set its heartbeat interval to 10 minutes (a silent host turns it red too) and add a cron entry on the host:

   ```cron
   */5 * * * * cd /opt/guardedsleep/docker/public-server && ./check-turn.sh
   ```

   The script pushes `up` or `down` itself; its exit status is 0 or 1 for any other scheduler. It reads only `.env` and the container environments; the push URL (a token) is never printed.

3. **Allocation count (abuse):** coturn logs nothing and has no counter in this profile; one relay port is bound per allocation, so the count of bound relay ports is the allocation count: `ss -H -u -a -n 'sport >= :49160 and sport <= :49359' | wc -l`. Near 200 (`--total-quota`) means exhaustion; lower `TURN_MAX_LIVE_PER_ADDR` or the quotas.

## Secret rotation

`TURN_SECRET` is shared by `signaling` (signs credentials) and `coturn` (verifies them). To rotate: put a new `openssl rand -hex 32` value into `.env` and run `docker compose up -d`; both containers are recreated together. Credentials already issued stop working for new allocations; clients get new ones at their next join or renewal (at most 5 minutes later). Run `./check-turn.sh` afterwards. Rotate after anyone who knew it leaves, and after any suspicion.

## Self-hosting variants

- **Another domain:** set `PUBLIC_HOST`, `TURN_URLS` and `STUN_URLS` (the server refuses URLs that name another host), a DNS record for the host, and `TURN_IPV4` / `TURN_IPV6` of your machine. Change the Traefik router host through `PUBLIC_HOST` only. For a browser client served from another origin, add that origin to `SIGNALING_ALLOWED_ORIGINS`.
- **Provider with 1:1 NAT (Oracle, AWS, GCP):** the private address must be bound and the public one advertised. Add `--external-ip=<public>/<private>` to coturn's command and put the private address into `TURN_IPV4`; `--denied-peer-ip` must still list the public address. [`docs/contributing/turn-server-vps.md`](../../docs/contributing/turn-server-vps.md) walks through such a host with the older single-purpose profile.
- **Without coturn:** delete the `coturn` service, `TURN_URLS`, `TURN_SECRET` and the TURN lines of `compose.yaml`; the server then sends no TURN credentials. For STUN, remove `STUN_DISABLED` from the `signaling` environment and publish `3478/udp` on it: the built-in STUN answers within 44 bytes and 2.2 times the request, with a per-address limit. Clients on strict NAT then cannot relay.
- **Without IPv6:** drop the two `--listening-ip`, `--relay-ip` and `--denied-peer-ip` entries that use `TURN_IPV6`.
- **The `web` profile** (`docker compose --profile web up -d`) serves the web client. It is **off on `gs.mydata.rocks` and stays off there**: whoever controls the origin that serves the web client controls the client completely (PSK, parent secret, microphone), so a web client is never co-hosted on the signaling origin, whose compromise must stay limited to metadata and denial of service. Give it its own host (`WEB_HOST`) and set `WEB_TAG`; the official web origin then goes into `SIGNALING_ALLOWED_ORIGINS`.

### Other reverse proxies

The signaling container speaks plain HTTP on `8787` and accepts WebSocket upgrades on any path, so any proxy that forwards upgrades works without prefix stripping; TLS ends at the proxy. Caddy forwards upgrades by itself:

```caddyfile
gs.example.org {
	reverse_proxy /signaling* signaling:8787
}
```

nginx needs the upgrade headers and a long read timeout:

```nginx
map $http_upgrade $connection_upgrade { default upgrade; '' close; }
server {
    listen 443 ssl;
    server_name gs.example.org;
    # ssl_certificate / ssl_certificate_key ...
    location /signaling {
        proxy_pass http://signaling:8787;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection $connection_upgrade;
        proxy_set_header Host $host;
        proxy_read_timeout 3600s;
    }
}
```

For every proxy: keep idle WebSockets open for more than 35 s; set `SIGNALING_TRUST_PROXY=1` (and `SIGNALING_TRUSTED_PROXY_HOPS=n` for n chained proxies) only when every connection passes through them, otherwise all clients share the proxy's address and the per-address limits fail; turn the proxy's access log off or keep it away from third parties (guardrail 6); health checks go to `http://127.0.0.1:8787/healthz` inside the container (the image's `HEALTHCHECK` does this). Server settings: [`signaling-server-docker.md`](../../docs/contributing/signaling-server-docker.md).

## Checklist before going live

- [ ] `docker compose config` passes with your `.env`; `docker compose ps` shows both services running.
- [ ] `https://gs.mydata.rocks/signaling/healthz` answers `ok` through Traefik.
- [ ] `./check-turn.sh` prints `check-turn: OK`.
- [ ] Firewall as above, `ufw logging off`; no `ports:` anywhere.
- [ ] The log search after a relayed test call finds no test client address in `docker compose logs`, Traefik's log or `journalctl`.
- [ ] Uptime Kuma monitors for both checks are green.
