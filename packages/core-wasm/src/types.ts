/**
 * Hand-written mirror of the serde JSON of the Rust core's Link API
 * (`crates/guardedsleep-core/src/link.rs`, `protocol.rs`). Field names are snake_case on purpose:
 * this is the wire form that crosses the WASM boundary. `core-wasm.test.ts` checks these types
 * against `messageKeys()` and a recorded sample of real actions.
 */

export type Role = "baby" | "parent";

export type LinkConfig = {
  role: Role;
  /** Pairing PSK, base64url without padding (32 bytes). The core derives the room id from it. */
  psk_b64url: string;
  /** The PSK verified successfully in an earlier run, so a key mismatch is never terminal. */
  paired_before?: boolean;
  /** 5000..=15000. */
  heartbeat_interval_ms: number;
  /** Escalation level 2 starts this long after the last sign of life; at least liveness + 20 s. */
  alarm_after_ms: number;
  noise_threshold: number;
  noise_hold_ms: number;
  /**
   * Verification also needs `attempt_dtls_fingerprint` (the remote certificate's SHA-256
   * fingerprint from WebRTC stats). Defaults to true when omitted.
   */
  require_dtls_fingerprint?: boolean;
};

export type Ringer = "normal" | "vibrate" | "silent" | "unknown";
export type UnitWarning =
  | "page_hidden"
  | "wake_lock_off"
  | "resumed_from_sleep"
  | "not_charging"
  | "mic_permission_lost";
export type Dnd = "off" | "alarms_only" | "priority" | "total" | "unknown";

/** Every field may be absent (unknown). A browser baby reports at most battery and charging. */
export type DeviceStatus = {
  battery_pct?: number;
  charging?: boolean;
  ringer?: Ringer;
  dnd?: Dnd;
  alarms_allowed?: boolean;
  dnd_access?: boolean;
  quiet_active?: boolean;
  next_alarm_min?: number;
  mic_silent?: boolean;
  noise_threshold?: number;
  /** Sorted, deduplicated, at most 8; omitted when empty. */
  unit_warnings?: UnitWarning[];
};

export type IceCandidate = {
  candidate: string;
  sdp_mid: string | null;
  sdp_m_line_index: number | null;
};

export type LinkState =
  | "connecting"
  | "connected"
  | "reconnecting"
  | "peer_stopped"
  | "failed_needs_user"
  | "stopped";

export type LinkStatus = {
  state: LinkState;
  /** Escalation level 0-3 while connecting / reconnecting, otherwise 0. */
  level: number;
  /** `failed_needs_user` key. */
  key: string | null;
  /** Troubleshooter hint from level 2. */
  cause: string | null;
  /** Parent: levels 2-3 are muted until this time (core clock). */
  acknowledged_until: number | null;
  /**
   * Monotonic time (input clock) of the last verified contact, for "last contact N min ago". Set
   * while connecting/reconnecting/peer_stopped after a first contact, null while connected.
   */
  last_alive_ms: number | null;
};

/** Every UI key the core can put into `LinkStatus.key`, `LinkStatus.cause` or a security warning. */
export const LINK_MESSAGE_KEYS = [
  "pairing_key_mismatch",
  "update_required",
  "server_unreachable",
  "peer_absent",
  "media_blocked",
  "wrong_key_attempt",
] as const;
export type LinkMessageKey = (typeof LINK_MESSAGE_KEYS)[number];

export type LinkInput =
  | { type: "start"; entropy: number }
  | { type: "stop" }
  | { type: "acknowledge_alarm" }
  | { type: "network_available" }
  | { type: "signaling_opened" }
  | { type: "signaling_message"; text: string }
  | { type: "signaling_closed" }
  | { type: "attempt_local_description"; attempt: number; sdp_type: string; sdp: string }
  | { type: "attempt_local_ice"; attempt: number; candidate: IceCandidate }
  | { type: "attempt_data_open"; attempt: number }
  | { type: "attempt_data_message"; attempt: number; text: string }
  | { type: "attempt_data_closed"; attempt: number }
  /** Remote DTLS certificate SHA-256 fingerprint from `RTCCertificateStats` (`AB:CD:..` or hex). */
  | { type: "attempt_dtls_fingerprint"; attempt: number; fingerprint: string }
  | { type: "attempt_pc_state"; attempt: number; state: string }
  | { type: "audio_level"; rms: number }
  | { type: "device_status"; status: DeviceStatus }
  | { type: "set_noise_gate"; threshold: number; hold_ms: number }
  | { type: "set_listen"; enabled: boolean }
  | { type: "request_probe" }
  | { type: "set_snooze"; until_ms: number | null }
  | { type: "request_quiet" }
  | { type: "quiet_applied"; state: DeviceStatus }
  | { type: "tick" };

export type LinkEvent =
  | { type: "status"; status: LinkStatus }
  | { type: "link_lost"; down_ms: number }
  | { type: "link_restored"; down_ms: number }
  | { type: "security_warning"; key: string }
  | { type: "peer_stopped" }
  | { type: "alarm_acknowledged"; until_ms: number }
  | { type: "noise_alert"; rms: number; probe: boolean; snoozed: boolean }
  | { type: "peer_heartbeat"; seq: number; rms: number | null }
  | { type: "peer_status"; status: DeviceStatus }
  | { type: "low_battery"; pct: number; critical: boolean }
  | { type: "charger_unplugged" }
  | { type: "missed_events"; count: number; max_rms: number; complete: boolean; down_ms: number }
  | { type: "noise_gate_changed"; threshold: number; hold_ms: number }
  | { type: "listen_changed"; enabled: boolean }
  | { type: "failed"; reason: string };

export type LinkAction =
  | { type: "open_signaling" }
  | { type: "close_signaling" }
  | { type: "send_signaling"; text: string }
  | { type: "create_attempt"; attempt: number; offerer: boolean }
  | { type: "close_attempt"; attempt: number }
  | { type: "create_offer"; attempt: number }
  | { type: "set_remote_description"; attempt: number; sdp_type: string; sdp: string }
  | { type: "create_answer"; attempt: number }
  | { type: "add_ice_candidate"; attempt: number; candidate: IceCandidate }
  | { type: "send_data"; attempt: number; text: string }
  | { type: "set_mic_enabled"; enabled: boolean }
  | { type: "apply_quiet" }
  | { type: "send_probe_tone"; duration_ms: number }
  | { type: "emit"; event: LinkEvent };

/** Exhaustive tag tables: adding a variant to a union above without listing it here fails `tsc`. */
export const LINK_ACTION_TYPES = {
  open_signaling: true,
  close_signaling: true,
  send_signaling: true,
  create_attempt: true,
  close_attempt: true,
  create_offer: true,
  set_remote_description: true,
  create_answer: true,
  add_ice_candidate: true,
  send_data: true,
  set_mic_enabled: true,
  apply_quiet: true,
  send_probe_tone: true,
  emit: true,
} as const satisfies Record<LinkAction["type"], true>;

export const LINK_EVENT_TYPES = {
  status: true,
  link_lost: true,
  link_restored: true,
  security_warning: true,
  peer_stopped: true,
  alarm_acknowledged: true,
  noise_alert: true,
  peer_heartbeat: true,
  peer_status: true,
  low_battery: true,
  charger_unplugged: true,
  missed_events: true,
  noise_gate_changed: true,
  listen_changed: true,
  failed: true,
} as const satisfies Record<LinkEvent["type"], true>;

export function isLinkAction(v: unknown): v is LinkAction {
  if (typeof v !== "object" || v === null) return false;
  const t = (v as { type?: unknown }).type;
  if (typeof t !== "string" || !(t in LINK_ACTION_TYPES)) return false;
  if (t !== "emit") return true;
  const ev = (v as { event?: { type?: unknown } }).event;
  return typeof ev?.type === "string" && ev.type in LINK_EVENT_TYPES;
}
