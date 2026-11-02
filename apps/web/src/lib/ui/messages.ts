import type { PairingError } from "../contract";
import type { MessageKey } from "../i18n/keys";

/** The catalogue message for a pairing failure (alpha-ux-flows.md section 4.4). */
export function pairErrorKey(error: PairingError): MessageKey {
  switch (error) {
    case "camera_denied":
      return "native.ui.pair.error.cameraDenied";
    case "not_our_code":
      return "native.ui.pair.error.notOurs";
    case "expired":
      return "native.ui.pair.error.expired";
    case "key_mismatch":
      return "native.ui.pair.error.mismatch";
    case "already_paired":
      return "native.ui.pair.error.alreadyPaired";
    case "unreachable":
      return "native.ui.pair.error.unreachable";
    case "incomplete_paste":
      return "native.ui.pair.error.incomplete";
  }
}

/**
 * Core message keys arrive in snake_case (`server_unreachable`) and live in the `core` group in
 * camelCase. An explicit table keeps a key the catalogue does not know from rendering as a raw
 * key: it returns null and the caller shows its generic text instead.
 */
const CORE_KEYS: Readonly<Record<string, MessageKey>> = {
  pairing_key_mismatch: "core.pairingKeyMismatch",
  update_required: "core.updateRequired",
  server_unreachable: "core.serverUnreachable",
  peer_absent: "core.peerAbsent",
  media_blocked: "core.mediaBlocked",
  wrong_key_attempt: "core.wrongKeyAttempt",
  unknown_parent_attempt: "core.unknownParentAttempt",
  clock_skew: "core.clockSkew",
  recovery_path_unavailable: "core.recoveryPathUnavailable",
  pair_again: "core.pairAgain",
  baby_unrecognised: "core.babyUnrecognised",
};

export function coreMessageKey(key: string | null): MessageKey | null {
  return key !== null && Object.hasOwn(CORE_KEYS, key) ? (CORE_KEYS[key] ?? null) : null;
}

/**
 * Web-only failures of a baby run that could not start (`mic_denied`, `mic_unavailable`); every
 * other failure key is a core key.
 */
const BABY_FAILURE_KEYS: Readonly<Record<string, MessageKey>> = {
  mic_denied: "webScreens.baby.failMicDenied",
  mic_unavailable: "webScreens.baby.failMicUnavailable",
};

export function isMicFailure(failureKey: string | null): boolean {
  return failureKey !== null && Object.hasOwn(BABY_FAILURE_KEYS, failureKey);
}

/** The real reason a baby run failed, or null when there is none to show. */
export function babyFailureKey(failureKey: string | null): MessageKey | null {
  if (failureKey === null) return null;
  return Object.hasOwn(BABY_FAILURE_KEYS, failureKey)
    ? (BABY_FAILURE_KEYS[failureKey] ?? null)
    : coreMessageKey(failureKey);
}
