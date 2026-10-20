import { execSync } from "node:child_process";

function isIpv4(ip: string): boolean {
  if (!/^(\d{1,3}\.){3}\d{1,3}$/.test(ip)) return false;
  return ip.split(".").every((o) => {
    const n = Number(o);
    return Number.isInteger(n) && n >= 0 && n <= 255;
  });
}

function isUsableLanIpv4(ip: string): boolean {
  return isIpv4(ip) && !ip.startsWith("127.");
}

function runCapture(cmd: string): string {
  return execSync(cmd, { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
}

/** Detect the host LAN IPv4 (same strategy as scripts/stack-lan-env.sh). */
export function detectLanIpv4(): string | null {
  let ip = "";
  try {
    ip = runCapture(
      `ip -4 route get 1.1.1.1 2>/dev/null | awk '{for (i = 1; i <= NF; i++) if ($i == "src") { print $(i + 1); exit }}'`,
    );
    if (isUsableLanIpv4(ip)) return ip;
  } catch {
    // try fallbacks
  }

  try {
    ip = runCapture(`hostname -I 2>/dev/null | awk '{ print $1 }'`);
    if (isUsableLanIpv4(ip)) return ip;
  } catch {
    // no LAN IP
  }

  return null;
}

export function detectLanIpv4OrThrow(): string {
  const ip = detectLanIpv4();
  if (!ip) {
    throw new Error(
      "Could not detect a LAN IPv4 on this host. Connect Wi-Fi/Ethernet and retry, or set GUARDEDSLEEP_LAN_IP.",
    );
  }
  return ip;
}

/** Prefer explicit override (hybrid / CI), else auto-detect. */
export function resolveLanIpv4(): string | null {
  const override = process.env.GUARDEDSLEEP_LAN_IP?.trim();
  if (override && isUsableLanIpv4(override)) return override;
  return detectLanIpv4();
}
