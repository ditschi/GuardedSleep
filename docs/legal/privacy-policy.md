# Privacy Policy

> STATUS: DRAFT -- Must be finalized before Google Play / IzzyOnDroid submission (F-Droid main repository is deferred, see [ADR-021](../../openspec/changes/archive/2026-10-20-adr-021-offline-lan-topologies-locales-and-distribution/design.md) §3).
> Language: English (German translation needed for EU compliance).

## 1. Overview

**GuardedSleep – Privacy Baby Monitor** (project repository: GuardedSleep) is an open-source baby monitor application. **Privacy is our core design principle.** Only your two paired devices can hear the audio or read the alerts. To find each other over the internet, the devices use a signaling server; by default that is the GuardedSleep project server. It sees network addresses and usage times, never the sound, the alerts or the pairing key, and it keeps no IP logs. You can switch it off.

For **project continuity** (updates, expectations if development slows), see [lifecycle.md](lifecycle.md) (non-binding operational notes; not a substitute for this policy).

## 2. Data We Process

### Kept on your device only

| Data                                                                                                                                            | Purpose                                         | Storage                                                                                                              |
| ----------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Audio from microphone                                                                                                                           | Noise level detection on baby unit              | Processed in real-time, never stored                                                                                 |
| App settings (thresholds, preferences)                                                                                                          | User configuration                              | App-private device storage                                                                                           |
| Pairing record: the pairing key (PSK), the parent secret, the list of signaling servers of the pairing ("mailboxes"), a TURN server you entered | Reconnecting the two devices without a new scan | App-private file, encrypted with a hardware-backed Android Keystore key (Android); memory only, lost on reload (web) |

The app stores no device names, no DTLS fingerprints and no recordings.

### Transmitted between your devices (end-to-end encrypted)

| Data                                                             | When                                                | Encryption             |
| ---------------------------------------------------------------- | --------------------------------------------------- | ---------------------- |
| Audio stream (child's voice)                                     | When noise threshold exceeded or listen mode active | DTLS-SRTP (end-to-end) |
| Heartbeat (connection status, coarse battery level, noise level) | About every 10 seconds while connected              | DTLS (end-to-end)      |
| Alert messages                                                   | When noise threshold exceeded                       | DTLS (end-to-end)      |

All data transmitted between devices is **end-to-end encrypted**. No server, relay, or third party can read this data.

### What servers on the way see

To set up a connection, the devices exchange sealed (encrypted and authenticated) setup messages through the signaling servers of the pairing. If no direct connection is possible, the encrypted audio may be relayed through a TURN server.

| Server                                                                                                                     | What it sees                                                                                                                                                                                                       | What it never sees                                                      |
| -------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------- |
| **Signaling server** (by default the GuardedSleep project server, `gs.mydata.rocks`, operated by the project in Germany)   | Your devices' IP addresses and connection times while connected; a room id derived from the pairing key that cannot be turned back into it; the device role (baby or parent); sealed setup messages it cannot open | Audio, alerts, settings, the pairing key, the content of setup messages |
| **TURN server** (only the one on the signaling server's own host, or one you entered)                                      | The relayed devices' IP addresses (including local network addresses), and the timing and volume of the encrypted packets it forwards; a short-lived user name that contains nothing derived from the pairing key  | Audio content, alerts, the pairing key                                  |
| **STUN server** (only the signaling server's own, or a host on the project's [curated list](../../data/stun-servers.json)) | Your device's public IP address                                                                                                                                                                                    | Anything else                                                           |

**No IP logs:** the project server logs no client IP addresses, neither in the signaling server, nor in the reverse proxy, nor in the TURN server. **No third party** is in the audio or alert path.

**Your choices:** the app names the project server in setup and in the connection settings. The advanced switch "Only same Wi-Fi or my own server" stops the device from contacting the project server, its STUN and its TURN. You can run your own signaling server instead ([self-hosting](../../docker/stack/README.md)); a local mode on the same Wi-Fi without any server is planned.

> **Owner decision needed:** whether the project server's operator acts as a data controller for the transient IP addresses and connection times it processes (this section says "keeps no IP logs"; the server privacy policy of `alpha-connection-model` task 10.3 must match it), the legal basis and retention to state here, the hoster as processor, and a link to that server policy and the imprint once `data/project-server.json` lists them.

## 3. Data We Do NOT Collect

- We do NOT collect analytics or usage data
- We do NOT use advertising or tracking
- We do NOT transmit audio to any cloud service; a TURN relay forwards it only end-to-end encrypted
- We do NOT store audio recordings (audio is processed in real-time and discarded)
- We do NOT require an account or registration
- We do NOT use Firebase, Google Analytics, or any third-party SDK that collects data

## 4. Children's Data

This app is designed for **parents and caregivers**, not for children. The app processes audio that may contain children's voices. This audio is:

- Processed locally on the device (noise detection)
- Transmitted only to the paired parent device (end-to-end encrypted)
- Never stored or recorded, and never readable by any server (a TURN relay forwards it only end-to-end encrypted)

We do not collect or store personal information of children, and no server can read it.

## 5. Permissions

| Permission             | Why                                                         | When                             |
| ---------------------- | ----------------------------------------------------------- | -------------------------------- |
| **Microphone**         | Audio monitoring on baby unit                               | When app is in baby mode         |
| **Camera**             | QR code scanning for device pairing                         | Only during pairing              |
| **Notifications**      | Alert parent when baby is detected awake                    | Always (baby monitor function)   |
| **DND Override**       | Ensure alerts play even in silent mode                      | Optional, user grants explicitly |
| **Foreground Service** | Keep monitoring active in background                        | When monitoring is active        |
| **Internet**           | Signaling, STUN/TURN and the connection between the devices | While pairing and monitoring     |

## 6. Data Storage and Security

- The pairing record (pairing key, parent secret, mailboxes) is encrypted with a key held in the **Android Keystore** (hardware-backed, not extractable); the web client keeps it in memory only
- App settings are stored locally on the device
- No user data is stored on any server; the project server keeps no IP logs
- **Device lock:** Users should use the **operating system** screen lock (PIN, pattern, biometrics). The MVP app does not add a separate in-app lock (see [ADR-001](../../openspec/changes/archive/2026-04-15-adr-001-mvp-scope-poc-gate-and-authentication/design.md)).

## 7. Your Rights (GDPR)

All data the app keeps is stored locally on your device:

- **Right to access**: All data is visible in the app settings
- **Right to deletion**: Delete saved connections in settings, or uninstall the app for complete removal
- **Right to data portability**: Not applicable (no server-side data)
- **Data controller**: For the data on your devices, you are the data controller. For the transient processing on the project server, see §2 (owner decision pending).

## 8. Open Source

The complete source code is available under the **GNU Affero General Public License v3.0 or later** ([`LICENSE`](../../LICENSE)). You can verify privacy claims by reviewing the code. Third-party forks must comply with AGPL and must not imply they are the official **GuardedSleep** product — see [`trademark.md`](trademark.md).

## 9. Changes

We will update this privacy policy as needed. Changes will be documented in the app's changelog.

## 10. Contact

[Developer contact email]

---

## Google Play Data Safety Declaration

For the Google Play Data Safety section:

> **Owner decision needed:** the answers below predate the project server and are not confirmed. Google treats data sent off the device as "collected" even when it is processed only transiently, and the project server processes IP addresses and connection times. Decide each answer (in particular "collect", "share with third parties" and whether processing is ephemeral) before the Play submission; the table is kept only as the previous draft.

| Question                                            | Previous draft answer (unconfirmed)    |
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
**Mitigations:** End-to-end encryption, sealed signaling messages, QR code pairing with physical verification, Android Keystore for keys, no IP logs on the project server and an off switch for it, kiosk mode on baby unit, **OS-level device lock** recommended for parent device.
**Residual risk:** Low. Content is processed only on the devices or end-to-end encrypted. The project server sees network addresses and connection times while connected and stores none of them.
**Conclusion:** Processing is proportionate and adequately protected.
