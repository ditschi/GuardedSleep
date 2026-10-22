# UI Design Specification

## Design Principles

1. **Can a tired parent at 3 AM, in a dark room, with one hand, use this reliably?**
2. Night mode is default -- baby monitors are primarily used in dark environments
3. Large touch targets (min 48dp, preferably 72dp+ for primary actions)
4. Information hierarchy: most important info visible from 1-2m distance
5. Actionable error messages: "What happened? Is my child safe? What should I do?"
6. No jargon -- no "WebRTC", "DTLS", "ICE", "DataChannel" in UI

## Color System

### Night Mode (Default)

| Element                  | Color             | Hex       |
| ------------------------ | ----------------- | --------- |
| Background               | Pure black (OLED) | `#000000` |
| Primary text             | Muted white       | `#AAAAAA` |
| Secondary text           | Dark gray         | `#666666` |
| Accent green (quiet)     | Dark green        | `#116611` |
| Accent yellow (caution)  | Dark yellow       | `#665511` |
| Accent red (alert)       | Dark red          | `#661111` |
| Accent red (alert pulse) | Muted red         | `#882222` |

### Baby Screen Ultra-Dark

Even darker than night mode. Status dots only: 4dp diameter.

## Screens

### 1. Home Screen (Adaptive)

**First launch (no saved connections):**

```
┌──────────────────────────────────┐
│                                  │
│   GuardedSleep                   │
│   Privacy Baby Monitor           │
│                                  │
│   ┌────────────────────────┐     │
│   │  🛏️  Start as Baby Unit │     │  ← 120dp high, full width
│   │  This device stays      │     │
│   │  with the child          │     │
│   └────────────────────────┘     │
│                                  │
│   ┌────────────────────────┐     │
│   │  📱  Join as Parent     │     │  ← 120dp high, full width
│   │  This device comes      │     │
│   │  with you                │     │
│   └────────────────────────┘     │
│                                  │
│   Step 1 of 3: Choose this       │
│   device's role                  │
│                                  │
│                         ⚙️       │
└──────────────────────────────────┘
```

**Returning user (saved connections exist):**

```
┌──────────────────────────────────┐
│                              ⚙️  │
│                                  │
│   ┌────────────────────────┐     │
│   │  Kinderzimmer Max       │     │  ← 80dp card, full width
│   │  Baby Unit • last: today│     │     One tap to reconnect
│   │  Tap to connect         │     │
│   └────────────────────────┘     │
│                                  │
│   ┌────────────────────────┐     │
│   │  Omas Telefon           │     │  ← More saved connections
│   │  Parent • last: 3d ago  │     │
│   └────────────────────────┘     │
│                                  │
│                                  │
│   ┌────────────────────────┐     │
│   │  + New Connection       │     │  ← Less prominent
│   └────────────────────────┘     │
│                                  │
└──────────────────────────────────┘
```

**Key decisions:**

- Returning user sees saved connections first (daily use case > first setup)
- One tap to reconnect (CRITICAL: most common action must be fastest)
- Swipe left on card to delete (with confirmation)
- Connection card shows: editable name, role, last connected, connection type icon
- Error on reconnect: "Device not reachable. Is [name] turned on and on the same network?" + "Retry"

### 2. Parent Screen

```
        ████  ← Ambient connection bar (green=yellow=red) + optional "Test" probe icon
┌──────────────────────────────────┐
│ ⚡ 72%   🔗 Good   🔔 Test   ⚙️   │  ← Status bar: battery, link, probe, settings
├──────────────────────────────────┤
│                                  │
│      ░░░░░░░░░░░░░░░░░░░░░░     │
│      ████████████░░░░░░░░░░     │  ← Noise level bars (35-40%)
│      ████████████░░░░░░░░░░     │     5-8 vertical bars (equalizer)
│      ████████████░░░░░░░░░░     │     Green → Yellow → Red gradient
│      ░░░░░░░░░░░░░░░░░░░░░░     │     Min height: 120dp
│                                  │
│     All quiet since 22:14        │  ← 20-24sp text
│                                  │
├──────────────────────────────────┤
│                                  │
│     ┌────────────────────┐       │
│     │                    │       │
│     │   🎧 LISTEN        │       │  ← Listen button (35-40%)
│     │   Hold or slide     │       │     Min 72dp high, 80% width
│     │   up to lock        │       │     Thumb-reachable (bottom third)
│     │                    │       │
│     └────────────────────┘       │
│                                  │
│     ┌──────┐                     │
│     │ 🔕   │  Snooze             │  ← 48dp, separate from listen
│     └──────┘                     │
│                                  │
└──────────────────────────────────┘
```

**Ambient connection affordance:**

- Thin full-width bar **above** the status row (or behind it) using the same colors as [Visual Alert Language](#7-visual-alert-language) -- green = connected, yellow = reconnecting, red = disconnected.
- When disconnected >60s, bar animates slowly to match connection-lost escalation urgency.

**Probe / test alert:**

- Small **Test** control (icon + label) in the status zone triggers a synthetic alert (see [notifications-alerts.md](../features/notifications-alerts.md)) so caregivers verify DND override + audio routing after updates or before leaving for an event.

**Status bar (Zone 1):**

- Left: Baby battery with icon, color changes at <30% (yellow) and <15% (red, blinking)
- Center: Connection quality as colored dot + text (Good / Medium / Poor)
- Right: Settings gear (small)
- Font: 14sp, with color feedback so text is secondary

**Noise level bars (Zone 2):**

- 5-8 vertical bars side by side (equalizer style), filling bottom to top
- Color gradient: green (quiet) → yellow (medium) → red (loud/threshold exceeded)
- When threshold exceeded: entire area pulses gently red (attention without panic)
- When quiet: bars show minimal baseline (not empty -- shows "system is working")
- Min bar height: 120dp (visible from 1-2m distance)

**"All quiet since..." (Zone 2b):**

- Large text (20-24sp): "All quiet since 22:14"
- When noise detected: text changes to "Noise detected" with pulsing color
- Primary reassurance element

**Listen button (Zone 3):**

- Push-to-listen: hold → audio streams. Release → stops.
- Slide-to-lock: while holding, slide up → locks. Tap again → stops.
- Visual states:
  - Idle: rounded button with headphone icon + "Listen"
  - Active (holding): button grows/brightens, waveform animation shows live audio
  - Locked (slide-to-lock): lock icon appears, button stays active
- Hint text (first time only): "Hold to listen, slide up to lock"

**Alert state:**

- Pulsing red border around noise bars
- Text: "Noise detected since 23:47 • Audio streaming"
- Prominent "Mute" button replaces Listen button
- Snooze options visible: [5 min] [15 min] [30 min] [1h]

### 3. Baby Screen

**Active mode (before kiosk lock):**

```
┌──────────────────────────────────┐
│                                  │
│          ✅ Connected             │
│      with [Parent device name]   │
│                                  │
│        ⚡ Charging (72%)          │
│                                  │
│     ── Monitoring active ──      │
│                                  │
│    ┌──────────────────────┐      │
│    │   🔒 Lock Screen      │      │
│    └──────────────────────┘      │
│                                  │
└──────────────────────────────────┘
```

**Kiosk mode (locked) -- auto-dimmed:**

```
┌──────────────────────────────────┐
│                                  │
│                                  │
│                                  │
│           📶  🔋                  │  ← Small icons (12dp):
│                                  │     Signal + Battery
│                                  │     (not dots -- recognizable icons)
│                                  │  ← Almost completely black
│                                  │
│                                  │
│                                  │
│                                  │  ← No text visible by default
│                                  │
└──────────────────────────────────┘
```

**After touch (screen wakes briefly, 3 seconds):**

```
┌──────────────────────────────────┐
│                                  │
│                                  │
│           📶  🔋 72%              │  ← Icons brighten + battery %
│                                  │
│     ── Monitoring active ──      │  ← Status text fades in
│                                  │
│                                  │
│                                  │
│    Hold & slide up to unlock     │  ← Unlock hint appears
│                                  │
└──────────────────────────────────┘
```

**Screen dimming behavior:**

- After kiosk lock: screen dims to minimum brightness over 5 seconds
- Then: screen goes to near-black (only tiny status icons visible at lowest brightness)
- Touch anywhere: screen wakes briefly (3 seconds), shows status + unlock hint, then dims again
- This prevents baby from being disturbed by screen glow
- Option in settings: screen completely off (icons only visible on touch)

**Status icons (instead of dots):**

- Signal strength icon: 📶 (3 bars style), color-coded (green/yellow/red)
- Battery icon: 🔋 with fill level, color-coded (green/yellow/red)
- Icons are 12dp -- small enough to minimize light, large enough to be recognizable
- Icons are always visible (at minimal brightness) unless "screen off" option is enabled

**Unlock mechanism: Hold + Slide (like push-to-talk pattern)**

- Touch and hold anywhere on screen → unlock button/area appears at bottom
- While holding, slide upward to the unlock area → screen unlocks
- Release before reaching unlock area = nothing happens, screen dims again
- Visual feedback: sliding progress indicator (like the listen button slide-to-lock, but in reverse)
- Consistent interaction pattern: same gesture family as push-to-listen + slide-to-lock on parent screen
- Nearly impossible for baby to trigger: requires sustained press + directional slide
- After unlock: 30s inactivity timeout → auto re-lock with 5s warning

**Connection loss on baby screen:**

- Signal icon → red, exclamation mark overlay
- Optional: subtle red pulse on icon (very muted, to not disturb child)
- Auto-reconnect in background
- On touch-wake: shows "Connection lost. Auto-reconnecting..." text

### 4. Settings Screen

> **MVP structure:** split **Basic** vs **Advanced** (toggle at bottom of Basic). Advanced hides heartbeat, signaling URL, codec experiments, security diagnostics, and debug export ([user-interface.md](../features/user-interface.md)). **Parent alert output and gain** live under **Basic → Notifications** ([ADR-011](../../openspec/changes/archive/2026-10-21-adr-011-audio-routing-parent-side/design.md)).

**Basic**

```
⚙️ Settings                      [ Show advanced settings ▼ ]

├── 🔔 Notifications …
│   ├── Threshold / duration / snooze …
│   ├── Alert audio output        [ Default | Speaker only | Speaker + default ]
│   ├── Alert playback gain       [slider]
│   └── Minimum gain during alert [toggle]
├── 🌙 Display …
├── 📱 Saved Connections …
├── 🔋 Battery & Background …
└── ℹ️ About / Help …
```

**Advanced** (collapsed by default)

```
⚙️ Settings                      [ Hide advanced settings ▲ ]

├── … (all Basic groups)
├── 🛠️ Advanced monitoring
│   ├── Heartbeat interval        [5s / 10s / … / 30s]
│   ├── Audio codec / bitrate    [Auto / …]
│   └── Signaling URL override   [text field]
├── 🧪 Diagnostics
│   ├── Export debug log
│   └── Emoji security fingerprint (pairing audit)
```

Legacy flat outline (reference only -- do **not** ship as a single list to users):

```
⚙️ Settings
│
├── 🔔 Notifications
│   ├── Volume threshold          [Slider with live preview]
│   ├── Duration before trigger   [3s / 5s / 10s / 15s]
│   ├── Alert tone                [Selection + preview]
│   ├── Vibration                 [On / Off]
│   ├── Screen wake               [On / Off]
│   └── Default snooze duration   [5 / 15 / 30 / 60 min]
│
├── 🌙 Display
│   ├── Night Mode                [Always dark / Light / Auto]
│   ├── Auto-switch               [By time / System setting]
│   └── Keep screen on            [On / Off]
│       (in listen mode)
│
├── 📱 Saved Connections
│   ├── [Device 1]                [Edit / Delete]
│   ├── [Device 2]                [Edit / Delete]
│   └── Delete all connections
│
├── 🔋 Battery & Background
│   ├── Battery optimization      [→ Open system settings]
│   └── Explanation text
│
├── 🔐 Security
│   ├── App lock (PIN/biometrics) [Phase 2 -- not MVP; use OS screen lock]
│   └── Connection details        [→ Fingerprint, encryption]
│
└── ℹ️ About
    ├── Version
    ├── Privacy policy
    ├── Open source licenses
    └── Support the developer     [→ Donate]
```

**Volume threshold slider:**

- Live preview: while adjusting, noise bars show current room level and chosen threshold as a line
- Labels: "More sensitive ← → Less sensitive" (no dB values)
- Default: slightly above middle

**Battery optimization deep-link:**

- Android-only (hidden on iOS in Phase 2)
- Opens device battery optimization settings directly
- OEM-specific hint if possible ("Samsung: Look for 'Don't restrict app in standby'")
- Link to dontkillmyapp.com

## 5. Phone vs. Tablet

### Breakpoints

| Width     | Device                     | Layout                  |
| --------- | -------------------------- | ----------------------- |
| < 600dp   | Phone                      | Single column (primary) |
| 600-840dp | Small tablet / large phone | Enlarged single column  |
| > 840dp   | Tablet                     | Two-column layout       |

### Tablet Parent Screen (Landscape)

```
┌─────────────────────────────┬──────────────────────────┐
│                             │                          │
│   Noise level bars          │   ⚡ 72%  Battery        │
│   (larger, more bars)       │   🔗 Good  Connection    │
│                             │                          │
│   [Mini-history: 10 min]   │   All quiet              │
│                             │   since 22:14            │
│                             │                          │
│                             │   [🎧 LISTEN button]     │
│                             │   [🔕 Snooze button]     │
│                             │                          │
└─────────────────────────────┴──────────────────────────┘
```

## 6. Alert Escalation

### Connection Loss

| Time     | Level      | Behavior                                                                                                            |
| -------- | ---------- | ------------------------------------------------------------------------------------------------------------------- |
| 0-20s    | Silent     | Auto-reconnect in background. Connection icon → yellow.                                                             |
| 20s-60s  | Gentle     | Subtle vibration + small banner: "Connection interrupted -- reconnecting..."                                        |
| 60s-3min | Urgent     | Louder tone + persistent notification + vibration. "Connection to baby device lost for [X]."                        |
| > 3min   | Full alarm | Max tone (override DND if possible), screen wake, repeating vibration. "Connection lost! Please check baby device." |

### Noise Alert

- Threshold exceeded → **vibration + screen on + app to foreground + volume meter / noise bars live + child audio** on the route selected per [ADR-011](../../openspec/changes/archive/2026-10-21-adr-011-audio-routing-parent-side/design.md) (daily profile still biases to speaker for audibility)
- No separate alarm ringtone -- the child's actual voice/cry IS the alert
- Audio plays on ALARM stream (works even in silent mode / DND)
- This is more natural and less anxiety-inducing than a synthetic alarm sound
- Repeated alerts: no escalation needed, audio already playing. Visual pulse indicator.
- Snooze: pauses notifications, monitoring continues, noise events recorded
- Re-trigger: after 2+ min of silence followed by new noise → new full alert

### Snooze Behavior

- Snooze pauses NOISE alerts only
- Connection loss and low battery warnings are NEVER snoozed (safety-critical)
- Visual indicator: "Snooze active: 12 min remaining" (yellow banner)
- Auto-resume after snooze expires

## 7. Visual Alert Language

| State              | Color         | Animation               | Meaning                        |
| ------------------ | ------------- | ----------------------- | ------------------------------ |
| All quiet          | Green/neutral | Static                  | Child sleeping                 |
| Light noise        | Yellow        | None                    | Below threshold, informational |
| Threshold exceeded | Muted red     | Gentle pulse (2s cycle) | Alert active                   |
| Connection shaky   | Yellow        | None                    | Brief interruption             |
| Connection lost    | Red           | Fast pulse (1s cycle)   | Urgent                         |
| Snooze active      | Yellow        | None                    | Notifications paused           |
| Low battery        | Orange        | None                    | Information                    |

## Open Questions

- Exact alert tone selection (warm, mid-pitch, not shrill)
- Historical mini-graph (10 min noise history) -- include in MVP or Phase 2?
- Screen completely off (not just dimmed) as baby mode option?
- Settings wizard on first start (guide through 2-3 most important settings)?
- Accessibility: WCAG 2.1 AA target, min contrast ratio 4.5:1
