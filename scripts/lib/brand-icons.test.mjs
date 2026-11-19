import assert from "node:assert/strict";
import test from "node:test";
import {
  farthest,
  normColor,
  normPath,
  parseSvg,
  parseVector,
  pathPoints,
  subpaths,
} from "./brand-icons.mjs";

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

test("parseVector reads a fill, a stroke and a gradient marker", () => {
  const [a, b, c] = parseVector(`<vector>
    <!-- #123456 in a comment is ignored -->
    <path android:fillColor="#FFFFFFFF" android:pathData="M0,0L1,1Z" />
    <path android:pathData="M0,0L1,1" android:strokeColor="#FF000000" />
    <path android:pathData="M0,0Z"><aapt:attr name="android:fillColor"><gradient /></aapt:attr></path>
  </vector>`);
  assert.equal(a.d, "M 0 0 L 1 1 Z");
  assert.equal(a.fill, "#FFFFFF");
  assert.equal(a.stroke, undefined);
  assert.equal(b.stroke, "#000000");
  assert.equal(c.gradient, true);
});

test("parseSvg reads ids, fills (attribute and style), the tile and ignores comments", () => {
  const { paths, background } = parseSvg(`<svg><!-- <path id="x" d="M0,0"/> -->
    <rect width="9" height="9" fill="#14202b"/>
    <path id="a" d="M1,2 L3,4Z" fill="#5DBFA6"/>
    <path d="M5,6Z" style="font-weight:800;fill:#14202b" />
  </svg>`);
  assert.equal(background, "#14202B");
  assert.deepEqual(
    paths.map((p) => [p.id, p.d, p.fill]),
    [
      ["a", "M 1 2 L 3 4 Z", "#5DBFA6"],
      [undefined, "M 5 6 Z", "#14202B"],
    ],
  );
});

test("subpaths splits at every move, so the wave can be told from the crescent pieces", () => {
  assert.deepEqual(subpaths(normPath("M0,0L1,1Z M5,5L6,6Z M8,8L9,9Z")), [
    "M 0 0 L 1 1 Z",
    "M 5 5 L 6 6 Z",
    "M 8 8 L 9 9 Z",
  ]);
  assert.deepEqual(subpaths(normPath("M0,0L1,1ZM5,5Z")), ["M 0 0 L 1 1 Z", "M 5 5 Z"]);
});

test("pathPoints samples a cubic including both end points", () => {
  const pts = pathPoints(normPath("M0,0C1,2 3,2 4,0"));
  assert.deepEqual(pts[0], [0, 0]);
  assert.deepEqual(pts.at(-1), [4, 0]);
});

test("pathPoints follows an arc on its circle, on the side the flags select", () => {
  // Half circle of radius 10 around (0, 0) from (-10, 0) to (10, 0): sweep 1 runs over the top of
  // the screen (negative y), sweep 0 under it.
  const over = pathPoints(normPath("M-10,0A10,10 0 0 1 10,0"));
  const under = pathPoints(normPath("M-10,0A10,10 0 0 0 10,0"));
  for (const [x, y] of over) assert.ok(Math.abs(Math.hypot(x, y) - 10) < 1e-6 && y <= 1e-9);
  for (const [x, y] of under) assert.ok(Math.abs(Math.hypot(x, y) - 10) < 1e-6 && y >= -1e-9);
});

test("pathPoints handles H, V and Z and rejects relative commands", () => {
  assert.deepEqual(pathPoints(normPath("M1,1H5V4Z")), [
    [1, 1],
    [5, 1],
    [5, 4],
  ]);
  assert.throws(() => pathPoints(normPath("M1,1h5")), /unsupported/);
});

test("farthest measures from the canvas centre (54, 54)", () => {
  assert.equal(farthest(normPath("M54,54L54,22")), 32);
});
