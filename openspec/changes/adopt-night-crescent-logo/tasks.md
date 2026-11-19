# Tasks

Branch `feat/night-crescent-logo`. Each implementation task is a brief for the `implementer` sub-agent. The sources are in `openspec/changes/adopt-night-crescent-logo/assets/`. Layer rule: resources in `app` and `platform`; in Kotlin only the small-icon references of `StatusNotifier` and `AlertController` change (Platform). No UI, State, Bindings or Core changes and no user copy.

## 1. Brand sources

- [ ] 1.1 **Master and docs.**
  - Goal: move the final set into `docs/brand/`:
    - `logo.svg` (replaces the #85 master), `logo-light.svg`, `wordmark*.svg`, `splash-*.svg`, `palette.md`;
    - the generator under `docs/brand/src/`, with a short `README` that names the Python dependencies (shapely, Pillow), says it is a design tool not run by CI, and that the SVGs are the source of truth;
    - the restored originals (`icon.*`, `splash.*`) stay as history.
  - Rewrite `docs/brand/README.md` per design §1 and §2: colours with contrast, the size table, the "gap must never be dropped" rule.
  - Acceptance: `just lint-text` (REUSE covers svg/png/py/md).

## 2. Android

- [ ] 2.1 **Launcher layers.**
  - Goal: `ic_launcher_foreground.xml` and `ic_launcher_monochrome.xml` as vector drawables converted from `assets/launcher-foreground.svg` and `assets/launcher-monochrome.svg` (filled paths, exact arcs kept). `ic_launcher_background.xml` = `#14202B`; keep the "launcher colour, not copy" comment.
  - Acceptance: `scripts/gradle.sh :app:lintDebug :app:assembleDebug` without vector warnings; the brand lint passes.
- [ ] 2.2 **Dev layers.**
  - Goal: `src/debug/res`:
    - a foreground with the amber DEV pill from `assets/launcher-dev.svg` (the pill text is outlined paths);
    - the background `#14202B`, replacing the mint dev colour of `add-dev-fdroid-repo`;
    - `ic_launcher_monochrome.xml` from `assets/launcher-monochrome-dev.svg` (release layer + rounded-square badge).
  - Acceptance: the debug build shows the pill. The brand lint check "debug monochrome = release + badge" passes, and it fails when the badge is removed (try, then revert).
- [ ] 2.3 **Notification icons.**
  - Goal: `platform/src/main/res/drawable/ic_stat_baby.xml` and `ic_stat_parent.xml` from `assets/notification-baby.svg` / `notification-parent.svg` (white single paths on transparent).
  - `StatusNotifier` and `AlertController` use `ic_stat_baby` when the device runs as the baby device and `ic_stat_parent` as the parent device, instead of `android.R.drawable.ic_lock_idle_alarm` / `ic_btn_speak_now` (design §3; the role is read where the notification is built).
  - Acceptance:
    - a JVM test asserts the small icon per role for the monitoring and the alert notification;
    - `git grep -n "android.R.drawable" apps/android/platform` lists no small icon;
    - `just verify-android` green. If a Roborazzi baseline changes, re-record it only in the pinned image and say so.

## 3. Web

- [ ] 3.1 **Favicons and PWA.**
  - Goal (design §5):
    - `apps/web/public/favicon.svg` = the 32 px glyph (`assets/favicon.svg`);
    - new `favicon-16.svg` = crescent only on the night tile, derived from the notification crescent geometry;
    - `index.html` links both (`sizes="16x16"` for the small one);
    - `pwa-assets.config.ts` background `#14202B`, also for `apple-touch-icon` 180 px.
  - Acceptance: `just build-web` emits the icons, and the manifest lists 192, 512 and maskable 512 on the night tile; `just test`.

## 4. Lint and checks

- [ ] 4.1 **Brand lint.**
  - Goal: extend `scripts/lib/brand-icons.mjs` and its tests per design §4. Update the `text-lint` row in `docs/ci-gates.md`.
  - Acceptance:
    - `node --test scripts/lib/*.test.mjs` and `just lint-text` green;
    - negative checks fail with clear messages, then revert: change one path number in the foreground, drop the gap subpath in the monochrome layer, add a colour to a notification drawable.
- [ ] 4.2 **Final checks.** `just lint` and `just test` green; report any SKIPPED task.

## 5. Owner

- [ ] 5.1 Owner: install the debug build on a phone (`just install-android --build`). Check the launcher on a dark and a light wallpaper, the themed icon with the dev badge, and the baby/parent notification icons in the status bar.

## Workflow follow-up

- Archive with `/opsx:archive` after 5.1. The `distribution` delta merges into `openspec/specs/distribution/spec.md`.
- `add-dev-fdroid-repo` design §9 mentions the mint dev colour; update it there, or let this change's archive note supersede it.
