# GuardedSleep logo A2b+ final set: writes every SVG into OUT (one level up).
import math, os, subprocess
from brand import *

OUT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
SW = 5.6            # final wave weight (dp), see weight-study.png


def W(name, s):
    open(os.path.join(OUT, name), "w").write(s)


def text_to_paths(src, dst):
    subprocess.run(["inkscape", os.path.join(OUT, src), "--export-text-to-path", "--export-plain-svg",
                    f"--export-filename={os.path.join(OUT, dst)}"], check=True, capture_output=True, timeout=120)


def crescent(oc, orr, ic, ir, f=lambda v: f"{v:.2f}"):
    (ox, oy), (in_x, in_y) = oc, ic
    d = math.hypot(in_x - ox, in_y - oy)
    a = (orr * orr - ir * ir + d * d) / (2 * d); h = math.sqrt(orr * orr - a * a)
    ux, uy = (in_x - ox) / d, (in_y - oy) / d
    px, py = ox + a * ux, oy + a * uy
    p1 = (px - h * uy, py + h * ux); p2 = (px + h * uy, py - h * ux)
    return f"M{f(p1[0])},{f(p1[1])}A{f(orr)},{f(orr)} 0 1 1 {f(p2[0])},{f(p2[1])}A{f(ir)},{f(ir)} 0 1 0 {f(p1[0])},{f(p1[1])}Z"


def rrect(x, y, w, h, r):
    r = min(r, w / 2, h / 2)
    return (f"M{x + r:.2f},{y:.2f}H{x + w - r:.2f}A{r:.2f},{r:.2f} 0 0 1 {x + w:.2f},{y + r:.2f}"
            f"V{y + h - r:.2f}A{r:.2f},{r:.2f} 0 0 1 {x + w - r:.2f},{y + h:.2f}H{x + r:.2f}"
            f"A{r:.2f},{r:.2f} 0 0 1 {x:.2f},{y + h - r:.2f}V{y + r:.2f}A{r:.2f},{r:.2f} 0 0 1 {x + r:.2f},{y:.2f}Z")


def main_crescent_scaled(cx, cy, r):
    # same proportions as the launcher crescent (outer 29, bite 19 at +12.5,-12.5)
    s = r / 29.0
    return crescent((cx, cy), r, (cx + 12.5 * s, cy - 12.5 * s), 19.0 * s)


HDR = ("<!-- GuardedSleep logo A2b+ (final). 108 dp adaptive-icon canvas; everything inside the 66 dp "
       "safe circle (centre 54,54). Filled outlines only: no strokes, masks or gradients. -->\n")

COL = mark(SW, SAGE, CREAM, ids=True)
LIGHT = mark(SW, SAGE_D, AMBER_D)

# ---------------- launcher ----------------
W("logo.svg", svg(f'<rect width="108" height="108" fill="{NIGHT}"/>\n' + COL, comment=HDR))
W("logo-light.svg", svg(f'<rect width="108" height="108" fill="{MINT}"/>\n' + LIGHT,
                        comment="<!-- light-surface variant (docs, store graphics); not a launcher layer -->\n"))
W("launcher-foreground.svg", svg(COL, comment=HDR))
W("launcher-background.svg", svg(f'<rect width="108" height="108" fill="{NIGHT}"/>',
                                 comment="<!-- adaptive icon background: night #14202B -->\n"))
mc, mw = mark_paths(SW, GAP_MONO)
MONO = f'<path id="mark" d="{mc} {mw}" fill="#000"/>'
W("launcher-monochrome.svg", svg(MONO, comment=(
    "<!-- themed (monochrome) layer: alpha only. Gap wave/crescent 2.75 dp (colour layer: 2 dp). "
    "The last subpath is the wave, closed on its own; the others are the crescent pieces. -->\n")))

# dev variant: amber DEV chip with a night halo, low in the safe zone (as in A)
# DEV pill sits in the free upper right (inside the bite, like the themed badge), so it covers
# none of the mark; amber with night text as in A.
CHIP = rrect(58.0, 30.0, 21.0, 10.0, 5.0)
DEVTXT = (f'<text x="68.5" y="37.55" font-family="Fira Sans" font-weight="800" font-size="7.0" '
          f'letter-spacing="0.5" text-anchor="middle" fill="{NIGHT}">DEV</text>')
DEVBODY = COL + f'\n<path d="{CHIP}" fill="{DEV}"/>\n' + DEVTXT
W("_d1.svg", svg(DEVBODY, comment="<!-- dev foreground layer (pairs with launcher-background.svg); DEV outlined -->\n"))
W("_d2.svg", svg(f'<rect width="108" height="108" fill="{NIGHT}"/>\n' + DEVBODY))
text_to_paths("_d1.svg", "launcher-dev.svg")
text_to_paths("_d2.svg", "logo-dev.svg")
for f in ("_d1.svg", "_d2.svg"):
    os.remove(os.path.join(OUT, f))
BADGE = rrect(65.5, 31.5, 11.0, 11.0, 2.6)
W("launcher-monochrome-dev.svg", svg(MONO + f'\n<path id="dev-badge" d="{BADGE}" fill="#000"/>', comment=(
    "<!-- themed dev layer: monochrome shapes unchanged + rounded-square badge in the free upper right "
    "(inside the bite, >= 2.75 dp from the crescent, inside the 66 dp safe circle) -->\n")))

# ---------------- notification (24 dp, white on transparent) ----------------
NB = main_crescent_scaled(11.6, 12.6, 9.6)
W("notification-baby.svg", svg(f'<path d="{NB}" fill="#FFFFFF"/>', "0 0 24 24", 24, 24,
                               "<!-- ic_stat baby unit: crescent only -->\n"))
NP = main_crescent_scaled(8.5, 12.6, 7.5)
BARS = rrect(17.5, 9.9, 2.1, 5.4, 1.05) + rrect(20.7, 7.3, 2.1, 10.6, 1.05)
W("notification-parent.svg", svg(f'<path d="{NP}{BARS}" fill="#FFFFFF"/>', "0 0 24 24", 24, 24,
                                 "<!-- ic_stat parent unit: crescent + two listening bars -->\n"))

# ---------------- favicon ----------------
def fav_mark(sw, gap, size, pad):
    s = (size - 2 * pad) / 59.0
    d = size / 2 - 54 * s
    return mark(sw, SAGE, CREAM, gap=gap, s=s, dx=d, dy=d)


W("favicon.svg", svg(f'<rect width="32" height="32" rx="7" fill="{NIGHT}"/>\n' + fav_mark(7.0, 2.6, 32, 2.6),
                     "0 0 32 32", 32, 32, "<!-- favicon: night tile, wave 7 dp / gap 2.6 dp for small sizes -->\n"))
# 16 px: the crescent alone (owner decision), the baby-notification crescent at outer radius 5.6.
W("favicon-16.svg", svg(f'<rect width="16" height="16" rx="3.5" fill="{NIGHT}"/>\n<path d="{main_crescent_scaled(8, 8, 5.6)}" fill="{SAGE}"/>',
                        "0 0 16 16", 16, 16,
                        "<!-- favicon 16 px: the crescent alone on the night tile (owner decision). The geometry is the baby-notification crescent of notification-baby.svg, scaled to an outer radius of 5.6 and centred. At 16 px the bite and horn of the full mark merge with the wave. -->\n"))
W("apple-touch-icon.svg", svg(f'<rect width="108" height="108" fill="{NIGHT}"/>\n' + COL, "12 12 84 84", 180, 180,
                              "<!-- apple-touch-icon 180: iOS rounds the corners itself -->\n"))
W("pwa-maskable.svg", svg(f'<rect x="0" y="0" width="108" height="108" fill="{NIGHT}"/>\n' + COL, "9 9 90 90", 512, 512,
                          "<!-- PWA maskable 512: mark spans 65 %, inside the 80 % maskable safe zone -->\n"))

# ---------------- wordmark ----------------
def wordmark(bg, markbody, c1, c2, tag):
    return svg(f'''<rect width="540" height="160" fill="{bg}"/>
<g transform="translate(80,80) scale(2.2) translate(-54,-54)">{markbody}</g>
<text x="160" y="94" font-family="Fira Sans" font-size="58" letter-spacing="-0.5"><tspan font-weight="600" fill="{c1}">Guarded</tspan><tspan font-weight="400" fill="{c2}">Sleep</tspan></text>
<text x="162" y="124" font-family="Fira Sans" font-weight="500" font-size="16.5" letter-spacing="2.6" fill="{tag}">PRIVACY BABY MONITOR</text>''',
               "0 0 540 160", 540, 160)


W("wordmark.svg", wordmark(MINT, LIGHT, INK, SAGE_D, INK_SUB))
W("wordmark-dark.svg", wordmark(NIGHT, COL, SNOW, SAGE, MIST_SUB))
text_to_paths("wordmark.svg", "wordmark-paths.svg")
text_to_paths("wordmark-dark.svg", "wordmark-dark-paths.svg")


# ---------------- splash ----------------
def splash(bg, markbody, c1, c2, tag):
    return svg(f'''<rect width="1080" height="1920" fill="{bg}"/>
<g transform="translate(540,820) scale(5.0) translate(-54,-54)">{markbody}</g>
<text x="540" y="1200" text-anchor="middle" font-family="Fira Sans" font-size="104" letter-spacing="-1"><tspan font-weight="600" fill="{c1}">Guarded</tspan><tspan font-weight="400" fill="{c2}">Sleep</tspan></text>
<text x="540" y="1268" text-anchor="middle" font-family="Fira Sans" font-weight="500" font-size="32" letter-spacing="6" fill="{tag}">PRIVACY BABY MONITOR</text>''',
               "0 0 1080 1920", 1080, 1920)


W("splash-dark.svg", splash(NIGHT, COL, SNOW, SAGE, MIST_SUB))
W("splash-light.svg", splash(MINT, LIGHT, INK, SAGE_D, INK_SUB))
print("svgs written")
