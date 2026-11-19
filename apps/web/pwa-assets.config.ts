import { defineConfig } from "@vite-pwa/assets-generator/config";

// PWA icons are generated at build time from public/favicon.svg (a byte-identical copy of
// docs/brand/logo.svg, checked by scripts/lint-brand-icons.mjs), so they never drift from the
// brand master and no PNG is committed. The mark already lies inside the 66/108 safe circle of the
// Android adaptive icon, so no extra padding: the plain and the maskable icon show the same tile as
// the Android launcher (background #F2FAF5, docs/brand/README.md).
// https://vite-pwa-org.netlify.app/assets-generator/config.html
const background = "#F2FAF5";

export default defineConfig({
  headLinkOptions: { preset: "2023" },
  preset: {
    // The generator keeps `transparent` icons transparent whatever the background option says,
    // and the spec wants the mint tile on every icon. The `apple` type is the opaque, background-
    // filled one, so it produces the plain "any" icons (names below); head links are switched off
    // in vite.config.ts, the manifest lists the files explicitly.
    transparent: { sizes: [] },
    apple: { sizes: [192, 512], padding: 0, resizeOptions: { background, fit: "contain" } },
    maskable: {
      sizes: [512],
      padding: 0,
      resizeOptions: { background, fit: "contain" },
    },
    assetName: (type, size) =>
      type === "maskable" ? `pwa-${size.width}-maskable.png` : `pwa-${size.width}.png`,
  },
  images: ["public/favicon.svg"],
});
