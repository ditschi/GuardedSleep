import { spawnSync } from "node:child_process";
import path from "node:path";
import { describe, expect, it } from "vitest";

const root = path.resolve(__dirname, "../../../../..");

describe("localization catalogue", () => {
  it("passes i18n-gen validation (key, argument and plural parity)", () => {
    const r = spawnSync(
      "bash",
      ["scripts/cargo.sh", "run", "-q", "-p", "i18n-gen", "--", "check"],
      {
        cwd: root,
        encoding: "utf8",
      },
    );
    expect(r.stderr + r.stdout, r.stderr).toContain("OK");
    expect(r.status).toBe(0);
  });
});
