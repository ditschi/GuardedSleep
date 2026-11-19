# GuardedSleep logo A2b+ palette

Contrast ratios are WCAG 2.x relative-luminance ratios, computed by `src/col.py`.
Graphical objects need 3:1 against what they sit on (WCAG 1.4.11). Text: 4.5:1, or 3:1 for large text.

## Core (night, the default)

| Token     | Hex       | Role                                                                | Contrast                                        |
| --------- | --------- | ------------------------------------------------------------------- | ----------------------------------------------- |
| night     | `#14202B` | launcher background, dark tile, favicon tile, splash-dark, DEV text | white 16.52, mint 15.55                         |
| sage      | `#5DBFA6` | crescent on night; "Sleep" in dark wordmark                         | on night **7.45**                               |
| cream     | `#F6D79A` | breath wave on night (warmed from `#F7E0AE`)                        | on night **11.88**; vs sage **1.60** (see note) |
| snow      | `#E8EFF5` | "Guarded" on night                                                  | on night 14.24                                  |
| mist-sub  | `#8FA3B3` | tagline on night                                                    | on night 6.34                                   |
| dev-amber | `#F2A65A` | DEV pill (dev builds only)                                          | night text on it 8.16; on night 8.16            |

Cream choice: `#F6D79A` rather than `#F5D594`. They are almost the same hue. The lighter one keeps a little
more luminance difference from sage (1.60 vs 1.57) and more contrast on night (11.88 vs 11.66).
It is noticeably warmer than A2b's `#F7E0AE` (1.71 vs sage) and stays clearly away from gold `#F5C76E` (1.40).

**Note on cream vs sage (1.60):** the two marks do not separate by luminance. They separate by hue
(yellow-cream vs blue-green, about 125 degrees apart) and by the 2 dp night gap that always lies between them.
At 48 px the gap is about 1.3 px. Under a warm blue-light filter (about 3000 K, simulated in
`home-strip-48-*-nightlight.png`) the cream turns orange and the sage turns grey-green, so the hue split stays.
For full colour-blind safety (deuteranopia), the gap does the work; it is never omitted.

## Light surfaces (docs, store graphics, wordmark.svg, splash-light)

| Token   | Hex       | Role                                                    | Contrast                                                        |
| ------- | --------- | ------------------------------------------------------- | --------------------------------------------------------------- |
| mint    | `#F2FAF5` | light tile / splash-light background                    |                                                                 |
| sage-d  | `#3A9A84` | crescent on light; "Sleep" on light                     | on mint **3.22** (large text / graphics only)                   |
| amber-d | `#B8741F` | breath wave on light (warm, keeps the "warm wave" idea) | on mint **3.56**; vs sage-d 1.10 (hue + mint gap separate them) |
| ink     | `#1E3A55` | "Guarded" on light                                      | on mint 11.03                                                   |
| ink-sub | `#4F6476` | tagline on light                                        | on mint 5.79                                                    |

The cream wave is **not** used on light surfaces (about 1.2:1 on mint). The light mark swaps it for amber-d.

## Themed icon preview tints (example only; Android picks these from the wallpaper)

| Mode  | Background | Foreground | Contrast |
| ----- | ---------- | ---------- | -------- |
| light | `#D3E8DD`  | `#1E4537`  | 8.34     |
| dark  | `#1C2925`  | `#A8D5C2`  | 9.30     |

## Notification glyphs

Pure white `#FFFFFF` on transparent; Android tints them itself.

## Not used

No gradients, masks or strokes in any final SVG. Monochrome layers use `#000` fill; only alpha counts.
