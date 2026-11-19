import { defineConfig } from "@vite-pwa/assets-generator/config";

// PWA icons are generated at build time from public/logo.svg (a byte-identical copy of the vector
// master docs/brand/logo.svg, checked by scripts/lib/brand-icons.mjs; the generator only emits
// icons for images inside public/), so they never drift from the brand and no PNG is committed.
// The master is a full-bleed night tile whose mark already lies inside the 66/108 safe circle of
// the Android adaptive icon, so no extra padding: the plain, the maskable and the apple-touch icon
// show the same tile as the Android launcher (background #14202B, docs/brand/README.md). The
// favicons are separate files in public/ (the 16 px one is the crescent alone).
// https://vite-pwa-org.netlify.app/assets-generator/config.html
const background = "#14202B";

export default defineConfig({
  headLinkOptions: { preset: "2023" },
  preset: {
    // The generator keeps `transparent` icons transparent whatever the background option says,
    // and the spec wants the night tile on every icon. The `apple` type is the opaque, background-
    // filled one, so it produces the plain "any" icons and the 180 px apple-touch icon (names
    // below); head links are switched off in vite.config.ts, the manifest lists the files
    // explicitly and index.html links the apple-touch icon.
    transparent: { sizes: [] },
    apple: { sizes: [180, 192, 512], padding: 0, resizeOptions: { background, fit: "contain" } },
    maskable: {
      sizes: [512],
      padding: 0,
      resizeOptions: { background, fit: "contain" },
    },
    assetName: (type, size) =>
      type === "maskable"
        ? `pwa-${size.width}-maskable.png`
        : size.width === 180
          ? "apple-touch-icon.png"
          : `pwa-${size.width}.png`,
  },
  images: ["public/logo.svg"],
});
