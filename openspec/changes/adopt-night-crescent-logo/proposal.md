# Proposal

## Why

`use-brand-launcher-icon` (#85) replaced the placeholder crescent with a clean redraw of the restored logo: a crescent, a dot and a thin sound wave on light mint. The owner then asked for a design review. Three expert critiques, four candidate designs, two jury rounds with a discussion, a combined proposal and two follow-up studies (2026-11-19) led to one refined mark, "A2b+":

- a fuller sage crescent;
- **no dot** (all three jurors: it carries no meaning and clutters 24 px and themed icons);
- a **bolder, warm cream breath wave** (calm small-big-small shape, low on the moon, running 3 dp past its outline);
- a **dark night tile** as the default, so the icon does not glare on a dark home screen at 3 AM (owner decision; it replaces the mint decision of `use-brand-launcher-icon`).

The app also still uses Android system icons for its notifications (`ic_lock_idle_alarm`, `ic_btn_speak_now`). Its status-bar presence, which tired parents look at most, therefore carries no brand and no role.

This changes the product's visible identity (Critical Reviewer gate, AGENTS.md §6). It does not change architecture, scope or the security model. Tier T0.

## What Changes

- **New vector master** `docs/brand/logo.svg` (A2b+), together with the light-surface variant, the wordmark lockups, splash sources, palette and the generator. The sources are in this change under `assets/`, and the design history is in the design.
- **Android launcher:** the adaptive icon gets the night tile `#14202B`, the A2b+ foreground and a themed monochrome layer with a 2.75 dp gap between wave and crescent.
- **Dev build:** an amber DEV pill in the free upper right replaces the mint dev background. The themed dev layer is the monochrome layer plus the rounded-square badge chosen on 2026-11-19.
- **Notification small icons:** a crescent for the baby device and a crescent with two listening bars for the parent device, white on transparent at 24 dp. They replace the system icons in `StatusNotifier` and `AlertController`.
- **Web:**
  - the favicon is the full mark from 32 px up and **the crescent only at 16 px** (owner decision);
  - `apple-touch-icon` at 180 px;
  - the PWA icons, maskable included, on the night tile, still generated at build time.
- **Brand lint** (`scripts/lib/brand-icons.mjs`) checks the new layers: the colour and monochrome layers against the master, the debug monochrome as the release layer plus a badge, the notification icons as pure white single paths, and the favicon pair.
- `docs/brand/README.md` documents the system: master, colours with contrast, sizes, which glyph is used where, and the "gap must never be dropped" rule (the cream and the sage differ by hue only).
- **Out of scope:**
  - in-app use of the mark (About, onboarding);
  - store graphics and fastlane metadata (T1);
  - an Android 12+ splash-screen theme beyond the system default from the launcher icon;
  - renaming the PWA `theme_color`.

Persona impact (`docs/concept/personas.md`): serves every parent persona at night. The icon no longer glares on a dark home screen, and notifications now show which device role is running. Nothing is traded off.

## Capabilities

### New Capabilities

### Modified Capabilities

- `distribution`: "Brand app icon" describes the night-crescent mark, the night tile and the size-specific glyphs. A new requirement covers role-specific notification icons.

## Impact

- `docs/brand/` (master, variants, README; the generator under `docs/brand/src/`).
- `apps/android/app/src/main/res/` (launcher layers, background colour), `apps/android/app/src/debug/res/` (dev layers), `apps/android/platform/src/main/res/drawable/` (new notification icons), `StatusNotifier.kt`, `AlertController.kt` (small icon references only, Platform layer).
- `apps/web/public/favicon.svg`, a new `favicon-16.svg` / `apple-touch-icon`, `apps/web/pwa-assets.config.ts`, `index.html` icon links.
- `scripts/lib/brand-icons.mjs` and its tests; `docs/ci-gates.md` (`text-lint` row).
- Screenshot baselines: the launcher icon is not in Roborazzi shots. Notification icons are only in shots if a test renders the notification; check and re-record in the pinned image if so.
