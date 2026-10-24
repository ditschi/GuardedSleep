/**
 * Wire types of the Rust core's Link API. The interfaces are generated from the Rust definitions
 * (`crates/guardedsleep-core/src/link.rs`, `protocol.rs`) by tsify in the `guardedsleep-wasm` build
 * and re-exported from the wasm-bindgen output (`pkg/`, built by `pnpm --filter
 * @guardedsleep/core-wasm build`). Field names are snake_case on purpose: this is the wire form
 * that crosses the WASM boundary. Only the runtime tables below are hand-written;
 * `core-wasm.test.ts` checks them against `messageKeys()` and a recorded sample of real actions.
 */

import type { InviteMailbox, LinkAction, LinkEvent } from "../pkg/guardedsleep_wasm.js";

export type {
  DeviceStatus,
  Dnd,
  IceCandidate,
  LinkAction,
  LinkConfig,
  InviteMailbox,
  LinkEvent,
  LinkInput,
  LinkState,
  LinkStatus,
  MailboxKind,
  Ringer,
  Role,
  UnitWarning,
} from "../pkg/guardedsleep_wasm.js";

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

/** Exhaustive tag tables: adding a variant to the Rust enum (hence the generated union) without listing it here fails `tsc`. */
export const LINK_ACTION_TYPES = {
  open_mailbox: true,
  mailbox_send: true,
  close_mailbox: true,
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

/** Why an invite text was rejected; the snake_case keys of the core's `InviteError`. */
export const INVITE_ERRORS = {
  not_our_code: true,
  incomplete_paste: true,
  expired: true,
} as const;
export type InviteErrorKey = keyof typeof INVITE_ERRORS;

export function isInviteErrorKey(v: unknown): v is InviteErrorKey {
  return typeof v === "string" && Object.hasOwn(INVITE_ERRORS, v);
}

/** A valid, unexpired invite. `psk` is the pairing key: keep it in memory only, never log it. */
export interface Invite {
  psk: Uint8Array;
  /** Kind and URL of each mailbox, in invite order (`relay` = own relay, `nostr`, `lan`). */
  mailboxes: InviteMailbox[];
  expiresAtEpochS: number;
}

export type ParseInviteResult = { ok: true; invite: Invite } | { ok: false; error: InviteErrorKey };

export type BuildInviteResult = { ok: true; text: string } | { ok: false; error: InviteErrorKey };
