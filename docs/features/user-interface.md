# Feature: User Interface

## Overview

The user interface must be simple, reliable, and usable under stress (tired parent, dark room, urgent situation). Design decisions here directly impact whether non-technical users will adopt the app.

**Personas, use cases, and failure observability** drive several MVP UX additions -- see [personas.md](../concept/personas.md), [use-cases.md](../concept/use-cases.md), [failure-model.md](../concept/failure-model.md), and [ADR-009](../../openspec/changes/archive/2026-10-21-adr-009-personas-and-failure-model/design.md).

## Requirements

> STATUS: DECIDED -- moved to [`openspec/specs/device-setup`](../../openspec/specs/device-setup/spec.md) (dark theme by default, Basic and Advanced settings, setup checklist), [`audio-monitoring`](../../openspec/specs/audio-monitoring/spec.md) (monitoring screen), [`parent-alerts`](../../openspec/specs/parent-alerts/spec.md) and [`connection-status`](../../openspec/specs/connection-status/spec.md). "One unified app, no persona-specific profiles" is a project rule in `openspec/config.yaml`. Ideas (emoji fingerprint, About/FAQ, nightlight, temperature display) are in the [backlog](../backlog.md).

### Multi-Language Support (i18n)

> STATUS: DECIDED -- locale policy per [ADR-021](../decisions/ADR-021-offline-lan-topologies-locales-and-distribution.md) §2 (owner, 2026-10-20), superseding the four-locale decision of 2026-10-19 (en, de, fr, es). Mechanism: [ADR-015](../decisions/ADR-015-native-clients-with-shared-rust-core.md) "Localization" (PROPOSED).

- English is the master catalogue. AI-translated catalogues ship for about 20 frequently spoken languages: en, de, fr, es, it, pt, nl, pl, sv, da, cs, ro, zh-Hans, ja, ko, hi, ar (right-to-left), tr, uk, id. Native speakers improve them over time; further locales can be added the same way without a new ADR
- A locale ships only if every key tagged `critical` (alerts, link loss, connection state, safety warnings) is present, so an alarm is never mixed-language; non-critical keys may fall back to English
- Each non-English value carries a `machineTranslated` or `reviewed` marker; review is an improvement path, not a shipping gate. Alert and connection-loss copy must still be understandable in each locale, see [ADR-013](../../openspec/changes/archive/2026-10-21-adr-013-connection-status-observability-and-copy/design.md))
- Translations are maintained once in a shared catalogue and reused by every client (Android, iOS, web, later desktop)
- Architecture supports easy addition of new languages; community can contribute translations
- UI text externalized from code from the start

## Dependencies

- [Audio Monitoring](audio-monitoring.md) -- volume level data for display
- [Communication](communication.md) -- transmitting sensor data to parent unit
- [Device Management](device-management.md) -- screen brightness control in baby mode

## Open Questions

- ~~Should night mode be automatic or manual toggle?~~ **RESOLVED**: Default dark, manual toggle + optional auto-switch in settings
- ~~Which i18n framework to use?~~ **RESOLVED (proposed)**: single platform-neutral catalogue with generated per-platform resources ([ADR-015](../decisions/ADR-015-native-clients-with-shared-rust-core.md))
- ~~How many languages for the initial release?~~ **RESOLVED, superseded**: English, German, French, Spanish (owner decision 2026-10-19) → about 20 AI-translated locales (ADR-021 §2)
- Which devices actually have accessible temperature/humidity sensors?
- Phone vs. tablet layout: responsive design approach? Breakpoints?

## Discarded Alternatives

_None yet -- this feature area is still in initial ideation._
