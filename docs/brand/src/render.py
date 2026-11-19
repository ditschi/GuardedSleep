# Renders every preview PNG of the A2b+ set from the SVGs in OUT.
import os
from PIL import Image, ImageDraw
from rend import *
from brand import NIGHT, MINT

OUT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
P = lambda n: os.path.join(OUT, n)

ink(P("logo.svg"), 1024, out=P("logo-1024.png"))
ink(P("logo-light.svg"), 1024, out=P("logo-light-1024.png"))

L512c = launcher(P("logo.svg"), 512, "circle"); L512c.save(P("launcher-circle-512.png"))
launcher(P("logo.svg"), 512, "squircle").save(P("launcher-squircle-512.png"))
L48 = launcher(P("logo.svg"), 48, "circle")
L48q = launcher(P("logo.svg"), 48, "squircle")

# home-screen strips at 48 px: dark, light, and each again under a warm blue-light filter
def strip_pair(kind):
    a = home_strip(L48, kind, 48)
    b = home_strip(L48q, kind, 48)
    im = Image.new("RGBA", (a.width, a.height * 2))
    im.alpha_composite(a, (0, 0)); im.alpha_composite(b, (0, a.height))
    return im

for kind in ("dark", "light"):
    s = strip_pair(kind)
    s.save(P(f"home-strip-48-{kind}.png"))
    warm_tint(s).save(P(f"home-strip-48-{kind}-nightlight.png"))

# themed icons (Android 13+): monochrome alpha, tinted
def themed(bg, fg, svgname, n=512):

    big = max(n * 3, 768)
    canvas = ink(P(svgname), big * 108 // 72)
    m = (canvas.width - big) // 2
    alpha = canvas.crop((m, m, m + big, m + big)).resize((n, n), Image.LANCZOS).split()[3]
    t = Image.new("RGBA", (n, n), bg)
    t.paste(Image.new("RGBA", (n, n), fg), (0, 0), alpha)
    t.putalpha(mask(n, "circle"))
    return t

TL = ("#D3E8DD", "#1E4537"); TD = ("#1C2925", "#A8D5C2")
themed(*TL, "launcher-monochrome.svg").save(P("themed-512-light.png"))
themed(*TD, "launcher-monochrome.svg").save(P("themed-512-dark.png"))
themed(*TL, "launcher-monochrome-dev.svg").save(P("themed-dev-512-light.png"))
themed(*TD, "launcher-monochrome-dev.svg").save(P("themed-dev-512-dark.png"))

# dev variant
launcher(P("logo-dev.svg"), 512, "circle").save(P("dev-variant.png"))

# notification preview: 24/48/96 on a dark status bar strip
def notif_strip():
    H = 140
    im = Image.new("RGBA", (620, H), (30, 31, 36, 255))
    x = 18
    for name in ("notification-baby.svg", "notification-parent.svg"):
        for n in (24, 48, 96):
            g = ink(P(name), n)
            im.alpha_composite(g, (x, (H - n) // 2 - 6))
            x += n + 18
        x += 30
    d = ImageDraw.Draw(im)
    d.text((18, H - 18), "baby: crescent", font=font(12), fill=(200, 200, 210))
    d.text((330, H - 18), "parent: crescent + listening bars", font=font(12), fill=(200, 200, 210))
    # a realistic status bar row at 24 px with neighbours
    bar = Image.new("RGBA", (620, 34), (16, 16, 20, 255))
    db = ImageDraw.Draw(bar)
    db.text((10, 9), "22:41", font=font(14), fill=(235, 235, 240))
    bar.alpha_composite(ink(P("notification-baby.svg"), 24), (64, 5))
    bar.alpha_composite(ink(P("notification-parent.svg"), 24), (96, 5))
    db.rectangle((128 + 4, 11, 128 + 18, 23), outline=(235, 235, 240), width=2)  # generic msg glyph
    db.rectangle((560, 11, 590, 23), outline=(235, 235, 240), width=2); db.rectangle((563, 14, 580, 20), fill=(235, 235, 240))
    out = Image.new("RGBA", (620, H + 34)); out.alpha_composite(bar, (0, 0)); out.alpha_composite(im, (0, 34))
    return out

notif_strip().save(P("notification-preview.png"))

# favicon set
f32 = ink(P("favicon.svg"), 32); f32.save(P("favicon-32.png"))
f16 = ink(P("favicon-16.svg"), 16); f16.save(P("favicon-16.png"))
ink(P("apple-touch-icon.svg"), 180, out=P("favicon-180.png"))
pwa = ink(P("pwa-maskable.svg"), 512); pwa.save(P("pwa-512-maskable.png"))

# maskable preview: full / circle / squircle / rounded with the 80 % safe zone ring
def maskable_preview():
    n = 256
    base = pwa.resize((n, n), Image.LANCZOS)
    im = Image.new("RGBA", (4 * (n + 24) + 24, n + 60), (233, 236, 240, 255))
    for i, kind in enumerate((None, "circle", "squircle", "rounded")):
        t = base.copy()
        if kind:
            t.putalpha(mask(n, kind))
        else:
            d = ImageDraw.Draw(t)
            r = n * 0.4
            d.ellipse((n / 2 - r, n / 2 - r, n / 2 + r, n / 2 + r), outline=(255, 80, 80, 255), width=2)
        im.alpha_composite(t, (24 + i * (n + 24), 16))
        label(im, kind or "full bleed + 80 % safe zone", (24 + i * (n + 24), n + 26), 14)
    return im

maskable_preview().save(P("pwa-512-maskable-preview.png"))

# favicon in a browser-tab-like strip (1x and 4x)
def fav_strip():
    im = Image.new("RGBA", (520, 120), (233, 236, 240, 255))
    for i, (bg, fg) in enumerate((((222, 225, 230), (30, 30, 30)), ((53, 54, 58), (230, 230, 230)))):
        y = 10 + i * 52
        ImageDraw.Draw(im).rounded_rectangle((10, y, 250, y + 40), 8, fill=bg)
        im.alpha_composite(f16, (22, y + 12))
        ImageDraw.Draw(im).text((46, y + 12), "GuardedSleep", font=font(14), fill=fg)
    im.alpha_composite(f16.resize((64, 64), Image.NEAREST), (270, 28))
    im.alpha_composite(f32.resize((64, 64), Image.NEAREST), (350, 28))
    im.alpha_composite(f32, (430, 44))
    label(im, "16 @4x", (270, 96), 12); label(im, "32 @2x", (350, 96), 12); label(im, "32", (430, 96), 12)
    return im

fav_strip().save(P("favicon-preview.png"))

# wordmarks and splashes
ink(P("wordmark-paths.svg"), 1080, 320, out=P("wordmark.png"))
ink(P("wordmark-dark-paths.svg"), 1080, 320, out=P("wordmark-dark.png"))
ink(P("splash-dark.svg"), 1080, 1920, out=P("splash-dark.png"))
ink(P("splash-light.svg"), 1080, 1920, out=P("splash-light.png"))
print("rendered")
