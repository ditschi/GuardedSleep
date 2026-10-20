import { SvelteKitPWA } from "@vite-pwa/sveltekit";
import tailwindcss from "@tailwindcss/vite";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vitest/config";

// @vite-pwa/sveltekit / vite-plugin-pwa still declare peer vite@7; Vite 8 works at runtime.
export default defineConfig({
  plugins: [
    tailwindcss(),
    sveltekit(),
    SvelteKitPWA({
      registerType: "autoUpdate",
      includeAssets: [
        "favicon.svg",
        "pwa-192.png",
        "pwa-512.png",
        "pwa-512-maskable.png",
        "robots.txt",
      ],
      manifest: {
        name: "GuardedSleep",
        short_name: "GuardedSleep",
        description: "GuardedSleep — privacy-focused baby monitor",
        theme_color: "#000000",
        background_color: "#000000",
        display: "standalone",
        start_url: "/",
        icons: [
          { src: "/pwa-192.png", sizes: "192x192", type: "image/png" },
          { src: "/pwa-512.png", sizes: "512x512", type: "image/png" },
          {
            src: "/pwa-512-maskable.png",
            sizes: "512x512",
            type: "image/png",
            purpose: "maskable",
          },
        ],
      },
      workbox: {
        globPatterns: ["**/*.{js,css,ico,png,svg,woff2,webmanifest}"],
        runtimeCaching: [],
      },
      devOptions: {
        enabled: false,
      },
    }),
  ] as any,
  // Svelte 5 + Vitest: use browser build of Svelte (mount is not on server).
  resolve: {
    conditions: process.env.VITEST ? ["browser"] : undefined,
  },
  test: {
    include: ["src/**/*.{test,spec}.ts"],
    environment: "jsdom",
    setupFiles: ["src/test/setup.ts"],
  },
});
