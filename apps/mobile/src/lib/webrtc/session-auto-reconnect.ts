/** Exponential backoff with cap — used when WebRTC/signaling drops. */
export const SESSION_AUTO_RECONNECT_MAX_ATTEMPTS = 36;

export function sessionReconnectDelayMs(attemptIndex: number): number {
  if (attemptIndex < 1) return 1400;
  return Math.min(45_000, Math.round(1400 * Math.pow(1.32, attemptIndex - 1)));
}
