# ADR-010: Baby-unit minimum hardware, self-test, and battery safety

- Status: DECIDED (owner, 2026-10-21, amended: no hardware minimum; trust comes from functional self-tests, see the owner decision below)
- Date: 2026-05-13
- Deciders: Product owner (user), coordinated design (Critical Review 2026-05)

> **Context note (2026-10-20):** the WebView assumptions apply to the legacy Capacitor shell only; the native Android baby unit has no WebView dependency ([ADR-015](../2026-10-21-adr-015-native-clients-with-shared-rust-core/design.md), [ADR-019](../2026-10-20-adr-019-language-repository-and-client-policy/design.md)). Paths under `apps/mobile` are historical; the web client is rebuilt as `apps/web` ([ADR-020](../2026-10-20-adr-020-toolchain-and-development-setup/design.md) §5).

> **Owner decision 2026-10-21 (amends Decision 1 and 2):** old Android hardware is not a problem in itself. No product-level minimum API beyond the build's `minSdk`, and no hard block. The baby unit is trusted when the fundamentals work and keep working: the app installs, the connection is established, the parent is notified when the baby unit is unreachable, and the baby microphone still receives sound and is not disabled. Instead of a one-off device verdict: a gentle warning during the first few uses, regular self-tests on the baby unit, and a safe alert on the parent for any potential issue; the parent decides whether a phone is too old. Detailed design after wave 3 ([backlog](../../../../docs/backlog.md) "Baby-unit self-tests"). Battery safety copy (Decision 3) and no whitelist (Decision 4) stand.

## Context

Marketing and architecture text encourage **repurposing old phones** as the baby unit. Without constraints, users may deploy devices with:

- Too-old Android / WebView (WebRTC reliability unknown).
- Degraded WiFi or microphone (false disconnects or missed cries).
- **Damaged or swollen LiPo batteries** -- realistic fire/safety risk when a device runs warm, charging, near a crib.

No document previously stated minimums or in-app self-checks.

## Options Considered

### Option A: No minimum requirements

- **Pros**: Maximum inclusivity; fewer support tickets about "unsupported device".
- **Cons**: Unpredictable WebRTC failures; safety risk undisclosed; reputation damage when old hardware fails at 3 AM.

### Option B: Minimum Android API + updatable WebView assumption + mandatory in-app self-test + explicit battery safety copy

- **Pros**: Sets a testable bar; honest about fire risk; PoC can validate one "borderline" device class.
- **Cons**: Some devices still excluded; self-test adds UX and engineering work.

### Option C: Curated whitelist of approved baby-unit models only

- **Pros**: Predictable support matrix.
- **Cons**: High maintenance; hostile to open-source ethos and global device diversity; stale within months.

## Decision

**Adopt Option B (PROPOSED -- exact API level to be fixed after PoC evidence).**

Concrete intent:

1. **Minimum Android API** -- choose after PoC (candidate starting point: API 26 / Android 8.0 as the floor for "supported baby unit"; lower may show a non-blocking warning or hard block -- TBD in PoC).
2. **Self-test on first baby-mode start** (or when user flags "old device") -- microphone sanity, WebView/User-Agent visibility, simple WiFi RTT or signaling reachability, battery level + health APIs where available. Outcome: "recommended / limited / not recommended" with plain-language explanation.
3. **Battery safety** -- in-app and store copy: do not use devices with swollen or damaged batteries; place on a stable, non-flammable surface; follow OEM charging guidance.
4. **No whitelist** (Option C rejected).

## Consequences

### Positive

- Aligns "reuse old phone" story with defensible safety and reliability.

### Negative

- Some users with very old hardware will be warned or blocked -- support messaging must stay respectful.

### Risks

- OEM variance in battery health APIs -- mitigated by conservative UX ("if unsure, replace the battery or use a different device").

## Discarded alternatives

| Alternative                 | Reason                                          |
| --------------------------- | ----------------------------------------------- |
| Option A -- no minimums     | Rejected after Critical Review CRIT-4.          |
| Option C -- model whitelist | Rejected -- unmaintainable; see concept review. |
