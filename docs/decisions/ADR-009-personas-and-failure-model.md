# ADR-009: Personas, use cases, and parent-side failure model

- Status: DECIDED (owner, 2026-10-21)
- Date: 2026-05-13
- Deciders: Product owner (user), coordinated design (Critical Review 2026-05)

## Context

GuardedSleep implicitly optimized for one user story -- **Persona B: daily home use** (sleep-deprived parent, nightly sessions, home WiFi, baby device often on charger). That group is the **majority user** and remains the **MVP default** for most UX and defaults; the gap was that **Persona B was never named**, so it could not be defended when trade-offs hurt it.

Two **additional** legitimate groups were under-specified:

1. **Persona A -- Event-only use** (wedding, restaurant, hotel) -- hostile networks (captive portal, client isolation), mobile data, headphone use, no charger.
2. **Persona C -- Hardware reuse** -- old phones/tablets as baby units without documented minimums or self-tests.

The Critical Review also surfaced an incorrect mitigation for "parent phone dead": an **acoustic alarm on the baby unit**. That contradicts the core purpose (do not wake a sleeping child). The correct model matches classic hardware baby monitors: the **parent notices a missing receiver-side indicator** and reconnects without touching the baby device ([failure-model.md](../concept/failure-model.md)).

This ADR binds those outcomes so feature docs and UX can align.

## Options Considered

### Option A: Status quo (single implicit persona)

- **Pros**: No documentation churn.
- **Cons**: Silent wrong defaults for event users and recyclers; failure model undocumented; marketing ("use devices you own") misaligned with engineering reality.

### Option B: Document three personas + failure model + use-case catalogue (no product changes yet)

- **Pros**: Clarifies trade-offs; enables persona-aware defaults later; no code required to adopt the model in docs.
- **Cons**: Requires ongoing discipline (every feature names persona impact).

### Option C: Option B plus baby-side acoustic alarm on parent-device failure

- **Pros**: Theoretically alerts even if parent phone is off.
- **Cons**: Wakes the child; contradicts mission; users rejected this in review.

## Decision

**Adopt Option B.** The project adopts:

- **Three named personas** (A event, **B daily home as primary default**, C recycler) and a use-case matrix in [personas.md](../concept/personas.md) and [use-cases.md](../concept/use-cases.md). Feature docs must name persona impact; **unspecified behavior defaults to optimizing for Persona B** unless an ADR states otherwise.
- A single failure-model document in [failure-model.md](../concept/failure-model.md), including the **explicit rejection** of baby-side acoustic alarms, cloud push/SMS/VoIP as alarm fallbacks, and mandatory second hardware.
- **Option C is REJECTED** for the same reasons as in the review.

**Personas vs. product UX:** The three personas **must not** drive separate app layouts, skins, or mandatory “modes” the user must pick before every trip. Real users move across contexts (home daily **and** occasional events). Personas inform **requirements, acceptance tests, and adaptive defaults** (network, battery, honesty about data use) -- not a forked experience.

**Binding product behaviors (documentation-level until implemented):**

1. Reconnect after saved pairing must be **one tap** from the parent device without physical access to the baby unit (see failure-model "Reconnection specification").
2. **Observability** on the parent side (persistent notification, visible connected-state affordance, onboarding training for "connected vs disconnected") is part of the safety story when the parent receiver fails.
3. **Phase 2 multi-parent** is not only convenience; it is the **privacy-preserving software failover** when one parent device is unresponsive (see failure-model F-1a).

## Consequences

### Positive

- Honest positioning vs. hardware baby monitors and vs. cloud apps.
- Clear acceptance tests for PoC and MVP (event network, indicator training, reconnect path).

### Negative

- More onboarding and copy; risk of "too much text" unless tightly edited.
- Headphones vs speaker trade-off resolved per [ADR-011](../../openspec/changes/archive/2026-10-21-adr-011-audio-routing-parent-side/design.md) (unified default + Advanced); optional metered-network alert emphasis in [notifications-alerts.md](../features/notifications-alerts.md).

### Risks

- Users who skip onboarding may still miss the indicator-loss pattern -- mitigated by persistent notification + probe-alarm habit (see feature docs).

## Discarded alternatives

| Alternative                             | Reason                                                       |
| --------------------------------------- | ------------------------------------------------------------ |
| Baby-side siren on connection loss      | Rejected -- wakes the child.                                 |
| Cloud push / SMS / VoIP dead-man switch | Rejected -- violates privacy-first / no cloud in alarm path. |
| Single implicit persona                 | Rejected -- caused CRIT-1 blind spots.                       |
