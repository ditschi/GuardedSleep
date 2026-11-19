#!/usr/bin/env node
// Keeps the Android launcher layers and the web icon sources in step with the vector master
// docs/brand/logo.svg (openspec change use-brand-launcher-icon, design section 6). The PWA PNGs are
// generated from apps/web/public/favicon.svg at build time, the Android vectors are hand-converted
// once, so this check is what makes "master changed, Android did not" fail `just lint`.
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const MASTER = "docs/brand/logo.svg";
const FAVICON = "apps/web/public/favicon.svg";
const RES = "apps/android/app/src/main/res";
const FOREGROUND = `${RES}/drawable/ic_launcher_foreground.xml`;
const MONOCHROME = `${RES}/drawable/ic_launcher_monochrome.xml`;
const BACKGROUND = `${RES}/values/ic_launcher_background.xml`;
// Optional: the dev variant (add-dev-fdroid-repo) may carry its own monochrome layer with a badge.
// Only its presence and well-formedness are checked, not equality with the master.
const DEBUG_MONOCHROME = "apps/android/app/src/debug/res/drawable/ic_launcher_monochrome.xml";
const PWA_CONFIG = "apps/web/pwa-assets.config.ts";
// Order of the shapes in the master and in the Android foreground.
const SHAPES = ["crescent", "crescent-body", "dot", "wave"];
const SAFE_RADIUS = 33; // 66 dp circle of the 108 dp adaptive-icon canvas, centred at 54,54

const errors = [];
const fail = (file, msg) => errors.push(`${file}: ${msg}`);
const read = (file) => {
  const path = join(root, file);
  if (!existsSync(path)) {
    fail(file, "missing");
    return null;
  }
  return readFileSync(path, "utf8");
};

/** Path data as a canonical string: commands and numbers separated by one space, 2 decimals. */
export function normPath(d) {
  const tokens = d.match(/[a-zA-Z]|-?\d*\.?\d+(?:e-?\d+)?/g) ?? [];
  return tokens
    .map((t) => (/^[a-zA-Z]$/.test(t) ? t : String(Math.round(Number(t) * 100) / 100)))
    .join(" ");
}
export const normColor = (c) => (c ?? "").toUpperCase().replace(/^#FF([0-9A-F]{6})$/, "#$1");
const attrs = (tag) =>
  Object.fromEntries([...tag.matchAll(/([\w:.-]+)="([^"]*)"/g)].map((m) => [m[1], m[2]]));

/** The shapes of logo.svg by id: path data, fill gradient stops, stroke. */
export function parseMaster(svg) {
  const gradients = {};
  for (const m of svg.matchAll(/<linearGradient\b([^>]*)>([\s\S]*?)<\/linearGradient>/g)) {
    const a = attrs(m[1]);
    gradients[a.id] = {
      y1: Number(a.y1),
      y2: Number(a.y2),
      stops: [...m[2].matchAll(/<stop\b([^>]*)\/>/g)].map((s) => {
        const sa = attrs(s[1]);
        return { offset: Number(sa.offset), color: normColor(sa["stop-color"]) };
      }),
    };
  }
  const shapes = {};
  for (const m of svg.matchAll(/<path\b([^>]*)\/>/g)) {
    const a = attrs(m[1]);
    const gradient = /^url\(#(.+)\)$/.exec(a.fill ?? "")?.[1];
    shapes[a.id] = {
      d: normPath(a.d ?? ""),
      gradient: gradient ? gradients[gradient] : undefined,
      stroke: a.stroke ? normColor(a.stroke) : undefined,
      strokeWidth: a["stroke-width"] ? Number(a["stroke-width"]) : undefined,
      cap: a["stroke-linecap"],
      join: a["stroke-linejoin"],
    };
  }
  return shapes;
}

/** The <path> elements of an Android vector drawable, in document order. */
export function parseVector(xml) {
  return [...xml.matchAll(/<path\b([\s\S]*?)(?:\/>|>([\s\S]*?)<\/path>)/g)].map((m) => {
    const a = attrs(m[1]);
    const g = /<gradient\b([^>]*)>([\s\S]*?)<\/gradient>/.exec(m[2] ?? "");
    let gradient;
    if (g) {
      const ga = attrs(g[1]);
      gradient = {
        y1: Number(ga["android:startY"]),
        y2: Number(ga["android:endY"]),
        stops: [...g[2].matchAll(/<item\b([^>]*)\/>/g)].map((s) => {
          const sa = attrs(s[1]);
          return { offset: Number(sa["android:offset"]), color: normColor(sa["android:color"]) };
        }),
      };
    }
    return {
      d: normPath(a["android:pathData"] ?? ""),
      fill: a["android:fillColor"] ? normColor(a["android:fillColor"]) : undefined,
      gradient,
      stroke: a["android:strokeColor"] ? normColor(a["android:strokeColor"]) : undefined,
      strokeWidth: a["android:strokeWidth"] ? Number(a["android:strokeWidth"]) : undefined,
      cap: a["android:strokeLineCap"],
      join: a["android:strokeLineJoin"],
    };
  });
}

const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

/** Points of a "M C C ..." wave path (cubic segments sampled) for the safe-zone check. */
export function wavePoints(d) {
  const n = d
    .split(" ")
    .filter((t) => !/^[a-zA-Z]$/.test(t))
    .map(Number);
  const pts = [];
  let [x0, y0] = n;
  for (let i = 2; i + 5 < n.length; i += 6) {
    const [x1, y1, x2, y2, x3, y3] = n.slice(i, i + 6);
    // Integer steps, so t = 1 (the segment's end, e.g. the right wave end) is always sampled;
    // a float step of 0.05 accumulates past 1 and skips it.
    for (let k = 0; k <= 20; k++) {
      const t = k / 20;
      const u = 1 - t;
      pts.push([
        u ** 3 * x0 + 3 * u * u * t * x1 + 3 * u * t * t * x2 + t ** 3 * x3,
        u ** 3 * y0 + 3 * u * u * t * y1 + 3 * u * t * t * y2 + t ** 3 * y3,
      ]);
    }
    [x0, y0] = [x3, y3];
  }
  return pts;
}

function main() {
  // Master and favicon.
  const masterSrc = read(MASTER);
  const faviconSrc = read(FAVICON);
  if (masterSrc !== null && faviconSrc !== null && masterSrc !== faviconSrc) {
    fail(FAVICON, `must be a byte-identical copy of ${MASTER} (cp ${MASTER} ${FAVICON})`);
  }
  const master = masterSrc ? parseMaster(masterSrc) : {};
  for (const id of SHAPES) if (!master[id]) fail(MASTER, `shape id="${id}" not found`);

  // Background colour: master comment, Android resource, PWA config.
  const masterBg = /background:\s*(#[0-9A-Fa-f]{6})/.exec(masterSrc ?? "")?.[1];
  const androidBg = /<color name="ic_launcher_background">(#[0-9A-Fa-f]{6,8})<\/color>/.exec(
    read(BACKGROUND) ?? "",
  )?.[1];
  const pwaBg = /const background = "(#[0-9A-Fa-f]{6})"/.exec(read(PWA_CONFIG) ?? "")?.[1];
  if (!masterBg) fail(MASTER, 'comment must name the background colour ("background: #RRGGBB")');
  if (!androidBg) fail(BACKGROUND, "ic_launcher_background colour not found");
  if (!pwaBg) fail(PWA_CONFIG, "`const background` not found");
  if (masterBg && androidBg && normColor(masterBg) !== normColor(androidBg)) {
    fail(BACKGROUND, `background ${androidBg} differs from ${MASTER} (${masterBg})`);
  }
  if (masterBg && pwaBg && normColor(masterBg) !== normColor(pwaBg)) {
    fail(PWA_CONFIG, `background ${pwaBg} differs from ${MASTER} (${masterBg})`);
  }

  // Foreground: the four shapes in order, same data, gradients and stroke.
  const fgSrc = read(FOREGROUND);
  if (fgSrc !== null && master.wave) {
    const fg = parseVector(fgSrc);
    if (fg.length !== SHAPES.length) {
      fail(
        FOREGROUND,
        `expected ${SHAPES.length} paths (${SHAPES.join(", ")}), found ${fg.length}`,
      );
    }
    SHAPES.forEach((id, i) => {
      const want = master[id];
      const got = fg[i];
      if (!want || !got) return;
      if (got.d !== want.d)
        fail(FOREGROUND, `path ${i + 1} (${id}): pathData differs from ${MASTER}`);
      if (want.gradient && !same(got.gradient, want.gradient)) {
        fail(
          FOREGROUND,
          `path ${i + 1} (${id}): gradient (y range or colour stops) differs from ${MASTER}`,
        );
      }
      if (want.stroke) {
        for (const [k, label] of [
          ["stroke", "strokeColor"],
          ["strokeWidth", "strokeWidth"],
          ["cap", "strokeLineCap"],
          ["join", "strokeLineJoin"],
        ]) {
          if (got[k] !== want[k])
            fail(
              FOREGROUND,
              `path ${i + 1} (${id}): ${label} ${got[k]} differs from ${MASTER} (${want[k]})`,
            );
        }
      }
    });
  }

  // Monochrome: one colour, the wave identical to the master, the cut-out shapes present.
  const monoSrc = read(MONOCHROME);
  if (monoSrc !== null && master.wave) {
    const mono = parseVector(monoSrc);
    if (mono.length < 3)
      fail(MONOCHROME, "expected the crescent, dot (with cut-out) and wave paths");
    const colours = new Set(mono.flatMap((p) => [p.fill, p.stroke]).filter(Boolean));
    for (const c of colours) {
      if (c !== "#FFFFFF") fail(MONOCHROME, `single-colour layer must use #FFFFFFFF, found ${c}`);
    }
    const wave = mono.find((p) => p.stroke);
    if (!wave || wave.d !== master.wave.d || wave.strokeWidth !== master.wave.strokeWidth) {
      fail(MONOCHROME, `the stroked wave differs from ${MASTER}`);
    }
  }

  // Optional debug variant (dev builds): the release monochrome shapes unchanged plus at least one
  // badge path, so the dev app keeps the current logo in themed icons and stays distinguishable.
  if (existsSync(join(root, DEBUG_MONOCHROME))) {
    const dbg = read(DEBUG_MONOCHROME) ?? "";
    if (!/<vector\b/.test(dbg) || !/<\/vector>\s*$/.test(dbg)) {
      fail(DEBUG_MONOCHROME, "not a well-formed vector drawable (needs <vector>…</vector>)");
    } else if (monoSrc !== null) {
      const release = parseVector(monoSrc);
      const debug = parseVector(dbg);
      const missing = release.filter((r) => !debug.some((d) => same(d, r)));
      if (missing.length > 0)
        fail(DEBUG_MONOCHROME, `differs from ${MONOCHROME} in ${missing.length} logo path(s)`);
      if (debug.length <= release.length)
        fail(DEBUG_MONOCHROME, "needs a badge path in addition to the logo paths");
    }
  }

  // Safe zone: wave (with half the stroke width) inside the 66 dp circle.
  if (master.wave) {
    const half = (master.wave.strokeWidth ?? 0) / 2;
    const far =
      Math.max(...wavePoints(master.wave.d).map(([x, y]) => Math.hypot(x - 54, y - 54))) + half;
    if (far > SAFE_RADIUS)
      fail(
        MASTER,
        `wave reaches ${far.toFixed(1)} dp from the centre, safe zone is ${SAFE_RADIUS} dp`,
      );
  }

  if (errors.length > 0) {
    console.error(`brand-icons: the launcher icon layers differ from ${MASTER}:`);
    for (const e of errors) console.error(`  ${e}`);
    console.error(
      "Update the Android vectors (and favicon.svg) to the master, or the master to the intended change.",
    );
    process.exit(1);
  }
  console.log("brand-icons: ok");
}

// Library for the unit test; the checks run only when executed directly.
if (process.argv[1] === fileURLToPath(import.meta.url)) main();
