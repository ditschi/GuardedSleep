# Design

## Context

How the mark was found (2026-11-19, multi-agent "logo lab"):

1. **Critique:** three experts reviewed the #85 logo through the brand, platform-icon and night-UX lenses. They found a spiky 9-hump wave, gradients, a dot touching the wave, no small-size glyphs, glare of the mint tile at night and closeness to the `--gs-ok` status green.
2. **Candidates** (contact sheets in the logo-lab output; the final set is in `assets/`):
   - **A** "Refined Crescent": a redraw of the current mark.
   - **B** "Shelter Moon": an arch over a crescent.
   - **C** "Cradle".
   - **D** "Sheltered Link".
3. **Jury round 1:** three judges (parent UX, brand, platform). Averages: A 7.8, B 7.3, D 6.2, C 5.0. Their favourites were A, B and A, with low confidence (0.60–0.65).
4. **Discussion, combination E** ("Hooded Moon": A's crescent under B's hood), then **round 2:** all three judges chose A. All three judged E **worse**: the moon shrank to 46 %, and the open hood reads as a loading spinner or power symbol. A second combination of A and E (F1, F2) did not beat A either: F1 is barely different, and F2's canopy dominates and its silhouette reads as a face with a moustache.
5. **Dot study** (owner: "the dot has no meaning for me"): all three judges said remove it (A2). It reads as noise or a star at 48 px, smudges the 24 px glyph, and in the themed icon it becomes an eye or a generic star-and-crescent.
6. **Wave study** (owner: "A2 is a bit boring"): all three judges chose a bolder warm cream wave, low on the moon (A2b). A centred wave splits the moon into a horn and a bowl, which reads as a cup or the sea. Gold is too close to Android's warning and battery amber.
7. **Final A2b+:** the jury's fixes, plus a thinner wave on the owner's request (5.6 dp instead of 6.6 dp; the weight study in `assets/weight-study.png` shows it still reads at 48 px).

Owner decisions: candidate A; the dark night tile as default; no dot; warm wave; 5.6 dp; crescent-only favicon at 16 px; rounded-square dev badge in themed icons.

Layers: resources in `app` (launcher) and `platform` (notification drawables); `StatusNotifier` and `AlertController` (Platform) change only their small-icon references. UI, State, Bindings and Core are not touched, and there is no user copy. Privacy guardrails: unaffected.

## 1. The mark (`assets/logo.svg`)

| Element       | Value                                                                                                   |
| ------------- | ------------------------------------------------------------------------------------------------------- |
| Canvas        | 108 × 108 dp, everything inside the 66 dp circle around (54, 54); farthest point 32.0 dp                |
| Night tile    | `#14202B`                                                                                               |
| Crescent      | sage `#5DBFA6`, outer circle r 29 at (54, 54), bite circle r 19 at (66.5, 41.5)                         |
| Wave          | cream `#F6D79A`, calm small-big-small, 5.6 dp, low (centre y ≈ 66), 3 dp past the moon outline per side |
| Gap           | 2 dp night between wave and crescent in colour; 2.75 dp in the monochrome layer                         |
| Construction  | filled outlines only (no stroke, mask or gradient); exact arcs on circle edges                          |
| Light variant | deep sage `#3A9A84`, amber `#B8741F` on mint `#F2FAF5` (wordmark and light splash only)                 |

Contrast (`assets/palette.md`): cream on night 11.9:1, sage on night 7.4:1; cream against sage is only 1.6:1. **They are separated by hue and the gap.** The gap must never be dropped, because for red-green colour blindness it is the only separation. The lint checks that it exists.

## 2. Size-specific glyphs

| Use                         | Glyph                                               | Why                                            |
| --------------------------- | --------------------------------------------------- | ---------------------------------------------- |
| Launcher, PWA, ≥ 32 px      | full mark on the night tile                         |                                                |
| Favicon 16 px               | crescent only (owner)                               | at 16 px the bite and horn merge with the wave |
| Notification, baby device   | crescent, white, 24 dp                              | a wave at 24 dp turns muddy (tested)           |
| Notification, parent device | smaller crescent + 2 listening bars, ≥ 1.5 dp clear | distinct from the baby glyph at 24 dp          |
| Dev launcher                | full mark + amber DEV pill in the bite              | the pill, not the text, is the signal at 48 px |
| Dev themed                  | monochrome layer + rounded-square badge             | themed icons ignore colour (owner, 2026-11-19) |

`favicon-16.svg` in `assets/` still draws the full mark. The implementation derives the crescent-only 16 px glyph from the notification crescent geometry on the night tile.

## 3. Notification icons in the platform

`StatusNotifier` (monitoring status) and `AlertController` (alerts, talk-back) choose the small icon by the device's current role: baby → `ic_stat_baby`, parent → `ic_stat_parent`. Both are vector drawables, white single paths on transparent (Android requires an alpha-only small icon). Where a notification is not tied to a role (if any), it uses the baby crescent as the brand default. The role is already known where the notification is built. No new state is needed.

- **Option A — two role glyphs (recommended).**
  - Pros: tells at a glance in the shade which device this is; the parent glyph matches the listening role.
  - Cons: two drawables and a role branch in two classes.
- **Option B — one crescent everywhere.**
  - Pros: simplest.
  - Cons: no role cue; parents with two GuardedSleep devices nearby (baby and parent on one account) cannot tell them apart.

Recommendation: A.

## 4. Lint (`scripts/lib/brand-icons.mjs`)

Extends today's checks:

- the colour and monochrome launcher layers match `docs/brand/logo.svg` (paths, fills, the gap subpath);
- the debug monochrome layer equals the release layer plus at least one badge path (already checked);
- `launcher-dev` equals the release foreground plus the pill;
- the notification drawables are single white paths with no other colours;
- `favicon.svg` equals the 32 px glyph source, and `favicon-16.svg` exists;
- the night-tile colour is the same in the Android background, the PWA config and the master.

The tests in `scripts/lib/brand-icons.test.mjs` cover the new parsers.

## 5. Web

- `favicon.svg` is the 32 px glyph (wave drawn slightly heavier for small sizes, as in `assets/favicon.svg`).
- `favicon-16.svg` is the crescent only.
- `apple-touch-icon` (180 px) and the PWA icons come from `pwa-assets.config.ts` with the night tile as background (today: mint).
- `index.html` links the 16 px icon with `sizes="16x16"` and the SVG as the default icon.
- The PWA `theme_color` stays as it is (out of scope).

## Risks

- **Hue-only contrast** between cream and sage (see §1); mitigated by the mandatory gap and the lint.
- **The dark tile almost disappears on a dark wallpaper.** The mark carries the icon alone, which is normal for dark icons; the owner accepted it.
- **Notification baselines:** if a Roborazzi test renders a notification, its baseline changes. Re-record in the pinned image, never by hand.
- **Generator dependency:** the Python generator uses shapely and Pillow from a scratch venv. It is a design tool and runs neither in CI nor in `just`. The committed SVGs are the source of truth, and the generator is kept for later edits (`docs/brand/src/README`).

## Discarded alternatives

- **Candidates B, C and D** (round 1 and 2 scores above); **E** (hood shrinks the moon, spinner reading); **F1 and F2** (no gain over A, face reading).
- **Keeping the dot**: no meaning, clutter at small sizes, face/eye reading in themed icons.
- **Dynamic wave**: reads as "noise detected", the alert state, and jitters at 48 px.
- **Centred wave**: splits the moon into a horn and a bowl.
- **Gold wave**: too close to warning and battery amber.
- **Mint tile as default** (decision of `use-brand-launcher-icon`): glare at night. The light variant stays for the wordmark and the light splash.
- **One notification icon for every role** (§3 B).
