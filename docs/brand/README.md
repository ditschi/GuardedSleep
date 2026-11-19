# Brand assets

Master files of the GuardedSleep logo, restored from the legacy Capacitor client (`apps/mobile/resources/`, removed in wave 1 with commit `88a7ff49`). Use of the name and logo is governed by the [trademark policy](../legal/trademark.md).

| File         | Content                                                                        |
| ------------ | ------------------------------------------------------------------------------ |
| `logo.svg`   | **Master.** Clean redraw of the logo, 108 × 108 viewBox (adaptive-icon canvas) |
| `icon.svg`   | Logo (crescent, dot and sound wave), 1024 × 1024 viewBox (Inkscape)            |
| `icon.png`   | The same logo as a 1024 × 1024 PNG on white                                    |
| `splash.svg` | Logo with the wordmark "GuardedSleep" and "Privacy-First Baby Monitor"         |
| `splash.png` | The same splash as a 2732 × 2732 PNG                                           |

`logo.svg` is the single vector master for every icon (`use-brand-launcher-icon`). It is a redraw of `icon.png` with four shapes (crescent, crescent body, dot, wave) and lies entirely inside the 66 dp safe circle centred at 54,54 of the Android adaptive-icon canvas. `icon.svg` and `splash.svg` are traced bitmaps (many small paths): historical sources, not drop-in vector drawables.

Colours (sampled from `icon.png`, listed in the comment of `logo.svg`): wave `#255176`; crescent rim `#81E5A7` to `#6ED79C`; crescent body `#78DCA0`, `#50A774` to `#317954`; dot `#78DBA2` to `#408C65`. Background of the launcher and PWA icons: light mint `#F2FAF5` (owner decision 2026-11-19).

Derived icons:

- Android: `ic_launcher_foreground.xml` and `ic_launcher_monochrome.xml` carry the master's path data, `ic_launcher_background.xml` the background colour. `scripts/lint-brand-icons.mjs` (in `just lint-text`) fails when they drift from `logo.svg`.
- Web: `apps/web/public/favicon.svg` is a byte-identical copy of `logo.svg`; the PWA PNGs are generated at build time from it (never committed).
