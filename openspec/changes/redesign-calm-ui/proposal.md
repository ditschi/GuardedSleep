# Proposal

## Why

The owner asked on 2026-11-14 for a modern, calm and professional look that works in a dark bedroom, plain-language status texts instead of technical terms, intuitive mode switching and settings, and an optional night mode. Today the parent screen mixes technical words ("link", "level"), the connected state is the loudest element on the screen (a green bar), the colour for the first 20 s of an outage is yellow in `link-supervision` but blue in `connection-status`, the baby kiosk shows state by hue alone, and Settings is one long list.

Three design directions (A "Calm Night", B "Precision Instrument", C "Warm Adaptive") were mocked up and the owner chose a combination; it then went through three review rounds with six reviewer roles (scores from 6–7 up to 8–9, no open `CRITICAL` or `IMPORTANT` finding; design.md "Review history"). The mockup is in [`mockup/mockup.html`](mockup/mockup.html).

This change alters core UX concepts and prior decisions: the colour and wording of the connection stages (`link-supervision` "Escalation feedback per device", `connection-status`), how a connected state looks, the names of two decided features ("Discreet" becomes "Vibrate only", "Snooze" becomes "Pause noise alerts"), how Listen is operated (hold-and-slide becomes a tap toggle), how long the restored notice stays, and the baby kiosk indicator. The Critical Reviewer therefore applies (AGENTS.md §6). Core behaviour (states, levels, timing, acknowledgment) is unchanged; only rendering, copy and placement change, plus two read-only status fields the UI needs (design §4).

## What Changes

- **One visual system** for Android and web: warm plum-black "Dusk" (default) and "Day" schemes, a bundled typeface (Plus Jakarta Sans, OFL) with tabular numerals, the horizon line as the one motif, fixed safety colours outside the theme.
- **Calm connected state:** a neutral status bar with a green dot on every parent screen while monitoring, including Settings and sheets; a coloured bar always means a change.
- **Plain-language stages** named by what the phone does: blue "Connecting"/"Reconnecting" only for the first 20 s, orange "Can't reach … · Still trying" from 20 s, red full screen from 1 min, loud alarm from 3 min; no "level", "link", "peer" or L0–L3 in the UI.
- **One full-screen alert template** for noise, connection stages and baby battery critical: label, headline, counter, one sentence with the next stage and its clock time, cause card with the first step, "I'm checking" as the only filled action, a quiet "Stop monitoring" at the end.
- **Banner priority** on the monitor (security › microphone silent › readiness › baby battery › weak connection › Pause › Vibrate only), at most two plus "+N more"; readiness shown on the monitor.
- **Home** shows both roles with their saved connections; **Settings** splits into Connections and This device; connections are read-only while monitoring; no switching while monitoring.
- **Night mode "Ember"**: Off by default, Off · On · Automatic (time window only), never applied to alerts, warning banners, bar states or safety chips. The web "Night screen" becomes "Black screen".
- **Renames:** "Discreet" → "Vibrate only" (with a one-line reminder on Home when starting with it), "Snooze" → "Pause noise alerts" with an 8 s Undo; Listen becomes a tap toggle.
- **Baby kiosk:** always-visible dim status line, shape-coded marks (fine, still trying, no connection, needs attention), a set-up confirmation for the first 60 s, a luminance cap.
- **Restored notice** stays until the parent taps OK.
- **Accessibility rules:** focus on the alert headline, live regions announce stage changes but never ticking timers, layouts that hold at 200 % text, ≥ 48 dp targets, gesture alternatives, defined back behaviour.
- **Copy:** about 90 new or changed keys in all 20 locales; neutral "the child", formal address in German, no spliced names, durations in words.
- **Docs:** `docs/technical/ui-design.md` rewritten from this change; two backlog `IDEA`s (louder reminders at the full-screen stage; automatic night mode from the light sensor).

**Out of scope:** new alert behaviour (louder reminders, a next-morning reset of Vibrate only, a 1 h pause; backlog), the light sensor for night mode (backlog `IDEA`), the alarm tones themselves (`add-connection-alarm-tone`), talk-back behaviour (`add-talk-back`), the second parent device (`multi-parent-pairings`), tablet layouts, video, any change to escalation timing or the core state machine. Role names stay "Parent device" and "Baby device".

**Tier:** T0 (alpha).

**Persona impact** (`docs/concept/personas.md`): serves Persona B (daily home use at 3 AM: calmer screen at night, clearer stages, tap-to-listen) most; Persona A (events and travel) gains plain cause lines for venue Wi-Fi and the stage colours readable in daylight; Persona C (old phones and tablets as baby device) gains the readable kiosk status line and the set-up confirmation. It trades off the expert user who wants technical detail (level numbers and protocol states disappear from the UI; a diagnostics view stays a backlog item) and costs one bundled font (a few hundred KB) on every install.

## Capabilities

### New Capabilities

- `ui-appearance`: colour schemes, fixed safety colours, night mode, the bundled typeface, touch targets, gesture alternatives, large-text layout and back behaviour on both clients.

### Modified Capabilities

- `connection-status`: neutral connected bar on every parent screen, plain stage wording, restored notice until OK, shape-coded baby marks, baby status line and set-up confirmation, full-screen connection alert and troubleshooter presentation, screen-reader announcements.
- `link-supervision`: level 0 renders blue (not yellow); baby marks by shape; two read-only outage timing fields in the link status.
- `parent-alerts`: "Vibrate only" and "Pause noise alerts" renames, banner priority, alert template, readiness on the monitor, Vibrate-only reminder, Undo.
- `audio-monitoring`: Listen as a tap toggle; "All quiet" hidden whenever nothing is monitored.
- `device-setup`: Stop monitoring from the More sheet and the alert screens, Settings structure, read-only connections while monitoring, the baby kiosk's brightness exceptions for the status line and set-up confirmation.
- `web-client`: "Black screen", same monitor and dock as Android, colour scheme before first paint, "Vibrate only" wording.
- `i18n`: copy conventions (neutral person, formal German, no name splicing, durations in words).

## Impact

- `apps/android/app` (theme, components, every screen), `apps/android/ui-contract` (a few presentation fields: banner list, night-mode setting, status counter fields), `apps/android/platform` (night-mode schedule, readiness ranking feed), Roborazzi baselines.
- `apps/web/src` (CSS tokens, components, screens, night mode, Black screen), `apps/e2e` (selectors stay on test ids; new specs for banners and 200 %).
- `crates/guardedsleep-core`: two additive read-only `LinkStatus` fields (design §4); `crates/guardedsleep-{ffi,wasm}` API snapshots.
- `locales/*.json` (about 90 keys), `crates/i18n-gen` unchanged.
- Fonts: `apps/android/app/src/main/res/font/`, `apps/web/src/assets/fonts/`, `LICENSES/OFL-1.1.txt`, `REUSE.toml`.
- Docs: `docs/technical/ui-design.md`, `docs/backlog.md`, `AGENTS.md` (no §9 row changes; §5.2a unchanged).
- Coordination: `add-connection-alarm-tone`, `add-talk-back`, `alternate-roles-and-apk-size`, `parent-pairing-entry`, `multi-parent-pairings`, `quiet-link-no-rtp`, `alpha-connection-model` (design §14).
- Backlog: picks up the UX review part of `PLANNED` "Connection-status UI review".
