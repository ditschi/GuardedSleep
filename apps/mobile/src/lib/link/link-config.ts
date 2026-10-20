import type { LinkConfig, Role } from "@guardedsleep/core-wasm";
import { heartbeatIntervalMs } from "$lib/app-preferences.js";
import { readBabyUnitPreferences } from "$lib/audio/baby-unit-preferences.js";

const MIN_HEARTBEAT_MS = 5_000;
const MAX_HEARTBEAT_MS = 15_000;
const DEFAULT_ALARM_AFTER_MS = 60_000;
/** The core requires detection (liveness) at least 20 s before the alarm level. */
const ALARM_MARGIN_MS = 20_000;

export function buildLinkConfig(
  role: Role,
  pskB64Url: string,
  pairedBefore: boolean,
  overrides?: { heartbeatMs?: number; threshold?: number; holdMs?: number },
): LinkConfig {
  const hb = Math.min(
    MAX_HEARTBEAT_MS,
    Math.max(MIN_HEARTBEAT_MS, overrides?.heartbeatMs ?? heartbeatIntervalMs()),
  );
  const liveness = 2 * hb + 5_000;
  const unit = readBabyUnitPreferences();
  return {
    role,
    psk_b64url: pskB64Url,
    paired_before: pairedBefore,
    heartbeat_interval_ms: hb,
    alarm_after_ms: Math.max(DEFAULT_ALARM_AFTER_MS, liveness + ALARM_MARGIN_MS),
    noise_threshold: overrides?.threshold ?? unit.threshold,
    noise_hold_ms: overrides?.holdMs ?? unit.holdMs,
  };
}
