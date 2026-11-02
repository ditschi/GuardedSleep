import { describe, expect, it } from "vitest";
import { browserDeviceName } from "./device-name";

const CHROME_ANDROID =
  "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Mobile Safari/537.36";
const SAFARI_IOS =
  "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";
const FIREFOX_LINUX = "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0";
const EDGE_WINDOWS =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36 Edg/152.0.0.0";

describe("browserDeviceName", () => {
  it("names the browser family and the system, nothing finer", () => {
    expect(browserDeviceName({ userAgent: CHROME_ANDROID })).toBe("Chrome on Android");
    expect(browserDeviceName({ userAgent: SAFARI_IOS })).toBe("Safari on iOS");
    expect(browserDeviceName({ userAgent: FIREFOX_LINUX })).toBe("Firefox on Linux");
    expect(browserDeviceName({ userAgent: EDGE_WINDOWS })).toBe("Edge on Windows");
  });

  it("leaves out model names and versions", () => {
    const name = browserDeviceName({ userAgent: CHROME_ANDROID });
    expect(name).not.toMatch(/Pixel|152|14/);
  });

  it("prefers the client hints platform and falls back gracefully", () => {
    expect(
      browserDeviceName({ userAgent: "Chrome/152", userAgentData: { platform: "Windows" } }),
    ).toBe("Chrome on Windows");
    expect(browserDeviceName({ userAgent: "Chrome/152" })).toBe("Chrome");
    expect(browserDeviceName({ userAgent: "" })).toBe("Browser");
  });
});
