import { get } from "svelte/store";
import { describe, expect, it } from "vitest";
// The Kotlin contract is the other half of the mirror; read as text so the two cannot drift apart.
import kotlin from "../../../../android/ui-contract/src/main/kotlin/org/guardedsleep/app/ui/model/UiContract.kt?raw";
import { createFakeController } from "./fake-controller";
import {
  defaultAppState,
  defaultBabyMonitor,
  defaultLinkUi,
  defaultParentMonitor,
  defaultPeerStatus,
  defaultSettings,
  makePairingState,
  type AlertOutput,
  type CauseHint,
  type DeviceCheck,
  type LinkHint,
  type LinkPhase,
  type LastTalkUi,
  type LinkQuality,
  type ListenMode,
  type MissedEventsUi,
  type PairingError,
  type PairingPhase,
  type PairingState,
  type ReadinessIssue,
  type Role,
  type SilenceRequest,
  type SilenceStatus,
  type SoundEventUi,
  type NoisePause,
  type TalkDeny,
  type TalkEnd,
  type TalkPermission,
  type TalkUi,
  type TalkUnavailable,
  type TalkVolume,
  type Theme,
} from "./models";

/** The text between the braces/parentheses that open at `start` (balanced). */
function block(text: string, start: number, open: string, close: string): string {
  let depth = 0;
  for (let i = start; i < text.length; i++) {
    const c = text.charAt(i);
    if (c === open) depth++;
    else if (c === close && --depth === 0) return text.slice(start + 1, i);
  }
  throw new Error("unbalanced block");
}

function kotlinEnum(name: string): string[] {
  const at = kotlin.indexOf(`enum class ${name} {`);
  if (at < 0) throw new Error(`enum ${name} not found`);
  const body = block(kotlin, kotlin.indexOf("{", at), "{", "}");
  return [...body.matchAll(/^\s*([A-Z][A-Z0-9_]*),?\s*$/gm)].map((m) => (m[1] ?? "").toLowerCase());
}

function kotlinFields(name: string): string[] {
  const at = kotlin.indexOf(`data class ${name}(`);
  if (at < 0) throw new Error(`data class ${name} not found`);
  const body = block(kotlin, kotlin.indexOf("(", at), "(", ")");
  // Top-level `val`s only: default values like `LinkUi()` contain no `val`.
  return [...body.matchAll(/\bval (\w+)\s*:/g)].map((m) => m[1] ?? "");
}

function kotlinFunctions(iface: string): string[] {
  const at = kotlin.indexOf(`interface ${iface} {`);
  const body = block(kotlin, kotlin.indexOf("{", at), "{", "}");
  return [...body.matchAll(/^\s{4}fun (\w+)\(/gm)].map((m) => m[1] ?? "");
}

function kotlinStateFlows(iface: string): string[] {
  const at = kotlin.indexOf(`interface ${iface} {`);
  const body = block(kotlin, kotlin.indexOf("{", at), "{", "}");
  return [...body.matchAll(/^\s{4}val (\w+): StateFlow/gm)].map((m) => m[1] ?? "");
}

const sorted = (xs: readonly string[]): string[] => [...xs].sort();

describe("enums mirror UiContract.kt", () => {
  // `satisfies` makes a missing or extra TypeScript member a compile error as well.
  const enums = {
    Role: ["baby", "parent"] satisfies Role[],
    LinkPhase: [
      "idle",
      "connecting",
      "connected",
      "reconnecting",
      "peer_stopped",
      "failed_needs_user",
      "stopped",
    ] satisfies LinkPhase[],
    CauseHint: [
      "server_unreachable",
      "peer_absent",
      "media_blocked",
      "pair_again",
      "baby_unrecognised",
    ] satisfies CauseHint[],
    LinkHint: ["clock_skew", "recovery_path_unavailable"] satisfies LinkHint[],
    LinkQuality: ["good", "medium", "poor", "unknown"] satisfies LinkQuality[],
    SilenceStatus: [
      "silent",
      "screen_may_light",
      "may_buzz",
      "may_ring",
      "unknown",
      "no_access",
    ] satisfies SilenceStatus[],
    ReadinessIssue: [
      "notifications_blocked",
      "alarm_volume_zero",
      "full_screen_blocked",
      "dnd_bypass_missing",
      "battery_optimized",
      "parent_battery_low",
      "headphones_connected",
      "never_tested",
    ] satisfies ReadinessIssue[],
    ListenMode: ["off", "hold", "locked"] satisfies ListenMode[],
    SilenceRequest: ["idle", "pending", "applied", "failed"] satisfies SilenceRequest[],
    PairingPhase: [
      "showing_code",
      "expired",
      "scanning",
      "connecting",
      "paired",
      "failed",
    ] satisfies PairingPhase[],
    PairingError: [
      "camera_denied",
      "not_our_code",
      "expired",
      "key_mismatch",
      "already_paired",
      "unreachable",
      "incomplete_paste",
    ] satisfies PairingError[],
    Theme: ["dark", "light", "system"] satisfies Theme[],
    AlertOutput: ["default", "speaker_only"] satisfies AlertOutput[],
    DeviceCheck: ["unknown", "ok", "limited", "not_recommended"] satisfies DeviceCheck[],
    TalkVolume: ["low", "medium", "high"] satisfies TalkVolume[],
    TalkEnd: [
      "released",
      "limit",
      "no_answer",
      "link_lost",
      "mic_failed",
      "alert",
      "switched_off",
      "stopped",
    ] satisfies TalkEnd[],
    TalkDeny: [
      "off",
      "unsupported",
      "muted",
      "busy",
      "cooldown",
      "budget",
      "rate",
      "unknown",
    ] satisfies TalkDeny[],
    TalkUnavailable: [
      "off",
      "other_talking",
      "unsupported",
      "muted",
      "not_connected",
      "cooldown",
    ] satisfies TalkUnavailable[],
    TalkPermission: ["none", "needed", "just_granted", "refused"] satisfies TalkPermission[],
    NoisePause: ["you", "other"] satisfies NoisePause[],
  };

  for (const [name, members] of Object.entries(enums)) {
    it(name, () => {
      expect(sorted(members)).toEqual(sorted(kotlinEnum(name)));
    });
  }
});

describe("TalkUi mirrors the sealed interface of UiContract.kt", () => {
  it("has the same variants with the same fields", () => {
    const at = kotlin.indexOf("sealed interface TalkUi {");
    const body = block(kotlin, kotlin.indexOf("{", at), "{", "}");
    const variants = [...body.matchAll(/data (?:object (\w+)|class (\w+)\(([^)]*)\))/g)].map(
      (m) => ({
        name: (m[1] ?? m[2] ?? "").replace(/^./, (c) => c.toLowerCase()),
        fields: [...(m[3] ?? "").matchAll(/\bval (\w+)\s*:/g)].map((f) => f[1] ?? ""),
      }),
    );
    const web = {
      idle: [],
      requesting: [],
      talking: ["untilElapsedMs"],
      denied: ["reason", "retryAtElapsedMs"],
      unavailable: ["reason", "retryAtElapsedMs"],
    } satisfies Record<TalkUi["kind"], string[]>;
    expect(sorted(variants.map((v) => v.name))).toEqual(sorted(Object.keys(web)));
    for (const v of variants) {
      expect(v.fields, v.name).toEqual(web[v.name as TalkUi["kind"]]);
    }
  });
});

describe("data classes mirror UiContract.kt", () => {
  const missed: MissedEventsUi = {
    downMs: 0,
    count: null,
    complete: true,
    endWallMs: null,
    loudest: null,
  };
  const sound: SoundEventUi = { atWallMs: 0, durationMs: 0 };
  const lastTalk: LastTalkUi = { name: "", atWallMs: 0, durationMs: 0 };
  const pairing: PairingState = makePairingState("paired");

  const classes: Record<string, object> = {
    LinkUi: defaultLinkUi(),
    PeerStatusUi: defaultPeerStatus(),
    MissedEventsUi: missed,
    SoundEventUi: sound,
    LastTalkUi: lastTalk,
    ParentMonitorState: defaultParentMonitor(),
    BabyMonitorState: defaultBabyMonitor(),
    PairingState: pairing,
    SettingsState: defaultSettings(),
    AppState: defaultAppState(),
  };

  for (const [name, sample] of Object.entries(classes)) {
    it(name, () => {
      expect(sorted(Object.keys(sample))).toEqual(sorted(kotlinFields(name)));
    });
  }
});

describe("defaults equal the Kotlin constructor defaults", () => {
  it("monitoring thresholds", () => {
    expect(defaultBabyMonitor().threshold).toBe(0.12);
    expect(defaultBabyMonitor().holdSeconds).toBe(5);
    expect(defaultSettings().threshold).toBe(0.12);
    expect(defaultSettings().holdSeconds).toBe(5);
    expect(defaultSettings().heartbeatSeconds).toBe(10);
    expect(defaultSettings().alertGainPct).toBe(100);
  });

  it("nothing is monitoring, paired or alerting at start", () => {
    expect(defaultAppState()).toMatchObject({ role: null, monitoring: false, paired: false });
    expect(defaultAppState().backgroundProtection).toBe(true);
    expect(defaultLinkUi()).toMatchObject({ phase: "idle", level: 0, quality: "unknown" });
    expect(defaultParentMonitor()).toMatchObject({ listen: "off", noiseAlertActive: false });
    expect(defaultPeerStatus().silence).toBe("unknown");
  });
});

describe("AppController mirrors UiContract.kt", () => {
  it("has the same state streams and actions", () => {
    const fake = createFakeController();
    const stores = kotlinStateFlows("AppController");
    const actions = kotlinFunctions("AppController");
    for (const name of stores) {
      expect(
        typeof (fake as unknown as Record<string, { subscribe?: unknown }>)[name]?.subscribe,
        name,
      ).toBe("function");
    }
    for (const name of actions) {
      expect(typeof (fake as unknown as Record<string, unknown>)[name], name).toBe("function");
    }
    // Nothing extra on the web side besides the stores, the recorded calls and the web-only
    // sound gate (browsers lock audio until a user gesture; Android has no counterpart) and the
    // Battery API reading of the device.
    const webOnly = [
      "soundUnlocked",
      "unlockSound",
      "deviceBattery",
      "pageGuard",
      "retryKeepAwake",
      "dismissAwayReport",
      // Talk-back: the browser's microphone support and suspended audio, and the permission
      // prompt that the Android screen hosts itself (`talkPermissionResult` is the shared answer).
      "webTalk",
      "requestTalkPermission",
    ];
    const own = Object.keys(fake).filter(
      (k) => !stores.includes(k) && k !== "calls" && !webOnly.includes(k),
    );
    expect(sorted(own)).toEqual(sorted(actions));
    expect(typeof fake.unlockSound).toBe("function");
    expect(typeof fake.soundUnlocked.subscribe).toBe("function");
  });
});

describe("makePairingState", () => {
  const payload = '{"v":3,"k":"AAAA-secret-psk","s":["wss://x"],"x":1}';

  it("keeps the payload out of strings and JSON", () => {
    const state = makePairingState("showing_code", { codePayload: payload, expiresAtElapsedMs: 5 });
    expect(state.codePayload).toBe(payload);
    for (const text of [String(state), `${state}`, JSON.stringify(state)]) {
      expect(text).not.toContain("secret-psk");
      expect(text).toContain("redacted");
    }
  });

  it("shows null for an absent payload", () => {
    expect(JSON.stringify(makePairingState("scanning"))).toContain('"codePayload":null');
  });

  it("keeps the redaction hooks off the enumerable keys", () => {
    expect(Object.keys(makePairingState("paired"))).toEqual([
      "phase",
      "codePayload",
      "expiresAtElapsedMs",
      "error",
    ]);
  });
});

describe("fake controller", () => {
  it("applies role, pairing, monitoring and settings transitions", () => {
    const c = createFakeController();
    c.chooseRole("parent");
    expect(get(c.app).role).toBe("parent");
    c.startPairing();
    expect(get(c.pairing)?.phase).toBe("showing_code");
    c.cancelPairing();
    expect(get(c.pairing)).toBeNull();
    c.startMonitoring();
    expect(get(c.app).monitoring).toBe(true);
    c.stopMonitoring();
    expect(get(c.app).monitoring).toBe(false);
    c.updateSettings((s) => ({ ...s, theme: "light", signalingUrls: ["wss://example.test"] }));
    expect(get(c.settings)).toMatchObject({
      theme: "light",
      signalingUrls: ["wss://example.test"],
    });
    c.setListen("locked");
    expect(get(c.parent).listen).toBe("locked");
    c.snooze(10);
    expect(get(c.parent).snoozedUntilWallMs).not.toBeNull();
    c.cancelSnooze();
    expect(get(c.parent).snoozedUntilWallMs).toBeNull();
  });

  it("records calls in order, including the ones without a state change", () => {
    const c = createFakeController();
    c.submitPairingCode("pasted");
    c.requestTestAlert();
    c.reportTestResult(true);
    expect(c.calls).toEqual([
      ["submitPairingCode", "pasted"],
      ["requestTestAlert"],
      ["reportTestResult", true],
    ]);
  });
});
