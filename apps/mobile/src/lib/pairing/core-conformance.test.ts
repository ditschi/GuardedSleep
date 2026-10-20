import { describe, expect, it } from "vitest";
import rawVectors from "../../../../../crates/guardedsleep-core/vectors/conformance.json";
import { parseHeartbeat, parseNoiseAlert, parsePairProof } from "@guardedsleep/protocol";

/**
 * Cross-implementation check (ADR-015 AC-7): the Rust core's golden vectors must agree with this
 * web client, otherwise a native baby unit and a browser parent cannot pair.
 */
type Vector = { name: string; kind: string; expect: unknown } & Record<string, unknown>;

const vectors = rawVectors as Vector[];

describe("Rust core conformance vectors vs. TypeScript implementation", () => {
  it.each(vectors.filter((v) => v.kind === "parse_data"))("DataChannel parse: $name", (v) => {
    const raw = v.raw as string;
    const expected = v.expect as Record<string, unknown> | null;
    const type = expected?.type;
    if (type === "heartbeat") {
      expect(parseHeartbeat(raw)).toMatchObject({ seq: expected!.seq, rms: expected!.rms });
    } else if (type === "noise_alert") {
      expect(parseNoiseAlert(raw)).toMatchObject({ rms: expected!.rms, probe: expected!.probe });
    } else if (type === "pair_proof") {
      expect(parsePairProof(raw)).toBe(expected!.mac);
    } else {
      expect(expected).toBeNull();
      expect(parseHeartbeat(raw) ?? parseNoiseAlert(raw) ?? parsePairProof(raw)).toBeUndefined();
    }
  });
});
