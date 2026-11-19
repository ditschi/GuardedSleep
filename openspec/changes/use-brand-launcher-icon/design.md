# Design

## Context

- Logo sources (PR #82): `docs/brand/icon.svg` (Inkscape trace of a bitmap, about 150 KB of small paths, 1024 viewBox), `icon.png` (1024 px on white), `splash.svg/png` (with wordmark). Colours in the trace: wave `#275072`, crescent light `#7FE3A6`-ish to dark `#2F7A4E`-ish gradient, dot light green.
- Android: `mipmap-anydpi/ic_launcher.xml` (adaptive icon: background colour `#1F2A44`, vector foreground, vector monochrome), placeholder crescent.
- Web: `apps/web/public/favicon.svg` (the 150 KB trace), `pwa-192.png`, `pwa-512.png`, `pwa-512-maskable.png` (black background), referenced from `vite.config.ts` (`VitePWA`).
- The logo is wide: the wave spans about 80 % of the canvas width, the crescent about 60 %.

Layer: resources and build tooling only. No change to UI, State, Platform, Bindings or Core code, and no user copy. Privacy guardrails: unaffected (static assets, no network).

## 1. Vector master

**Option A — redraw a clean `docs/brand/logo.svg` (recommended).** Three or four shapes: crescent (two arcs, with the inner highlight as a second shape or a linear gradient), dot (circle), wave (one stroked path converted to a filled outline for Android), named colours in a comment. Viewbox 108 × 108 laid out for the adaptive-icon canvas, so the same coordinates go into Android.

- Pros: a few KB; scalable and editable; gradients and paths map 1 : 1 to Android `<vector>` (`<gradient>` via `aapt:attr`); one source for every platform.
- Cons: a redraw is a small design task; it must stay faithful to the original (compared side by side against `icon.png` in review).

**Option B — use the traced `icon.svg` directly.**

- Pros: no redraw.
- Cons: thousands of path nodes; too heavy for an Android vector drawable (render cost, lint warning on path length) and for the favicon; the trace has rough edges at small sizes.

**Option C — PNG foreground per density from `icon.png`.**

- Pros: pixel-identical to the original.
- Cons: five rasters to maintain, no monochrome layer for free, blurry on high-density launchers, and contrary to "one vector master".

Recommendation: A. The wave is a stroke in the original; Android vectors support strokes too (`strokeWidth`, `strokeLineCap="round"`), which keeps the path short; it is used directly if the visual check (task 1.3) shows no difference.

## 2. Fitting the safe zone

Adaptive icons draw on 108 dp; launchers show at least the inner 72 dp, and only a 66 dp circle is guaranteed. The whole logo, wave ends included, is scaled into the 66 dp circle (wave about 60 dp wide, centred). The crescent then appears smaller than in the original square icon; that is the cost of a mark that is wider than tall.

Discarded: letting the wave run off the edges (looks like an intentional bleed on square masks, but is cut mid-curve on circles and squircles, which the spec scenario forbids).

## 3. Background colour

**Option A — light background (near-white mint, e.g. `#F2FAF5`) (recommended).**

- Pros: the logo was designed on white (`icon.png`, `splash.png`); the dark-blue wave keeps its contrast; matches the store/marketing look.
- Cons: a bright tile on a dark home screen at night.

**Option B — keep the dark navy `#1F2A44`.**

- Pros: calm at night; matches the app's dark theme.
- Cons: the `#275072` wave nearly disappears on navy; the wave would have to be recoloured, which changes the logo.

**Option C — black, like the current `pwa-512-maskable.png`.**

- Pros: matches the PWA's `theme_color`.
- Cons: same contrast problem for the wave as B.

Recommendation: A, as the owner's brand call; the UX role confirms in review. Users who want a dark home screen use themed icons (§4). The dev build of `add-dev-fdroid-repo` overrides this background with a clearly different colour.

## 4. Monochrome layer

All shapes of the master in one colour (`#FFFFFFFF`, the system tints it). The wave crosses the crescent, so where they overlap the wave is cut out of the crescent by a 1.5 dp gap (even-odd or a separate outline path), otherwise it merges into one blob in a single colour.

## 5. Web and PWA icons

**Option A — generate the PWA PNGs at build time from `logo.svg` with `vite-plugin-pwa`'s `pwaAssets` (`@vite-pwa/assets-generator`) (recommended).**

- Pros: nothing generated is committed (AGENTS.md §5.2); the PWA icons can never drift from the master; maskable padding and background come from one config.
- Cons: a new dev dependency that pulls in `sharp` (native binary, needs an `allowBuilds` entry with a doc-link comment); justify it in the PR.

**Option B — a script (Inkscape/ImageMagick) renders the PNGs, which stay committed, with a stamp check in `just lint`.**

- Pros: no new npm dependency.
- Cons: Inkscape and ImageMagick are not in mise or the build images, so CI cannot regenerate; a stamp only proves someone ran the script; generated files in git.

Recommendation: A. `favicon.svg` becomes a copy of `logo.svg` (a vector, not generated): the check in §6 compares them.

## 6. Keeping Android in sync

The Android foreground and monochrome XML are hand-converted once from `logo.svg`. A small Node script (`scripts/lint-brand-icons.mjs`, run in `just lint-text`) parses `logo.svg`, `apps/web/public/favicon.svg` and the two Android vectors and fails if path data, fills or the background colour differ (normalised whitespace and number formatting). Gives the spec's "lint fails until Android matches again" without a generator in the Android build.

Discarded: generating the Android XML from the SVG at build time (one more generator in Gradle for two files that change once a year).

## 7. Effect on other work

- `add-dev-fdroid-repo` task 1.4: overrides `ic_launcher_background` in `src/debug/`; with this change it gets a real foreground. If that change lands first, it overrides the placeholder; no conflict either way.
- The F-Droid dev repository can use a PNG export of the Android icon as its repository icon (that change's task 2.2).

## Discarded alternatives

- **Traced SVG as master, PNG foregrounds, wave bleeding off the edges, dark background with a recoloured wave, committed PNGs with a stamp, Android XML generated at build time:** see §1 to §6.
- **Commissioning a new logo:** the owner already has one; out of scope.
