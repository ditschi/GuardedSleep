# Brand assets

The GuardedSleep logo, the "night crescent" mark (`adopt-night-crescent-logo`, successor of `use-brand-launcher-icon`). Use of the name and logo is governed by the [trademark policy](../legal/trademark.md).

The SVG files here are the **source of truth**. The Python generator in [`src/`](src/README.md) produced them and is only kept for later edits; nothing in CI or `just` runs it.

## The mark

A sage crescent, a warm cream breath wave running low across it and 3 dp past its outline, and a dark gap between the two, on the night tile. No dot. Everything lies inside the 66 dp safe circle around (54, 54) of the 108 dp Android adaptive-icon canvas (farthest point 32.0 dp). The shapes are filled outlines only: no strokes, masks or gradients, so each converts 1:1 to a VectorDrawable.

| Element    | Value                                                                           |
| ---------- | ------------------------------------------------------------------------------- |
| Night tile | `#14202B`                                                                       |
| Crescent   | sage `#5DBFA6`, outer circle r 29 at (54, 54), bite circle r 19 at (66.5, 41.5) |
| Wave       | cream `#F6D79A`, 5.6 dp, centre y about 66, calm small-big-small                |
| Gap        | 2 dp of night between wave and crescent (2.75 dp in the monochrome layer)       |

Contrast (full table in [`palette.md`](palette.md)): cream on night 11.9:1, sage on night 7.4:1, but cream against sage only 1.6:1. The two are separated by hue and by the gap alone. **The gap must never be dropped**: for red-green colour blindness it is the only separation. `scripts/lib/brand-icons.mjs` checks that it exists.

The light variant (`logo-light.svg`, wordmark, light splash) uses deep sage `#3A9A84` and amber `#B8741F` on mint `#F2FAF5`. The cream wave is not used on light surfaces.

## Which glyph where

| Use                             | Glyph                                                           | File                                        |
| ------------------------------- | --------------------------------------------------------------- | ------------------------------------------- |
| Launcher, PWA, favicon 32 px up | full mark on the night tile                                     | `logo.svg`, `launcher-*.svg`, `favicon.svg` |
| Favicon 16 px                   | crescent only (the bite and horn merge with the wave at 16 px)  | `favicon-16.svg`                            |
| Notification, baby device       | crescent, white, 24 dp                                          | `notification-baby.svg`                     |
| Notification, parent device     | smaller crescent and two listening bars, white                  | `notification-parent.svg`                   |
| Dev launcher                    | full mark and an amber DEV pill in the bite                     | `launcher-dev.svg`, `logo-dev.svg`          |
| Dev themed icon                 | monochrome layer and a rounded-square badge                     | `launcher-monochrome-dev.svg`               |
| Splash, wordmark                | mark with "GuardedSleep" (Fira Sans, outlined in `*-paths.svg`) | `splash-*.svg`, `wordmark*.svg`             |

## Files

| File                                                                        | Content                                                                                                                                         |
| --------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `logo.svg`                                                                  | **Master.** Night tile and mark, 108 x 108 viewBox (adaptive-icon canvas)                                                                       |
| `logo-light.svg`                                                            | Light-surface variant                                                                                                                           |
| `logo-dev.svg`                                                              | Master plus the DEV pill                                                                                                                        |
| `launcher-*.svg`                                                            | Adaptive-icon layers (background, foreground, monochrome, dev variants)                                                                         |
| `notification-*.svg`                                                        | 24 dp small icons, pure white on transparent                                                                                                    |
| `favicon.svg`, `favicon-16.svg`, `apple-touch-icon.svg`, `pwa-maskable.svg` | Web icon sources                                                                                                                                |
| `wordmark*.svg`, `splash-*.svg`                                             | Lockups and splash sources                                                                                                                      |
| `palette.md`                                                                | Colours with contrast ratios                                                                                                                    |
| `src/`                                                                      | Generator (design tool, not run by CI)                                                                                                          |
| `icon.svg`, `icon.png`, `splash.svg`, `splash.png`                          | The earlier logo, restored from the legacy Capacitor client (`apps/mobile/resources/`, removed in wave 1 with commit `88a7ff49`). History only. |

## Derived icons

- Android: `ic_launcher_foreground.xml` and `ic_launcher_monochrome.xml` carry the path data of `launcher-foreground.svg` and `launcher-monochrome.svg`, `ic_launcher_background.xml` the night colour. The debug build adds the DEV pill and the badge. The notification icons are `ic_stat_baby.xml` and `ic_stat_parent.xml` in the `platform` module. `scripts/lib/brand-icons.mjs` (in `just lint-text`) fails when they drift from the SVGs here.
- Web: `apps/web/public/favicon.svg` and `favicon-16.svg` are byte-identical copies of the files here. The PWA and apple-touch PNGs are generated at build time on the night tile (never committed).
