#!/usr/bin/env bash
# Task 10.1 (alpha-connection-model, design sections 7 and 9): the public-server stack keeps client addresses
# out of every container log, and coturn's Binding answer stays within the amplification bound.
#
# Runs docker/public-server/compose.yaml (the template filled with test values, a local override for the
# network) with a signaling image and coturn on a user-defined bridge, makes one relayed test call from a
# client at a known address (allocation, permissions including a denied private peer and a denied host
# address, data both ways, a rejected credential; the credential is issued by the signaling server to a v3 room
# of one baby and one parent, who must both receive ice_servers; plus HTTP and WebSocket probes), then searches
# every container log for the client's address (dotted and hex), the denied peer, the secret and the issued
# user name and password.
# A positive control repeats the call against coturn logging verbosely to stdout and must FIND the address,
# otherwise the search proves nothing. Also prints coturn's Binding response size for a bare 20-byte request.
#
# Needs Docker. Builds the signaling image from the working tree (docker layer cache: fast after the first
# run) unless GS_TEST_SIGNALING_IMAGE names an existing image. Without Docker it prints SKIPPED (GS_STRICT=1
# turns that into a failure). Everything it starts is removed by a trap.
# Env: GS_TEST_SIGNALING_IMAGE (existing image to use), GS_PUBSRV_OUT (target/public-server-logs).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=scripts/lib/require.sh
source "$ROOT/scripts/lib/require.sh"
require public-server-logs docker

# shellcheck source=tools/poc-images.env
source "$ROOT/tools/poc-images.env"
# Same digest as docker/web/Dockerfile and scripts/poc/turn-lifetime.sh; runs the client script only.
NODE_IMAGE=node:22-alpine@sha256:0a7108bf6c7bf5de370ffb1a3ed6be93d405b43ff159f681a8d18c0e2bc2e402
if [[ -n "${GS_TEST_SIGNALING_IMAGE:-}" ]]; then
	SIGNALING_IMAGE="$GS_TEST_SIGNALING_IMAGE"
else
	SIGNALING_IMAGE=guardedsleep-signaling:public-server-test
	echo "public-server-logs: building $SIGNALING_IMAGE (several minutes when the cache is cold)"
	docker build -q -f "$ROOT/docker/signaling-server/Dockerfile" -t "$SIGNALING_IMAGE" "$ROOT" >/dev/null
fi
OUT="${GS_PUBSRV_OUT:-$ROOT/target/public-server-logs}"
SRC="$ROOT/docker/public-server"
SUFFIX="$$"
PROJECT="gs-pubsrv-$SUFFIX"
NET="gs-pubsrv-net-$SUFFIX"
CTL="gs-pubsrv-ctl-$SUFFIX"
WORK="$(mktemp -d)"
mkdir -p "$OUT"

# TEST-NET-2 (198.51.100.0/24) is outside every --denied-peer-ip range, so the client may relay to itself.
CLIENT_IP=198.51.100.77
TURN_V4=198.51.100.10
CTL_V4=198.51.100.11
TURN_V6=fd00:5eed:a::10
DENIED_PEER=10.9.9.9
HEX_CLIENT="$(printf '%02x%02x%02x%02x' 198 51 100 77)"

compose() {
	GS_TEST_SIGNALING_IMAGE="$SIGNALING_IMAGE" GS_TEST_NET="$NET" docker compose -p "$PROJECT" \
		--env-file "$WORK/.env" -f "$SRC/compose.yaml" -f "$ROOT/scripts/poc/public-server-logs.compose.yaml" "$@"
}
cleanup() {
	compose logs --no-color >"$OUT/stack.log" 2>&1 || true
	compose down --remove-orphans >/dev/null 2>&1 || true
	docker rm -f "$CTL" >/dev/null 2>&1 || true
	docker network rm "$NET" >/dev/null 2>&1 || true
	rm -rf "$WORK"
}
trap cleanup EXIT

SECRET="$(openssl rand -hex 32)"
# The template, filled with test values (the real file is what compose.yaml documents).
sed -e "s|^TURN_SECRET=.*|TURN_SECRET=$SECRET|" \
	-e "s|^SIGNALING_TAG=.*|SIGNALING_TAG=test|" \
	-e "s|^TURN_IPV4=.*|TURN_IPV4=$TURN_V4|" \
	-e "s|^TURN_IPV6=.*|TURN_IPV6=$TURN_V6|" "$SRC/.env.template" >"$WORK/.env"

docker network create --subnet 198.51.100.0/24 --ipv6 --subnet fd00:5eed:a::/64 "$NET" >/dev/null
compose up -d --no-build --pull never signaling coturn
sleep 3
compose ps --format '{{.Service}} {{.State}}'
for s in signaling coturn; do
	[[ "$(compose ps --format '{{.State}}' "$s")" == running && "$(docker inspect -f '{{.RestartCount}}' "$(compose ps -q "$s")")" == 0 ]] || {
		echo "public-server-logs: $s is not running:" >&2
		compose logs --no-color "$s" >&2 || true
		exit 1
	}
done
SIGNALING_IP="$(docker inspect -f "{{(index .NetworkSettings.Networks \"$NET\").IPAddress}}" "$(compose ps -q signaling)")"

run_client() { # <coturn ip>
	docker run --rm --network "$NET" --ip "$CLIENT_IP" -e TURN_SECRET="$SECRET" -e TURN_HOST="$1" -e DENIED_HOST="$TURN_V4" \
		-e SIGNALING_URL="http://$SIGNALING_IP:8787" -e PUBLIC_HOST=gs.mydata.rocks \
		-v "$ROOT/scripts/poc/public-server-logs-client.mjs:/client.mjs:ro" "$NODE_IMAGE" node /client.mjs
}
echo "== relayed test call against the stack (coturn logging to /dev/null)"
run_client "$TURN_V4" | tee "$OUT/client.log"
ISSUED_USER="$(sed -n 's/^ISSUED_USERNAME //p' "$OUT/client.log")"
ISSUED_PASS="$(sed -n 's/^ISSUED_CREDENTIAL //p' "$OUT/client.log")"
[[ -n "$ISSUED_USER" && -n "$ISSUED_PASS" ]] || { echo "public-server-logs: no issued credential" >&2; exit 1; }
grep -q 'VERDICT PASS' "$OUT/client.log" || { echo "public-server-logs: the test call failed" >&2; exit 1; }
sleep 2

echo "== positive control: the same call against coturn logging verbosely to stdout"
mapfile -t ARGS < <(docker inspect -f '{{range .Config.Cmd}}{{println .}}{{end}}' "$(compose ps -q coturn)")
# The image entrypoint evals each argument; --listening-ip/--relay-ip keep the real values, the control
# gets its own IPv4 so both can run side by side.
CTL_ARGS=()
for a in "${ARGS[@]}"; do
	case "$a" in
	--log-file=*) CTL_ARGS+=(--log-file=stdout --verbose) ;;
	--no-stdout-log) ;;
	--listening-ip="$TURN_V4" | --relay-ip="$TURN_V4") CTL_ARGS+=("${a%%=*}=$CTL_V4") ;;
	--listening-ip="$TURN_V6" | --relay-ip="$TURN_V6") CTL_ARGS+=("${a%%=*}=fd00:5eed:a::11") ;;
	"") ;;
	*) CTL_ARGS+=("$a") ;;
	esac
done
docker run -d --name "$CTL" --network "$NET" --ip "$CTL_V4" --ip6 fd00:5eed:a::11 --cap-drop ALL --cap-add NET_BIND_SERVICE \
	"$COTURN_IMAGE" "${CTL_ARGS[@]}" >/dev/null
sleep 3
run_client "$CTL_V4" >"$OUT/control-client.log" || true
sleep 2
docker logs "$CTL" >"$OUT/control-coturn.log" 2>&1 || true

echo "== monitoring check (docker/public-server/check-turn.sh): fine with the shared secret, red after a mismatch"
# check-turn.sh runs from the stack directory with the test project through COMPOSE; the compose wrapper's
# variables are exported for it.
export GS_TEST_SIGNALING_IMAGE="$SIGNALING_IMAGE" GS_TEST_NET="$NET"
CHECK_COMPOSE="docker compose -p $PROJECT --env-file $WORK/.env -f $SRC/compose.yaml -f $ROOT/scripts/poc/public-server-logs.compose.yaml"
COMPOSE="$CHECK_COMPOSE" TURN_HOST="$TURN_V4" "$SRC/check-turn.sh"
# Mismatch: a coturn with another secret next to the signaling server's own secret (the control container
# is coturn with a different --static-auth-secret; the check reads the signaling container's secret).
mapfile -t BAD_ARGS < <(printf '%s\n' "${CTL_ARGS[@]}" | sed "s|^--static-auth-secret=.*|--static-auth-secret=$(openssl rand -hex 32)|")
docker rm -f "$CTL" >/dev/null
docker run -d --name "$CTL" --network "$NET" --ip "$CTL_V4" --ip6 fd00:5eed:a::11 --cap-drop ALL --cap-add NET_BIND_SERVICE \
	"$COTURN_IMAGE" "${BAD_ARGS[@]}" >/dev/null
sleep 3
# Point the check at the mismatching coturn: it execs into the compose service "coturn", so it is told the
# mismatch by a host that answers with the wrong secret: the control's address.
if COMPOSE="$CHECK_COMPOSE" TURN_HOST="$CTL_V4" "$SRC/check-turn.sh" 2>"$OUT/check-turn-mismatch.log"; then
	echo "public-server-logs: check-turn.sh did not fail on a TURN_SECRET mismatch" >&2
	exit 1
fi
echo "mismatch detected: $(cat "$OUT/check-turn-mismatch.log")"

echo "== log search"
compose logs --no-color >"$OUT/stack.log" 2>&1
status=0
search() { # <label> <file> <pattern>
	if grep -n -i -F -e "$3" "$2" >"$OUT/hits.txt"; then
		echo "FOUND $1: '$3' in $2 ($(wc -l <"$OUT/hits.txt") lines), first: $(head -n1 "$OUT/hits.txt" | cut -c1-200)"
		return 0
	fi
	return 1
}
for svc in coturn signaling; do
	docker logs "$(compose ps -q "$svc")" >"$OUT/$svc.log" 2>&1
	echo "$svc: $(wc -l <"$OUT/$svc.log") log lines"
done
for f in "$OUT/stack.log" "$OUT/coturn.log" "$OUT/signaling.log"; do
	for pat in "$CLIENT_IP" "$HEX_CLIENT" "$DENIED_PEER" "$SECRET" "$ISSUED_USER" "$ISSUED_PASS"; do
		if search "stack (must be absent)" "$f" "$pat"; then status=1; fi
	done
done
echo "control coturn: $(wc -l <"$OUT/control-coturn.log") log lines"
if search "control (must be present)" "$OUT/control-coturn.log" "$CLIENT_IP"; then
	:
else
	echo "public-server-logs: the control run did not log the client address, so the search proves nothing" >&2
	status=1
fi
grep -h BINDING_RESPONSE_BYTES "$OUT/client.log"
if [[ $status -ne 0 ]]; then
	echo "public-server-logs: FAILED (logs in $OUT)" >&2
	exit 1
fi
echo "public-server-logs: PASSED (client $CLIENT_IP, denied peer $DENIED_PEER absent from every stack log)"
