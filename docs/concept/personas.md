# Personas

> STATUS: PROPOSED -- to be finalized via [ADR-009](../decisions/ADR-009-personas-and-failure-model.md) (in preparation).
>
> **Why this document exists**: Until 2026-05, the project **implicitly** optimized for **Persona B -- daily home** ("sleep-deprived parent at 3 AM", nursery + living room, same WiFi every night) **without naming that persona**. The [concept review 2026-05](../reviews/concept-review-2026-05.md) (finding CRIT-1) surfaced that this **unwritten default** caused trade-offs to silently hurt two **other** groups: **Persona A** (event-only) and **Persona C** (hardware recyclers). This document **names all three** so features can declare whom they optimize for.

## Persona summary (read this first)

| ID    | Short name                      | Who                                                                                           | Default for MVP?                                                                    |
| ----- | ------------------------------- | --------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| **B** | **Alltags-Eltern (Daily Home)** | Nightly baby monitor at home; baby device often on charger; parent device is their main phone | **Yes** -- most screens, defaults, and copy should assume B unless stated otherwise |
| **A** | Festtags-Eltern (Event)         | Hotel, wedding, restaurant; hostile WiFi; headphones; rare use                                | No -- special flows and copy                                                        |
| **C** | Hardware-Recycler               | Old/spare phone or tablet as baby unit (often **with** B)                                     | Partial -- baby-side self-test and warnings                                         |

## How to use this document

- Every feature document in [docs/features/](../features/) should have a **Persona Impact** subsection that names which persona(s) the feature primarily serves and which persona(s) it might trade off against.
- When a feature decision splits the personas (e.g. CRIT-7 audio routing: Persona A wants headphones, Persona B wants speaker), an ADR is required ([AGENTS.md §6.2](../../AGENTS.md)).
- Personas are advisors, not gates. The user remains the final decision-maker on scope.

## Schema

Each persona is defined by the same eight fields, so they can be compared apples-to-apples:

1. **Setting** -- where and when GuardedSleep is used
2. **Hardware** -- what devices the persona owns and brings to the situation
3. **Network** -- realistic network situation (WLAN type, mobile data, captive portals, AP isolation)
4. **Battery assumption** -- charged/not charged, charger present/absent
5. **Expectation** -- what the persona expects the app to do
6. **Failure tolerance** -- what failure modes the persona will accept vs. abandon the app over
7. **Anti-expectation** -- explicit things the persona does NOT want (helps catch unwanted features)
8. **Frequency** -- how often the persona uses the app (daily, weekly, yearly)

---

## Persona A -- "Festtags-Eltern" (Event-Use)

> Example: a couple at a wedding reception, the child sleeps in a hotel room two floors up; or a family at a restaurant where the toddler naps in the stroller around the corner.

| Field                  | Value                                                                                                                                                                                                                                                                             |
| ---------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Setting**            | Hotel, restaurant, festival hall, holiday rental, friends' house. Often unfamiliar environment, never set up before. Child in a separate room, often with significant distance (one or two floors, 30–100 m).                                                                     |
| **Hardware**           | Both parents' current personal phones. **No** dedicated baby device, no chargers, no tripod, no stand. The "baby unit" is whatever phone they put down next to the sleeping child.                                                                                                |
| **Network**            | Hotel/venue WLAN with **captive portal**, often **client isolation** between APs, frequently **WebSocket / STUN / TURN ports blocked**. Mobile data on both devices is realistic in most EU countries.                                                                            |
| **Battery assumption** | Both phones started the day at 80–100 %; by the time monitoring starts they may be at 30–60 %. Chargers stayed at home or in the suitcase. The baby-side phone is **not** plugged in.                                                                                             |
| **Expectation**        | "Install it, place it, hear when something is up, keep enjoying the evening." Specifically: setup in < 5 min, no fiddling, no router login pages, no per-network tinkering. The 100 m-equivalent reception range of a hardware baby monitor is the baseline they compare against. |
| **Failure tolerance**  | Very low. Persona A uses GuardedSleep once a year. If the first attempt fails (Hotel WiFi blocks WebRTC, no signal in the dining room), they uninstall and tell every friend "the privacy baby monitor app doesn't work". The reputation cost per failure is high.                |
| **Anti-expectation**   | Do **not** ask me to configure an "advanced" tab. Do **not** show me technical mode names ("hotspot / signaling / VPN"). Do **not** force speaker output through my phone -- I might be wearing Bluetooth in-ears during a wedding speech.                                        |
| **Frequency**          | 1–5 times per year.                                                                                                                                                                                                                                                               |

### Key risks for Persona A

- **CRIT-2** -- Captive-WLAN and client isolation defeat the "same WiFi" default path.
- **CRIT-3** -- Mobile data + roaming costs are invisible to the user today.
- **CRIT-7** -- The current draft routes alerts through the speaker even with headphones -- not acceptable at a wedding reception.
- **IMP-2** -- Without a pre-departure test, Persona A only discovers problems at the venue.

---

## Persona B -- "Alltags-Eltern" (Daily Home)

> **Primary persona (MVP default).** Most users, most weeks. UX defaults (e.g. audio routing bias to speaker for audibility, auto-resume baby after reboot, battery-optimization onboarding, saved-connection card first on the home screen) should **optimize for B** unless the user picks another profile at first launch.

> Example: a family with a 9-month-old, baby sleeps in their nursery from 19:30, parents are in the living room or kitchen, monitor runs every night.

| Field                  | Value                                                                                                                                                                                                                                                                      |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Setting**            | Their own apartment or house. Two rooms, possibly two floors, 5–15 m line of sight. Same routine every evening.                                                                                                                                                            |
| **Hardware**           | Parents' main phones AS parent-units. The baby unit is often a **dedicated device**: either a kept-on-purpose old phone (see Persona C) or a tablet. The baby device is **always plugged into the charger**, often on a stand or shelf.                                    |
| **Network**            | Their home WLAN. Both devices in the same SSID, no captive portal, no client isolation. WiFi may be patchy in some corners (router on the other end of the apartment) but generally stable.                                                                                |
| **Battery assumption** | Baby unit: **continuous charging**, not a concern. Parent unit: the parents' own phone, often goes to <30 % by midnight if not on a charger.                                                                                                                               |
| **Expectation**        | Works every single night without intervention. After a Phone reboot or OS update, monitoring continues automatically. Sleep-deprived 3-AM operation must be single-handed and not require eyes adapting to bright light.                                                   |
| **Failure tolerance**  | Medium. Persona B will tolerate a one-off glitch ("yesterday it disconnected, I had to retap") if it doesn't repeat. A second failure in the same week leads to abandonment.                                                                                               |
| **Anti-expectation**   | Don't drain my main phone's battery in the background just to keep the indicator alive. Don't surprise me with a notification flood -- I configured this once and I want it stable. Don't change my role on me ("Start as baby?" -- if I always pick parent, stop asking). |
| **Frequency**          | Daily, ~10 hours per session.                                                                                                                                                                                                                                              |

### Key risks for Persona B

- **CRIT-4** -- If the baby unit is an old phone (Persona C overlap), inadequate hardware silently degrades reliability over months.
- **CRIT-5 (revised)** -- After a parent-phone crash or OS update, automatic reconnect must be one-tap; otherwise Persona B silently runs unmonitored.
- **IMP-5** -- Auto-resume after reboot is a hard requirement, not a nice-to-have.
- **IMP-6** -- This persona is most exposed to long-term lifecycle risk (security patches, app store availability).

---

## Persona C -- "Hardware-Recycler" (Old Phones/Tablets)

> Example: a tech-aware parent who has an Android 8 device sitting in a drawer and wants to repurpose it as the baby unit, possibly combining with Persona B or A patterns.

| Field                  | Value                                                                                                                                                                                                                                                                    |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Setting**            | Same as the persona they overlap with (typically Persona B). The defining trait is **what device** they choose for the baby side.                                                                                                                                        |
| **Hardware**           | A 3–7 year old Android phone or tablet. **Older Android version** (often API 24–28), **possibly a non-updatable WebView**, **aged battery** (degraded capacity, sometimes swollen), **weaker WiFi radio**, **lower-quality microphone**, **no recent security patches**. |
| **Network**            | Whatever Persona B has. The weak WiFi radio causes more reconnects than a modern device.                                                                                                                                                                                 |
| **Battery assumption** | Plugged in continuously. The aged battery introduces a **fire-safety concern** if it is swollen or otherwise damaged.                                                                                                                                                    |
| **Expectation**        | "The phone in my drawer should be useful again." Privacy and sustainability are values for this persona. They expect the app to **tell them** if their device is suitable.                                                                                               |
| **Failure tolerance**  | Medium-high IF the app is honest about limitations. Will accept "this device is borderline, expect occasional reconnects". Will not accept silent unreliability.                                                                                                         |
| **Anti-expectation**   | Don't refuse to install on old hardware out of paranoia. Don't tell me to buy a new phone for monitoring -- that defeats the point. But also: don't pretend everything is fine if the device clearly is not.                                                             |
| **Frequency**          | Daily (overlaps with Persona B).                                                                                                                                                                                                                                         |

### Key risks for Persona C

- **CRIT-4** -- No documented minimum-hardware requirements today.
- **SUG-7** -- No self-test exists to validate microphone/WiFi/battery health.
- **Battery fire risk** is not mentioned anywhere in the current documentation despite being the most realistic safety incident.
- Older Android versions imply older WebView -> WebRTC feature gaps that may break silently.

---

## Persona Impact Matrix

This matrix shows which features primarily serve which persona, and which features create cross-persona trade-offs (= candidate for ADR).

| Feature                                                            | Persona A (Event)                         | Persona B (Daily)               | Persona C (Recycler)   | Notes                                                                                                                            |
| ------------------------------------------------------------------ | ----------------------------------------- | ------------------------------- | ---------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| [QR Pairing](../features/connectivity-pairing.md)                  | Primary                                   | Primary                         | Primary                | Aligned -- no trade-off.                                                                                                         |
| Same-WiFi connectivity                                             | Limited (often blocked by captive portal) | Primary                         | Primary                | CRIT-2: needs explicit Out-of-Home alternative.                                                                                  |
| Signaling-Server Internet mode                                     | Primary (often the only path)             | Backup                          | Backup                 | Needs to be more visible in Persona A's onboarding.                                                                              |
| Hotspot connectivity                                               | Edge case (works only in same building)   | Rare                            | Rare                   | Currently presented as fallback; OK.                                                                                             |
| [Battery optimization deep-link](../features/device-management.md) | Less important (event-only)               | Critical                        | Critical               | Context-aware nag (e.g. after failed reconnect / pre-event), not a separate persona UI.                                          |
| [Kiosk mode on baby unit](../features/device-management.md)        | Useful (avoid guest tampering at venue)   | Critical (prevent toddler taps) | Critical               | Aligned.                                                                                                                         |
| [Listen mode push-to-talk](../features/communication.md)           | Primary                                   | Primary                         | Primary                | Aligned.                                                                                                                         |
| Headphone-vs-speaker alarm routing                                 | Headphone-friendly                        | Speaker-friendly                | Speaker-friendly       | **Unified** default: follow system; Advanced can prefer speaker -- [ADR-011](../decisions/ADR-011-audio-routing-parent-side.md). |
| Mobile data / roaming warnings                                     | Critical                                  | Rare                            | Rare                   | CRIT-3: **unified** UX -- warn when OS reports metered/roaming on Internet path, regardless of persona.                          |
| Auto-resume after reboot                                           | Less important                            | Critical                        | Critical               | IMP-5: single sensible default for all (baby unit auto-resume on); Advanced may expose toggles.                                  |
| Baby-unit self-test (mic, WiFi, battery)                           | Useful                                    | Useful                          | **Critical**           | Drives [ADR-010](../decisions/ADR-010-baby-unit-hardware-requirements.md) minimum-hardware decision.                             |
| Heartbeat / connection-loss escalation                             | Critical                                  | Critical                        | Critical (more often)  | Aligned.                                                                                                                         |
| Multi-parent (Phase 2)                                             | Backup at distance                        | Convenience + failover          | Convenience + failover | Failover dimension is new -- see [failure-model.md](failure-model.md).                                                           |
| In-app data-volume monitor                                         | Critical                                  | Optional                        | Optional               | Persona A-specific.                                                                                                              |
| Probe-alarm button                                                 | Critical (pre-departure test)             | Useful (post-update test)       | Useful                 | SUG-2: add to MVP.                                                                                                               |
| Hardware-vs-software comparison FAQ                                | Useful (manage expectations)              | Useful                          | Useful                 | SUG-5: low-effort, high-trust.                                                                                                   |

## Unified app -- personas do not fork the UI

> STATUS: DECIDED -- product direction (2026-05); aligns with [ADR-009](../decisions/ADR-009-personas-and-failure-model.md) and [user-interface.md](../features/user-interface.md).

The same app install must serve **Persona B tonight and Persona A next month** without looking or behaving like a different product. **Connection path, alert emphasis, and data-use honesty** adapt to **observed context** (same SSID but isolated AP vs. real LAN path, metered cellular vs. home WiFi, captive portal failure). Users opt into **specific behaviors** only where unavoidable (e.g. roaming acknowledgment, Advanced audio override) -- not by declaring a lifelong “persona profile” at first launch.

**Baby unit is an old phone (Persona C overlap):** optional guided [baby-unit self-test](../features/device-management.md) remains a **device fact**, not a persona mode (offer it from Advanced or when the app detects very old API / WebView).

### Discarded: persona profile at first launch

| Approach                                                                                                | Reason                                                                                                                                                                   |
| ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| First-launch question “mainly home / travel / events” that switches chrome, defaults, or routing policy | Same user crosses contexts; creates false expectations and maintenance of parallel UX paths. Replaced by **one onboarding** + **context-aware** connectivity and alerts. |

## Discarded persona candidates

| Candidate                                | Reason                                                                                            |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------- |
| "Childcare professional / nanny network" | Distinct trust model, not in MVP scope; revisit in Phase 3+ if community asks.                    |
| "Hearing-impaired parent"                | Important accessibility persona, but folded into the i18n / accessibility deferred set (Phase 2). |
| "Pet owner / pet monitor"                | Out of scope per [vision.md](vision.md) Non-Goals -- GuardedSleep is for child monitoring.        |
