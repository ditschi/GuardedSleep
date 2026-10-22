# Tasks

Coarse tasks; refine them from the wave-3 plan before `/opsx:apply`. Depends on `add-core-facades`.

- [ ] 1.1 Scaffold `apps/web` (Vite 8, Svelte 5, vite-plugin-pwa, TS strict); stores mirroring `UiContract.kt`
- [ ] 1.2 Link driver on the WASM core; both roles to parity with the specs (pairing, link supervision, alerts, status, audio)
- [ ] 1.3 Browser-specific behaviour per the `web-client` delta
- [ ] 1.4 Checks: `svelte-check --fail-on-warnings`, Vitest, Playwright with axe and overflow, storage test
- [ ] 1.6 STUN: use the configured relay's built-in STUN by default and drop the sipgate/linphone defaults of the legacy client; third-party STUN only via `add-stun-server-choice` (signaling-relay "Client ICE configuration")
- [ ] 1.5 Switch the web image to `apps/web`; delete `apps/mobile`; `just ci` green
