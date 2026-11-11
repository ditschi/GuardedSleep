/* tslint:disable */
/* eslint-disable */
/**
 * Coarse baby-phone state shown to the parent (ADR-018). Every field may be unknown (`None`).
 */
export interface DeviceStatus {
    /**
     * Rounded to 5 by [`DeviceStatus::normalized`].
     */
    battery_pct?: number;
    charging?: boolean;
    ringer?: Ringer;
    dnd?: Dnd;
    alarms_allowed?: boolean;
    /**
     * The app may control Do Not Disturb.
     */
    dnd_access?: boolean;
    /**
     * Our own silence rule is active.
     */
    quiet_active?: boolean;
    /**
     * Minutes until the next alarm clock, rounded to multiples of 15 and capped at
     * [`MAX_NEXT_ALARM_MIN`].
     */
    next_alarm_min?: number;
    /**
     * The microphone delivered only digital silence for 30 s (set by the core, not the platform).
     */
    mic_silent?: boolean;
    /**
     * The baby's live noise-gate threshold (0.01..=1.0), set by the core.
     */
    noise_threshold?: number;
    /**
     * Reliability warnings reported by the baby platform; sorted and deduplicated by
     * [`DeviceStatus::normalized`] and the parser, omitted on the wire when empty.
     */
    unit_warnings?: UnitWarning[];
    /**
     * Talk-back as this recipient sees it (set by the core of the baby, never by the platform).
     */
    talk?: TalkStatus;
    /**
     * Who talks right now, from this recipient's point of view (set by the core).
     */
    talker?: Talker;
    /**
     * Noise detection is paused while a talk plays (set by the core; omitted when not paused).
     */
    noise_paused?: boolean;
    /**
     * While a talk is denied for the cooldown or the budget: milliseconds until the next request
     * can pass, counted from when this status was sent (set by the core).
     */
    talk_retry_in_ms?: number;
    /**
     * `uncertain` after three talks in a row that the playback check could not confirm.
     */
    talk_audible?: TalkAudible;
}

/**
 * One WebRTC ICE server entry of an attempt. STUN entries carry no user name or credential.
 */
export interface IceServer {
    urls: string[];
    username?: string;
    credential?: string;
}

/**
 * One mailbox of an invite: its kind and the URL the platform opens (`wss://host[:port][/path]` for a relay, `ws://ip:port` for a LAN mailbox).
 */
export interface InviteMailbox {
    kind: MailboxKind;
    url: string;
}

/**
 * Talk-back volume of the baby unit, a setting of the baby platform (`set_talk_back`).
 */
export type TalkLevel = "low" | "medium" | "high";

/**
 * The baby unit's playback check of one talk (`talk_playback`, `talk-back` "Playback check").
 */
export type PlaybackResult = "heard" | "unconfirmed" | "unknown";

/**
 * The talk state a parent UI shows (`LinkEvent::TalkChanged`).
 */
export type TalkState = { state: "requesting" } | { state: "talking"; until_ms: number } | { state: "ended"; reason: TalkEndReason } | { state: "denied"; reason: TalkDenyReason | null };

/**
 * Whether the baby unit lets this parent talk (`device_status.talk`, `talk-back` "Honest
 * playability").
 */
export type TalkStatus = "on" | "off" | "muted" | "unsupported";

/**
 * Who talks to the child right now, from the recipient's point of view
 * (`device_status.talker`).
 */
export type Talker = "none" | "you" | "other";

/**
 * Why a baby unit may stop monitoring reliably (mainly a web/laptop baby). Variant order is the
 * stable wire order.
 */
export type UnitWarning = "page_hidden" | "wake_lock_off" | "resumed_from_sleep" | "not_charging" | "mic_permission_lost";

/**
 * Why a talk ended on the parent (`TalkState::Ended`).
 */
export type TalkEndReason = "released" | "limit" | "no_answer" | "link_lost" | "mic_failed" | "alert" | "switched_off" | "stopped";

/**
 * Why the baby unit denied a `talk_request`. The retry hints are milliseconds from the denial.
 */
export type TalkDenyReason = { kind: "off" } | { kind: "unsupported" } | { kind: "muted" } | { kind: "busy" } | { kind: "cooldown"; retry_in_ms: number } | { kind: "budget"; retry_in_ms: number } | { kind: "rate" };

/**
 * `device_status.talk_audible`: the baby unit's own microphone could not hear three talks in a
 * row, so the voice may not be audible there. A warning only, never a denial.
 */
export type TalkAudible = "ok" | "uncertain";

export interface IceCandidate {
    candidate: string;
    sdp_mid: string | null;
    sdp_m_line_index: number | null;
}

export interface LinkConfig {
    role: Role;
    /**
     * Pairing PSK (base64url). The room id is derived from it.
     */
    psk_b64url: string;
    /**
     * 32 fresh random bytes for the in-core CSPRNG (envelope nonces, session nonces, parent
     * secret). The platform draws a new seed for every run (Android `SecureRandom`, web
     * `crypto.getRandomValues`); reusing one would repeat nonces under a long-lived key.
     * Secret-class: never logged. `Link::new` fails without exactly 32 bytes, takes the bytes
     * and wipes its copy. JSON form: 64 hex characters.
     */
    seed: string;
    /**
     * The PSK was verified successfully in an earlier run, so a mismatch is never terminal.
     */
    paired_before?: boolean;
    /**
     * 5000..=15000.
     */
    heartbeat_interval_ms: number;
    /**
     * Escalation level 2 starts this long after the last sign of life.
     */
    alarm_after_ms: number;
    noise_threshold: number;
    noise_hold_ms: number;
    /**
     * A link only becomes verified once the platform also reported the remote DTLS certificate
     * fingerprint (`attempt_dtls_fingerprint`) and it matches the remote SDP. Defaults to `true`;
     * switch off only for a platform that cannot read WebRTC certificate stats.
     */
    require_dtls_fingerprint?: boolean;
    /**
     * The parent secret the platform persisted from the last [`LinkEvent::ParentPinned`] (baby:
     * the pinned parent; parent: the secret it drew, acknowledged or not). `None` before the first
     * pairing. Secret-class: never logged; `Link::new` takes the bytes and wipes its copy. JSON
     * form: 64 hex characters.
     */
    pinned_parent?: string;
    /**
     * The end of the first-pairing window in Unix seconds: the expiry of the invite. Parent:
     * the raw secret is sent only before it (give the parsed invite's expiry on the first run and
     * the last `window_until_s` of `ParentPinned` on later runs; `None` after the baby
     * acknowledged, so the parent only proves). Baby: `parent_pin` is accepted only before it;
     * with `None` a baby that was paired before accepts no first parent at all.
     */
    pin_window_until_s?: number;
    /**
     * Parent: this device's name, sent to the baby over the verified DataChannel only (at most 64
     * characters, control characters removed). Empty: no name. Never a fingerprint or an id.
     */
    device_name?: string;
    /**
     * The pairing's mailbox set: every `relay` and `lan` entry is one relay mailbox the core opens
     * at that address. Baby: the set is also sent to the parent after each verification (the
     * mailbox update); empty: none is sent.
     */
    mailboxes?: InviteMailbox[];
    /**
     * Development builds only: accept loopback `l:` entries in a received mailbox update, like
     * `parse_invite`'s flag of the same name.
     */
    allow_dev_loopback?: boolean;
    /**
     * "Only same Wi-Fi or my own server" (`alpha-connection-model` design section 8). With it on
     * the core drops every `relay` entry whose host is `project_host`: from `mailboxes` at start
     * and from every received mailbox update (an update left empty is ignored). It never opens or
     * retries such a mailbox, so it asks for no project STUN or TURN either, and the set the baby
     * sends to the parent leaves it out. Without a `project_host` it filters nothing.
     */
    only_local?: boolean;
    /**
     * The project server's host (`data/project-server.json`), lower-case DNS name, for
     * `only_local`. `Link::new` rejects anything else (`InvalidProjectHost`).
     */
    project_host?: string;
}

export interface LinkStatus {
    state: LinkState;
    /**
     * Escalation level 0-3 while `connecting` / `reconnecting`, otherwise 0.
     */
    level: number;
    /**
     * `failed_needs_user` key (see [`message_keys`]).
     */
    key: string | null;
    /**
     * Troubleshooter hint from level 2 (see [`message_keys`]).
     */
    cause: string | null;
    /**
     * Parent: sound and vibration of levels 2-3 are muted until this time.
     */
    acknowledged_until: number | null;
    /**
     * Monotonic time (same clock as the inputs) of the last verified contact with the peer, so a
     * UI can show "last contact N min ago". Set while `connecting` (after a first contact),
     * `reconnecting` and `peer_stopped`; `None` while `connected` (the contact is "now", and a
     * moving value would re-emit the status on every message) and before any contact.
     */
    last_alive_ms: number | null;
}

export interface SessionConfig {
    role: Role;
    room_id: string;
    /**
     * QR pairing PSK (base64url). `None` disables `pair_proof` (dev only).
     */
    psk_b64url: string | null;
    heartbeat_interval_ms: number;
    /**
     * The session ends (parent: with `LinkLost`) when the verified peer sent no DataChannel
     * message for this long.
     */
    heartbeat_stale_ms: number;
    pairing_timeout_ms: number;
    noise_threshold: number;
    noise_hold_ms: number;
}

export type Action = { type: "send_signaling"; text: string } | { type: "create_offer" } | { type: "set_remote_description"; sdp_type: string; sdp: string } | { type: "create_answer" } | { type: "add_ice_candidate"; candidate: IceCandidate } | { type: "send_data"; text: string } | { type: "set_mic_enabled"; enabled: boolean } | { type: "emit"; event: SessionEvent } | { type: "close_transport" };

export type Dnd = "off" | "alarms_only" | "priority" | "total" | "unknown";

export type Input = { type: "signaling_open" } | { type: "signaling_message"; text: string } | { type: "signaling_closed" } | { type: "local_description"; sdp_type: string; sdp: string } | { type: "local_ice_candidate"; candidate: IceCandidate } | { type: "data_channel_open" } | { type: "data_channel_message"; text: string } | { type: "data_channel_closed" } | { type: "peer_connection_state"; state: string } | { type: "audio_level"; rms: number } | { type: "set_listen"; enabled: boolean } | { type: "request_probe" } | { type: "tick" } | { type: "close" };

export type LinkAction = { type: "open_mailbox"; id: MailboxId; kind: MailboxKind; url: string } | { type: "mailbox_send"; id: MailboxId; frame: string } | { type: "close_mailbox"; id: MailboxId } | { type: "create_attempt"; attempt: number; offerer: boolean; ice_servers?: IceServer[] } | { type: "close_attempt"; attempt: number } | { type: "create_offer"; attempt: number } | { type: "set_remote_description"; attempt: number; sdp_type: string; sdp: string } | { type: "create_answer"; attempt: number } | { type: "add_ice_candidate"; attempt: number; candidate: IceCandidate } | { type: "send_data"; attempt: number; text: string } | { type: "set_mic_enabled"; enabled: boolean } | { type: "apply_quiet" } | { type: "send_probe_tone"; duration_ms: number } | { type: "set_talk_mic"; attempt: number; enabled: boolean } | { type: "set_playback_ducked"; attempt: number; ducked: boolean } | { type: "play_talk_chime"; attempt: number; level: TalkLevel } | { type: "set_talk_playback"; attempt: number; enabled: boolean; level: TalkLevel; fade_in_ms: number; until_ms: number } | { type: "emit"; event: LinkEvent };

export type LinkEvent = { type: "status"; status: LinkStatus } | { type: "link_lost"; down_ms: number } | { type: "link_restored"; down_ms: number } | { type: "security_warning"; key: string } | { type: "peer_stopped" } | { type: "alarm_acknowledged"; until_ms: number } | { type: "noise_alert"; rms: number; probe: boolean; snoozed: boolean } | { type: "noise_escalated"; after_ms: number } | { type: "peer_heartbeat"; seq: number; rms: number | null } | { type: "peer_status"; status: DeviceStatus } | { type: "low_battery"; pct: number; critical: boolean } | { type: "charger_unplugged" } | { type: "missed_events"; count: number; max_rms: number; complete: boolean; down_ms: number } | { type: "noise_gate_changed"; threshold: number; hold_ms: number } | { type: "listen_changed"; enabled: boolean } | { type: "failed"; reason: string } | { type: "parent_pinned"; secret: string; window_until_s?: number } | { type: "parent_connected"; name: string } | { type: "mailboxes_updated"; mailboxes: InviteMailbox[] } | { type: "hint"; key: string; active: boolean } | { type: "talk_changed"; state: TalkState } | { type: "talk_active"; active: boolean; duration_ms: number } | { type: "noise_paused"; paused: boolean } | { type: "talk_playback"; result: PlaybackResult };

export type LinkInput = { type: "start"; entropy: number } | { type: "wall_clock"; unix_s: number } | { type: "stop" } | { type: "acknowledge_alarm" } | { type: "network_available" } | { type: "mailbox_opened"; id: MailboxId } | { type: "mailbox_closed"; id: MailboxId; reason: string } | { type: "mailbox_frame"; id: MailboxId; bytes: string } | { type: "attempt_local_description"; attempt: number; sdp_type: string; sdp: string } | { type: "attempt_local_ice"; attempt: number; candidate: IceCandidate } | { type: "attempt_data_open"; attempt: number } | { type: "attempt_data_message"; attempt: number; text: string } | { type: "attempt_data_closed"; attempt: number } | { type: "attempt_dtls_fingerprint"; attempt: number; fingerprint: string } | { type: "attempt_pc_state"; attempt: number; state: string } | { type: "audio_level"; rms: number } | { type: "device_status"; status: DeviceStatus } | { type: "set_noise_gate"; threshold: number; hold_ms: number } | { type: "set_listen"; enabled: boolean } | { type: "request_probe" } | { type: "set_snooze"; until_ms: number | null } | { type: "set_noise_escalation"; after_ms: number | null } | { type: "acknowledge_noise" } | { type: "request_quiet" } | { type: "quiet_applied"; state: DeviceStatus } | { type: "set_talk"; pressed: boolean } | { type: "talk_mic_attached" } | { type: "talk_mic_failed" } | { type: "alert_preempts_talk" } | { type: "talk_chime_done"; attempt: number } | { type: "set_talk_back"; enabled: boolean; level: TalkLevel; chime: boolean } | { type: "talk_settings_opened" } | { type: "set_talk_muted"; muted: boolean } | { type: "set_raw_capture"; raw: boolean } | { type: "talk_rtp_outside_grant"; attempt: number } | { type: "talk_level"; rms: number } | { type: "tick" };

export type LinkState = "connecting" | "connected" | "reconnecting" | "peer_stopped" | "failed_needs_user" | "stopped";

export type MailboxKind = "relay" | "lan";

export type Ringer = "normal" | "vibrate" | "silent" | "unknown";

export type Role = "baby" | "parent";

export type SessionEvent = { type: "state_changed"; state: SessionState } | { type: "pairing_verified" } | { type: "heartbeat"; seq: number; rms: number | null; battery: number | null } | { type: "noise_alert"; rms: number; probe: boolean } | { type: "link_lost"; reason: string } | { type: "failed"; reason: string };

export type SessionState = "connecting" | "joined" | "connected" | "verified" | "failed" | "closed";


/**
 * ADR-017 link. `now_ms` must be monotonic (clamp `Date.now()` so it never goes backwards).
 */
export class WasmLink {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Returns a JSON array of `LinkAction`s.
     */
    handle(now_ms: number, input_json: string): string;
    constructor(config_json: string);
    roomId(): string;
}

export class WasmSession {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Returns a JSON array of actions.
     */
    handle(now_ms: number, input_json: string): string;
    constructor(config_json: string);
}

/**
 * Builds the v4 invite text from a JSON array of `{"kind","url"}` mailboxes (the
 * `packages/core-wasm` wrapper passes typed values). Throws the snake_case error key if the inputs
 * are not a valid invite or the protected entries alone exceed the size budget.
 */
export function buildInvite(psk: Uint8Array, mailboxes_json: string, expires_at_epoch_s: number, allow_dev_loopback: boolean): string;

export function conformanceEval(case_json: string): string;

export function deriveRoomId(psk: Uint8Array): string;

/**
 * "Only same Wi-Fi or my own server" for a new invite: a JSON array of `{"kind","url"}` mailboxes
 * without the relay entries whose host is `project_host`. Throws `not_our_code` for bad JSON.
 */
export function filterOnlyLocal(mailboxes_json: string, project_host: string): string;

/**
 * JSON array of every UI key the core can emit.
 */
export function messageKeys(): string;

/**
 * Parses a scanned or pasted v4 invite. Returns JSON, `{"type":"ok","psk_b64url":..,"mailboxes":
 * [{"kind":"relay"|"lan","url":..}],"expires_at_epoch_s":..}` or `{"type":"error","error":
 * "not_our_code"|"incomplete_paste"|"expired"}`; the `packages/core-wasm` wrapper turns it into a
 * typed result. `now_epoch_s` is wall-clock seconds; `allow_dev_loopback` is for dev builds only.
 */
export function parseInvite(text: string, now_epoch_s: number, allow_dev_loopback: boolean): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_wasmlink_free: (a: number, b: number) => void;
    readonly __wbg_wasmsession_free: (a: number, b: number) => void;
    readonly buildInvite: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly conformanceEval: (a: number, b: number, c: number) => void;
    readonly deriveRoomId: (a: number, b: number, c: number) => void;
    readonly filterOnlyLocal: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly messageKeys: (a: number) => void;
    readonly parseInvite: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmlink_handle: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmlink_new: (a: number, b: number, c: number) => void;
    readonly wasmlink_roomId: (a: number, b: number) => void;
    readonly wasmsession_handle: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmsession_new: (a: number, b: number, c: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
    readonly __wbindgen_export: (a: number, b: number) => number;
    readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export3: (a: number, b: number, c: number) => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
