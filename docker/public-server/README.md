# GuardedSleep public server (signaling and TURN)

The deployment of the project server `gs.mydata.rocks`, and the working example for anyone who runs their own: the Rust signaling server behind an existing Traefik, plus coturn for STUN and TURN on host networking. Design and rationale: [`design.md` section 9](../../openspec/changes/alpha-connection-model/design.md) and section 7 of the change `alpha-connection-model`. Files:

| File                                                                         | Purpose                                                                            |
| ---------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| [`compose.yaml`](compose.yaml)                                               | `signaling`, `coturn` and the optional `web` profile (off)                         |
| [`compose.lego.yaml`](compose.lego.yaml)                                     | override for the lego alternative: the `acme` service and coturn's certificate     |
| [`.env.template`](.env.template)                                             | every variable with its comment; copy to `.env` (git-ignored, holds `TURN_SECRET`) |
| [`check-turn.sh`](check-turn.sh)                                             | the TURN allocation check for monitoring                                           |
| [`acme-loop.sh`](acme-loop.sh), [`acme-deploy-hook.sh`](acme-deploy-hook.sh) | lego alternative: the `acme` service (issue, daily renewal check, install)         |
| [`reload-turn-cert.sh`](reload-turn-cert.sh)                                 | lego alternative, host cron: make coturn use a new certificate (restart, SIGUSR2)  |

## What this stack assumes

- **Traefik runs outside this repository** (the owner's homelab repository, `compose/contabo/traefik`): a Docker network named `traefik`, an HTTPS entrypoint (its name goes into `TRAEFIK_ENTRYPOINT`), the certificate resolver `letsencrypt`, the file middleware `secure-headers@file`, **access log off**, idle timeouts of at least 35 s for WebSockets, and the `turns` entrypoint and TCP router of [turns: through Traefik](#turns-through-traefik-default) (Traefik v3).
- **Real client addresses at Traefik.** The signaling server takes the client address from the last `X-Forwarded-For` hop (`SIGNALING_TRUST_PROXY=1`, one hop). Traefik must therefore see real IPv6 client addresses (an IPv6-enabled `traefik` network or host-mode ports); behind docker-proxy all IPv6 clients would share one address and every per-address limit would be wrong.
- **The host has its public IPv4 and IPv6 on the interface** (no 1:1 NAT, as on Contabo), so coturn listens and relays on them directly and no `--external-ip` mapping is needed. On a provider with NAT, see [Self-hosting variants](#self-hosting-variants).
- **DNS:** `gs.mydata.rocks` points at the host, DNS only (no proxying CDN, which would see the traffic).

## First deployment

1. On the host: `git clone` the repository (a read-only deploy key while the repository is private) and `cd docker/public-server`.
2. `cp .env.template .env` and edit it. Set `TURN_SECRET` to the output of `openssl rand -hex 32`, and give coturn the same value through a file (never on its command line, where `docker inspect` would show it): `umask 077; printf 'static-auth-secret=%s\n' "$(sed -n 's/^TURN_SECRET=//p' .env)" > turnserver.conf; sudo chown 65534:"$(id -g)" turnserver.conf; chmod 640 turnserver.conf` (coturn runs as `nobody`, uid 65534; your group keeps `check-turn.sh` able to read it; the file is git-ignored; `TURN_CONF_FILE` in `.env` moves it). Check `TRAEFIK_ENTRYPOINT` against your Traefik, and check `TURN_IPV4` and `TURN_IPV6` against `ip -br addr`.
3. Set `SIGNALING_TAG` to a `signaling-server-vX.Y.Z` release, without the prefix (`vX.Y.Z`). **No such release exists until the owner cuts the first one after group 9 of `alpha-connection-model`**; the template's `vX.Y.Z` fails to pull on purpose, and the compose file never accepts `latest` or `dev`. The images must be readable by the host: a public GHCR package, or `docker login ghcr.io` with a read-only token.
4. Open the firewall (below).
5. `docker compose up -d`, then run the checks of [Monitoring](#monitoring) once by hand.
6. Add the `turns` entrypoint and the TCP router to Traefik ([next section](#turns-through-traefik-default)) before or together with step 5; `TURN_URLS` in the template already lists the `turns:` URL.

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
| 80, 443     | tcp      | Traefik (ACME and the WebSocket)                                     |
| 3478        | udp, tcp | STUN and TURN                                                        |
| 5349        | tcp      | `turns:` (TLS, terminated by Traefik; with lego by coturn itself)    |
| 49160-49359 | udp      | relay ports (`TURN_MIN_PORT` to `TURN_MAX_PORT`; one per allocation) |

```bash
ufw default deny incoming
ufw allow 22/tcp && ufw allow 80/tcp && ufw allow 443/tcp
ufw allow 3478 && ufw allow 5349/tcp && ufw allow 49160:49359/udp
ufw logging off   # the default writes blocked source addresses to the kernel log
ufw enable
```

**Docker's published ports bypass `ufw`.** This stack publishes none: `signaling` joins only the `traefik` network, and `coturn` uses host networking, so its sockets are reachable exactly where `ufw` allows. Keep it that way (a `ports:` entry would open the port regardless of the rules above). Keep `TURN_TOTAL_QUOTA` equal to the relay range (200 ports).

## `turns:` through Traefik (default)

`turns:` (TURN over TLS on 5349) helps behind firewalls that pass only TLS-looking traffic. By default **Traefik terminates the TLS** with the Let's Encrypt certificate it already holds for `PUBLIC_HOST` and forwards the decrypted stream as plain TURN-over-TCP to coturn's 3478. coturn runs with `--no-tls` (DTLS is off without `--dtls`; coturn 4.18 has no `--no-dtls` option) and has no certificate.

Why: no certificate files in coturn, no reload job, no second ACME client, and no HTTP-01 coexistence constraint with Traefik's own resolver. The local proof is `just public-server-turns-traefik`.

**Static configuration** (an entrypoint of its own; [entryPoints reference](https://doc.traefik.io/traefik/routing/entrypoints/#respondingtimeouts)). Publish `5349:5349` on the Traefik container as well.

```yaml
entryPoints:
  turns:
    address: ":5349"
    transport:
      respondingTimeouts:
        readTimeout: 0 # Traefik v3 defaults to 60 s; a TURN-over-TCP connection is long-lived
```

**Dynamic configuration** (file provider; [TCP routers](https://doc.traefik.io/traefik/routing/routers/#configuring-tcp-routers), [TLS stores and `defaultGeneratedCert`](https://doc.traefik.io/traefik/https/tls/#certificates-stores)):

```yaml
tcp:
  routers:
    guardedsleep-turns:
      entryPoints: [turns]
      rule: HostSNI(`*`)
      service: guardedsleep-coturn
      tls:
        certResolver: letsencrypt
        domains:
          - main: gs.mydata.rocks
  services:
    guardedsleep-coturn:
      loadBalancer:
        servers:
          - address: "<host public IPv4>:3478" # coturn uses host networking
tls:
  stores:
    default:
      defaultGeneratedCert:
        resolver: letsencrypt
        domain:
          main: gs.mydata.rocks
```

- `HostSNI(`*`)` matches every client, also one without SNI (some TURN stacks send none); the default store's `defaultGeneratedCert` gives such a client the certificate for `PUBLIC_HOST` instead of Traefik's self-signed default.
- The service address is the host's public IPv4 because coturn listens on `TURN_IPV4` and `TURN_IPV6` only (not on `127.0.0.1` or the Docker bridge). A Traefik inside Docker reaches the host through that address; the firewall rule for 3478 already allows it.
- TLS versions: Traefik's default minimum is TLS 1.2 (the local test confirms a TLS 1.1 handshake fails); enforce it explicitly with your TLS options if you want it independent of Traefik's defaults ([TLS options](https://doc.traefik.io/traefik/https/tls/#tls-options)).
- **`readTimeout: 0`.** Traefik v3.7.17 in the local test kept an idle and an active TURN-over-TCP connection open for 75 s also with its default 60 s `readTimeout` (TCP routers do not apply it), so the setting is defensive; keep it, because the default is documented for the entrypoint and a later release may enforce it on TCP.

**Only `turns:` goes through Traefik; coturn stays on host networking.** STUN, TURN over UDP and TCP on 3478 and the relay ports reach coturn directly, for four reasons:

- STUN answers "which public address do you see me at"; behind a proxy coturn would see the proxy and hand out a wrong server-reflexive address, which breaks direct internet paths. For `turns:` this does not matter, because only relayed traffic uses it.
- The relay ports (49160–49359/udp) are allocated per allocation on coturn's public address and announced to the peer, which then sends there directly; a proxy cannot route ports it does not know in advance, and Traefik's UDP routing has no notion of an allocation.
- Publishing 200 UDP ports through Docker is slow and can lose the client address; coturn's own image documentation recommends host networking.
- With its public addresses on the interface, coturn listens and relays on them without `--external-ip` and can deny them as relay peers, so TURN never reaches services on the host.

**A file route, not Docker labels.** For a container on host networking, Traefik's Docker provider targets the host as Traefik sees it (`host.docker.internal` or the bridge gateway), and a TCP service label can set only the port, not the address. coturn would then have to listen on the Docker bridge too, which ties it to Docker internals and adds an address only for routing. The default store's `defaultGeneratedCert` exists only in the file provider anyway, and the `turns` entrypoint and its published port live in the Traefik configuration, so the router sits next to them.

**Trade-offs.** coturn sees the proxy as the client address for `turns:` (harmless: coturn logs nothing, and the per-address limits live in the signaling server, which sees the real client). One more hop for relayed TLS traffic. The proxy terminates the TLS, but the media inside is DTLS-SRTP end to end between the two devices, so the proxy sees only the TURN framing and encrypted payloads. The firewall is unchanged (5349/tcp, Traefik listens there).

**Check.** `openssl s_client -connect gs.mydata.rocks:5349 -servername gs.mydata.rocks </dev/null | openssl x509 -noout -subject -issuer -dates`, and a `turns:` allocation (`turnutils_uclient -S` inside the coturn container with the REST secret, as `check-turn.sh` does for UDP, aimed at `gs.mydata.rocks:5349`). Alert on expiry with an Uptime Kuma "TLS certificate expiry" option on a TCP/TLS monitor of `gs.mydata.rocks:5349`.

## Alternative: own certificate with lego

For a proxy that cannot terminate TCP/TLS (a pure HTTP proxy, or a Traefik you do not want to open a TCP entrypoint on), coturn terminates TLS itself. Add `compose.lego.yaml` (`docker compose -f compose.yaml -f compose.lego.yaml up -d`, or `COMPOSE_FILE=compose.yaml:compose.lego.yaml` in `.env`, which also lets the cron job below run without flags) and set `ACME_EMAIL`, `TURN_TLS_ARG_1=--cert=/certs/crt.pem` and `TURN_TLS_ARG_2=--pkey=/certs/key.pem` in `.env` (the commented block of the template). Do not route 5349 through Traefik in this variant. `just public-server-turns` proves it locally.

`turns:` (TURN over TLS on 5349) helps behind firewalls that pass only TLS-looking traffic. coturn needs a certificate as files, so the `acme` service obtains a **dedicated certificate for `PUBLIC_HOST`** with [lego](https://go-acme.github.io/lego/) (pinned tag, Renovate bumps it) instead of exporting Traefik's `acme.json` (an exporter would read every private key on the host). The service has no Docker socket, only `CHOWN` as a capability, a read-only root file system, and its own volume `acme-state` for lego's account key; coturn mounts only the `turn-certs` volume (`crt.pem` chain, `key.pem` key, mode 0600 for coturn's user) read-only.

**How it validates (HTTP-01 through Traefik).** Let's Encrypt fetches `http://PUBLIC_HOST/.well-known/acme-challenge/<token>` on port 80. The `acme` service carries a Traefik router for exactly that host and path on the **plain HTTP entrypoint** (`TRAEFIK_HTTP_ENTRYPOINT`, default `web`); lego's built-in server answers on 8080 only while a challenge runs. Prerequisites in your Traefik (CHECK against the owner's files, task 10.3): port 80 is open and reaches that entrypoint over IPv4 and IPv6; a global HTTP-to-HTTPS redirect on that entrypoint does not swallow the path (Let's Encrypt would follow it to a router that does not exist); and **Traefik's own resolver is not an HTTP-01 resolver on the same entrypoint** (its internal challenge router matches the same path for every host and answers 404 for tokens it does not know, which would shadow ours). If Traefik uses `httpChallenge` there, switch the `acme` service to DNS-01 with a zone-scoped token (design section 9, discarded alternative) or move Traefik's resolver to `tlsChallenge`.

**First issuance.**

1. Set `ACME_EMAIL` in `.env` (expiry notices only). Optional dry run first: `ACME_SERVER=letsencrypt-staging` (untrusted chain, no rate limits), `docker compose up -d acme`, `docker compose logs acme`, then remove the line and `docker compose down acme && docker volume rm guardedsleep-public_acme-state guardedsleep-public_turn-certs` before the real issuance.
2. `docker compose up -d` starts `acme`; it runs `lego run` at once (a few seconds). `docker compose logs acme` ends with `acme: certificate installed for coturn`. A failure (DNS, the route, a rate limit) is logged and retried every 6 hours.
3. Run `./reload-turn-cert.sh` once: it sees the first certificate and **restarts coturn once** (a coturn started without a certificate has no TLS listener; a reload does not create one). Running allocations drop and clients reconnect.
4. Check: `openssl s_client -connect gs.mydata.rocks:5349 -servername gs.mydata.rocks </dev/null | openssl x509 -noout -subject -issuer -dates`, and a `turns:` allocation (`turnutils_uclient -S` inside the coturn container with the REST secret, as `check-turn.sh` does for UDP).
5. Keep the `turns:` URL out of `TURN_URLS` until now (the template lists it; remove it for the first start, or clients try it and wait for a timeout before falling back), then add it and `docker compose up -d` (recreates `signaling` only).

**Before a certificate exists** (lego variant) coturn starts normally and logs `cannot start TLS and DTLS listeners because certificate file is not set properly` (a warning without client data): it serves STUN and `turn:` on 3478, nothing listens on 5349, and `signaling` does not advertise `turns:` (step 5 not done yet).

**Renewal.** `acme` runs `lego run` once a day; lego renews only when the certificate is due (a third of its lifetime left, or when the CA's renewal information says so), installs the new files through the deploy hook (temporary name, then rename, key first) and logs it. Host cron applies it to coturn:

```cron
17 4 * * * cd /opt/guardedsleep/docker/public-server && ./reload-turn-cert.sh
```

[`reload-turn-cert.sh`](reload-turn-cert.sh) compares the SHA-256 of `crt.pem` with the one it applied last (`~/.local/state/guardedsleep/turn-cert.sha256`) and, on a change, sends **SIGUSR2** to coturn (`docker compose kill -s USR2 coturn`), which re-reads `--cert` and `--pkey` without dropping allocations (handler `reload_ssl_certs` in coturn's `src/apps/relay/mainrelay.c`, added in 4.5.0.8; the local test proves the 4.18.0 image serves the new certificate afterwards without a restart). The signal comes from the host and not from the `acme` container because that would need the Docker socket (root on the host) or a shared PID namespace (which exposes coturn's command line, with `TURN_SECRET`, to the container that talks to the internet). Alert on expiry as well: an Uptime Kuma "TLS certificate expiry" option on a TCP/TLS monitor of `gs.mydata.rocks:5349` turns red before a broken renewal matters (with a 90-day certificate and renewal at 30 days left, the script has weeks of margin).

**Rotation / replacing the certificate.** After a suspected key leak or a host name change, start over: `docker compose down acme coturn`, `docker volume rm guardedsleep-public_turn-certs guardedsleep-public_acme-state`, `rm ~/.local/state/guardedsleep/turn-cert.sha256`, then repeat the first issuance (new account key, new certificate key). The old certificate stays valid until it expires; if its key leaked, revoke it as well (lego's `revoke` command; see `docker compose run --rm --no-deps --entrypoint lego acme --help` for the pinned version).

**Local proof.** `just public-server-turns` runs the stack parts with a test CA made at run time: coturn with no certificate (UDP `turn:` works, 5349 refused), the real deploy hook in the real `acme` container, `reload-turn-cert.sh` (restart on the first certificate, SIGUSR2 on the second), a TLS allocation with relayed data both ways, and a refused TLS 1.1 handshake. The live HTTP-01 issuance against Let's Encrypt is the owner's task 10.3.

## Logs and IP addresses

The project server logs no client IP addresses (AGENTS.md section 7, guardrails 1 and 6). What each part does:

- **Traefik:** access log off, its own log at `ERROR` (homelab repository).
- **signaling:** logs no addresses or room ids; the per-address limits live in memory only.
- **coturn: logs nothing** (`--log-file=/dev/null --no-stdout-log`). coturn writes client and peer addresses at several levels, and a denied peer is an `ERROR` line with the peer's address (for example `A peer IP 10.x.x.x denied in the range ...`), and `--log-min-level` cannot go above `error`, so no level keeps addresses out; discarding the log does. Restarts and crash loops stay visible through `docker compose ps` and the monitoring below. To debug, put `TURN_LOG_FILE=stdout` into `.env`, run `docker compose up -d`, reproduce, then **remove it and run `docker compose up -d` again**: while it is set the logs contain addresses.
- **web** (profile `web`, a separate origin only): the image's nginx runs with `access_log off` and its http error log on `/dev/null` (`docker/web/nginx.default.conf`), since nginx appends `client: <address>` to connection errors at every level. Only startup and worker messages reach stderr. Check: `docker logs <web container> 2>&1 | grep -cE '([0-9]{1,3}\.){3}[0-9]{1,3}|[0-9a-f]*:[0-9a-f]*:[0-9a-f:]+'` after a few requests with an `X-Forwarded-For` header, expect 0.
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
2. **TURN, with an authenticated allocation** (a STUN or TCP check on 3478 would not notice a `TURN_SECRET` mismatch that breaks every relayed call). [`check-turn.sh`](check-turn.sh) runs one allocation with `turnutils_uclient` inside the coturn container, with a credential signed by the secret in `turnserver.conf` (the one coturn verifies with; the script also fails when `.env` holds a different `TURN_SECRET`, which is what the signaling container gets), and fails when coturn refuses it. In Uptime Kuma add a **Push** monitor, copy its push URL into `KUMA_PUSH_URL` in `.env`, set its heartbeat interval to 10 minutes (a silent host turns it red too) and add a cron entry on the host:

   ```cron
   */5 * * * * cd /opt/guardedsleep/docker/public-server && ./check-turn.sh
   ```

   The script pushes `up` or `down` itself; its exit status is 0 or 1 for any other scheduler. It reads only `.env` and `turnserver.conf` (no `docker inspect`); the push URL (a token) is never printed.

3. **Allocation count (abuse):** coturn logs nothing and has no counter in this profile; one relay port is bound per allocation, so the count of bound relay ports is the allocation count: `ss -H -u -a -n 'sport >= :49160 and sport <= :49359' | wc -l`. Near 200 (`--total-quota`) means exhaustion; lower `TURN_MAX_LIVE_PER_ADDR` or the quotas. `check-turn.sh` does this count and warns at 80 % of the quota.

## Secret rotation

`TURN_SECRET` is shared by `signaling` (signs credentials) and `coturn` (verifies them). To rotate: put a new `openssl rand -hex 32` value into `.env`, rewrite `turnserver.conf` with it (First deployment, step 2) and run `docker compose up -d --force-recreate`; both containers are recreated together (coturn reads the file only at start). Credentials already issued stop working for new allocations; clients get new ones at their next join or renewal (at most 5 minutes later). Run `./check-turn.sh` afterwards. Rotate after anyone who knew it leaves, and after any suspicion.

## Self-hosting variants

- **Another domain:** set `PUBLIC_HOST`, `TURN_URLS` and `STUN_URLS` (the server refuses URLs that name another host), a DNS record for the host, and `TURN_IPV4` / `TURN_IPV6` of your machine. Change the Traefik router host through `PUBLIC_HOST` only. For a browser client served from another origin, add that origin to `SIGNALING_ALLOWED_ORIGINS`.
- **Provider with 1:1 NAT (Oracle, AWS, GCP):** the private address must be bound and the public one advertised. Add `--external-ip=<public>/<private>` to coturn's command and put the private address into `TURN_IPV4`; `--denied-peer-ip` must still list the public address. [`docs/contributing/turn-server-vps.md`](../../docs/contributing/turn-server-vps.md) walks through such a host with the older single-purpose profile.
- **Without coturn:** delete the `coturn` service, `TURN_URLS`, `TURN_SECRET` and the TURN lines of `compose.yaml`; the server then sends no TURN credentials. For STUN, remove `STUN_DISABLED` from the `signaling` environment and publish `3478/udp` on it: the built-in STUN answers within 44 bytes and 2.2 times the request, with a per-address limit. Clients on strict NAT then cannot relay.
- **Signaling only, another path or more proxies:** the router rule is `Host(PUBLIC_HOST) && PathPrefix(/signaling)`; the server accepts upgrades on any path, so another prefix needs only that label (no prefix stripping). Raise `SIGNALING_TRUSTED_PROXY_HOPS` to the number of proxies in front of the server when a CDN sits before Traefik; optional limits (`SIGNALING_MAX_PARENTS`, `SIGNALING_HELLO_TIMEOUT_MS`, the per-address rates) are listed in [`signaling-server-docker.md`](../../docs/contributing/signaling-server-docker.md) and go into the `signaling` environment.
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
- [ ] `turns:` works: Traefik has the `turns` entrypoint (port 5349 published, `readTimeout: 0`) and the TCP router, the `openssl s_client` check shows the certificate for `PUBLIC_HOST`, a `turns:` allocation succeeds, `TURN_URLS` lists the `turns:` URL. (Lego variant instead: certificate issued, `docker compose logs acme`, `./reload-turn-cert.sh` ran once and is in cron.)
- [ ] Firewall as above, `ufw logging off`; no `ports:` anywhere.
- [ ] The log search after a relayed test call finds no test client address in `docker compose logs`, Traefik's log or `journalctl`.
- [ ] Uptime Kuma monitors for both checks are green.
