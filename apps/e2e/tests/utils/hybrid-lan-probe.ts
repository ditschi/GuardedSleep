import { execSync } from "node:child_process";
import WebSocket from "ws";

export async function probeHttpReachable(url: string, timeoutMs = 4000): Promise<boolean> {
  try {
    execSync(`curl -fsS --max-time ${Math.ceil(timeoutMs / 1000)} "${url}"`, {
      stdio: ["ignore", "ignore", "ignore"],
    });
    return true;
  } catch {
    return false;
  }
}

export function probeWsReachable(url: string, timeoutMs = 4000): Promise<boolean> {
  return new Promise((resolve) => {
    const ws = new WebSocket(url, { rejectUnauthorized: false });
    const timer = setTimeout(() => {
      ws.terminate();
      resolve(false);
    }, timeoutMs);
    ws.once("open", () => {
      clearTimeout(timer);
      ws.close();
      resolve(true);
    });
    ws.once("error", () => {
      clearTimeout(timer);
      resolve(false);
    });
  });
}
