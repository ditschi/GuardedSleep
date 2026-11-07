#!/bin/sh
# Entrypoint of the acme service (compose.yaml): obtain the certificate for PUBLIC_HOST, then check once a day
# whether it is due. `lego run` is "get or renew": without a certificate it issues one, with one it renews only
# when due, so most iterations do nothing. HTTP-01 answers come from lego's own server on 8080 (Traefik routes
# /.well-known/acme-challenge/ there). A failed run (DNS, Traefik route, rate limit) is retried every 6 hours;
# Let's Encrypt allows 5 validation failures per host and hour, so this never trips the limit.
# lego v5 flags: https://go-acme.github.io/lego/usage/cli/ (check `lego run --help` when moving versions).
set -eu
: "${PUBLIC_HOST:?}" "${ACME_EMAIL:?}"
while :; do
	if lego --path /data run --accept-tos --email "$ACME_EMAIL" --server "${ACME_SERVER:-letsencrypt}" \
		--domains "$PUBLIC_HOST" --key-type EC256 --http --http.address :8080 \
		--deploy-hook /acme-deploy-hook.sh; then
		sleep 86400
	else
		echo "acme: lego run failed; retrying in 6 hours" >&2
		sleep 21600
	fi
done
