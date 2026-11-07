#!/usr/bin/env bash
# Log canary over the files an emulator-gate run produced (AGENTS.md section 7.6): fails if a gate PSK,
# a key derived from it (envelope key, relay admission key), the room id, an invite carrying the key or
# plaintext SDP appears in any of them in any encoding (raw, hex, base64, base64url, percent-escaped).
# The scanner proves itself first (a planted value in every encoding must be found) and prints only
# canary names and file:line, never a value. Paths are relative to the repository root.
#   scripts/poc/canary-scan.sh target/poc-gate
# Not covered here: the seed (the platform draws it per run), the parent secret (drawn at run time on
# the device, in the Keystore) and issued TURN passwords (the gate's coturn uses a static test user);
# the simulation scans the seed and the parent secret, `just public-server-logs` the TURN password.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib/canary.env
source "$ROOT/scripts/poc/lib/canary.env"
cd "$ROOT"
[[ $# -gt 0 ]] || {
	echo "usage: canary-scan.sh <file|dir>..." >&2
	exit 2
}
exec scripts/cargo.sh run -q -p guardedsleep-sim --bin gs-sim -- canary-scan \
	--psk-b64url "$GATE_PSK" --psk-b64url "$GATE_PSK_OTHER" "$@"
