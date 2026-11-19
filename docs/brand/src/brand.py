# GuardedSleep A2b+ palette and SVG builders.
from geo import build, poly_d, transform, scale_circles

NIGHT = "#14202B"     # launcher background, dark tile
SAGE = "#5DBFA6"      # crescent on night
CREAM = "#F6D79A"     # breath wave on night (warm cream)
MINT = "#F2FAF5"      # light tile / light surfaces
SAGE_D = "#3A9A84"    # crescent on light
AMBER_D = "#B8741F"   # wave on light (warm, >=3:1 on mint)
INK = "#1E3A55"       # "Guarded" on light
INK_SUB = "#4F6476"   # tagline on light
SNOW = "#E8EFF5"      # "Guarded" on night
MIST_SUB = "#8FA3B3"  # tagline on night
DEV = "#F2A65A"       # DEV marker amber

GAP = 2.0             # colour layer gap wave <-> crescent
GAP_MONO = 2.75       # themed monochrome gap


def svg(body, vb="0 0 108 108", w=108, h=108, comment=""):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vb}" width="{w}" height="{h}">\n'
            f"{comment}{body}\n</svg>\n")


def geometry(sw, gap=GAP, **kw):
    return build(sw, gap, **kw)


def mark_paths(sw, gap=GAP, s=1.0, dx=0.0, dy=0.0, **kw):
    """returns (crescent d, wave d) in the target coordinate space"""
    g = build(sw, gap, **kw)
    circles = scale_circles(g["circles"], s, dx, dy)
    cres = " ".join(poly_d(transform(p, s, dx, dy), circles) for p in g["cres"])
    wave = poly_d(transform(g["wave"], s, dx, dy), circles)
    return cres, wave


def mark(sw, cres_col, wave_col, gap=GAP, s=1.0, dx=0.0, dy=0.0, ids=False, **kw):
    c, w = mark_paths(sw, gap, s, dx, dy, **kw)
    ic = ' id="crescent"' if ids else ""
    iw = ' id="wave"' if ids else ""
    return f'<path{ic} d="{c}" fill="{cres_col}"/>\n<path{iw} d="{w}" fill="{wave_col}"/>'
