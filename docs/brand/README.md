# Brand assets

Master files of the GuardedSleep logo, restored from the legacy Capacitor client (`apps/mobile/resources/`, removed in wave 1 with commit `88a7ff49`). Use of the name and logo is governed by the [trademark policy](../legal/trademark.md).

| File         | Content                                                                |
| ------------ | ---------------------------------------------------------------------- |
| `icon.svg`   | Logo (crescent, dot and sound wave), 1024 × 1024 viewBox (Inkscape)    |
| `icon.png`   | The same logo as a 1024 × 1024 PNG on white                            |
| `splash.svg` | Logo with the wordmark "GuardedSleep" and "Privacy-First Baby Monitor" |
| `splash.png` | The same splash as a 2732 × 2732 PNG                                   |

The SVGs are traced bitmaps (many small paths), so they are sources, not drop-in Android vector drawables. The web client already uses the logo (`apps/web/public/favicon.svg`, `pwa-*.png`); the native Android launcher still shows a placeholder crescent.
