# Proposal

## Why

The native Android app shows a placeholder crescent as its launcher icon ("until the brand asset exists", `ic_launcher_foreground.xml`), although the project has a designed logo: a green crescent with a dot and a dark-blue sound wave. The logo was lost from the repository when the Capacitor client was removed in wave 1; only the web client kept copies (`apps/web/public/favicon.svg`, `pwa-*.png`). The master files are back in `docs/brand/` (PR #82). Testers and later store listings should see one recognisable mark on every platform, and the dev channel (`add-dev-fdroid-repo`) needs a real icon to derive its marked variant from.

This touches the product identity (the visible mark), but not the architecture, scope or security model; the Critical Reviewer role applies because it settles a brand decision. Tier T0.

## What Changes

- **Clean vector master:** `docs/brand/logo.svg`, a redrawn version of the traced `icon.svg` with a few clean shapes (crescent, dot, wave) and named colours, a few KB instead of about 147 KB. The traced files stay as historical sources.
- **Android adaptive launcher icon:** foreground, background and monochrome layers from the clean master. The logo is scaled so that the whole wave stays inside the adaptive-icon safe zone (66 dp circle of the 108 dp canvas) and is not clipped by any launcher mask. A light background keeps the designed colours; the wave stays dark blue.
- **Themed icon (Android 13+):** a single-colour monochrome layer of the same shapes.
- **Web icons:** `favicon.svg` replaced by the clean master; `pwa-192.png`, `pwa-512.png` and `pwa-512-maskable.png` regenerated from it with the same background as Android, so Android, web and PWA show the same mark. The PWA `theme_color` / `background_color` stay (they are the app's dark theme, not the icon).
- **Dev variant hook:** the dev build of `add-dev-fdroid-repo` overrides only the background colour (and optionally a corner badge), so it builds on these layers instead of the placeholder.
- **Docs:** `docs/brand/README.md` names the clean master, the colours and how the derived icons are produced.
- **Out of scope:** in-app use of the logo (splash screen, About screen, onboarding), store graphics (feature graphic, screenshots, fastlane metadata: T1), a wordmark redesign, the QR code (stays without a logo, `web-client` spec).

Persona impact (`docs/concept/personas.md`): serves every persona a little (the app is easy to find on a crowded home screen, also at 3 AM); trades nothing off. No user copy changes.

## Capabilities

### New Capabilities

### Modified Capabilities

- `distribution`: a new requirement that the Android launcher icon and the web/PWA icons show the GuardedSleep logo from one vector master, fully inside the adaptive-icon safe zone, with a themed monochrome layer.

## Impact

- `docs/brand/` (new `logo.svg`, README update; depends on PR #82).
- `apps/android/app/src/main/res/drawable/ic_launcher_foreground.xml`, `ic_launcher_monochrome.xml`, `res/values/ic_launcher_background.xml` (the `mipmap-anydpi` adaptive icon XML stays).
- `apps/web/public/favicon.svg`, `pwa-192.png`, `pwa-512.png`, `pwa-512-maskable.png`; a small reproducible generator script for the PNGs (`scripts/brand-icons.sh`, using a tool already in the toolchain or the build image) so they are never edited by hand.
- Roborazzi baselines are unaffected (they do not render the launcher icon); web E2E is unaffected.
- `add-dev-fdroid-repo` task 1.4 builds on the new background layer.
