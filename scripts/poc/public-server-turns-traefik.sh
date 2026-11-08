#!/usr/bin/env bash
# Task 10.4 (alpha-connection-model, design section 9): turns: on 5349 terminated by Traefik, the DEFAULT of
# docker/public-server (the lego variant is scripts/poc/public-server-turns.sh).
#
# Runs coturn from the real docker/public-server/compose.yaml (template filled with test values, bridge override of
# public-server-logs.compose.yaml; default flags --no-tls --no-dtls, no certificate) behind a Traefik container with
# the config the owner runs live (docker/public-server/README.md): entrypoint `turns` on :5349 with
# respondingTimeouts.readTimeout 0, a TCP router HostSNI(`*`) with tls, forwarding plain TCP to coturn's 3478.
# Differences, because ACME cannot run here: the certificates come from a throw-away test CA (made at run time, never
# committed, deleted by the trap) through `tls.certificates` (SNI match) and `tls.stores.default.defaultCertificate`
# (what a client without SNI gets) instead of certResolver and defaultGeneratedCert. Proves:
#   a. a TLS allocation through Traefik:5349 with a REST credential, data relayed both ways;
#   b. the connection still relays after 75 s (Traefik's default readTimeout is 60 s), both with traffic every 15 s and
#      idle; a second Traefik WITHOUT readTimeout 0 is the positive control (it should break; if it does not, that is
#      recorded, not a failure);
#   c. a client without SNI gets the default-store certificate and allocates;
#   d. a TLS 1.1 handshake is refused;
#   e. a wrong credential is rejected.
#
# Needs Docker. Without it prints SKIPPED (GS_STRICT=1 turns that into a failure). Everything it starts is removed
# by a trap. Env: GS_PUBSRV_OUT (target/public-server-turns-traefik).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=scripts/lib/require.sh
source "$ROOT/scripts/lib/require.sh"
require public-server-turns-traefik docker

# shellcheck source=tools/poc-images.env
source "$ROOT/tools/poc-images.env"
OUT="${GS_PUBSRV_OUT:-$ROOT/target/public-server-turns-traefik}"
SRC="$ROOT/docker/public-server"
SUFFIX="$$"
PROJECT="gs-turnst-$SUFFIX"
NET="gs-turnst-net-$SUFFIX"
WORK="$(mktemp -d)"
mkdir -p "$OUT" "$WORK/traefik"

TURN_V4=203.0.113.10
TURN_V6=fd00:5eed:c::10
TRAEFIK_OK=203.0.113.20   # readTimeout 0 (the owner's config)
TRAEFIK_DEFAULT=203.0.113.21 # no transport block: Traefik's default readTimeout, the positive control
HOST_NAME=gs.mydata.rocks
HOLD_S=75

TAG=public-server-turns-traefik
# shellcheck source=scripts/poc/lib/public-server.sh
source "$ROOT/scripts/poc/lib/public-server.sh"
BG=()
cleanup() {
	docker rm -f "gs-traefik-ok-$SUFFIX" "gs-traefik-default-$SUFFIX" >/dev/null 2>&1 || true
	if ((${#BG[@]})); then kill "${BG[@]}" 2>/dev/null || true; fi
	docker ps -aq --filter "name=gs-turnst-client-$SUFFIX" | xargs -r docker rm -f >/dev/null 2>&1 || true
	ps_cleanup --volumes
}
trap cleanup EXIT

SECRET="$(openssl rand -hex 32)"
ps_fill_env "$SECRET"

echo "== docker compose config with the template: Traefik variant, coturn without TLS and without certificate"
CONFIG="$(compose config)"
grep -q -e '- --no-tls$' <<<"$CONFIG" || { echo "$TAG: compose config lacks --no-tls" >&2; exit 1; }
for absent in 'goacme/lego' '--cert=' '--pkey=' 'turn-certs' '--tls-listening-port'; do
	if grep -q -- "$absent" <<<"$CONFIG"; then echo "$TAG: the default compose config contains $absent" >&2; exit 1; fi
done

echo "== test CA, certificate 0x11 (SAN $HOST_NAME, SNI match) and default-store certificate 0x22, made inside the coturn image"
chmod 755 "$WORK" "$WORK/traefik"
docker run --rm --user 0 -v "$WORK:/w" -e H="$HOST_NAME" --entrypoint sh "$COTURN_IMAGE" -c '
set -e; cd /w/traefik
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -keyout ca.key -out ca.pem -days 2 -subj /CN=gs-test-ca >/dev/null 2>&1
for n in sni default; do
  s=11; name="$H"; [ $n = default ] && { s=22; name=default.invalid; }
  printf "subjectAltName=DNS:%s\n" "$name" > ext
  openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -keyout $n.key -out $n.csr -subj "/CN=$name" >/dev/null 2>&1
  openssl x509 -req -in $n.csr -CA ca.pem -CAkey ca.key -out $n.crt -days 1 -extfile ext -set_serial 0x$s >/dev/null 2>&1
done
chmod 644 *.pem *.crt *.key'

# The two Traefik configurations differ only in the readTimeout block (the owner's static config has it).
cat >"$WORK/traefik/static-ok.yml" <<YML
entryPoints:
  turns:
    address: ":5349"
    transport:
      respondingTimeouts:
        readTimeout: 0
providers:
  file:
    filename: /etc/traefik/dynamic.yml
log:
  level: INFO
YML
grep -v -e 'transport:' -e 'respondingTimeouts:' -e 'readTimeout: 0' "$WORK/traefik/static-ok.yml" >"$WORK/traefik/static-default.yml"
cat >"$WORK/traefik/dynamic.yml" <<YML
tcp:
  routers:
    guardedsleep-turns:
      entryPoints: [turns]
      rule: HostSNI(\`*\`)
      service: guardedsleep-coturn
      tls: {}
  services:
    guardedsleep-coturn:
      loadBalancer:
        servers:
          - address: "$TURN_V4:3478"
tls:
  certificates:
    - certFile: /etc/traefik/sni.crt
      keyFile: /etc/traefik/sni.key
  stores:
    default:
      defaultCertificate:
        certFile: /etc/traefik/default.crt
        keyFile: /etc/traefik/default.key
YML

docker network create --subnet 203.0.113.0/24 --ipv6 --subnet fd00:5eed:c::/64 "$NET" >/dev/null

traefik_up() { # <name> <ip> <static file>
	docker run -d --name "$1-$SUFFIX" --network "$NET" --ip "$2" -v "$WORK/traefik:/etc/traefik:ro" \
		"$TRAEFIK_IMAGE" --configFile=/etc/traefik/"$3" >/dev/null
}
client() { # <ip of this client> <traefik ip> <mode> [env...]
	local cip="$1" thost="$2" mode="$3"
	shift 3
	docker run --rm --name "gs-turnst-client-$SUFFIX-$cip" --network "$NET" --ip "$cip" -e MODE="$mode" -e TURN_HOST="$thost" \
		-e TURN_SECRET="$SECRET" -e SERVER_NAME="$HOST_NAME" -e CA_FILE=/w/ca.pem -e EXPECT_SERIAL=11 "$@" \
		-v "$ROOT/scripts/poc:/poc:ro" -v "$WORK/traefik:/w:ro" "$NODE_IMAGE" node /poc/public-server-turns-client.mjs
}

compose up -d --no-build --pull never coturn
traefik_up gs-traefik-ok "$TRAEFIK_OK" static-ok.yml
traefik_up gs-traefik-default "$TRAEFIK_DEFAULT" static-default.yml
sleep 6
ps_require_running 0 coturn
for t in ok default; do
	[[ "$(docker inspect -f '{{.State.Running}}' "gs-traefik-$t-$SUFFIX")" == true ]] || { docker logs "gs-traefik-$t-$SUFFIX" >&2; exit 1; }
done
docker logs "gs-traefik-ok-$SUFFIX" >"$OUT/traefik-ok.log" 2>&1

echo "== b. long-lived connections ($HOLD_S s) start now, in the background: through readTimeout 0 (must survive) and through the"
echo "   default readTimeout (control), each with a Refresh every 15 s (active) and with nothing sent (idle)"
hold() { # <name> <client ip> <traefik ip> [env...]
	local n="$1"
	shift
	client "$1" "$2" tls -e HOLD_S=$HOLD_S "${@:3}" >"$OUT/client-hold-$n.log" 2>&1 &
	BG+=($!)
	PIDS[$n]=$!
}
declare -A PIDS
hold ok-active 203.0.113.77 "$TRAEFIK_OK"
hold ok-idle 203.0.113.80 "$TRAEFIK_OK" -e HOLD_IDLE=1
hold control-active 203.0.113.78 "$TRAEFIK_DEFAULT" -e EXPECT_BREAK=1
hold control-idle 203.0.113.81 "$TRAEFIK_DEFAULT" -e EXPECT_BREAK=1 -e HOLD_IDLE=1

echo "== a. TLS allocation through Traefik, SNI $HOST_NAME, data relayed both ways"
client 203.0.113.79 "$TRAEFIK_OK" tls | tee "$OUT/client-a.log"
grep -q 'VERDICT PASS' "$OUT/client-a.log"

echo "== c. no SNI: the default-store certificate (serial 22), allocation works"
client 203.0.113.79 "$TRAEFIK_OK" tls -e SERVER_NAME= -e EXPECT_SERIAL=22 | tee "$OUT/client-c.log"
grep -q 'VERDICT PASS' "$OUT/client-c.log"
grep -q 'no SNI' "$OUT/client-c.log"

echo "== d. TLS 1.1 refused"
client 203.0.113.79 "$TRAEFIK_OK" old | tee "$OUT/client-d.log"
grep -q 'VERDICT PASS' "$OUT/client-d.log"

echo "== e. wrong credential rejected"
client 203.0.113.79 "$TRAEFIK_OK" badcred | tee "$OUT/client-e.log"
grep -q 'VERDICT PASS' "$OUT/client-e.log"

echo "== b. waiting for the long-lived connections"
declare -A RC
for n in ok-active ok-idle control-active control-idle; do
	RC[$n]=0
	wait "${PIDS[$n]}" || RC[$n]=$?
	echo "-- $n:"
	cat "$OUT/client-hold-$n.log"
done
BG=()
for n in ok-active ok-idle; do
	[[ "${RC[$n]}" == 0 ]] || { echo "$TAG: the connection through Traefik with readTimeout 0 ($n) did not survive $HOLD_S s" >&2; exit 1; }
	grep -q 'HOLD connection still open after' "$OUT/client-hold-$n.log"
done
for n in control-active control-idle; do
	if [[ "${RC[$n]}" == 0 ]]; then
		echo "CONTROL $n: the default readTimeout broke the connection ($(grep -o 'closed by the peer side after [0-9]*s' "$OUT/client-hold-$n.log"))"
	elif grep -q 'survived' "$OUT/client-hold-$n.log"; then
		echo "NOTE $n: the connection SURVIVED $HOLD_S s with Traefik's default readTimeout; readTimeout 0 is defensive only here"
	else
		echo "$TAG: the control client $n failed for another reason" >&2
		exit 1
	fi
done

echo "$TAG: PASSED (TLS allocation through Traefik, $HOLD_S s with readTimeout 0, no-SNI default certificate, TLS 1.1 refused, wrong credential rejected)"
