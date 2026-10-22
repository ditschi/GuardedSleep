# Design

## Context

Sources: ADR-019 (permanent full web client, Rust as the only protocol source), ADR-020 §5 (stack, pnpm 11, lint and tests, browser security), ADR-018 §2 ("Web client" row) and §6 (laptop baby unit, "keep this unit awake"), AGENTS.md §7.2 (web: pairing material in memory only), `alpha-ux-flows.md` (web sections). Legacy reference: `apps/mobile`.

## Decisions

- **Plain Vite + Svelte 5, no SvelteKit** (owner D5): SvelteKit 3 had been GA for three weeks and agents write older patterns; the app has about five screens and no SSR. Revisit at beta.
- **Same contract models as Android**: Svelte stores mirror `UiContract.kt`; UI never imports WASM types.
- **Discarded alternatives:** SvelteKit 3; Leptos or Dioxus (Rust UI, smaller ecosystem for accessibility); Compose for Web (immature); Biome or ESLint (Prettier + `svelte-check` suffice at T0).
- **Image tags**: the web image keeps its tag rules until Release Please (T1) introduces the `app` component; a later change modifies `distribution`.

## Risks / Trade-offs

- Browsers throttle background tabs: the client says so plainly and warns after the page was in the background.
