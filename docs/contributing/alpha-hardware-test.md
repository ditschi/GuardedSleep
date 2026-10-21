# Alpha hardware test plan

`STATUS: UNDER DISCUSSION` — companion to [ADR-018](../decisions/ADR-018-alpha-milestone-scope-and-trust-features.md) §4/§5 and [ADR-016](../decisions/ADR-016-test-strategy-local-virtual-physical.md) Stage 3. Results are recorded per device as a local `gs-device-report/1` ([test-schemas.md](../technical/test-schemas.md) §3); nothing is uploaded.

## Tester briefing (read first — conditions from the Critical Review 2026-10-20)

- **This is an alpha. With a real child, keep an independent baby monitor running** (or an adult nearby) until the 8 h soak (Round 3) has passed on exactly this device pair.
- **Connection-loss alarms come after 60 s.** Between 20 s and 60 s the parent unit only shows orange/red; sound starts at 60 s (soft tone every 10 s), continuous from 3 min.
- **The parent unit is a single point of failure.** If it dies or the app is killed, nobody is alerted; the baby unit only shows a dim pulse.
- **A browser parent works only while its page is open and the screen is on.** Never use a browser as the only parent unit overnight.
- **Round 1 runs on home Wi-Fi with a LAN or self-hosted signaling server.** Mobile-data rounds only with a TURN server configured.
- **Record every run:** device, Android version, OEM skin, network type. Nothing counts as "protected" on a device until the soak passed on it; otherwise label results "not soak-tested".

## 0. Before any hardware round

- [ ] Host simulation green: `cargo test -p guardedsleep-sim -p guardedsleep-core` (AC-9, AC-10 on the host).
- [ ] Emulator gate green: `scripts/poc/local-gate.sh` (AC-1 … AC-4, AC-10; 30 min Doze).
- [ ] Debug APK installed on both units (`adb install -r -g app-debug.apk`; app id `org.guardedsleep.app.alpha`).
- [ ] Signaling: a LAN or self-hosted server as **primary** (`just stack-lan` on a PC/NAS, or the Traefik setup) and a public server as **fallback** in Advanced → signaling URLs; TURN configured for mobile-data runs. Hotspot without internet (UC-4) is not supported in the alpha.
- [ ] Note per device: manufacturer, model, Android version, OEM skin, battery health, whether it is rooted.

## 1. Devices

| Role                     | Minimum                               | Why                                         |
| ------------------------ | ------------------------------------- | ------------------------------------------- |
| Reference                | one recent Pixel or stock-like device | baseline behaviour                          |
| Aggressive OEM           | one Samsung **or** Xiaomi/OPPO        | battery manager, DND and full-screen quirks |
| Old phone as baby (C)    | Android 8–10 phone                    | Persona C, process killing, weak Wi-Fi      |
| Laptop / desktop browser | Chrome and Edge                       | web baby and web parent roles               |
| Optional                 | tablet as baby                        | screen size, charging habits                |

## 2. Round 1 — native ↔ native at home (Wi-Fi, LAN server)

| ID                 | Scenario                                             | Pass when                                                                          |
| ------------------ | ---------------------------------------------------- | ---------------------------------------------------------------------------------- |
| S-pair-qr          | Baby shows code, parent scans it                     | connected ≤ 15 s; code closes after pairing; expired code shows "Show new code"    |
| S-pair-paste       | Paste the code instead of scanning                   | same as above                                                                      |
| S-one-tap          | Stop both, start parent from home                    | connected ≤ 15 s without touching the baby unit                                    |
| S-noise-locked     | Parent locked, cry/noise near baby unit              | full-screen alert + vibration + child audio ≤ 2 s                                  |
| S-noise-silent-dnd | Parent on silent **and** in DND                      | alert still audible (alarm stream, DND bypass)                                     |
| S-probe            | Test alert from parent                               | full chain on parent; **nothing audible or visible changes on the baby unit**      |
| S-listen           | Push-to-listen, then slide-to-lock                   | audio within 2 s; lock persists; auto-stop after 60 min                            |
| S-wifi-baby-short  | Baby Wi-Fi off 20 s, on again                        | no level 2; reconnects by itself; listen resumes if it was on                      |
| S-wifi-baby-long   | Baby Wi-Fi off 90 s                                  | level 2 alarm ≤ 62 s after last contact; restored banner afterwards; no QR re-scan |
| S-wifi-parent      | Same two cases on the parent unit                    | same expectations, parent side shows them                                          |
| S-kill-baby        | Swipe away / force-stop the baby app                 | parent level 2 ≤ 62 s; "Tap to resume" notification on the baby                    |
| S-kill-parent      | Force-stop the parent app, reopen                    | parent resumes monitoring by itself (sticky service) or with one tap               |
| S-stop-baby        | Stop monitoring on the baby unit                     | parent shows one "Baby unit stopped monitoring" alert, no escalation               |
| S-reboot-baby      | Reboot the baby unit overnight-style                 | parent reaches level 2 (auto-start after reboot is deferred)                       |
| S-router-reboot    | Reboot the router                                    | recovers without user action; level depends on outage length                       |
| S-server-restart   | Restart the signaling server while connected         | no visible change; later outages still recover                                     |
| S-battery          | Run the baby to < 15 % unplugged; unplug the charger | low-battery and "no longer charging" notices on the parent                         |

## 3. Round 2 — child-side silence (baby unit)

| ID                | Scenario                                                    | Pass when                                                                                     |
| ----------------- | ----------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| S-silence-call    | Call the baby unit                                          | no ring, no vibration, **screen stays dark**                                                  |
| S-silence-sms     | SMS and a messenger message                                 | same                                                                                          |
| S-silence-charge  | Plug/unplug the charger                                     | no sound; screen stays dark (also check "wake screen when plugged in" OEM setting)            |
| S-silence-lowbat  | System low-battery warning                                  | no sound                                                                                      |
| S-silence-alarm   | Alarm clock set on the baby unit                            | **rings** (allowed by design); parent showed "Alarms can still sound" and the next alarm time |
| S-silence-status  | Turn ringer to vibrate / normal manually during monitoring  | parent chip changes to "may buzz" / "may ring" within one heartbeat interval                  |
| S-silence-remote  | Revoke the app's DND rule manually, then "Silence remotely" | rule re-applied; result shown on both units                                                   |
| S-silence-restore | Stop monitoring                                             | previous ringer/DND mode is back                                                              |
| S-silence-crash   | Force-stop the baby during monitoring, reopen the app       | leftover rule removed and the user told so                                                    |
| S-app-silent      | Whole round                                                 | GuardedSleep itself never makes a sound, vibration or haptic on the baby unit                 |

## 4. Round 3 — overnight soak (≥ 8 h, ADR-016 §5)

Both phones in Doze, baby on charger (Persona B) and once on battery; repeat on the old phone. Record the soak metrics of `gs-device-report/1`: heartbeats expected/received, max gap, gaps over the alarm threshold, false alarms, battery start/end, foreground-service alive samples. Pass: no false level-2 alarm, no unnoticed monitoring loss, battery drain documented.

## 5. Round 4 — browser roles

Chromium-based browsers only for the alpha (Chrome, Edge): Firefox does not expose the DTLS certificate in its stats, so the mandatory fingerprint check fails and the page shows "This browser can't verify the secure connection".

| ID            | Scenario                                        | Pass / record                                                                                |
| ------------- | ----------------------------------------------- | -------------------------------------------------------------------------------------------- |
| S-web-baby    | Laptop Chrome/Edge as baby ↔ native parent      | pairing, noise alert, probe, outage recovery; parent shows silence status "unknown"          |
| S-web-parent  | Android Chrome as parent ↔ native baby          | alerts with the tab open; document what happens with the tab backgrounded and the screen off |
| S-web-reload  | Reload the web page                             | re-pairing required (PSK is memory-only) — shown honestly                                    |
| S-mobile-data | Both phones on mobile data, public server, TURN | pairing and recovery work (Persona A smoke)                                                  |

## 6. Reporting

For each device: one report file, failures with steps, timestamps (date only in shareable copies) and screenshots; no IP addresses, SSIDs or pairing codes in anything shared.
