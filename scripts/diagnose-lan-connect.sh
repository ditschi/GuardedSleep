#!/usr/bin/env bash
# Quick LAN connectivity checklist for phone + laptop browser setups.
# Usage (repo root, with dev stack running): just diagnose-lan
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WEB_PORT="${WEB_PORT:-5173}"
SIGNAL_PORT="${SIGNAL_PORT:-8787}"
E2E_SIGNAL_PORT="${GUARDEDSLEEP_E2E_SIGNAL_PORT:-18787}"
WEB_HTTPS_PORT="${WEB_HTTPS_PORT:-8443}"

if [[ -f "$ROOT/docker/stack/.env" ]]; then
  # shellcheck disable=SC1090
  eval "$(grep -E '^(WEB_HTTPS_PORT)=' "$ROOT/docker/stack/.env" | sed 's/^/export /')"
fi

LAN_IP="$(bash "$ROOT/scripts/stack-lan-env.sh" --dry-run 2>/dev/null | sed -n 's/^# Host LAN IP: //p' | head -1)"

echo "=== GuardedSleep LAN connectivity diagnose ==="
echo ""

if [[ -z "$LAN_IP" ]]; then
  echo "FAIL: Could not detect LAN IPv4. Connect Wi-Fi/Ethernet."
  exit 1
fi

echo "LAN IP:            $LAN_IP"
echo "Dev PWA:           http://${LAN_IP}:${WEB_PORT}/"
echo "Dev signaling:     ws://${LAN_IP}:${SIGNAL_PORT}     (just dev)"
echo "E2E signaling:     ws://${LAN_IP}:${E2E_SIGNAL_PORT}  (just e2e)"
if [[ -f "$ROOT/docker/stack/.env" ]]; then
  echo "Docker signaling:  wss://${LAN_IP}:${WEB_HTTPS_PORT} (same origin as the PWA)"
  echo "Docker PWA:        https://${LAN_IP}:${WEB_HTTPS_PORT}/"
fi
echo ""

fail=0

probe_http() {
  local label="$1"
  local url="$2"
  local optional="${3:-}"
  if curl -fsS --max-time 3 "$url" >/dev/null 2>&1; then
    echo "OK   $label — $url"
  else
    if [[ "$optional" == "optional" ]]; then
      echo "SKIP $label — $url (not running)"
    else
      echo "FAIL $label — $url (is the stack running? firewall open?)"
      fail=1
    fi
  fi
}

probe_http "Vite dev (LAN)" "http://${LAN_IP}:${WEB_PORT}/"
probe_http "Signaling HTTP health" "http://${LAN_IP}:${SIGNAL_PORT}/"
probe_http "E2E signaling HTTP health" "http://${LAN_IP}:${E2E_SIGNAL_PORT}/" "optional"

echo ""
echo "WebSocket probe:"
if (
  cd "$ROOT/apps/e2e"
  node --input-type=module -e "
import WebSocket from 'ws';
const url = process.argv[1];
await new Promise((resolve, reject) => {
  const ws = new WebSocket(url);
  const t = setTimeout(() => { ws.terminate(); reject(new Error('timeout')); }, 4000);
  ws.once('open', () => { clearTimeout(t); ws.close(); resolve(); });
  ws.once('error', (e) => { clearTimeout(t); reject(e); });
});
" "ws://${LAN_IP}:${SIGNAL_PORT}"
) 2>/dev/null; then
  echo "OK   ws://${LAN_IP}:${SIGNAL_PORT}"
else
  echo "FAIL ws://${LAN_IP}:${SIGNAL_PORT}"
  fail=1
fi

echo ""
echo "Common pitfalls:"
echo "  • QR / invite uses ws://127.0.0.1 — on the phone that is the phone itself, not your laptop."
echo "    Fix: Settings → General → WebSocket URL = ws://${LAN_IP}:${SIGNAL_PORT} (or Docker wss://…), then regenerate QR."
echo "  • Phone and laptop must be on the same Wi-Fi; guest/isolated VLAN blocks LAN."
echo "  • Host firewall must allow TCP ${WEB_PORT} and ${SIGNAL_PORT} (e.g. ufw allow ${SIGNAL_PORT}/tcp)."
echo "  • Docker stack: open https://${LAN_IP}:${WEB_HTTPS_PORT}/ on the phone and accept the self-signed cert."
echo ""

if [[ "$fail" -ne 0 ]]; then
  echo ""
  echo "Result: FAIL — fix the items above, then retry."
  exit 1
fi

echo "Result: OK — LAN endpoints reachable from this host."
