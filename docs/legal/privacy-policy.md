# Privacy Policy

> STATUS: DRAFT -- Must be finalized before Google Play / IzzyOnDroid submission (F-Droid main repository is deferred, see [ADR-021](../../openspec/changes/archive/2026-10-20-adr-021-offline-lan-topologies-locales-and-distribution/design.md) §3).
> Language: English (German translation needed for EU compliance).

## 1. Overview

**GuardedSleep – Privacy Baby Monitor** (project repository: GuardedSleep) is an open-source baby monitor application. **Privacy is our core design principle.** The app is designed so that no personal data leaves your device unless absolutely necessary for the monitoring function.

For **project continuity** (updates, expectations if development slows), see [lifecycle.md](lifecycle.md) (non-binding operational notes; not a substitute for this policy).

## 2. Data We Process

### On-Device Only (never transmitted to any server)

| Data                                   | Purpose                              | Storage                              |
| -------------------------------------- | ------------------------------------ | ------------------------------------ |
| Audio from microphone                  | Noise level detection on baby unit   | Processed in real-time, never stored |
| App settings (thresholds, preferences) | User configuration                   | Device storage (SharedPreferences)   |
| Pairing keys (PSK, DTLS fingerprints)  | Encrypted connection between devices | Android Keystore (hardware-secured)  |
| Device names, saved connections        | Reconnection without QR code         | Encrypted device storage             |

### Transmitted Between Your Devices (end-to-end encrypted)

| Data                              | When                                                | Encryption             |
| --------------------------------- | --------------------------------------------------- | ---------------------- |
| Audio stream (child's voice)      | When noise threshold exceeded or listen mode active | DTLS-SRTP (end-to-end) |
| Heartbeat (battery level, status) | Every 10 seconds while connected                    | DTLS (end-to-end)      |
| Noise level data                  | Continuously while connected                        | DTLS (end-to-end)      |
| Alert messages                    | When noise threshold exceeded                       | DTLS (end-to-end)      |

All data transmitted between devices is **end-to-end encrypted**. No server, relay, or third party can read this data.

### Metadata Visible to Third-Party Servers (Internet Mode Only)

If you use Internet mode (signaling server + STUN), the following metadata is visible:

| Server               | What it sees                              | What it does NOT see                   |
| -------------------- | ----------------------------------------- | -------------------------------------- |
| **Signaling server** | Which devices connect, when, IP addresses | Audio, messages, settings, any content |
| **STUN server**      | Public IP addresses of both devices       | Audio, messages, connection content    |

**Mitigation:** You can self-host the signaling server for full control. The Android app uses the STUN service of the signaling server you configured by default; the web client still uses EU-based public STUN servers until it follows. In local mode (WiFi/hotspot), no server is involved at all.

## 3. Data We Do NOT Collect

- We do NOT collect analytics or usage data
- We do NOT use advertising or tracking
- We do NOT transmit audio to any cloud service
- We do NOT store audio recordings (audio is processed in real-time and discarded)
- We do NOT require an account or registration
- We do NOT use Firebase, Google Analytics, or any third-party SDK that collects data

## 4. Children's Data

This app is designed for **parents and caregivers**, not for children. The app processes audio that may contain children's voices. This audio is:

- Processed locally on the device (noise detection)
- Transmitted only to the paired parent device (end-to-end encrypted)
- Never stored, recorded, or transmitted to any server

We do not collect, store, or transmit personal information of children to any server.

## 5. Permissions

| Permission             | Why                                         | When                             |
| ---------------------- | ------------------------------------------- | -------------------------------- |
| **Microphone**         | Audio monitoring on baby unit               | When app is in baby mode         |
| **Camera**             | QR code scanning for device pairing         | Only during pairing              |
| **Notifications**      | Alert parent when baby is detected awake    | Always (baby monitor function)   |
| **DND Override**       | Ensure alerts play even in silent mode      | Optional, user grants explicitly |
| **Foreground Service** | Keep monitoring active in background        | When monitoring is active        |
| **Internet**           | Signaling server connection (internet mode) | Only in internet mode            |

## 6. Data Storage and Security

- Cryptographic keys (PSK) are stored in the **Android Keystore** (hardware-backed, not extractable)
- App settings are stored locally on the device
- No data is stored on any server
- **Device lock:** Users should use the **operating system** screen lock (PIN, pattern, biometrics). The MVP app does not add a separate in-app lock (see [ADR-001](../../openspec/changes/archive/2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/design.md)).

## 7. Your Rights (GDPR)

Since all data is stored locally on your device:

- **Right to access**: All data is visible in the app settings
- **Right to deletion**: Delete saved connections in settings, or uninstall the app for complete removal
- **Right to data portability**: Not applicable (no server-side data)
- **Data controller**: You are the data controller. We do not process your data.

## 8. Open Source

The complete source code is available under the **GNU Affero General Public License v3.0 or later** ([`LICENSE`](../../LICENSE)). You can verify privacy claims by reviewing the code. Third-party forks must comply with AGPL and must not imply they are the official **GuardedSleep** product — see [`trademark.md`](trademark.md).

## 9. Changes

We will update this privacy policy as needed. Changes will be documented in the app's changelog.

## 10. Contact

[Developer contact email]

---

## Google Play Data Safety Declaration

For the Google Play Data Safety section:

| Question                                            | Answer                                 |
| --------------------------------------------------- | -------------------------------------- |
| Does your app collect or share user data?           | No                                     |
| Is all collected data encrypted?                    | Yes (DTLS-SRTP end-to-end)             |
| Can users request data deletion?                    | Yes (uninstall or delete in settings)  |
| Does your app share data with third parties?        | No                                     |
| Does your app collect any of the listed data types? | No (no analytics, no ads, no tracking) |

## DSFA (Data Protection Impact Assessment) -- Summary

> Required because the app processes audio that may contain children's voices.

**Processing activity:** Real-time audio monitoring of a room where a child sleeps.
**Data subjects:** Children (audio), parents/caregivers (device data).
**Risks:** Unauthorized access to audio stream, unauthorized pairing.
**Mitigations:** End-to-end encryption, QR code pairing with physical verification, Android Keystore for keys, kiosk mode on baby unit, **OS-level device lock** recommended for parent device.
**Residual risk:** Low. All processing is local or P2P encrypted. No server-side data storage.
**Conclusion:** Processing is proportionate and adequately protected.
