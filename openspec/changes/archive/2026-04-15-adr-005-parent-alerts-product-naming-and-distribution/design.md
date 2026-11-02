# ADR-005: Parent Alerts, Audio Behavior, Product Naming, Store Listings

- Status: DECIDED
- Date: 2026-04-15
- Deciders: UX + user

> **Partially superseded (2026-10-20):** the Distribution line (F-Droid) → [ADR-021](../2026-10-20-adr-021-offline-lan-topologies-locales-and-distribution/design.md) §3: Play and GitHub APK at alpha, IzzyOnDroid at beta, F-Droid main repository `DEFERRED`.

> **Migrated to OpenSpec (2026-10-22):** "Noise alert behavior" and "Connection-loss alerts" are normative in [`openspec/specs/parent-alerts`](../../../specs/parent-alerts/spec.md) and `link-supervision`. Naming, store ASO and distribution stay binding until `distribution` is migrated.

## Context

Parents must notice noise events reliably; branding and store discoverability must reflect privacy positioning.

## Decision

### Noise alert behavior (MVP)

On noise threshold exceeded, **in order**:

1. **Vibration**
2. **Screen on**, app **foreground**, **volume meter / noise bars** show live level
3. **Immediate playback of child audio** on speaker -- **no separate alarm ringtone**; the child's audio **is** the alert
4. Audio routing uses streams that **respect override of silent/DND** where technically possible (notification channel, `USAGE_ALARM` / documented native behavior -- see notifications feature doc)

### Connection-loss alerts

Escalating notifications per feature spec (timings implemented as agreed in notifications docs).

### Product naming

- **Product name:** **GuardedSleep – Privacy Baby Monitor**
- **Repository / engineering name:** **GuardedSleep** (npm scope **`@guardedsleep`**, Docker **`guardedsleep-*`** images, Android **`org.guardedsleep.app`**; GitHub repo path may differ)

### Store ASO

- Full title may exceed Play character limits -- use shortened title if needed (e.g. "GuardedSleep Baby Monitor") and full name in long description.
- Descriptions must include searchable terms: baby monitor, privacy, encrypted, open source.

### Distribution

- **Google Play** + **F-Droid** (superseded, see ADR-021 §3: IzzyOnDroid at beta, F-Droid main deferred) + direct APK; build flavors `playStore` vs `foss` as documented.

## Consequences

### Positive

- Less anxiety than siren-like tones; clearer "what you hear is real" experience.

### Negative

- If parent mistakes speaker routing, child audio might be quiet -- mitigated by vibration + screen + visual meter.

## Discarded alternatives

- Separate loud ringtone for every noise event -- rejected in favor of child audio as primary alert sound.
