#!/usr/bin/env bash
# Task 9.1 (alpha-connection-model, design section 7): an allocation made with a 60-second coturn
# REST credential keeps relaying (Refresh, CreatePermission, ChannelBind succeed, data flows) for
# 11+ minutes, past --stale-nonce=600 and long past the credential expiry.
#
# Runs the gate's coturn image (coturn/coturn:4.18.0, same pin as env-up.sh and local-gate.sh) with the
# flags of the planned project profile (--use-auth-secret, --stale-nonce=600; the gate's own coturn
# uses a static --user instead). Coturn and the client sit on a user-defined bridge network, because
# rootless Docker's host network is not the real host network. The secret is generated per run.
#
# Takes about 12 minutes. Needs Docker, else prints SKIPPED (GS_STRICT=1 turns that into a failure).
# Env: DURATION_S (700), GS_TURN_LIFETIME_OUT (target/turn-lifetime).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=scripts/lib/require.sh
source "$ROOT/scripts/lib/require.sh"
require turn-lifetime docker

COTURN_IMAGE=coturn/coturn:4.18.0
# Same digest as docker/web/Dockerfile (node 22 LTS, alpine); runs the client script only.
NODE_IMAGE=node:22-alpine@sha256:0a7108bf6c7bf5de370ffb1a3ed6be93d405b43ff159f681a8d18c0e2bc2e402
OUT="${GS_TURN_LIFETIME_OUT:-$ROOT/target/turn-lifetime}"
DURATION_S="${DURATION_S:-700}"
[[ "$DURATION_S" =~ ^[0-9]{3,4}$ ]] || {
	echo "turn-lifetime: DURATION_S must be an integer of 3 or 4 digits (got '$DURATION_S')" >&2
	exit 2
}
SUFFIX="$$"
NET="gs-turnlife-net-$SUFFIX"
TURN="gs-turnlife-coturn-$SUFFIX"
CLIENT="gs-turnlife-client-$SUFFIX"
mkdir -p "$OUT"

cleanup() {
	docker logs "$TURN" >"$OUT/coturn.log" 2>&1 || true
	docker rm -f "$CLIENT" "$TURN" >/dev/null 2>&1 || true
	docker network rm "$NET" >/dev/null 2>&1 || true
}
trap cleanup EXIT

SECRET="$(openssl rand -hex 32)"
docker network create "$NET" >/dev/null
# The image entrypoint evals its arguments: no shell characters in the values.
docker run -d --name "$TURN" --network "$NET" "$COTURN_IMAGE" \
	--log-file=stdout --simple-log --verbose --fingerprint \
	--use-auth-secret --static-auth-secret="$SECRET" --realm=guardedsleep.poc \
	--listening-port=3478 --min-port=49160 --max-port=49260 \
	--no-tcp-relay --stale-nonce=600 >/dev/null
TURN_IP="$(docker inspect -f "{{(index .NetworkSettings.Networks \"$NET\").IPAddress}}" "$TURN")"
sleep 2
[[ "$(docker inspect -f '{{.State.Running}}' "$TURN")" == true ]] || {
	echo "turn-lifetime: coturn did not start:" >&2
	docker logs "$TURN" >&2 || true
	exit 1
}
docker logs "$TURN" 2>&1 | grep -i -m1 'coturn-\|version' || true

status=0
docker run --rm --name "$CLIENT" --network "$NET" -e TURN_SECRET="$SECRET" -e TURN_HOST="$TURN_IP" \
	-e DURATION_S="$DURATION_S" -v "$ROOT/scripts/poc/turn-lifetime-client.mjs:/client.mjs:ro" \
	"$NODE_IMAGE" node /client.mjs | tee "$OUT/client.log" || status=$?

docker logs "$TURN" >"$OUT/coturn.log" 2>&1 || true
echo "turn-lifetime: coturn lines relevant to the verdict (full log: $OUT/coturn.log)"
grep -i -E 'nonce|expired|refresh|session .*(deleted|closed)|channel|permission|incorrect|error' "$OUT/coturn.log" |
	grep -v -F "$SECRET" | tail -n 40 || true
if [[ $status -ne 0 ]]; then
	echo "turn-lifetime: FAILED (exit $status)" >&2
	exit "$status"
fi
echo "turn-lifetime: PASSED"
