import { describe, expect, it } from "vitest";
import { loadCoreNode } from "@guardedsleep/core-wasm/node";
import rawVectors from "../../../../../crates/guardedsleep-core/vectors/conformance.json";

/**
 * The generated WASM core that the web client ships must agree with the Rust core's golden vectors
 * (ADR-015 AC-7), otherwise a native baby unit and a browser parent cannot pair.
 */
type Vector = { name: string; kind: string; expect: unknown } & Record<string, unknown>;

const vectors = rawVectors as Vector[];
const core = loadCoreNode();

describe("Rust core conformance vectors vs. the generated WASM core", () => {
  it("has vectors", () => {
    expect(vectors.length).toBeGreaterThan(0);
  });

  it.each(vectors)("$kind: $name", (v) => {
    expect(core.conformanceEval(JSON.stringify(v))).toEqual(v.expect);
  });
});
