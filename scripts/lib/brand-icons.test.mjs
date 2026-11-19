import assert from "node:assert/strict";
import test from "node:test";
import { normColor, normPath, parseVector, wavePoints } from "./brand-icons.mjs";

test("normPath separates tokens and rounds to 2 decimals", () => {
  assert.equal(normPath("M1.234,5.678C0.1 0.2,3 4,5.005 6Z"), "M 1.23 5.68 C 0.1 0.2 3 4 5.01 6 Z");
  assert.equal(normPath("M1,-2L.5.25"), "M 1 -2 L 0.5 0.25");
});

test("normPath reads exponent tokens as numbers", () => {
  assert.equal(normPath("M1e-3 2e2"), "M 0 200");
});

test("normColor drops an opaque alpha and upper-cases", () => {
  assert.equal(normColor("#FF12abCD"), "#12ABCD");
  assert.equal(normColor("#12abcd"), "#12ABCD");
  assert.equal(normColor(undefined), "");
});

test("parseVector reads a self-closing stroked path", () => {
  const [p] = parseVector(`<vector><path android:pathData="M0,0L1,1" android:strokeColor="#FFFFFFFF"
    android:strokeWidth="2" android:strokeLineCap="round" /></vector>`);
  assert.equal(p.d, "M 0 0 L 1 1");
  assert.equal(p.stroke, "#FFFFFF");
  assert.equal(p.strokeWidth, 2);
  assert.equal(p.cap, "round");
  assert.equal(p.gradient, undefined);
});

test("parseVector reads a nested gradient", () => {
  const [p] = parseVector(`<path android:pathData="M0,0Z"><aapt:attr name="android:fillColor">
    <gradient android:startY="1" android:endY="9" android:type="linear">
      <item android:color="#FF112233" android:offset="0" />
      <item android:color="#445566" android:offset="1" />
    </gradient></aapt:attr></path>`);
  assert.deepEqual(p.gradient, {
    y1: 1,
    y2: 9,
    stops: [
      { offset: 0, color: "#112233" },
      { offset: 1, color: "#445566" },
    ],
  });
});

test("wavePoints samples one cubic segment including both end points", () => {
  const pts = wavePoints(normPath("M0,0C1,2 3,2 4,0"));
  assert.deepEqual(pts[0], [0, 0]);
  assert.deepEqual(pts.at(-1), [4, 0]);
  assert.equal(pts.length, 21);
});
