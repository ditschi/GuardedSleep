# GuardedSleep logo A2b+ geometry: filled outlines only (no strokes, masks or gradients),
# so every shape converts 1:1 to an Android VectorDrawable <path android:pathData>.
import math
from shapely.geometry import Point, LineString, Polygon, MultiPolygon
from shapely import affinity

OUTER = (54.0, 54.0, 29.0)      # moon outline
BITE = (66.5, 41.5, 19.0)       # bite circle (fuller crescent)
WAVE_Y = 66.0                   # LOW position
AMP = 5.8                       # calm small-big-small breath
PAST = 3.0                      # dp the wave runs past the moon outline on each side
SAFE_R = 33.0                   # 66 dp safe circle, centre 54,54


def circ(c, n=512):
    return Point(c[0], c[1]).buffer(c[2], quad_segs=n // 4)


def centerline(x0, x1, y=WAVE_Y, amp=AMP, cycles=1.5, n=600):
    pts = []
    for i in range(n + 1):
        t = i / n
        env = 0.5 + 0.5 * math.sin(math.pi * t)
        pts.append((x0 + (x1 - x0) * t, y - amp * env * math.cos(2 * math.pi * cycles * (t - 0.5))))
    return LineString(pts)


def wave_span(sw, past=PAST, y=WAVE_Y, outer=OUTER):
    # end points so that the round cap reaches `past` dp beyond the outline at the wave height
    half = math.sqrt(outer[2] ** 2 - (y - outer[1]) ** 2)
    return outer[0] - half - past + sw / 2, outer[0] + half + past - sw / 2


def build(sw, gap, amp=AMP, past=PAST, y=WAVE_Y, outer=OUTER, bite=BITE, min_area=40.0):
    x0, x1 = wave_span(sw, past, y, outer)
    cl = centerline(x0, x1, y, amp)
    wave = cl.buffer(sw / 2, quad_segs=48)
    cut = cl.buffer(sw / 2 + gap, quad_segs=48)
    cres = circ(outer).difference(circ(bite)).difference(cut)
    parts = list(cres.geoms) if isinstance(cres, MultiPolygon) else [cres]
    kept = [p for p in parts if p.area >= min_area]           # drops the lower-horn sliver
    dropped = [p for p in parts if p.area < min_area]
    circles = [outer, bite,
               (x0, cl.coords[0][1], sw / 2), (x1, cl.coords[-1][1], sw / 2),
               (x0, cl.coords[0][1], sw / 2 + gap), (x1, cl.coords[-1][1], sw / 2 + gap)]
    return dict(wave=wave, cres=kept, dropped=dropped, circles=circles, x0=x0, x1=x1, cl=cl)


# ---------- polygon -> compact SVG path (arcs where the ring lies on a known circle) ----------
def _on(p, c, tol=0.02):
    return abs(math.hypot(p[0] - c[0], p[1] - c[1]) - c[2]) < tol


def ring_to_d(coords, circles, fmt=lambda v: f"{v:.2f}", step=0.9):
    pts = list(coords)[:-1]
    n = len(pts)
    lab = []
    for i in range(n):
        a, b = pts[i], pts[(i + 1) % n]
        L = None
        for k, c in enumerate(circles):
            if _on(a, c) and _on(b, c) and math.dist(a, b) < c[2] * 0.3:
                L = k
                break
        lab.append(L)
    # rotate so we start at a label change
    start = 0
    for i in range(n):
        if lab[i] != lab[i - 1]:
            start = i
            break
    pts = pts[start:] + pts[:start]
    lab = lab[start:] + lab[:start]
    runs = []
    i = 0
    while i < n:
        j = i
        while j + 1 < n and lab[j + 1] == lab[i]:
            j += 1
        runs.append((lab[i], [pts[k] for k in range(i, j + 1)] + [pts[(j + 1) % n]]))
        i = j + 1
    P = lambda p: f"{fmt(p[0])},{fmt(p[1])}"
    d = "M" + P(runs[0][1][0])
    for L, rp in runs:
        if L is not None:
            cx, cy, r = circles[L]
            tot = 0.0
            for a, b in zip(rp, rp[1:]):
                da = math.atan2(b[1] - cy, b[0] - cx) - math.atan2(a[1] - cy, a[0] - cx)
                da = (da + math.pi) % (2 * math.pi) - math.pi
                tot += da
            sweep = 1 if tot > 0 else 0
            if abs(tot) > 2 * math.pi - 1e-3:  # full circle: split in two
                mid = rp[len(rp) // 2]
                d += f"A{fmt(r)},{fmt(r)} 0 0 {sweep} {P(mid)}A{fmt(r)},{fmt(r)} 0 0 {sweep} {P(rp[-1])}"
            else:
                large = 1 if abs(tot) > math.pi else 0
                d += f"A{fmt(r)},{fmt(r)} 0 {large} {sweep} {P(rp[-1])}"
        else:
            # free curve: resample by arc length, Catmull-Rom -> cubic Bezier
            acc, keep = 0.0, [rp[0]]
            for a, b in zip(rp, rp[1:]):
                acc += math.dist(a, b)
                if acc >= step:
                    keep.append(b)
                    acc = 0.0
            if keep[-1] != rp[-1]:
                if len(keep) > 1 and math.dist(keep[-1], rp[-1]) < step * 0.4:
                    keep[-1] = rp[-1]
                else:
                    keep.append(rp[-1])
            if len(keep) == 2:
                d += "L" + P(keep[1])
                continue
            for k in range(len(keep) - 1):
                p0 = keep[max(k - 1, 0)]; p1 = keep[k]; p2 = keep[k + 1]; p3 = keep[min(k + 2, len(keep) - 1)]
                c1 = (p1[0] + (p2[0] - p0[0]) / 6, p1[1] + (p2[1] - p0[1]) / 6)
                c2 = (p2[0] - (p3[0] - p1[0]) / 6, p2[1] - (p3[1] - p1[1]) / 6)
                d += f"C{P(c1)} {P(c2)} {P(p2)}"
    return d + "Z"


def poly_d(poly, circles, **kw):
    poly = Polygon(poly.exterior.coords[::-1]) if not poly.exterior.is_ccw else poly
    d = ring_to_d(poly.exterior.coords, circles, **kw)
    for hole in poly.interiors:
        d += ring_to_d(hole.coords, circles, **kw)
    return d


def scale_circles(circles, s, dx=0, dy=0):
    return [(c[0] * s + dx, c[1] * s + dy, c[2] * s) for c in circles]


def transform(geom, s, dx=0, dy=0):
    return affinity.translate(affinity.scale(geom, s, s, origin=(0, 0)), dx, dy)
