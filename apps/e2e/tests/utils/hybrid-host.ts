import { execSync } from "node:child_process";
import { detectLanIpv4OrThrow, resolveLanIpv4 } from "./lan-ip.js";

/** Android emulator alias for the host machine (maps to host 127.0.0.1). */
export const EMULATOR_HOST_ALIAS = "10.0.2.2";

function adbCmd(): string {
  return process.env.GUARDEDSLEEP_ADB?.trim() || "adb";
}

export function adbDeviceId(): string | null {
  try {
    const out = execSync(`${adbCmd()} devices`, {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    const line = out
      .split("\n")
      .map((l) => l.trim())
      .find((l) => /^(emulator-\d+|[0-9a-f]{8,})\tdevice$/i.test(l));
    return line?.split("\t")[0] ?? null;
  } catch {
    return null;
  }
}

export function isAdbEmulator(deviceId = adbDeviceId()): boolean {
  return deviceId?.startsWith("emulator-") ?? false;
}

/**
 * Host IP as seen from the adb device: emulator → 10.0.2.2; physical phone → LAN IPv4.
 * Override with GUARDEDSLEEP_HYBRID_HOST.
 */
export function resolveHybridHostIp(): string {
  const override = process.env.GUARDEDSLEEP_HYBRID_HOST?.trim();
  if (override) return override;
  if (isAdbEmulator()) return EMULATOR_HOST_ALIAS;
  return resolveLanIpv4() ?? detectLanIpv4OrThrow();
}
