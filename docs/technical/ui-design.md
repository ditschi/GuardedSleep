# UI design

> **Reference only (2026-11-19):** this page summarises the built "calm" redesign from the OpenSpec change [`redesign-calm-ui`](../../openspec/changes/redesign-calm-ui/proposal.md) (design in its [`design.md`](../../openspec/changes/redesign-calm-ui/design.md), PRs #69–#80). The **specs are the source of truth**: [`openspec/specs/`](../../openspec/specs/) and, until the change is archived, its delta specs ([`ui-appearance`](../../openspec/changes/redesign-calm-ui/specs/ui-appearance/spec.md), [`connection-status`](../../openspec/changes/redesign-calm-ui/specs/connection-status/spec.md), [`parent-alerts`](../../openspec/changes/redesign-calm-ui/specs/parent-alerts/spec.md), [`device-setup`](../../openspec/changes/redesign-calm-ui/specs/device-setup/spec.md), [`link-supervision`](../../openspec/changes/redesign-calm-ui/specs/link-supervision/spec.md), [`audio-monitoring`](../../openspec/changes/redesign-calm-ui/specs/audio-monitoring/spec.md), [`web-client`](../../openspec/changes/redesign-calm-ui/specs/web-client/spec.md), [`i18n`](../../openspec/changes/redesign-calm-ui/specs/i18n/spec.md)). Where this page and a spec differ, the spec wins.
>
> **Visual reference:** the reviewed mockup [`mockup.html`](../../openspec/changes/redesign-calm-ui/mockup/mockup.html) (54 frames, token panel, EN/DE copy table). Frame numbers below refer to it.
>
> **Status rule for this file:** every section carries a `STATUS` tag; everything tagged `BUILT` is on `main` on Android and web unless the section says otherwise.

## 1. Principles

> `STATUS: BUILT`

1. **Calm by default, unmistakable when it matters.** Connected is neutral; a coloured bar always means a change. Alert surfaces are dark so they do not light the room.
2. **People words.** Stages are named by what the phone does. No "level", "link", "peer", L0–L3, protocol terms or IP addresses in the UI. The baby is "the child".
3. **Never colour alone.** Every state has colour, a symbol and words; the baby kiosk adds shape.
4. **Each state is said once.** A banner is the single statement of a state. Information is never drawn as a disabled button.
5. **Nothing looks calm while nothing is monitored.** "All quiet" hides when the microphone is silent, the baby stopped or the report is incomplete.
6. **Thumb zone, one hand.** Listen is a tap toggle; Talk is the only hold. Targets are at least 48 dp.

## 2. Colour schemes and safety colours

> `STATUS: BUILT`

One token set for both clients (values in the mockup's token panel, source `mockup/gen_css.py`):

| Scheme | Use                      | Key values                                                                                 |
| ------ | ------------------------ | ------------------------------------------------------------------------------------------ |
| Dusk   | dark, default            | background `#141218`, primary `#C9BCEB`                                                    |
| Day    | light                    | white cards with a 1 px `#E0D8DF` border on `#FAF7F4`                                      |
| Ember  | night mode (parent only) | black background, text `#D19586` (7.9:1), variant `#A87365` (5.0:1); dim through backlight |

Body text reaches at least 4.5:1 in every scheme, text in the main monitoring states at least 7:1. The old dark-theme setting maps to Dusk or Day.

**Fixed safety colours** live outside the schemes and never change with Dusk, Day or night mode: info blue, warning orange, alert red, caution yellow, ok green (a dot only, when calm), plus the full-screen alert palettes:

| Palette             | Background | Slab      | Used for                       |
| ------------------- | ---------- | --------- | ------------------------------ |
| Noise (wine)        | `#250A10`  | `#7A1E30` | noise alert                    |
| No connection (red) | `#2A0709`  | `#8C1D24` | connection alert from 1 min    |
| Loud alarm (red)    | `#3A080B`  | `#951E27` | loud alarm, with a hazard band |
| Battery critical    | `#1F1209`  | `#7A3D12` | baby battery below 5 % (amber) |

Red is reserved for "you can't hear the child". Slab text is at least 7.4:1 with a mandatory 2 px light border. In Dusk and Day no non-alert element except the logo uses a hue within 20° of a safety hue; brand apricot appears only in the logo.

In code: Android `SafetyColors` (a `CompositionLocal` independent of the scheme), web `--safety-*` custom properties on `:root` that no `[data-scheme]` rule overrides. The scheme attribute is set by a small module before the first paint (no inline script, CSP unchanged).

## 3. Typography and motif

> `STATUS: BUILT`

- **Plus Jakarta Sans** (SIL OFL 1.1), bundled in the APK and served from the web client's own origin; never loaded from a third-party server. Subset to Latin and Latin Extended. Other scripts (Cyrillic for uk, Arabic, Devanagari, CJK, Hangul) use the system font; no second face is bundled.
- Tabular numerals for counters and times. Sizes 12 (caps labels) / 14 (floor for running text) / 15 / 16 / 20 / 28 / 52 (counters).
- **Motif:** the horizon line: the logo, the hairline under the status bar and the Home header, and the monitor's sound strip (last minute of level history, dashed line "A sound above this line sends an alert").

## 4. Navigation

> `STATUS: BUILT` (Android); web see the note below

```
Home (both roles, up to 2 connections each, "Start") ──▶ Parent monitor │ Baby kiosk
 └ Settings: Connections · This device
Monitor: status bar · tool row [Alert chip][night][gear] · hero line · sound strip · meta line
         dock: Listen (tap) · Hold to talk · Pause noise alerts · More (sheet; Stop monitoring apart)
```

- **One Home with both roles** ("Parent device", "Baby device"), each with its saved connections. Starting a connection chooses the role. The first start shows two large choices without "Use as default".
- **Settings** has two groups. **Connections**: saved connections per role with rename and forget; read-only while monitoring ("Stop monitoring to change connections"). **This device**: parent rows Alerts, Display, Open at start, Check setup, Language, About, Advanced; baby rows Sound detection, Quiet mode, Talk-back, Open at start, Check setup, Language, About, Advanced (no Display). Risky settings stay in Advanced. The connection alarm tone is in This device › Alerts.
- **Open at start**: _Ask every time_ (default), _Baby unit_, _Parent unit_, and the parent's automatic-start connection, which starts monitoring once per app start. "Choose another connection" stops that start and shows Home.
- **No switching while monitoring.** The way out is Stop monitoring (confirm) → Home → choose. The stop confirmation silences and acknowledges nothing.
- **Back:** monitor → app to background (monitoring continues); full-screen alert → nothing; sheet → closes; Settings → monitor. Web: confirm before leaving while monitoring.
- **Web:** same monitor and dock (Hold to talk with Space held). Settings stays one page with both groups until the web has saved connections (task 4.6 open). The web adds the sound gate, the keep-page-open and background banners and **Black screen** (formerly "Night screen"; a black cover, first tap shows status, any alert replaces it).

## 5. Connection states

> `STATUS: BUILT`

Colour comes from the core's level and state only; the platform adds no timers. The core exposes `down_since_ms` and `next_level_at_ms` so the UI can show the counter and the next stage's clock time without copying thresholds.

| Core state, timing                      | Look                                                | Copy (EN, example connection "Nursery")                                                                | Feedback                        |
| --------------------------------------- | --------------------------------------------------- | ------------------------------------------------------------------------------------------------------ | ------------------------------- |
| connected                               | neutral bar, green dot, link symbol                 | "Connected · Nursery"                                                                                  | none                            |
| `connecting`, 0–20 s                    | blue, refresh                                       | "Connecting · Nursery" / "Waiting for the baby device. …"                                              | none                            |
| `reconnecting`, 0–20 s                  | blue, refresh                                       | "Reconnecting · Nursery" / "Short drops are normal."                                                   | none                            |
| `connecting`, 20–60 s                   | orange, 2 px outline, broken link                   | "Not connected yet · Nursery" / "… Is monitoring started on it?"                                       | quiet notification              |
| `reconnecting`, 20–60 s                 | orange, 2 px outline, broken link                   | "Can't reach Nursery · Still trying" / "If it isn't back after 1 minute, this phone vibrates …"        | quiet notification              |
| from 1 min                              | red full screen (first connect: "Couldn't connect") | label "Your phone is vibrating", "Please check on the child", counter, "Loud alarm in 2 min (01:07) …" | vibration, soft tone every 10 s |
| acknowledged                            | red, static, pause                                  | label "Alarm paused", "Comes back at 03:17 if still no connection"                                     | silent until it ends            |
| from 3 min, or after the acknowledgment | red with hazard band, speaker                       | label "Loud alarm sounding", "Still no connection to Nursery", "Go to the child now."                  | loud alarm on the alarm stream  |
| `peer_stopped`                          | grey, pause                                         | "Stopped · Nursery" / "Check who stopped it on the baby device."                                       | one alert, no escalation        |
| needs the user                          | red container                                       | "The code didn't match" / "One device needs an update"                                                 | as before                       |
| restored                                | card until OK, check circle                         | "Connected again" / "Monitoring was interrupted for 1 min 48 s." plus the missed-sound result          | notification, same text         |

- **Blue only for the first 20 s**, orange from 20 s, red full screen from 1 min, loud alarm from 3 min. Yellow is no longer a connection colour.
- The status bar is on every parent screen while monitoring, including Settings and sheets.
- The **restored card** stays until the parent taps OK; a new outage replaces it.
- Troubleshooter causes in the cause card: "The baby device isn't online", "This phone has no internet", "The connection is blocked on this network". "Try now" resets the backoff. Before the first connection, "Wrong connection?" stops with one tap in the first minute and goes through the stop confirmation from 1 min.
- Full EN/DE copy is in the mockup's copy table (★ marks `critical` keys).

## 6. Alert template

> `STATUS: BUILT`

One template for noise, connection from 1 min and baby battery critical (frames 08, 12, 13, 25, 26, 31), top to bottom:

1. Flat status strip and a caps label saying what the phone does (both hidden from accessibility).
2. Headline, then a counter with its label below (spoken in words).
3. One sentence with the next stage and its clock time; decorative progress segments with words ("Phone vibrating now").
4. Cause card: cause line, step 1 visible, "Try now", "More steps" folded.
5. A quiet "Stop monitoring" text button at the end of the content.
6. One filled, pinned action: "I'm checking" (connection, until acknowledged; none while acknowledged), "Listen" / "Stop sound" (noise).

The body scrolls; the action stays pinned at 200 % text. Sound and vibration are as built.

## 7. Banners on the monitor

> `STATUS: BUILT`

One global order: **security warning › microphone silent › readiness (worst item, with "Fix") › baby battery low or unplugged › baby device may ring › weak connection › noise alerts paused › Vibrate only**. Browser banners (sound gate, keep this page open, back from the background) rank below readiness.

At most **two banners show; the rest fold into "+N more"** (a 48 dp text link; spoken "2 more notices: …"). When the Vibrate-only banner is folded, the alert chip shows its label again. Each new warning banner is announced once. The ranking is one pure function per client (Kotlin in `ui-contract`, TypeScript in `apps/web`) with a mirror test. The talk-back noise pause is the talk panel's status line, not a banner.

Renames: "Discreet" → **"Vibrate only"** (one reminder line on Home while it is set); "Snooze" → **"Pause noise alerts"** (10 or 30 min) with an 8 s Undo, offered only when no noise event is running.

## 8. Baby kiosk

> `STATUS: BUILT`

Near-black at minimum brightness, no sound or vibration, no night mode setting. A dim status line (`#7D7671`, 4.7:1) is always visible ("Listening · Travel cot", "Parent device not responding — still trying", "Needs attention: not charging · 12 %"). **Marks by shape**, readable at arm's length without hue:

| State                            | Mark                                                    |
| -------------------------------- | ------------------------------------------------------- |
| fine                             | short solid bar, 44 × 6 dp, centred                     |
| waiting, or first 20 s of a drop | no mark                                                 |
| still trying (20–60 s)           | dashed top edge                                         |
| no connection (from 1 min)       | 5 dp frame, slow 4 s pulse (static with reduced motion) |
| needs attention                  | bar plus a separate 10 dp square                        |

No mark is brighter than `#5A1414` (relative luminance cap). For up to 60 s after the first verified contact and on every touch-wake, a set-up confirmation shows at wake brightness: "Connected to Anna's phone · Listening" (fallback "Connected to the parent device"). During a talk only the talk indicator shows. Unlocking has an accessibility action with confirmation.

## 9. Night mode

> `STATUS: BUILT`

- **Off · On · Automatic**, Off by default, on the **parent device only**. Controls: the monitor tool row, the More sheet and Settings › Display.
- **Automatic uses a daily time window only** (device clock, no light sensor; the sensor is a backlog `IDEA`). A manual change wins until the next start or end of the window.
- Night mode applies Ember. It **never** applies to full-screen alerts, the status bar from 20 s on and in grey or red states, warning banners or safety chips: they keep the fixed safety colours.

## 10. Connection alarm tone

> `STATUS: BUILT` (change `add-connection-alarm-tone`, #65/#70)

This device › Alerts › "Connection alarm tone": Classic (default), **Low** (tag "Hard of hearing", hint "Hard of hearing? Choose Low."), Beacon, Rising, Chime. A preview plays soft once, then the alarm, at reduced volume. "Test connection alarm" warns that it plays at full alarm volume. A real alert stops any preview. Noise alerts always play the child's own sound.

## 11. Accessibility

> `STATUS: BUILT`

- **Alert entry:** focus moves to the headline; focus returns on close; nothing is said twice.
- **Live regions** announce stage changes only (polite in the first minute, assertive from 1 min, stopped baby, needs user, microphone silent). Timers and countdowns are never live and carry word labels ("1 minute 12 seconds").
- **200 % text:** alert bodies scroll with the action pinned; the counter shrinks above 1.5×; the tool row wraps to two rows (chip on row 1, night and gear on row 2; order status → alert → night → settings); below 600 dp height only the status bar and Listen stay pinned. Font-scale 2.0 snapshots cover the monitor and the alerts.
- **Targets** ≥ 48 dp, dock tiles ≥ 72 dp; buttons never wrap or cut their label.
- **Gesture alternatives:** Listen is a toggle with a state description; Talk keeps the screen reader's double-tap-and-hold; kiosk unlock is an accessibility action; every sheet has a visible Close.
- **Disclosures** ("More steps") are buttons with an expanded state. The icon-only alert chip says "Alerts for sounds: Vibrate only, Change".
- **Copy conventions** (`i18n`): neutral person, formal German, names only in "State · Name" labels, durations in words ("1 min 48 s"), never `m:ss`.

## 12. Open items

> `STATUS: PLANNED` (tasks in [`tasks.md`](../../openspec/changes/redesign-calm-ui/tasks.md))

- Web Settings sub-pages and Open at start once the web has saved connections (task 4.6).
- Emulator gate check of the restored card (task 8.3).
- Timed-talk accessibility action "Talk for up to 30 s": open owner question (design §15); not built.
