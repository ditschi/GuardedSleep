#!/usr/bin/env bash
# Task 10.2 (alpha-connection-model, design section 9): turns: on 5349 of the public-server stack, LEGO variant (the
# alternative since task 10.4; the default, TLS at Traefik, is scripts/poc/public-server-turns-traefik.sh).
#
# Runs docker/public-server/compose.yaml plus compose.lego.yaml (the template filled with test values, the bridge override of
# public-server-logs.compose.yaml) with coturn and the acme service, and proves, with TEST certificates made at run
# time (a throw-away CA and two leaf certificates, never committed, deleted by the trap):
#   1. coturn starts with NO certificate: no TLS listener, but turn: over UDP still allocates;
#   2. the acme deploy hook (acme-deploy-hook.sh, run in the real acme container with its hardening) installs
#      certificate A; reload-turn-cert.sh restarts coturn (first certificate); a TLS 1.2+ client verifies the chain,
#      allocates and relays data both ways over the TLS stream; a TLS 1.1 handshake fails;
#   3. certificate B installed; reload-turn-cert.sh sends SIGUSR2; the next handshake serves B without a restart
#      (same container start time); a second run of the script does nothing.
# The acme service itself starts against an unreachable ACME directory (it only has to survive its failure loop);
# Let's Encrypt HTTP-01 through Traefik cannot run locally and is the owner's task 10.3.
#
# Needs Docker. Without it prints SKIPPED (GS_STRICT=1 turns that into a failure). Everything it starts is removed
# by a trap. Env: GS_PUBSRV_OUT (target/public-server-turns).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=scripts/lib/require.sh
source "$ROOT/scripts/lib/require.sh"
require public-server-turns docker

# shellcheck source=tools/poc-images.env
source "$ROOT/tools/poc-images.env"
OUT="${GS_PUBSRV_OUT:-$ROOT/target/public-server-turns}"
SRC="$ROOT/docker/public-server"
SUFFIX="$$"
PROJECT="gs-turns-$SUFFIX"
NET="gs-turns-net-$SUFFIX"
WORK="$(mktemp -d)"
mkdir -p "$OUT" "$WORK/fake"

CLIENT_IP=203.0.113.77
TURN_V4=203.0.113.10
TURN_V6=fd00:5eed:b::10
HOST_NAME=gs.mydata.rocks

TAG=public-server-turns
PS_LEGO=1 # the lego variant (compose.lego.yaml); the default, Traefik, is scripts/poc/public-server-turns-traefik.sh
# shellcheck source=scripts/poc/lib/public-server.sh
source "$ROOT/scripts/poc/lib/public-server.sh"
cleanup() { ps_cleanup --volumes; }
trap cleanup EXIT

SECRET="$(openssl rand -hex 32)"
ps_fill_env "$SECRET"
echo "ACME_SERVER=https://acme.invalid/directory" >>"$WORK/.env"

echo "== docker compose config with the template"
CONFIG="$(compose config)"
for needle in '--cert=/certs/crt.pem' '--pkey=/certs/key.pem' 'goacme/lego:v'; do
	grep -q -- "$needle" <<<"$CONFIG" || { echo "public-server-turns: compose config lacks $needle" >&2; exit 1; }
done
if grep -q -e '--no-tls$' -e '--dtls' <<<"$CONFIG"; then
	echo "public-server-turns: coturn must not carry --no-tls or --dtls" >&2
	exit 1
fi

echo "== test CA and two leaf certificates (SAN $HOST_NAME, serials 0x11 and 0x22), made inside the coturn image"
chmod 755 "$WORK"
docker run --rm --user 0 -v "$WORK:/w" -e H="$HOST_NAME" --entrypoint sh "$COTURN_IMAGE" -c '
set -e; cd /w
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -keyout ca.key -out ca.pem -days 2 -subj /CN=gs-test-ca >/dev/null 2>&1
printf "subjectAltName=DNS:%s\n" "$H" > ext
for n in a b; do
  s=11; [ $n = b ] && s=22
  openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -keyout fake/$n.key -out $n.csr -subj "/CN=$H" >/dev/null 2>&1
  openssl x509 -req -in $n.csr -CA ca.pem -CAkey ca.key -out fake/$n.crt -days 1 -extfile ext -set_serial 0x$s >/dev/null 2>&1
done
chmod 755 fake; chmod 644 ca.pem fake/*'

docker network create --subnet 203.0.113.0/24 --ipv6 --subnet fd00:5eed:b::/64 "$NET" >/dev/null

client() { # <mode> [env...]
	local mode="$1"
	shift
	docker run --rm --network "$NET" --ip "$CLIENT_IP" -e MODE="$mode" -e TURN_HOST="$TURN_V4" -e TURN_SECRET="$SECRET" \
		-e SERVER_NAME="$HOST_NAME" -e CA_FILE=/w/ca.pem "$@" -v "$ROOT/scripts/poc:/poc:ro" -v "$WORK:/w:ro" \
		"$NODE_IMAGE" node /poc/public-server-turns-client.mjs
}
install_cert() { # <a|b>: the real hook, in the real acme container, fed fake lego variables
	compose run --rm --no-deps -T --entrypoint /bin/sh -v "$WORK/fake:/fake:ro" \
		-e LEGO_HOOK_CERT_PATH="/fake/$1.crt" -e LEGO_HOOK_CERT_KEY_PATH="/fake/$1.key" acme /acme-deploy-hook.sh
}
reload() {
	GS_CERT_STATE="$WORK/state" COMPOSE="docker compose ${COMPOSE_ARGS[*]}" GS_TEST_SIGNALING_IMAGE=unused GS_TEST_NET="$NET" \
		"$SRC/reload-turn-cert.sh"
}
started() { docker inspect -f '{{.State.StartedAt}}' "$(compose ps -q coturn)"; }

echo "== 1. no certificate yet"
compose up -d --no-build --pull never coturn acme
sleep 4
ps_require_running 0 coturn acme
compose logs --no-color acme 2>&1 | grep -q 'retrying in 6 hours' || { echo "public-server-turns: acme did not reach its failure branch" >&2; compose logs --no-color acme >&2; exit 1; }
client nocert | tee "$OUT/client-nocert.log"
grep -q 'VERDICT PASS' "$OUT/client-nocert.log"
reload | tee "$OUT/reload-0.log"
grep -q 'no certificate' "$OUT/reload-0.log"
# A container error is not "no certificate": with acme stopped the script must fail (cron mails it).
compose stop acme >/dev/null
if reload >"$OUT/reload-err.log" 2>&1; then
	echo "public-server-turns: reload-turn-cert.sh exited 0 with the acme container stopped" >&2
	exit 1
fi
grep -q 'FAILED: cannot query the acme container' "$OUT/reload-err.log"
compose start acme >/dev/null

echo "== 2. certificate A (serial 11): hook, first-certificate restart, TLS allocation"
install_cert a
t0="$(started)"
reload | tee "$OUT/reload-1.log"
grep -q 'coturn restarted' "$OUT/reload-1.log"
sleep 4
[[ "$(started)" != "$t0" ]] || { echo "public-server-turns: coturn was not restarted" >&2; exit 1; }
client tls -e EXPECT_SERIAL=11 | tee "$OUT/client-a.log"
grep -q 'VERDICT PASS' "$OUT/client-a.log"
client old | tee "$OUT/client-old.log"
grep -q 'VERDICT PASS' "$OUT/client-old.log"
# The key must be unreadable for anyone but nobody (mode 0600) and the container must stay read-only.
docker run --rm -v "${PROJECT}_turn-certs:/c:ro" --entrypoint sh "$COTURN_IMAGE" -c 'stat -c "%a %u %n" /c/key.pem /c/crt.pem' | tee "$OUT/modes.log"
grep -q '^600 ' "$OUT/modes.log"

echo "== 3. certificate B (serial 22): SIGUSR2 reload without a restart"
install_cert b
t1="$(started)"
reload | tee "$OUT/reload-2.log"
grep -q 'SIGUSR2' "$OUT/reload-2.log"
sleep 2
[[ "$(started)" == "$t1" ]] || { echo "public-server-turns: coturn restarted on a renewal" >&2; exit 1; }
client tls -e EXPECT_SERIAL=22 | tee "$OUT/client-b.log"
grep -q 'VERDICT PASS' "$OUT/client-b.log"
reload | tee "$OUT/reload-3.log"
grep -q 'unchanged' "$OUT/reload-3.log"

echo "public-server-turns: PASSED (no certificate, restart on first, SIGUSR2 on renewal, TLS 1.1 refused)"
