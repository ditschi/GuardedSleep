#!/usr/bin/env bash
# Shared plumbing of scripts/poc/public-server-logs.sh (task 10.1) and public-server-turns.sh (task 10.2): the
# production docker/public-server/compose.yaml, filled with test values, plus the bridge override
# public-server-logs.compose.yaml, run as a throw-away compose project.
#
# Source it after setting: ROOT SRC (docker/public-server) PROJECT NET WORK (a mktemp -d) OUT TAG (script name for
# messages), and for ps_fill_env TURN_V4 TURN_V6; SIGNALING_IMAGE is optional (an unused service gets a dummy).
# PS_LEGO=1 (set before sourcing) adds docker/public-server/compose.lego.yaml and its .env values (the lego variant of turns:).
# Provides: COMPOSE_ARGS (array, everything after `docker compose`), compose, ps_fill_env, ps_cleanup, ps_require_running.
# shellcheck shell=bash

COMPOSE_ARGS=(-p "$PROJECT" --env-file "$WORK/.env" -f "$SRC/compose.yaml" -f "$ROOT/scripts/poc/public-server-logs.compose.yaml")
if [[ "${PS_LEGO:-0}" == 1 ]]; then COMPOSE_ARGS+=(-f "$SRC/compose.lego.yaml"); fi

compose() {
	GS_TEST_SIGNALING_IMAGE="${SIGNALING_IMAGE:-unused}" GS_TEST_NET="$NET" docker compose "${COMPOSE_ARGS[@]}" "$@"
}

# ps_fill_env <turn secret>: the template, filled with test values, as $WORK/.env, and the coturn config file
# ($WORK/turnserver.conf, mounted through TURN_CONF_FILE; the secret is not on coturn's command line).
ps_fill_env() {
	sed -e "s|^TURN_SECRET=.*|TURN_SECRET=$1|" \
		-e "s|^SIGNALING_TAG=.*|SIGNALING_TAG=test|" \
		-e "s|^TURN_IPV4=.*|TURN_IPV4=$TURN_V4|" \
		-e "s|^TURN_IPV6=.*|TURN_IPV6=$TURN_V6|" "$SRC/.env.template" >"$WORK/.env"
	printf 'static-auth-secret=%s\n' "$1" >"$WORK/turnserver.conf"
	chmod 644 "$WORK/turnserver.conf" # coturn runs as nobody; $WORK is a throw-away directory
	echo "TURN_CONF_FILE=$WORK/turnserver.conf" >>"$WORK/.env"
	if [[ "${PS_LEGO:-0}" == 1 ]]; then
		printf '%s\n' "ACME_EMAIL=test@example.invalid" "TURN_TLS_ARG_1=--cert=/certs/crt.pem" "TURN_TLS_ARG_2=--pkey=/certs/key.pem" >>"$WORK/.env"
	fi
}

# ps_cleanup [--volumes]: keep the stack log, remove the project (and its volumes), the network and $WORK.
# The caller removes its own extra containers first (they sit on the network).
ps_cleanup() {
	compose logs --no-color >"$OUT/stack.log" 2>&1 || true
	compose down --remove-orphans "$@" >/dev/null 2>&1 || true
	docker network rm "$NET" >/dev/null 2>&1 || true
	rm -rf "$WORK"
}

# ps_require_running <0|1> <service>...: every service must be running; with 1 also never restarted.
ps_require_running() {
	local check_restarts="$1" s
	shift
	for s in "$@"; do
		[[ "$(compose ps --format '{{.State}}' "$s")" == running ]] || ps_not_running "$s"
		if [[ "$check_restarts" == 1 && "$(docker inspect -f '{{.RestartCount}}' "$(compose ps -q "$s")")" != 0 ]]; then
			ps_not_running "$s"
		fi
	done
}
ps_not_running() {
	echo "$TAG: $1 is not running:" >&2
	compose logs --no-color "$1" >&2 || true
	exit 1
}
