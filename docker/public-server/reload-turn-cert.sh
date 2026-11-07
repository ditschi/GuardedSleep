#!/usr/bin/env bash
# Make coturn use the certificate the acme service installed (design section 9, task 10.2).
#
# Run it from this directory by cron on the host, for example once a day (check-turn.sh uses the same pattern):
#   17 4 * * * cd /opt/guardedsleep/docker/public-server && ./reload-turn-cert.sh
# It compares the SHA-256 of crt.pem in the turn-certs volume with the one it loaded last (a state file) and acts
# only on a change:
#   - first certificate (no state yet): `docker compose restart coturn`, because coturn started without a
#     certificate has no TLS listener, and a reload does not create one;
#   - later certificate: SIGUSR2, which re-reads --cert and --pkey without dropping allocations
#     (coturn src/apps/relay/mainrelay.c, handler reload_ssl_certs; added in 4.5.0.8).
# Exit status: 0 when nothing is wrong (including "no certificate yet", which is normal before the first issuance),
# 1 when the acme container cannot be queried or coturn cannot be restarted or signalled, so cron mails the error.
# Why the host and not the acme container: signalling coturn from there would need the Docker socket (root on
# the host) or a shared PID namespace (which shows coturn's command line, with TURN_SECRET, to the container
# that talks to the internet). The host already holds both and already runs check-turn.sh.
#
# Env: COMPOSE (default "docker compose"), GS_CERT_STATE (default ~/.local/state/guardedsleep/turn-cert.sha256).
set -euo pipefail
cd "$(dirname "$0")"

read -ra COMPOSE_CMD <<<"${COMPOSE:-docker compose}"
STATE="${GS_CERT_STATE:-${XDG_STATE_HOME:-$HOME/.local/state}/guardedsleep/turn-cert.sha256}"
fail() {
	echo "reload-turn-cert: FAILED: $1" >&2
	exit 1
}

# sha256sum inside the running acme container (it mounts the volume read-write; a one-off `run` container would
# carry the Traefik labels too). A missing file is the normal state before the first issuance and is told apart
# from a compose or container error by the word "absent".
if ! PROBE="$("${COMPOSE_CMD[@]}" exec -T acme sh -c 'if [ -f /certs/crt.pem ]; then sha256sum /certs/crt.pem; else echo absent; fi' 2>&1)"; then
	fail "cannot query the acme container (not running? see: docker compose ps acme): $PROBE"
fi
if [[ "$PROBE" == absent ]]; then
	echo "reload-turn-cert: no certificate yet (see: docker compose logs acme)"
	exit 0
fi
NOW="${PROBE%% *}"
[[ "$NOW" =~ ^[0-9a-f]{64}$ ]] || fail "unexpected answer from the acme container: $PROBE"
OLD="$(cat "$STATE" 2>/dev/null || true)"
if [[ "$NOW" == "$OLD" ]]; then
	echo "reload-turn-cert: unchanged"
	exit 0
fi
if [[ -z "$OLD" ]]; then
	"${COMPOSE_CMD[@]}" restart coturn || fail "docker compose restart coturn failed"
	echo "reload-turn-cert: first certificate, coturn restarted"
else
	"${COMPOSE_CMD[@]}" kill -s USR2 coturn || fail "docker compose kill -s USR2 coturn failed (coturn not running?)"
	echo "reload-turn-cert: renewed certificate, coturn reloaded (SIGUSR2)"
fi
# The state moves only after coturn took the certificate, written atomically so a crash never leaves it empty.
mkdir -p "$(dirname "$STATE")"
printf '%s\n' "$NOW" >"$STATE.tmp"
mv "$STATE.tmp" "$STATE"
