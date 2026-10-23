import { svelte } from "@sveltejs/vite-plugin-svelte";
import { VitePWA } from "vite-plugin-pwa";
import { defineConfig } from "vitest/config";

// Plain Vite + Svelte 5, no SvelteKit (rebuild-web-client design.md).
export default defineConfig({
  plugins: [
    svelte(),
    VitePWA({
      registerType: "autoUpdate",
      // No inline registration script (CSP: no inline scripts); main.ts registers the worker.
      injectRegister: false,
      includeAssets: ["favicon.svg", "robots.txt"],
      manifest: {
        name: "GuardedSleep",
        short_name: "GuardedSleep",
        description: "GuardedSleep: a privacy-first baby monitor",
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
        globPatterns: ["**/*.{js,css,html,ico,png,svg,wasm,webmanifest}"],
        // History API routes (/baby, /parent, …) all serve the shell when offline.
        navigateFallback: "/index.html",
        runtimeCaching: [],
      },
      devOptions: { enabled: false },
    }),
  ],
  // Svelte 5 under Vitest: resolve the browser build (mount does not exist in the server build).
  resolve: {
    conditions: process.env.VITEST ? ["browser"] : undefined,
  },
  test: {
    include: ["src/**/*.test.ts"],
    environment: "jsdom",
    setupFiles: ["src/test/setup.ts"],
  },
});
