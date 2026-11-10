#!/usr/bin/env bash
# TURN monitoring check (design section 9, "Monitoring"): one authenticated allocation against the coturn of
# this stack with a credential signed by the secret in turnserver.conf (the file coturn verifies with; the
# secret is not on coturn's command line and `docker inspect` is not used). A STUN or TCP check on 3478
# cannot notice a TURN_SECRET mismatch between coturn and the signaling server (for example after a
# half-finished rotation); this one fails then: on a TURN_SECRET in .env that differs from the file, and
# when coturn refuses the allocation.
#
# Run it from this directory by cron on the host, for example every 5 minutes:
#   */5 * * * * cd /opt/guardedsleep/docker/public-server && ./check-turn.sh
# With KUMA_PUSH_URL (env or .env) it reports to an Uptime Kuma push monitor (status up or down) with
# https://github.com/louislam/uptime-kuma/wiki/Push-monitor ; set the monitor's heartbeat to 10 minutes so a
# silent host also turns it red. Without it the script only exits 0 (allocation works) or 1.
# The push URL carries a token: it is never printed. No client data is involved; the allocation comes from
# the host itself and logs nothing (coturn logs to /dev/null).
#
# Env: COMPOSE (default "docker compose"), TURN_HOST (default TURN_IPV4 of .env), KUMA_PUSH_URL,
# TURN_CONF_FILE (default ./turnserver.conf, as in compose.yaml); the script needs read access to that file.
set -euo pipefail
cd "$(dirname "$0")"

COMPOSE="${COMPOSE:-docker compose}"
dotenv() { # <name>: the value of NAME= in .env, empty when absent
	[[ -f .env ]] && sed -n "s/^$1=//p" .env | tail -n1 || true
}
TURN_HOST="${TURN_HOST:-$(dotenv TURN_IPV4)}"
KUMA_PUSH_URL="${KUMA_PUSH_URL:-$(dotenv KUMA_PUSH_URL)}"
TURN_CONF_FILE="${TURN_CONF_FILE:-$(dotenv TURN_CONF_FILE)}"
TURN_CONF_FILE="${TURN_CONF_FILE:-./turnserver.conf}"

push() { # <up|down> <message>
	[[ -n "$KUMA_PUSH_URL" ]] || return 0
	curl -fsS --max-time 10 --get --data-urlencode "status=$1" --data-urlencode "msg=$2" "$KUMA_PUSH_URL" >/dev/null || true
}
fail() {
	echo "check-turn: FAILED: $1" >&2
	push down "$1"
	exit 1
}

[[ -n "$TURN_HOST" ]] || fail "no TURN_HOST and no TURN_IPV4 in .env"
SIGNALING="$($COMPOSE ps -q signaling)"
COTURN="$($COMPOSE ps -q coturn)"
[[ -n "$SIGNALING" && -n "$COTURN" ]] || fail "signaling or coturn container is not running"
# The secret coturn verifies with, from the file it mounts.
[[ -r "$TURN_CONF_FILE" ]] || fail "cannot read the coturn config file (TURN_CONF_FILE)"
SECRET="$(sed -n 's/^static-auth-secret=//p' "$TURN_CONF_FILE" | tail -n1)"
[[ -n "$SECRET" ]] || fail "the coturn config file has no static-auth-secret"
# The signaling server gets TURN_SECRET from .env: a different value there is a half-finished rotation.
ENV_SECRET="${TURN_SECRET:-$(dotenv TURN_SECRET)}"
if [[ -n "$ENV_SECRET" && "$ENV_SECRET" != "$SECRET" ]]; then
	fail "TURN_SECRET in .env differs from static-auth-secret in the coturn config file"
fi

# turnutils_uclient builds the REST user name (expiry:name) itself with -W; -n 1 -c sends one packet and no RTCP.
# Its exit status is 0 even when the allocation fails, so the verdict is its output: a failed allocation prints
# "Cannot complete Allocation". The secret is passed on the command line inside the container only.
OUT="$(docker exec "$COTURN" timeout 30 turnutils_uclient -n 1 -c -W "$SECRET" -u gs-monitor -e 192.0.2.1 "$TURN_HOST" 2>&1)" || true
if grep -q 'Cannot complete Allocation' <<<"$OUT"; then
	fail "TURN allocation refused (TURN_SECRET mismatch between signaling and coturn, or coturn down)"
fi
grep -q 'start_mclient' <<<"$OUT" || fail "TURN allocation check produced no result"

# Allocation count (abuse alarm, review 2026-11 R2): coturn binds one relay port per allocation and keeps
# no counter with logging off, so count the bound UDP ports of the relay range (coturn runs on the host
# network). WARN at 80 % of --total-quota; the check still passes, the allocation itself works.
MIN_PORT="${TURN_MIN_PORT:-$(dotenv TURN_MIN_PORT)}"; MIN_PORT="${MIN_PORT:-49160}"
MAX_PORT="${TURN_MAX_PORT:-$(dotenv TURN_MAX_PORT)}"; MAX_PORT="${MAX_PORT:-49359}"
QUOTA="${TURN_TOTAL_QUOTA:-$(dotenv TURN_TOTAL_QUOTA)}"; QUOTA="${QUOTA:-200}"
if command -v ss >/dev/null 2>&1; then
	LIVE="$(ss -H -u -a -n "sport >= :$MIN_PORT and sport <= :$MAX_PORT" | wc -l)"
	if (( LIVE * 100 >= QUOTA * 80 )); then
		MSG="WARN: $LIVE of $QUOTA TURN allocations in use (80 % threshold); lower TURN_MAX_LIVE_PER_ADDR or the quotas"
		echo "check-turn: $MSG" >&2
		push up "$MSG"
		exit 0
	fi
else
	echo "check-turn: ss not found, allocation count skipped" >&2
fi
echo "check-turn: OK"
push up OK
