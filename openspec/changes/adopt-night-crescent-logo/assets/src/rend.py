import subprocess, os, tempfile, math
from PIL import Image, ImageDraw, ImageFont

TMP = tempfile.mkdtemp(prefix="gsr-", dir=os.path.dirname(os.path.abspath(__file__)))
FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"
FONTB = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"


def font(sz, bold=False):
    try:
        return ImageFont.truetype(FONTB if bold else FONT, sz)
    except OSError:
        return ImageFont.load_default()


def ink(svg_text_or_path, w, h=None, out=None):
    h = h or w
    if svg_text_or_path.lstrip().startswith("<"):
        src = os.path.join(TMP, f"s{abs(hash(svg_text_or_path))}.svg")
        open(src, "w").write(svg_text_or_path)
    else:
        src = svg_text_or_path
    out = out or os.path.join(TMP, f"o{abs(hash((src, w, h)))}.png")
    subprocess.run(["inkscape", src, "--export-type=png", f"--export-filename={out}",
                    "-w", str(w), "-h", str(h)], check=True, capture_output=True, timeout=120)
    return Image.open(out).convert("RGBA")


def _mask(n, kind):
    S = n * 4
    m = Image.new("L", (S, S), 0)
    if kind == "circle":
        ImageDraw.Draw(m).ellipse((0, 0, S - 1, S - 1), fill=255)
    elif kind == "squircle":
        px = m.load()
        for y in range(S):
            v = (y - (S - 1) / 2) / (S / 2)
            for x in range(S):
                u = (x - (S - 1) / 2) / (S / 2)
                if u ** 4 + v ** 4 <= 1:
                    px[x, y] = 255
    elif kind == "rounded":
        ImageDraw.Draw(m).rounded_rectangle((0, 0, S - 1, S - 1), radius=int(S * 0.22), fill=255)
    return m.resize((n, n), Image.LANCZOS)


_MC = {}


def mask(n, kind):
    if (n, kind) not in _MC:
        _MC[(n, kind)] = _mask(n, kind)
    return _MC[(n, kind)]


def launcher(full_svg, n, kind="circle"):
    """full 108-dp svg (with background) -> masked n px launcher icon (72 dp viewport)"""
    big = max(n * 3, 768)
    canvas = ink(full_svg, big * 108 // 72)
    m = (canvas.width - big) // 2
    vp = canvas.crop((m, m, m + big, m + big)).resize((n, n), Image.LANCZOS)
    vp.putalpha(mask(n, kind))
    return vp


def fake_icons(n):
    """generic, unbranded neighbour icons for the home-screen strip"""
    S = n * 4
    out = []
    specs = [("#E5533D", "tri"), ("#3B82F6", "env"), ("#F4B400", "sun"), ("#8E5CF7", "note"),
             ("#22A06B", "grid"), ("#EC4899", "heart")]
    for col, g in specs:
        im = Image.new("RGBA", (S, S), (0, 0, 0, 0))
        d = ImageDraw.Draw(im)
        d.ellipse((0, 0, S - 1, S - 1), fill=col)
        w = "#FFFFFF"
        c = S / 2
        if g == "tri":
            d.polygon([(c - S * .15, c - S * .2), (c - S * .15, c + S * .2), (c + S * .22, c)], fill=w)
        elif g == "env":
            d.rectangle((c - S * .24, c - S * .16, c + S * .24, c + S * .16), fill=w)
            d.line([(c - S * .24, c - S * .16), (c, c + S * .03), (c + S * .24, c - S * .16)], fill=col, width=S // 24)
        elif g == "sun":
            d.ellipse((c - S * .14, c - S * .14, c + S * .14, c + S * .14), fill=w)
            for k in range(8):
                a = k * math.pi / 4
                d.line([(c + S * .2 * math.cos(a), c + S * .2 * math.sin(a)),
                        (c + S * .28 * math.cos(a), c + S * .28 * math.sin(a))], fill=w, width=S // 20)
        elif g == "note":
            d.rounded_rectangle((c - S * .18, c - S * .24, c + S * .18, c + S * .24), radius=S // 24, fill=w)
            for k in range(3):
                y = c - S * .1 + k * S * .09
                d.line([(c - S * .1, y), (c + S * .1, y)], fill=col, width=S // 30)
        elif g == "grid":
            for i in range(2):
                for j in range(2):
                    x = c - S * .2 + i * S * .22
                    y = c - S * .2 + j * S * .22
                    d.rounded_rectangle((x, y, x + S * .17, y + S * .17), radius=S // 40, fill=w)
        elif g == "heart":
            r = S * .11
            d.ellipse((c - 2 * r, c - S * .16, c, c - S * .16 + 2 * r), fill=w)
            d.ellipse((c, c - S * .16, c + 2 * r, c - S * .16 + 2 * r), fill=w)
            d.polygon([(c - 2 * r + 1, c - S * .16 + r * 1.2), (c + 2 * r - 1, c - S * .16 + r * 1.2), (c, c + S * .22)], fill=w)
        out.append(im.resize((n, n), Image.LANCZOS))
    return out


def wallpaper(w, h, kind):
    im = Image.new("RGB", (w, h))
    d = ImageDraw.Draw(im)
    if kind == "dark":
        a, b = (24, 26, 38), (46, 34, 60)
    else:
        a, b = (236, 230, 216), (200, 218, 232)
    for y in range(h):
        t = y / max(h - 1, 1)
        d.line([(0, y), (w, y)], fill=tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3)))
    return im.convert("RGBA")


def home_strip(ours, kind="dark", n=48, labels=True, title=None, count=5):
    icons = fake_icons(n)
    row = icons[:2] + [ours] + icons[2:4]
    names = ["Video", "Mail", "GuardedSleep", "Weather", "Notes"]
    if count == 3:
        row, names = row[1:4], names[1:4]
    pad = n // 2
    cell = n + pad * 2
    W = cell * len(row)
    H = n + pad * 2 + (n // 3 if labels else 0)
    im = wallpaper(W, H, kind)
    d = ImageDraw.Draw(im)
    f = font(max(9, n // 5))
    for i, ic in enumerate(row):
        x = i * cell + pad
        im.alpha_composite(ic, (x, pad))
        if labels:
            t = names[i] if len(names[i]) < 10 else "GuardedS…"
            tw = d.textlength(t, font=f)
            d.text((x + n / 2 - tw / 2, pad + n + 3), t, font=f,
                   fill=(235, 235, 240) if kind == "dark" else (30, 30, 36))
    return im


def warm_tint(im, k=(1.0, 0.80, 0.52)):
    """approximate a blue-light filter / night light (~3000 K) by channel scaling"""
    r, g, b, a = im.split()
    r = r.point(lambda v: int(v * k[0])); g = g.point(lambda v: int(v * k[1])); b = b.point(lambda v: int(v * k[2]))
    return Image.merge("RGBA", (r, g, b, a))


def label(im, text, xy, sz=18, col=(30, 34, 40), bold=False):
    ImageDraw.Draw(im).text(xy, text, font=font(sz, bold), fill=col)
