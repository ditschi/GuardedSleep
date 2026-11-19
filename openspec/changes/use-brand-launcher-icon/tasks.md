# Tasks

Branch `feat/brand-launcher-icon`, based on `main` after PR #82 (`docs/brand/`). Each implementation task is a brief for the `implementer` sub-agent. Layer rule: resources and build tooling only; no UI, State, Platform, Bindings or Core code; no user copy.

## 1. Vector master and Android icon

- [x] 1.1 **Clean master.** Goal: `docs/brand/logo.svg` as design §1 and §2: 108 × 108 viewBox, crescent (with its inner highlight), dot and wave as a few shapes, whole logo inside the 66 dp circle centred at 54,54, colours sampled from `docs/brand/icon.png` and listed in a comment. Files: `docs/brand/logo.svg`, `docs/brand/README.md` (name the master, colours, background, how the derived icons are made). Acceptance: file under 8 KB; a side-by-side render (e.g. `magick`/`inkscape` locally, not committed) of `logo.svg` next to `icon.png` attached to the PR; owner confirms likeness.
- [ ] 1.2 **Android layers.** Goal: `ic_launcher_foreground.xml` and `ic_launcher_monochrome.xml` carry the master's shapes (gradient via `aapt:attr` if used; the monochrome cut-out of design §4); `ic_launcher_background.xml` gets the light colour of design §3; keep the comment style ("launcher colour, not copy"). Files: `apps/android/app/src/main/res/drawable/ic_launcher_{foreground,monochrome}.xml`, `res/values/ic_launcher_background.xml`. Acceptance: `scripts/gradle.sh :app:lintDebug` (or the Android lint the repo runs) shows no vector-path warning; `scripts/gradle.sh :app:assembleDebug` builds.
- [ ] 1.3 **Visual check.** Goal: render the adaptive icon under circle, squircle and square masks and as a themed icon, without committing the renders. Acceptance: screenshots from an emulator or `just install-android` on a phone, attached to the PR; both wave ends visible under the circle mask.

## 2. Web

- [ ] 2.1 **Favicon.** Goal: `apps/web/public/favicon.svg` = `docs/brand/logo.svg` (byte-identical; design §5). Acceptance: `just build-web` and the browser tab shows the logo.
- [ ] 2.2 **PWA icons at build time.** Goal: `vite-plugin-pwa` `pwaAssets` with `@vite-pwa/assets-generator` from `logo.svg`, maskable and plain icons with the light background, replacing the committed `pwa-192.png`, `pwa-512.png`, `pwa-512-maskable.png` (delete them). Add the dependency with `pnpm add -D` (no invented version), the `sharp` `allowBuilds` entry in `pnpm-workspace.yaml` with a doc-link comment, and justify it in the PR. Files: `apps/web/vite.config.ts`, `apps/web/package.json`, `pnpm-lock.yaml`, `pnpm-workspace.yaml`, `apps/web/public/`. Acceptance: `just build-web` emits the icons and the manifest lists them; `just test` (Vitest) and the PWA manifest checks in `just e2e` stay green; `pnpm audit` clean.

## 3. Sync check and docs

- [ ] 3.1 **Brand icon lint.** Goal: `scripts/lint-brand-icons.mjs` as design §6, wired into `just lint-text`; named in the `lint` row of `docs/ci-gates.md`. Acceptance: passes on the tree; fails with a clear message when one path number in `ic_launcher_foreground.xml` is changed (try locally, revert).
- [ ] 3.2 Final checks: `just lint` and `just test` green; report any SKIPPED task.

## 4. Owner

- [ ] 4.1 Owner: confirm the likeness of the redraw (1.1) before merge. (Background colour decided 2026-11-19: `#F2FAF5`, design §3.)

## Workflow follow-up

- Archive with `/opsx:archive`; the `distribution` delta merges into `openspec/specs/distribution/spec.md`.
- `add-dev-fdroid-repo` 1.4 then overrides only the background colour of the real icon.
