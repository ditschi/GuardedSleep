import { loadCoreNode } from "@guardedsleep/core-wasm/node";
import { describe, expect, it } from "vitest";
import { ALARM_TONE_STORAGE_KEY, readAlarmToneSetting, storeAlarmToneSetting } from "./alarm-tone";
import { ALARM_TONE_IDS, DEFAULT_ALARM_TONE } from "./contract";

describe("connection alarm tone setting", () => {
  it("lists the tones of the core in the same order", () => {
    const core = loadCoreNode();
    expect(core.alarmTones().map((t) => t.id)).toEqual([...ALARM_TONE_IDS]);
    expect(core.resolveAlarmTone("").id).toBe(DEFAULT_ALARM_TONE);
  });

  it("remembers the id and reads it back unchecked (the core resolves it)", () => {
    const store = new Map<string, string>();
    const storage = {
      setItem: (k: string, v: string) => void store.set(k, v),
      getItem: (k: string) => store.get(k) ?? null,
    };
    expect(readAlarmToneSetting(storage)).toBeNull();
    storeAlarmToneSetting(storage, "chime");
    expect(store.get(ALARM_TONE_STORAGE_KEY)).toBe("chime");
    expect(readAlarmToneSetting(storage)).toBe("chime");
    storeAlarmToneSetting(storage, "garbage");
    expect(readAlarmToneSetting(storage)).toBe("garbage");
  });

  it("survives storage that throws", () => {
    const broken = {
      setItem: () => {
        throw new Error("blocked");
      },
      getItem: () => {
        throw new Error("blocked");
      },
    };
    expect(() => storeAlarmToneSetting(broken, "low")).not.toThrow();
    expect(readAlarmToneSetting(broken)).toBeNull();
  });
});
