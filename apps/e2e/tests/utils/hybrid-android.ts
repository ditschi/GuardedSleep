import { execSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { resolveHybridHostIp } from "./hybrid-host.js";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "../../../..");

export type HybridLanScheme = "ws" | "wss";

export const hybridSignalPort = process.env.GUARDEDSLEEP_E2E_SIGNAL_PORT ?? "18787";
export const hybridHttpsPort = process.env.GUARDEDSLEEP_HYBRID_HTTPS_PORT ?? "8443";
export const hybridUseLan = process.env.GUARDEDSLEEP_HYBRID_LAN === "1";
export const hybridDevServer = process.env.CAPACITOR_DEV_SERVER ?? "http://localhost:5173";
export const hybridSignalWs = process.env.E2E_SIGNALING_URL ?? `ws://127.0.0.1:${hybridSignalPort}`;

export function hybridLanHttpBase(): string {
  const host = resolveHybridHostIp();
  const port = process.env.GUARDEDSLEEP_HYBRID_HTTP_PORT ?? "5173";
  return `http://${host}:${port}`;
}

export function hybridLanSignalWs(): string {
  const host = resolveHybridHostIp();
  return `ws://${host}:${hybridSignalPort}`;
}

export function hybridLanHttpsBase(): string {
  const host = resolveHybridHostIp();
  return `https://${host}:${hybridHttpsPort}`;
}

export function hybridLanSignalWss(): string {
  const host = resolveHybridHostIp();
  return `wss://${host}:${hybridHttpsPort}/signaling`;
}

export function hybridLanUrls(scheme: HybridLanScheme): { devServer: string; signalUrl: string } {
  if (scheme === "wss") {
    return { devServer: hybridLanHttpsBase(), signalUrl: hybridLanSignalWss() };
  }
  return { devServer: hybridLanHttpBase(), signalUrl: hybridLanSignalWs() };
}

export function hybridRuntimeUrls(scheme: HybridLanScheme = "ws"): {
  devServer: string;
  signalWs: string;
  signalUrl: string;
} {
  if (hybridUseLan) {
    const urls = hybridLanUrls(scheme);
    return { devServer: urls.devServer, signalWs: urls.signalUrl, signalUrl: urls.signalUrl };
  }
  return { devServer: hybridDevServer, signalWs: hybridSignalWs, signalUrl: hybridSignalWs };
}

function adbCmd(): string {
  return process.env.GUARDEDSLEEP_ADB?.trim() || "adb";
}

function run(cmd: string, cwd = repoRoot): void {
  execSync(cmd, { stdio: "inherit", cwd, env: process.env });
}

function runCapture(cmd: string, cwd = repoRoot): string {
  return execSync(cmd, { encoding: "utf8", cwd, env: process.env }).trim();
}

function runAdb(args: string): void {
  run(`${adbCmd()} ${args}`);
}

function runGradleAssembleDebug(): void {
  if (process.env.GUARDEDSLEEP_DOCKER_GRADLE === "1") {
    run(
      "docker compose -f docker/android-build/compose.yaml run --rm apk ./gradlew assembleDebug --no-daemon",
    );
    return;
  }
  run("cd apps/mobile/android && ./gradlew assembleDebug", repoRoot);
}

export function requireAdbDevice(): string {
  const out = runCapture(`${adbCmd()} devices`);
  const line = out
    .split("\n")
    .map((l) => l.trim())
    .find((l) => /^(emulator-\d+|[0-9a-f]{8,})\tdevice$/i.test(l));
  if (!line) {
    throw new Error(
      "No adb device in 'device' state. Connect a phone (USB debugging) or start an emulator, then re-run.",
    );
  }
  return line.split("\t")[0]!;
}

/** @deprecated Use requireAdbDevice — kept for existing hybrid specs. */
export function requireEmulatorDevice(): string {
  return requireAdbDevice();
}

export function hybridApkPath(): string {
  const dir = join(repoRoot, "apps/mobile/android/app/build/outputs/apk/debug");
  if (!existsSync(dir)) {
    throw new Error(`Debug APK directory missing: ${dir}`);
  }
  const apk = readdirSync(dir).find(
    (f) => f.startsWith("guardedsleep-debug") && f.endsWith(".apk"),
  );
  if (!apk) {
    throw new Error(`No guardedsleep-debug-v*.apk in ${dir} — run assembleDebug first`);
  }
  return join(dir, apk);
}

export function setupHybridAndroidApk(opts?: { scheme?: HybridLanScheme }): void {
  const scheme = opts?.scheme ?? "ws";
  requireAdbDevice();
  if (!hybridUseLan) {
    runAdb("reverse tcp:5173 tcp:5173");
    runAdb(`reverse tcp:${hybridSignalPort} tcp:${hybridSignalPort}`);
  }
  const { devServer } = hybridRuntimeUrls(scheme);
  run("pnpm --filter @guardedsleep/protocol build");
  if (scheme === "wss") {
    run(
      `VITE_DEV_CONNECT_HINTS=true VITE_SIGNALING_URL=${hybridLanSignalWss()} pnpm --filter @guardedsleep/mobile build`,
    );
  }
  run(`CAPACITOR_DEV_SERVER=${devServer} pnpm --filter @guardedsleep/mobile exec cap sync android`);
  runGradleAssembleDebug();
  const apk = hybridApkPath();
  if (!existsSync(apk)) {
    throw new Error(`Debug APK missing after assembleDebug: ${apk}`);
  }
  runAdb(`install -r "${apk}"`);
  runAdb("shell pm grant org.guardedsleep.app android.permission.RECORD_AUDIO");
  runAdb("shell pm grant org.guardedsleep.app android.permission.POST_NOTIFICATIONS");
}

export function launchHybridBaby(
  room: string,
  overrides?: { devServer?: string; signalUrl?: string; scheme?: HybridLanScheme },
): void {
  requireAdbDevice();
  const scheme = overrides?.scheme ?? "ws";
  const urls = hybridRuntimeUrls(scheme);
  const devServer = overrides?.devServer ?? urls.devServer;
  const signalUrl = overrides?.signalUrl ?? urls.signalUrl;
  const q = new URLSearchParams({
    room,
    signal: signalUrl,
    e2e_force_noise_alert: "1",
    e2e_synthetic_audio: "1",
  });
  const deepLink = `${devServer}/baby?${q.toString()}`;
  // `adb shell` re-joins args for the device shell, so the URL needs its own quotes or `&` splits it.
  runAdb(
    `shell am start -a android.intent.action.VIEW -d "'${deepLink}'" org.guardedsleep.app/.MainActivity`,
  );
}

/** @deprecated Use hybridLanHttpBase */
export function hybridLanBaseUrl(): string {
  return hybridLanHttpBase();
}
