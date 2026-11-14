# Plus Jakarta Sans (bundled font)

Plus Jakarta Sans is the typeface of both clients (`redesign-calm-ui` design section 10). It is
licensed under the SIL Open Font License 1.1 (`LICENSES/OFL-1.1.txt`; the upstream `OFL.txt` below is
the same text with the copyright line and is not copied here because of its trailing whitespace).
Copyright 2020 The Plus Jakarta Sans Project Authors (https://github.com/tokotype/PlusJakartaSans).
The upstream file declares no Reserved Font Name. The shipped files are subsets and instances of the
upstream file, never sold on their own.

Fonts are served from the app's own origin (web) or bundled in the APK (Android). Nothing is loaded
from Google at runtime.

## Upstream (pinned)

Official source: the Google Fonts repository, `ofl/plusjakartasans/`, at commit
`3e5667764ccfea32ce0c3d58a3a4756de890adc1` (2026-11-13).

| Upstream file               | URL                                                                                                                                       | sha256                                                             |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| `PlusJakartaSans[wght].ttf` | https://raw.githubusercontent.com/google/fonts/3e5667764ccfea32ce0c3d58a3a4756de890adc1/ofl/plusjakartasans/PlusJakartaSans%5Bwght%5D.ttf | `89b3fb38aa0d275d7a731d0d817a4f1622b316b4d7fbdedcf02ee9099ff68bc8` |
| `OFL.txt`                   | https://raw.githubusercontent.com/google/fonts/3e5667764ccfea32ce0c3d58a3a4756de890adc1/ofl/plusjakartasans/OFL.txt                       | `995c7199cab65954f545996326755daee7b63cc6b42b06c13da1f9502ab08a99` |

## Shipped files (derived)

| File                                                                | sha256                                                             |
| ------------------------------------------------------------------- | ------------------------------------------------------------------ |
| `apps/web/src/assets/fonts/plus-jakarta-sans-latin.woff2`           | `49061f342942698099d1c6eba5fccbadb3eb9e1916ebd2921348e1c7d990bc2e` |
| `apps/android/app/src/main/res/font/plus_jakarta_sans_regular.ttf`  | `6a6bcab056de0fdb2c7dfc72c1e01e8b2195ea200fcb5251e94493d5da0147a7` |
| `apps/android/app/src/main/res/font/plus_jakarta_sans_medium.ttf`   | `2b25d6d0b5c825b9d69bf8d00a41fc813c68dc2d2fb9ec9fc02848ed36bc313a` |
| `apps/android/app/src/main/res/font/plus_jakarta_sans_semibold.ttf` | `cbf55e251945b1e072c3caa2c26d38ed9522a7976ee58d25bcbd666bfa261267` |
| `apps/android/app/src/main/res/font/plus_jakarta_sans_bold.ttf`     | `ac9569437615fb0b1f7b675a62fb11e3d25b4a78f305d4f5bf818b2bc663eaf8` |

Android uses four static instances (400, 500, 600, 700). The web uses one variable font limited to
the 400 to 700 weight range.

## How they were made

fonttools via `uvx` (https://fonttools.readthedocs.io/). With `U` and `F` below:

```sh
U="U+0020-007E,U+00A0-024F,U+1E00-1EFF,U+2000-206F,U+20AC,U+2122,U+2190-2193,U+2212,U+2215,U+FEFF"
F="kern,liga,ccmp,locl,mark,mkmk,calt,case,tnum,pnum"
# web: weight range 400-700, subset, woff2
uvx --with brotli --from fonttools fonttools varLib.instancer "PlusJakartaSans[wght].ttf" wght=400:700 -o lim.ttf
uvx --with brotli --from fonttools pyftsubset lim.ttf --unicodes="$U" --layout-features="$F" \
  --flavor=woff2 --output-file=plus-jakarta-sans-latin.woff2
# android: one static instance per weight (400 regular, 500 medium, 600 semibold, 700 bold), subset
uvx --from fonttools fonttools varLib.instancer "PlusJakartaSans[wght].ttf" wght=400 --update-name-table -o i400.ttf
uvx --from fonttools pyftsubset i400.ttf --unicodes="$U" --layout-features="$F" \
  --output-file=plus_jakarta_sans_regular.ttf
```

## Coverage

The subset keeps Basic Latin, Latin-1, Latin Extended-A/B, Latin Extended Additional (Vietnamese),
general punctuation, the euro sign, the trade mark sign, arrows and the minus sign. That covers
every Latin-script locale of `locales/`.

**Not covered by the typeface itself** (checked against the upstream cmap, 721 code points, no
Cyrillic, Arabic, Devanagari, CJK or Hangul): `uk` (Cyrillic), `ar`, `hi`, `ja`, `ko`, `zh-Hans`.
These locales, and the endonym list of the language picker, fall back to the system font through
the `font-family` stack (web) and the platform fallback (Android). Their digits still use the
bundled font where it has them.
