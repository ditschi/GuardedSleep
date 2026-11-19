# One contact sheet (2400x1600) of the whole A2b+ set.
import os
from PIL import Image, ImageDraw
from rend import font, ink, launcher, label
from brand import *

OUT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
P = lambda n: os.path.join(OUT, n)
S = Image.new("RGBA", (2400, 1600), (233, 236, 240, 255))
D = ImageDraw.Draw(S)


def put(name_or_im, x, y, w=None, h=None, cap=None, nearest=False):
    im = name_or_im if isinstance(name_or_im, Image.Image) else Image.open(P(name_or_im)).convert("RGBA")
    if w and not h:
        h = round(im.height * w / im.width)
    if w:
        im = im.resize((w, h), Image.NEAREST if nearest else Image.LANCZOS)
    S.alpha_composite(im, (x, y))
    if cap:
        label(S, cap, (x, y + im.height + 6), 15)
    return im


label(S, "GuardedSleep logo A2b+  —  final set", (30, 18), 30, bold=True)
label(S, f"night {NIGHT} · sage {SAGE} · cream {CREAM} · wave 5.6 dp, 2 dp gap (2.75 dp themed) · wave 3 dp past the moon",
      (720, 30), 17, (70, 80, 92))

# column 1: master
put("logo-1024.png", 30, 80, 460, cap="logo.svg (108 dp, night tile)")
put("logo-light-1024.png", 30, 590, 220, cap="logo-light.svg")
mono = Image.new("RGBA", (220, 220), (40, 42, 48, 255))
m = ink(P("launcher-monochrome.svg"), 220)
white_layer = Image.new("RGBA", m.size, (210, 225, 218, 255)); white_layer.putalpha(m.split()[3])
mono.alpha_composite(white_layer)
put(mono, 270, 590, cap="launcher-monochrome.svg")

# column 2: launcher
put("launcher-circle-512.png", 530, 80, 290, cap="launcher-circle-512")
put("launcher-squircle-512.png", 840, 80, 290, cap="launcher-squircle-512")
put("home-strip-48-dark.png", 530, 410, 440, cap=None)
put("home-strip-48-light.png", 530, 616, 440, cap="home-strip-48 dark / light (circle + squircle)")
put("home-strip-48-dark-nightlight.png", 1030, 410, 240, cap=None)
put("home-strip-48-light-nightlight.png", 1030, 530, 240, cap="… under a warm blue-light filter")

# column 3: themed + dev
put("themed-512-light.png", 1300, 80, 200, cap="themed light")
put("themed-512-dark.png", 1520, 80, 200, cap="themed dark")
put("dev-variant.png", 1300, 320, 200, cap="dev-variant")
put("themed-dev-512-dark.png", 1520, 320, 200, cap="themed dev badge")

# column 4: wordmark
put("wordmark.png", 1750, 80, 620, cap=None)
put("wordmark-dark.png", 1750, 274, 620, cap="wordmark(-dark).svg + *-paths.svg (Fira Sans, outlined)")

# notification + favicon
put("notification-preview.png", 1750, 500, 620, cap="notification-baby / -parent  (24 / 48 / 96 px)")
put("favicon-preview.png", 1300, 590, 430, cap="favicon-16 / -32 (tab, 1:1 and zoomed)")
put("favicon-180.png", 1300, 735, 120, cap="favicon-180")
put("pwa-512-maskable.png", 1440, 735, 120, cap="pwa-512-maskable")

# row 2: splashes, maskable preview, size ladder
put("splash-dark.png", 30, 880, 360, 640)
label(S, "splash-dark (default) / splash-light, 1080x1920", (30, 1528), 15)
put("splash-light.png", 400, 880, 360, 640)

put("pwa-512-maskable-preview.png", 790, 900, 900, cap="PWA maskable: full bleed with 80 % safe zone, circle, squircle, rounded")

# size ladder of the launcher on night + light surround
lad = Image.new("RGBA", (1580, 300), (0, 0, 0, 0))
ld = ImageDraw.Draw(lad)
ld.rectangle((0, 0, 1580, 150), fill=(18, 18, 24, 255)); ld.rectangle((0, 150, 1580, 300), fill=(244, 244, 240, 255))
x = 20
for n in (24, 32, 48, 64, 96, 128):
    ic = launcher(P("logo.svg"), n)
    lad.alpha_composite(ic, (x, 75 - n // 2)); lad.alpha_composite(ic, (x, 225 - n // 2))
    ld.text((x, 133), f"{n}", font=font(12), fill=(200, 200, 210))
    x += n + 40
for i, name in enumerate(("favicon-16.png", "favicon-32.png")):
    im = Image.open(P(name)).convert("RGBA")
    lad.alpha_composite(im, (x + i * 60, 75 - im.height // 2)); lad.alpha_composite(im, (x + i * 60, 225 - im.height // 2))
    ld.text((x + i * 60, 133), name[8:10], font=font(12), fill=(200, 200, 210))
x += 140
# palette swatches
for i, (hx, nm) in enumerate(((NIGHT, "night"), (SAGE, "sage"), (CREAM, "cream"), (MINT, "mint"),
                              (SAGE_D, "sage-d"), (AMBER_D, "amber-d"), (DEV, "dev"))):
    xx = x + i * 92
    ld.rounded_rectangle((xx, 160, xx + 80, 240), 10, fill=hx, outline=(150, 150, 150))
    ld.text((xx, 246), nm, font=font(13), fill=(40, 40, 50)); ld.text((xx, 264), hx, font=font(12), fill=(80, 80, 90))
put(lad, 790, 1250, cap="launcher size ladder (24–128 px, circle) and favicons on dark / light · palette (palette.md)")
S.convert("RGB").save(P("sheet.png"))
print("sheet")
