#!/bin/sh
# lego deploy hook (acme-loop.sh): install the new chain and key into the turn-certs volume that coturn reads.
# Each file is written under a temporary name and renamed, key first, so coturn never reads a half-written
# file. The key belongs to nobody (coturn's user in its image) with mode 0600; the chain is public.
# Variables: LEGO_HOOK_CERT_PATH (chain, leaf first) and LEGO_HOOK_CERT_KEY_PATH, set by lego v5
# (cmd/internal/hook/metadata.go). The reload is reload-turn-cert.sh on the host (README).
set -eu
: "${LEGO_HOOK_CERT_PATH:?}" "${LEGO_HOOK_CERT_KEY_PATH:?}"
CERTS="${CERTS_DIR:-/certs}"
umask 077
cp "$LEGO_HOOK_CERT_KEY_PATH" "$CERTS/.key.pem.new"
cp "$LEGO_HOOK_CERT_PATH" "$CERTS/.crt.pem.new"
# chmod first: after the chown this root has no CAP_FOWNER and could no longer change the mode.
chmod 0600 "$CERTS/.key.pem.new"
chmod 0644 "$CERTS/.crt.pem.new"
chown 65534:65534 "$CERTS/.key.pem.new" "$CERTS/.crt.pem.new"
mv "$CERTS/.key.pem.new" "$CERTS/key.pem"
mv "$CERTS/.crt.pem.new" "$CERTS/crt.pem"
echo "acme: certificate installed for coturn"
