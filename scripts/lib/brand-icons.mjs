#!/usr/bin/env node
// Keeps the Android launcher and notification layers and the web icon sources in step with the
// vector master docs/brand/logo.svg and the SVG set next to it (openspec change
// adopt-night-crescent-logo, design section 4; it replaces the checks of use-brand-launcher-icon).
// The PWA PNGs are generated from apps/web/public/logo.svg at build time, the Android vectors are
// converted from the SVGs once, so this check is what makes "master changed, Android did not" fail
// `just lint`. The notification icons and the DEV layers are checked against their SVG sources too.
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const BRAND = "docs/brand";
const MASTER = `${BRAND}/logo.svg`;
const SRC = {
  foreground: `${BRAND}/launcher-foreground.svg`,
  monochrome: `${BRAND}/launcher-monochrome.svg`,
  dev: `${BRAND}/launcher-dev.svg`,
  monochromeDev: `${BRAND}/launcher-monochrome-dev.svg`,
  baby: `${BRAND}/notification-baby.svg`,
  parent: `${BRAND}/notification-parent.svg`,
  favicon: `${BRAND}/favicon.svg`,
  favicon16: `${BRAND}/favicon-16.svg`,
};
const WEB = "apps/web";
const WEB_LOGO = `${WEB}/public/logo.svg`;
const WEB_FAVICON = `${WEB}/public/favicon.svg`;
const WEB_FAVICON_16 = `${WEB}/public/favicon-16.svg`;
const WEB_INDEX = `${WEB}/index.html`;
const PWA_CONFIG = `${WEB}/pwa-assets.config.ts`;
const RES = "apps/android/app/src/main/res";
const DEBUG_RES = "apps/android/app/src/debug/res";
const STAT = "apps/android/platform/src/main/res/drawable";
const ANDROID = {
  foreground: `${RES}/drawable/ic_launcher_foreground.xml`,
  monochrome: `${RES}/drawable/ic_launcher_monochrome.xml`,
  background: `${RES}/values/ic_launcher_background.xml`,
  debugForeground: `${DEBUG_RES}/drawable/ic_launcher_foreground.xml`,
  debugMonochrome: `${DEBUG_RES}/drawable/ic_launcher_monochrome.xml`,
  debugBackground: `${DEBUG_RES}/values/ic_launcher_background.xml`,
  baby: `${STAT}/ic_stat_baby.xml`,
  parent: `${STAT}/ic_stat_parent.xml`,
};
const SAFE_RADIUS = 33; // 66 dp circle of the 108 dp adaptive-icon canvas, centred at 54,54
const WHITE = "#FFFFFF";

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
const stripComments = (s) => s.replace(/<!--[\s\S]*?-->/g, "");

/** The subpaths of a normalised path ("M ... Z M ... Z") as separate normalised strings. */
export const subpaths = (d) => d.split(/ (?=M )/);

/** The <path> elements of an SVG in document order: id, normalised data, fill, and the first <rect> fill. */
export function parseSvg(svg) {
  const body = stripComments(svg);
  const paths = [...body.matchAll(/<path\b([^>]*?)\/>/g)].map((m) => {
    const a = attrs(m[1]);
    const fill = a.fill ?? /fill:\s*(#[0-9a-fA-F]{6})/.exec(a.style ?? "")?.[1];
    return {
      id: a.id,
      d: normPath(a.d ?? ""),
      fill: fill ? normColor(fill) : undefined,
      stroke: a.stroke,
      hasGradient: /url\(#/.test(m[1]),
    };
  });
  const rect = /<rect\b([^>]*?)\/>/.exec(body);
  const rectFill = rect ? attrs(rect[1]).fill : undefined;
  return { paths, background: rectFill ? normColor(rectFill) : undefined };
}

/** The <path> elements of an Android vector drawable, in document order. */
export function parseVector(xml) {
  return [...stripComments(xml).matchAll(/<path\b([\s\S]*?)(?:\/>|>([\s\S]*?)<\/path>)/g)].map(
    (m) => {
      const a = attrs(m[1]);
      return {
        d: normPath(a["android:pathData"] ?? ""),
        fill: a["android:fillColor"] ? normColor(a["android:fillColor"]) : undefined,
        gradient: /<gradient\b/.test(m[2] ?? ""),
        stroke: a["android:strokeColor"] ? normColor(a["android:strokeColor"]) : undefined,
      };
    },
  );
}

/**
 * Points of a path with absolute M, L, H, V, C, A and Z, curves and arcs sampled, for the
 * safe-zone check. Relative commands (the outlined DEV text) are not supported and throw.
 */
export function pathPoints(d) {
  const tokens = d.split(" ");
  const pts = [];
  let i = 0;
  let x = 0;
  let y = 0;
  let start = [0, 0];
  const num = () => Number(tokens[i++]);
  const STEPS = 24;
  while (i < tokens.length) {
    const cmd = tokens[i++];
    if (cmd === "M" || cmd === "L") {
      [x, y] = [num(), num()];
      if (cmd === "M") start = [x, y];
      pts.push([x, y]);
    } else if (cmd === "H") {
      x = num();
      pts.push([x, y]);
    } else if (cmd === "V") {
      y = num();
      pts.push([x, y]);
    } else if (cmd === "Z") {
      [x, y] = start;
    } else if (cmd === "C") {
      const [x1, y1, x2, y2, x3, y3] = [num(), num(), num(), num(), num(), num()];
      for (let k = 0; k <= STEPS; k++) {
        const t = k / STEPS;
        const u = 1 - t;
        pts.push([
          u ** 3 * x + 3 * u * u * t * x1 + 3 * u * t * t * x2 + t ** 3 * x3,
          u ** 3 * y + 3 * u * u * t * y1 + 3 * u * t * t * y2 + t ** 3 * y3,
        ]);
      }
      [x, y] = [x3, y3];
    } else if (cmd === "A") {
      const [rx0, ry0, , large, sweep, x2, y2] = [num(), num(), num(), num(), num(), num(), num()];
      pts.push(...arcPoints(x, y, rx0, ry0, large, sweep, x2, y2, STEPS));
      [x, y] = [x2, y2];
    } else {
      throw new Error(`unsupported path command ${cmd}`);
    }
  }
  return pts;
}

/** Circular arc (rx = ry, no rotation) from (x1,y1) to (x2,y2) as sampled points, SVG endpoint form. */
function arcPoints(x1, y1, rx, ry, large, sweep, x2, y2, steps) {
  const r = Math.max(rx, ry);
  const dx = (x1 - x2) / 2;
  const dy = (y1 - y2) / 2;
  const half = Math.hypot(dx, dy);
  if (half === 0) return [];
  const rr = Math.max(r, half);
  const k = Math.sqrt(Math.max(0, rr * rr - half * half)) * (large === sweep ? -1 : 1);
  const cx = (x1 + x2) / 2 + (k * dy) / half;
  const cy = (y1 + y2) / 2 - (k * dx) / half;
  let a1 = Math.atan2(y1 - cy, x1 - cx);
  let a2 = Math.atan2(y2 - cy, x2 - cx);
  if (sweep === 1 && a2 < a1) a2 += 2 * Math.PI;
  if (sweep === 0 && a2 > a1) a2 -= 2 * Math.PI;
  const out = [];
  for (let s = 0; s <= steps; s++) {
    const a = a1 + ((a2 - a1) * s) / steps;
    out.push([cx + rr * Math.cos(a), cy + rr * Math.sin(a)]);
  }
  return out;
}

/** Largest distance of a path's points from the canvas centre (54, 54). */
export function farthest(d) {
  return Math.max(...pathPoints(d).map(([x, y]) => Math.hypot(x - 54, y - 54)));
}

/** Fails when a shape reaches outside the 66 dp safe circle of the adaptive-icon canvas. */
function checkSafeZone(file, label, d) {
  const far = farthest(d);
  if (far > SAFE_RADIUS)
    fail(
      file,
      `${label} reaches ${far.toFixed(1)} dp from the centre, safe zone is ${SAFE_RADIUS} dp`,
    );
}

const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

/** Compares Android paths with the SVG source paths one to one (data, and fill when given). */
function comparePaths(file, label, got, want, { fill = true } = {}) {
  if (got.length !== want.length) {
    fail(file, `expected ${want.length} path(s) like ${label}, found ${got.length}`);
    return;
  }
  want.forEach((w, i) => {
    const g = got[i];
    if (g.d !== w.d)
      fail(file, `path ${i + 1}${w.id ? ` (${w.id})` : ""}: pathData differs from ${label}`);
    if (fill && w.fill && g.fill !== w.fill)
      fail(
        file,
        `path ${i + 1}${w.id ? ` (${w.id})` : ""}: fill ${g.fill} differs from ${label} (${w.fill})`,
      );
  });
}

/** A single-colour layer: every fill is opaque white and nothing else (stroke, gradient) is used. */
function checkWhiteOnly(file, paths) {
  for (const p of paths) {
    if (p.fill !== WHITE) fail(file, `single-colour layer must use #FFFFFFFF, found ${p.fill}`);
    if (p.stroke) fail(file, "single-colour layer must not use strokes");
    if (p.gradient) fail(file, "single-colour layer must not use gradients");
  }
}

const colourLiterals = (xml) =>
  [...stripComments(xml).matchAll(/#[0-9A-Fa-f]{6,8}\b/g)].map((m) => m[0]);

function main() {
  // ---- Master: filled crescent and wave on the night tile, no strokes or gradients.
  const masterSrc = read(MASTER);
  const master = masterSrc ? parseSvg(masterSrc) : { paths: [], background: undefined };
  const crescent = master.paths.find((p) => p.id === "crescent");
  const wave = master.paths.find((p) => p.id === "wave");
  if (masterSrc !== null) {
    if (!crescent) fail(MASTER, 'shape id="crescent" not found');
    if (!wave) fail(MASTER, 'shape id="wave" not found');
    if (!master.background) fail(MASTER, "night tile <rect fill=...> not found");
    for (const p of master.paths) {
      if (p.stroke || p.hasGradient) fail(MASTER, `shape ${p.id}: only filled outlines allowed`);
    }
  }
  const night = master.background;

  // ---- The same night tile in the Android resources and the PWA config.
  const bgOf = (src) =>
    /<color name="ic_launcher_background">(#[0-9A-Fa-f]{6,8})<\/color>/.exec(src ?? "")?.[1];
  const androidBg = bgOf(read(ANDROID.background));
  const debugBg = bgOf(read(ANDROID.debugBackground));
  const pwaBg = /const background = "(#[0-9A-Fa-f]{6})"/.exec(read(PWA_CONFIG) ?? "")?.[1];
  if (!androidBg) fail(ANDROID.background, "ic_launcher_background colour not found");
  if (!debugBg) fail(ANDROID.debugBackground, "ic_launcher_background colour not found");
  if (!pwaBg) fail(PWA_CONFIG, "`const background` not found");
  for (const [file, value] of [
    [ANDROID.background, androidBg],
    [ANDROID.debugBackground, debugBg],
    [PWA_CONFIG, pwaBg],
  ]) {
    if (night && value && normColor(value) !== night)
      fail(file, `night tile ${value} differs from ${MASTER} (${night})`);
  }

  // ---- Launcher foreground: the master's crescent and wave.
  const fgSvg = read(SRC.foreground);
  const fgSource = fgSvg ? parseSvg(fgSvg).paths : [];
  if (crescent && wave && fgSvg !== null) {
    comparePaths(SRC.foreground, MASTER, fgSource, [crescent, wave]);
  }
  const fgXml = read(ANDROID.foreground);
  const fg = fgXml ? parseVector(fgXml) : [];
  if (crescent && wave && fgXml !== null) {
    comparePaths(ANDROID.foreground, MASTER, fg, [crescent, wave]);
  }

  // ---- Monochrome: the source layer, white only; the wave stays its own subpath (the gap).
  const monoSvg = read(SRC.monochrome);
  const monoSource = monoSvg ? parseSvg(monoSvg).paths : [];
  const monoXml = read(ANDROID.monochrome);
  const mono = monoXml ? parseVector(monoXml) : [];
  if (monoXml !== null) {
    checkWhiteOnly(ANDROID.monochrome, mono);
    if (monoSvg !== null)
      comparePaths(ANDROID.monochrome, SRC.monochrome, mono, monoSource, { fill: false });
    // Cream and sage are told apart by hue; in one colour only the gap separates wave and
    // crescent, so the wave must remain a separate closed subpath next to the crescent pieces.
    if (wave && mono.length === 1) {
      const parts = subpaths(mono[0].d);
      if (parts.length < 2 || parts.at(-1) !== wave.d) {
        fail(
          ANDROID.monochrome,
          `the wave must be the last, separate subpath (the 2.75 dp gap to the crescent), like ${MASTER}`,
        );
      }
    } else if (wave && mono.length !== 1) {
      fail(
        ANDROID.monochrome,
        `expected one path with the crescent pieces and the wave, found ${mono.length}`,
      );
    }
  }

  // ---- Debug build: the release layers plus the DEV pill and the themed badge.
  const devSvg = read(SRC.dev);
  const dev = devSvg ? parseSvg(devSvg).paths : [];
  const dbgFgXml = read(ANDROID.debugForeground);
  if (dbgFgXml !== null && devSvg !== null && fgXml !== null) {
    const dbgFg = parseVector(dbgFgXml);
    if (dbgFg.length <= fg.length) {
      fail(ANDROID.debugForeground, "needs the DEV pill after the logo paths");
    } else if (!same(dbgFg.slice(0, fg.length), fg)) {
      fail(
        ANDROID.debugForeground,
        `must start with the release foreground paths of ${ANDROID.foreground}`,
      );
    }
    comparePaths(ANDROID.debugForeground, SRC.dev, dbgFg, dev);
    // The launcher-dev source must itself be the release foreground plus the pill.
    comparePaths(SRC.dev, SRC.foreground, dev.slice(0, fgSource.length), fgSource);
    // Pill (and its outlined text) inside the safe zone; the text uses relative commands.
    const pill = dev[fgSource.length];
    if (pill) checkSafeZone(SRC.dev, "DEV pill", pill.d);
  }
  const dbgMonoXml = read(ANDROID.debugMonochrome);
  const dbgMonoSvg = read(SRC.monochromeDev);
  if (dbgMonoXml !== null && monoXml !== null) {
    if (!/<vector\b/.test(dbgMonoXml) || !/<\/vector>\s*$/.test(dbgMonoXml)) {
      fail(ANDROID.debugMonochrome, "not a well-formed vector drawable (needs <vector>…</vector>)");
    } else {
      const dbgMono = parseVector(dbgMonoXml);
      checkWhiteOnly(ANDROID.debugMonochrome, dbgMono);
      const missing = mono.filter((r) => !dbgMono.some((d) => same(d, r)));
      if (missing.length > 0)
        fail(
          ANDROID.debugMonochrome,
          `differs from ${ANDROID.monochrome} in ${missing.length} logo path(s)`,
        );
      if (dbgMono.length <= mono.length)
        fail(ANDROID.debugMonochrome, "needs a badge path in addition to the logo paths");
      if (dbgMonoSvg !== null) {
        const want = parseSvg(dbgMonoSvg).paths;
        comparePaths(ANDROID.debugMonochrome, SRC.monochromeDev, dbgMono, want, { fill: false });
        comparePaths(
          SRC.monochromeDev,
          SRC.monochrome,
          want.slice(0, monoSource.length),
          monoSource,
          { fill: false },
        );
        const badge = want.at(-1);
        if (badge) checkSafeZone(SRC.monochromeDev, "badge", badge.d);
      }
    }
  }

  // ---- Notification small icons: one white path each, the SVG source's data, no other colour.
  for (const role of ["baby", "parent"]) {
    const svg = read(SRC[role]);
    const xml = read(ANDROID[role]);
    if (svg === null || xml === null) continue;
    const want = parseSvg(svg).paths;
    const got = parseVector(xml);
    if (got.length !== 1) fail(ANDROID[role], `expected a single path, found ${got.length}`);
    checkWhiteOnly(ANDROID[role], got);
    comparePaths(ANDROID[role], SRC[role], got, want, { fill: false });
    const other = colourLiterals(xml).filter((c) => normColor(c) !== WHITE);
    if (other.length > 0)
      fail(
        ANDROID[role],
        `only white is allowed in a notification icon, found ${other.join(", ")}`,
      );
    if (/aapt:attr|<gradient|strokeColor/.test(stripComments(xml)))
      fail(ANDROID[role], "no gradient or stroke in a notification icon");
  }

  // ---- Web: byte-identical copies of the sources; the 16 px favicon is the crescent alone.
  for (const [web, src] of [
    [WEB_LOGO, MASTER],
    [WEB_FAVICON, SRC.favicon],
    [WEB_FAVICON_16, SRC.favicon16],
  ]) {
    const a = read(web);
    const b = read(src);
    if (a !== null && b !== null && a !== b)
      fail(web, `must be a byte-identical copy of ${src} (cp ${src} ${web})`);
  }
  const fav16 = read(SRC.favicon16);
  if (fav16 !== null) {
    const p = parseSvg(fav16);
    if (p.paths.length !== 1)
      fail(
        SRC.favicon16,
        `the 16 px favicon is the crescent alone: one path, found ${p.paths.length}`,
      );
    if (night && p.background !== night)
      fail(SRC.favicon16, `night tile ${p.background} differs from ${MASTER} (${night})`);
  }
  const fav32 = read(SRC.favicon);
  if (fav32 !== null && night && parseSvg(fav32).background !== night)
    fail(SRC.favicon, `night tile differs from ${MASTER} (${night})`);
  const index = read(WEB_INDEX);
  if (index !== null && !/<link[^>]*href="\/favicon-16\.svg"[^>]*sizes="16x16"/.test(index))
    fail(WEB_INDEX, 'must link /favicon-16.svg with sizes="16x16"');
  if (index !== null && !/<link[^>]*rel="icon"[^>]*href="\/favicon\.svg"/.test(index))
    fail(WEB_INDEX, "must link /favicon.svg as the default icon");

  // ---- Safe zone: the mark inside the 66 dp circle.
  for (const p of [crescent, wave]) {
    if (p) checkSafeZone(MASTER, p.id, p.d);
  }

  if (errors.length > 0) {
    console.error(
      `brand-icons: the icon layers differ from ${MASTER} and the SVG set in ${BRAND}:`,
    );
    for (const e of errors) console.error(`  ${e}`);
    console.error(
      "Update the Android vectors and the web copies to the SVG sources, or the sources to the intended change.",
    );
    process.exit(1);
  }
  console.log("brand-icons: ok");
}

// Library for the unit test; the checks run only when executed directly.
if (process.argv[1] === fileURLToPath(import.meta.url)) main();
