# Running your own TURN server on a VPS (coturn, Docker Compose)

> **Scope:** a test and self-hosting guide for one user-entered TURN server, which is the only TURN the alpha allows (AGENTS.md §7.1 and §9). It is not the hardened beta profile: that one (short-lived HMAC credentials issued after the pairing proof, Caddy + signaling + TURN bundle) is `PLANNED` (T1) in [`docs/backlog.md`](../backlog.md). Walk-through on **Oracle Cloud Infrastructure (OCI)**; other providers differ only in the firewall section.

Files: [`docker/turn/compose.yaml`](../../docker/turn/compose.yaml) and [`docker/turn/turn.env.example`](../../docker/turn/turn.env.example).

## 1. What the TURN server does here

- Two phones normally connect directly (LAN, or through NAT with STUN). When both sit behind restrictive NATs (mobile data, hotel or venue Wi-Fi, Persona A), no direct path exists and the attempt ends in `media_blocked`. A TURN server relays the UDP packets between them.
- **The relay cannot read the audio.** Media and the DataChannel stay DTLS-SRTP/DTLS encrypted end to end and are bound to the pairing proof; TURN only forwards ciphertext.
- **The relay does see metadata:** both devices' public IP addresses, when a session runs and how much data flows. That is why the operator must be you (or someone you trust), and why logs need care (§8).

## 2. Requirements and sizing

| Item      | Minimum                                                                                              |
| --------- | ---------------------------------------------------------------------------------------------------- |
| VM        | 1 vCPU, 1 GB RAM, any Linux with Docker Engine + Compose v2; amd64 or arm64                          |
| Network   | a static public IPv4 address; UDP inbound allowed                                                    |
| Traffic   | one audio session is about 50–100 kbit/s per direction; a 10-hour night is roughly 0.5–1 GB in total |
| DNS (opt) | an `A` record such as `turn.example.org`, needed only for TLS (§7)                                   |

On OCI the **Always Free** tier is enough: `VM.Standard.A1.Flex` (Ampere, arm64; the coturn image is multi-arch) or `VM.Standard.E2.1.Micro` (amd64), with 10 TB outbound traffic per month. Oracle may reclaim Always Free instances that stay idle for a long time ([Always Free resources](https://docs.oracle.com/en-us/iaas/Content/FreeTier/freetier_topic-Always_Free_Resources.htm)); a TURN server used only now and then can hit that, so check the instance before a test night.

## 3. Prepare the Oracle instance

1. **Create the instance:** Compute → Instances → Create. Image: Canonical Ubuntu 24.04 (or Oracle Linux 9). Shape: one of the above. Keep "Assign a public IPv4 address" on. Add your SSH key.
2. **Make the public IP static:** Instance → Attached VNICs → primary VNIC → IPv4 addresses → edit → **Reserved public IP**. An ephemeral IP changes when the instance is stopped, and coturn would then advertise a stale address.
3. **Note both addresses.** OCI uses 1:1 NAT: the VM only sees its private address.

   ```bash
   ip -4 -br addr show        # private address, e.g. 10.0.0.23 on enp0s6
   ```

   The public address is on the instance details page ("Public IPv4 address").

### 3.1 Cloud firewall (VCN security list or NSG)

Networking → Virtual cloud networks → your VCN → Security Lists → Default Security List → **Add Ingress Rules**, source `0.0.0.0/0`, stateful:

| Protocol | Destination port | Purpose                                      |
| -------- | ---------------- | -------------------------------------------- |
| UDP      | 3478             | TURN/STUN (the main path for WebRTC)         |
| TCP      | 3478             | TURN over TCP, for networks that block UDP   |
| UDP      | 49160-49200      | relay ports (must match `TURN_MIN/MAX_PORT`) |
| TCP      | 5349             | only with TLS (§7)                           |

Egress stays at the default "all".

### 3.2 Host firewall

OCI platform images ship their own firewall that **rejects everything except SSH**, even when the security list allows it. This is the most common reason a fresh OCI TURN server is unreachable.

Ubuntu images (iptables with a final `REJECT` rule; `-I` inserts above it):

```bash
sudo iptables -I INPUT -p udp --dport 3478 -j ACCEPT
sudo iptables -I INPUT -p tcp --dport 3478 -j ACCEPT
sudo iptables -I INPUT -p udp --dport 49160:49200 -j ACCEPT
sudo netfilter-persistent save
```

Oracle Linux images (firewalld):

```bash
sudo firewall-cmd --permanent --add-port=3478/udp --add-port=3478/tcp --add-port=49160-49200/udp
sudo firewall-cmd --reload
```

Other providers (Hetzner, netcup, IONOS, …): open the same ports in the provider firewall and in `ufw`/`nftables` if active. Most of them assign the public IP directly to the VM; then `TURN_PUBLIC_IP` and `TURN_PRIVATE_IP` are the same address.

### 3.3 Docker

Install Docker Engine and the Compose plugin from Docker's apt or dnf repository ([install guide](https://docs.docker.com/engine/install/)), then:

```bash
sudo usermod -aG docker "$USER"   # log out and in again
docker compose version
```

## 4. Deploy

Copy the two files to the VM (a full clone is not needed):

```bash
mkdir -p ~/guardedsleep-turn && cd ~/guardedsleep-turn
# from your machine: scp docker/turn/compose.yaml docker/turn/turn.env.example <vm>:guardedsleep-turn/
cp turn.env.example .env
openssl rand -hex 24              # paste as TURN_PASSWORD
nano .env                         # set TURN_PUBLIC_IP, TURN_PRIVATE_IP, TURN_REALM, TURN_PASSWORD
chmod 600 .env
docker compose up -d
docker compose logs -f            # expect "Relay ports initialization done" and no ERROR lines
```

Notes on the compose file:

- **Host networking** (`network_mode: host`): coturn must own the relay port range and see the real client addresses. Port publishing through docker-proxy is not an option for a port range.
- **Password characters:** the coturn image entrypoint runs `eval "echo $arg"` on every argument, so `$`, backticks, quotes or spaces in `.env` values are evaluated by the shell. Hex passwords avoid this.
- **Hardening already set:** read-only root file system, all capabilities dropped except `NET_BIND_SERVICE` (the binary needs it in the bounding set to start), `no-new-privileges`, `--fingerprint`, `--stale-nonce`, quotas and a bandwidth cap per session, UDP relay only, and `--denied-peer-ip` for every private, loopback, link-local and CGNAT range. The denylist matters on a cloud VM: without it, anyone holding the credential could use the server as a proxy into the provider network or reach the metadata service at `169.254.169.254`.

## 5. Verify

Run the checks in order; each one narrows down where a failure sits.

### 5.1 Allocation from outside (any machine with Docker)

```bash
docker run --rm --network host --entrypoint turnutils_uclient coturn/coturn:4.18.0 \
  -u guardedsleep -w '<TURN_PASSWORD>' -n 20 -m 1 -y <TURN_PUBLIC_IP>
```

- `Total connect time is …` and `Total lost packets 0 (0.000000%)`: authentication, allocation and relaying work.
- `ERROR … Cannot complete Allocation` or a timeout: firewall (§3.1/§3.2) or wrong `TURN_PRIVATE_IP`.
- `401` in the server log (`credentials are incorrect`): wrong user or password.
- Allocations succeed but **all packets are lost**: with `-y` both test clients relay through the same server, so their packets go to the server's own public IP. That needs hairpin NAT at the provider. Check with the browser test below; if only this case fails, avoid relay-only on both phones at once (§5.3).

### 5.2 Browser (Trickle ICE)

Open the WebRTC sample page [Trickle ICE](https://webrtc.github.io/samples/src/content/peerconnection/trickle-ice/), remove the default server, add `turn:<TURN_PUBLIC_IP>:3478` with user and password, and click "Gather candidates". A line of type **`relay`** with your public IP and a port from 49160–49200 means the server works for WebRTC. The page runs in your browser and talks only to the servers you enter; still, use the test credential and rotate it afterwards (§8).

### 5.3 With the Android app (debug build)

The release UI does not take TURN credentials yet (see §9), so the end-to-end test uses the debug launch extras that the emulator gate uses ([`scripts/poc/gate.sh`](../../scripts/poc/gate.sh), `launch()`). Put the two phones on networks without a direct path (for example both on mobile data, or one with `relay_only`), install the debug APK (`just build-android`), and start both roles with the same fresh PSK:

```bash
PSK="$(head -c 32 /dev/urandom | base64 | tr '+/' '-_' | tr -d '=\n')"
for role in baby parent; do
  serial=$([ "$role" = baby ] && echo <baby-serial> || echo <parent-serial>)
  adb -s "$serial" shell am start -n org.guardedsleep.app.alpha/org.guardedsleep.app.debug.DebugLaunchActivity \
    --es role "$role" --es psk_b64url "$PSK" --es signal_urls "wss://<your-signaling-host>/" \
    --es turn_url "turn:<TURN_PUBLIC_IP>:3478" --es turn_user guardedsleep --es turn_pass '<TURN_PASSWORD>' \
    --es relay_only true --es auto_listen true
done
```

Pass: monitoring connects, and `docker compose logs` shows new sessions for the user `guardedsleep` with allocations on relay ports in your range. The PSK lives only in this shell; never paste it into logs or issues.

## 6. Running signaling on the same VPS

The Rust signaling server ([`docker/signaling-server`](../../docker/signaling-server/README.md)) also answers STUN on UDP 3478. Only one process can own that port: start signaling with `STUN_DISABLED=1`. coturn answers STUN binding requests without authentication, so clients that derive the STUN URL from the relay host keep working.

## 7. Optional: TLS (`turns:`)

`turns:` on TCP 5349 (or 443) helps in networks that allow only HTTPS-like traffic. It is not needed for the first test.

1. Point an `A` record at the VM and get a certificate (for example `certbot certonly --standalone -d turn.example.org`, with TCP 80 open for the challenge).
2. Copy `fullchain.pem` and `privkey.pem` into a directory readable by the container user (`nobody`), for example `~/guardedsleep-turn/certs/`, and repeat that in a certbot deploy hook on renewal.
3. In `compose.yaml`, replace `--no-tls` with `--cert=/certs/fullchain.pem`, `--pkey=/certs/privkey.pem`, `--tls-listening-port=5349` and `--no-tlsv1_2` (TLS 1.3 only), and mount `./certs:/certs:ro`.
4. Open TCP 5349 in §3.1 and §3.2, restart, and test with `turns:turn.example.org:5349?transport=tcp` in §5.2.

## 8. Operation

- **Logs contain client IP addresses** (session start lines). Keep the rotation in the compose file (3 × 10 MB) short, never ship the logs anywhere, and once the server works lower the level with `--log-min-level=warning`.
- **Credential:** treat `TURN_PASSWORD` like a password; it is in `.env` (mode 600) and in the phones. Rotate it after tests in shared places (§5.2): new value in `.env`, `docker compose up -d`, update the phones.
- **Updates:** Renovate does not see this VM. Check [coturn releases](https://github.com/coturn/coturn/releases) monthly, change `COTURN_TAG`, `docker compose pull && docker compose up -d`; keep the OS patched (`unattended-upgrades` on Ubuntu).
- **Health:** `docker compose ps` and the allocation test in §5.1. Watch the traffic graph in the OCI console for unexpected volume, which would point to a leaked credential.

## 9. Known gaps in the app (alpha)

- **Android settings take only the TURN URL** (Advanced → "Relay server (TURN), optional"), no user name or password. A coturn with authentication, which is the only safe setup, therefore works today only through the debug launch (§5.3); the QR pairing payload carries no TURN data either. Closing that gap is app work and needs an OpenSpec change (credential fields, sealed storage under guardrail §7.2).
- **The web client has no TURN setting yet.**
- **Static credential:** the planned beta profile replaces it with `--use-auth-secret` and short-lived HMAC credentials (TTL ≤ 10 minutes) that the app obtains only after the pairing proof.

## 10. Troubleshooting

| Symptom                                                   | Likely cause and fix                                                                                                       |
| --------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Container restarts, `turnserver: Operation not permitted` | `cap_add: NET_BIND_SERVICE` was removed; restore it                                                                        |
| Container restarts and prints the option list             | an unknown or removed flag (coturn 4.18 dropped several `--no-*` flags); check `docker compose logs`                       |
| Timeout from outside, nothing in the server log           | OCI security list or host firewall (§3.1, §3.2); `sudo iptables -L INPUT -n --line-numbers` must list ACCEPT before REJECT |
| `Cannot bind` / `Address already in use` on 3478          | the signaling server's STUN or another service owns the port (§6)                                                          |
| `relay` candidate shows the private 10.x address          | `--external-ip` is missing or `TURN_PUBLIC_IP` is wrong                                                                    |
| Allocation works, phones still fail                       | relay ports (UDP 49160–49200) closed in one of the two firewalls, or hairpin case (§5.1)                                   |
| `403` on create permission / channel bind                 | the peer address is in a denied range; expected for private peers, so test between real public networks                    |
