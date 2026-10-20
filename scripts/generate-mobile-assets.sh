#!/usr/bin/env bash
# Regenerate Android launcher/splash and PWA icons from apps/mobile/resources/.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RES="$ROOT/apps/mobile/resources"
ANDROID_RES="$ROOT/apps/mobile/android/app/src/main/res"
STATIC="$ROOT/apps/mobile/static"
LIB_ASSETS="$ROOT/apps/mobile/src/lib/assets"

ICON="$RES/icon.png"
SPLASH="$RES/splash.png"
BG="#000000"

if ! command -v convert >/dev/null 2>&1; then
  echo "ImageMagick (convert) is required." >&2
  exit 1
fi

if [[ ! -f "$ICON" || ! -f "$SPLASH" ]]; then
  echo "Missing $ICON or $SPLASH — add source assets under apps/mobile/resources/." >&2
  exit 1
fi

make_foreground() {
  local size="$1"
  local out="$2"
  local inner=$((size * 72 / 100))
  convert "$ICON" \
    -resize "${inner}x${inner}" \
    -background none \
    -gravity center \
    -extent "${size}x${size}" \
    "$out"
}

make_launcher() {
  local size="$1"
  local out="$2"
  local inner=$((size * 85 / 100))
  convert -size "${size}x${size}" "xc:${BG}" \
    \( "$ICON" -resize "${inner}x${inner}" \) \
    -gravity center \
    -composite \
    "$out"
}

make_splash() {
  local width="$1"
  local height="$2"
  local out="$3"
  convert "$SPLASH" \
    -gravity center \
    -resize "${width}x${height}^" \
    -extent "${width}x${height}" \
    -background "${BG}" \
    "$out"
}

echo "→ Android launcher foregrounds"
make_foreground 108 "$ANDROID_RES/mipmap-mdpi/ic_launcher_foreground.png"
make_foreground 162 "$ANDROID_RES/mipmap-hdpi/ic_launcher_foreground.png"
make_foreground 216 "$ANDROID_RES/mipmap-xhdpi/ic_launcher_foreground.png"
make_foreground 324 "$ANDROID_RES/mipmap-xxhdpi/ic_launcher_foreground.png"
make_foreground 432 "$ANDROID_RES/mipmap-xxxhdpi/ic_launcher_foreground.png"

echo "→ Android launcher icons"
make_launcher 48 "$ANDROID_RES/mipmap-mdpi/ic_launcher.png"
make_launcher 72 "$ANDROID_RES/mipmap-hdpi/ic_launcher.png"
make_launcher 96 "$ANDROID_RES/mipmap-xhdpi/ic_launcher.png"
make_launcher 144 "$ANDROID_RES/mipmap-xxhdpi/ic_launcher.png"
make_launcher 192 "$ANDROID_RES/mipmap-xxxhdpi/ic_launcher.png"

echo "→ Android round launcher icons"
cp "$ANDROID_RES/mipmap-mdpi/ic_launcher.png" "$ANDROID_RES/mipmap-mdpi/ic_launcher_round.png"
cp "$ANDROID_RES/mipmap-hdpi/ic_launcher.png" "$ANDROID_RES/mipmap-hdpi/ic_launcher_round.png"
cp "$ANDROID_RES/mipmap-xhdpi/ic_launcher.png" "$ANDROID_RES/mipmap-xhdpi/ic_launcher_round.png"
cp "$ANDROID_RES/mipmap-xxhdpi/ic_launcher.png" "$ANDROID_RES/mipmap-xxhdpi/ic_launcher_round.png"
cp "$ANDROID_RES/mipmap-xxxhdpi/ic_launcher.png" "$ANDROID_RES/mipmap-xxxhdpi/ic_launcher_round.png"

echo "→ Android splash screens"
make_splash 480 320 "$ANDROID_RES/drawable/splash.png"
make_splash 320 480 "$ANDROID_RES/drawable-port-mdpi/splash.png"
make_splash 480 800 "$ANDROID_RES/drawable-port-hdpi/splash.png"
make_splash 720 1280 "$ANDROID_RES/drawable-port-xhdpi/splash.png"
make_splash 960 1600 "$ANDROID_RES/drawable-port-xxhdpi/splash.png"
make_splash 1280 1920 "$ANDROID_RES/drawable-port-xxxhdpi/splash.png"
make_splash 480 320 "$ANDROID_RES/drawable-land-mdpi/splash.png"
make_splash 800 480 "$ANDROID_RES/drawable-land-hdpi/splash.png"
make_splash 1280 720 "$ANDROID_RES/drawable-land-xhdpi/splash.png"
make_splash 1600 960 "$ANDROID_RES/drawable-land-xxhdpi/splash.png"
make_splash 1920 1280 "$ANDROID_RES/drawable-land-xxxhdpi/splash.png"

echo "→ PWA / web icons"
make_launcher 192 "$STATIC/pwa-192.png"
make_launcher 512 "$STATIC/pwa-512.png"
make_launcher 410 "$STATIC/pwa-512-maskable.png"
convert -size 512x512 "xc:${BG}" \
  \( "$STATIC/pwa-512-maskable.png" \) \
  -gravity center \
  -composite \
  "$STATIC/pwa-512-maskable.png"

cp "$RES/icon.svg" "$STATIC/favicon.svg"
cp "$RES/icon.svg" "$LIB_ASSETS/favicon.svg"

echo "Done."
