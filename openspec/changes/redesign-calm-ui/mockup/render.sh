#!/bin/sh
# Re-generate mockup.html and render it to PNG pieces for review.
#   sh render.sh                      # pieces go to ./out (not committed)
#   OUT=/some/dir CHROME=/path/to/chrome-headless-shell H=26000 sh render.sh
# Needs python3 with Pillow and a headless Chromium (for example from `pnpm exec playwright install`).
set -eu
D=$(cd "$(dirname "$0")" && pwd)
OUT=${OUT:-$D/out}
CHROME=${CHROME:-chrome-headless-shell}
mkdir -p "$OUT"
python3 "$D/gen.py"
timeout 120 "$CHROME" --no-sandbox --hide-scrollbars --virtual-time-budget=4000 \
  --window-size=1600,"${H:-26000}" --screenshot="$OUT/shot.png" "file://$D/mockup.html" 2>/dev/null
python3 - "$OUT" <<'PY'
import sys
from PIL import Image
out = sys.argv[1]
im = Image.open(out + "/shot.png").convert("RGB")
w, h = im.size
last = 0
for y in range(h - 1, 0, -10):
    if any(sum(abs(c - d) for c, d in zip(im.getpixel((x, y)), (14, 14, 17))) > 12 for x in range(0, w, 25)):
        last = y
        break
print("height", h, "last", last)
y = 0
i = 0
while y < last:
    im.crop((0, y, 1600, min(y + 1000, last + 30))).save(out + "/r%02d.png" % i)
    y += 1000
    i += 1
print("pieces", i)
PY
