import type { CauseHint, LinkPhase } from "../contract";

export type NoiseKind = "new" | "continue";

/** One-shot parent notices (alpha-ux-flows 4.13): none repeats, none loops. */
export type Notice =
  | { type: "peer_stopped" }
  | { type: "link_restored"; downMs: number }
  | { type: "low_battery"; pct: number; critical: boolean }
  | { type: "charger_unplugged" }
  | { type: "security_warning" }
  /** `count` is null unless the baby's summary was complete. */
  | { type: "missed_events"; count: number | null; downMs: number };

/** What the alert layer needs to know about the link to drive levels 1 to 3. */
export interface LinkSnapshot {
  phase: LinkPhase;
  level: number;
  /** "I'm checking": sound is muted while true. */
  acknowledged: boolean;
  causeHint: CauseHint | null;
  /** The link had verified before; a terminal failure then still alarms (defence in depth). */
  wasConnected: boolean;
}

/**
 * Everything the parent model can ask the platform to do with the user's attention. The web
 * implementation is `AlertPlayer` (tones after the sound-unlock gesture); tests record the calls.
 * Copy is never resolved here: a notice carries data and, where the core has one, its message key.
 */
export interface AlertSink {
  linkState(snapshot: LinkSnapshot): void;
  /** `link_lost`: escalation reached level 2 (once per outage). */
  linkLost(downMs: number): void;
  noiseAlert(kind: NoiseKind, probe: boolean, snoozed: boolean): void;
  noiseEnded(): void;
  /** "Stop sound": ends the sound of the current noise event. */
  stopNoiseSound(): void;
  notice(notice: Notice): void;
  /** Monitoring ended: cancel every loop. */
  clear(): void;
}
