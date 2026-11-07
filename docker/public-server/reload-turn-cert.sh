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
# Why the host and not the acme container: signalling coturn from there would need the Docker socket (root on
# the host) or a shared PID namespace (which shows coturn's command line, with TURN_SECRET, to the container
# that talks to the internet). The host already holds both and already runs check-turn.sh.
#
# Env: COMPOSE (default "docker compose"), GS_CERT_STATE (default ~/.local/state/guardedsleep/turn-cert.sha256).
set -euo pipefail
cd "$(dirname "$0")"

COMPOSE="${COMPOSE:-docker compose}"
STATE="${GS_CERT_STATE:-${XDG_STATE_HOME:-$HOME/.local/state}/guardedsleep/turn-cert.sha256}"

# sha256sum inside the running acme container (it mounts the volume read-write; a one-off `run` container would
# carry the Traefik labels too).
NOW="$($COMPOSE exec -T acme sha256sum /certs/crt.pem 2>/dev/null | cut -d' ' -f1 || true)"
if [[ -z "$NOW" ]]; then
	echo "reload-turn-cert: no certificate in the turn-certs volume yet (see: docker compose logs acme)"
	exit 0
fi
OLD="$(cat "$STATE" 2>/dev/null || true)"
if [[ "$NOW" == "$OLD" ]]; then
	echo "reload-turn-cert: unchanged"
	exit 0
fi
if [[ -z "$OLD" ]]; then
	$COMPOSE restart coturn
	echo "reload-turn-cert: first certificate, coturn restarted"
else
	$COMPOSE kill -s USR2 coturn
	echo "reload-turn-cert: renewed certificate, coturn reloaded (SIGUSR2)"
fi
mkdir -p "$(dirname "$STATE")"
printf '%s\n' "$NOW" >"$STATE"
