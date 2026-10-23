# Spike: community infrastructure for signaling, STUN and TURN

Measured 2026-10-23 from one residential host in Germany. The probes sent throwaway-key ephemeral Nostr events with random content and one STUN Binding request per server. The scripts and raw data lived in the session scratchpad and are not committed; re-run them before relying on single numbers.

## Summary

1. **Nostr ephemeral events work as a signaling mailbox.** 29 relays were probed, and 14 delivered 20/20 ephemeral events (kind 25050). None asked for auth or proof of work, and 4 KB and 16 KB payloads were accepted. 11 of the 14 are in the EU, run by about 10 operators. Delivery took p50 22–231 ms and p95 < 600 ms, and idle subscriptions survived 95 s.
2. **"Ephemeral" is not "unstored".** strfry, the most common relay software, keeps ephemeral events for about 300 s (`ephemeralEventsLifetimeSeconds`) and serves them to any new subscriber in that window. nostr-rs-relay, memlay and ditto do not keep them. Payloads must therefore be end-to-end encrypted, and the tags and keys must rotate.
3. **Rate limits are the constraint.** 3 of the 11 EU write-accepting relays limited a burst of about 3 events/s, after 0, 10 and 15 events. Relays may carry connection setup only (about 2–6 events per attempt, candidates bundled). Heartbeats must not go through relays.
4. **Churn.** About 6 of the 40 first-batch hosts were dead (DNS or TLS failures), so the relay list needs regular re-vetting.
5. **Prior art.** Trystero (v0.26.0, 2026-10-04) uses Nostr as its default WebRTC signaling strategy, with 5 relays per room and SDP encrypted with AES-GCM. A WebRTC signaling kind is not standardised; voistr uses NIP-44-encrypted events.

## Viable EU relays (2026-10-23)

| Relay                                            | Host, country   | Software       | p50 / p95 ms      | Keeps ephemeral ~5 min           |
| ------------------------------------------------ | --------------- | -------------- | ----------------- | -------------------------------- |
| wss://nostr.mikoshi.de                           | residential, DE | nostr-rs-relay | 22 / 30           | no                               |
| wss://nostr.chaima.info                          | Contabo, FR     | nostr-rs-relay | 27 / 31           | no                               |
| wss://relay.dwadziesciajeden.pl                  | Scaleway, PL    | nostr-rs-relay | 41 / 72           | no                               |
| wss://relay.snort.social                         | LNVPS, IE       | memlay         | 62 / 97           | no                               |
| wss://nos.lol and wss://nostr.mom (one operator) | Hetzner, DE     | strfry         | 128–231 / 132–565 | yes                              |
| wss://nostr.sathoarder.com                       | FR              | strfry         | 128 / 141         | yes                              |
| wss://nostr.data.haus                            | Hetzner, DE     | strfry         | 131 / 227         | yes                              |
| wss://nostr.hifish.org                           | Init7, CH       | strfry         | 133 / 144         | yes                              |
| wss://relay.angor.io                             | Contabo, FR     | strfry         | 134 / 350         | yes                              |
| wss://nostr.21crypto.ch                          | Init7, CH       | strfry         | 143 / 152         | yes (publishes a privacy policy) |
| wss://nostr.azzamo.net                           | OVH, PL         | strfry         | 144 / 150         | yes                              |

The rest were left out:

- **Low rates only:** nostr.oxtr.dev and nostr.stakey.net.
- **Rate-limited from the first event:** nostr.self-determined.de.
- **Accepted events but never delivered them:** nostr.huszonegy.world, so a relay's `OK` is not proof of delivery.
- **Writes refused** (payment, membership or whitelist): nostr.wine, nostr.land, relay.azzamo.net, nostr.einundzwanzig.space and others.

## Other mailboxes

| Option                                             | Verdict                                                                                              |
| -------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Public MQTT test brokers (EMQX, HiveMQ, Mosquitto) | Rejected as primary. Their operators say not to rely on them, and anyone can subscribe to any topic. |
| WebTorrent WSS trackers                            | Rejected as primary. Only about 3 live trackers, and the swarm semantics are awkward for two peers.  |
| IPFS / libp2p                                      | Rejected. Heavy, with no Kotlin path.                                                                |
| Matrix                                             | Rejected. Needs an account per device and stores everything.                                         |
| ntfy.sh                                            | Rejected. A single operator (US-hosted) that stores messages for 12 h.                               |
| Supabase / Firebase                                | Rejected. Cloud vendors, and Firebase is Google.                                                     |

## STUN (EU, answered 2026-10-23)

| Server                                 | Operator                  | Location          |
| -------------------------------------- | ------------------------- | ----------------- |
| stun.nextcloud.com, ports 443 and 3478 | Nextcloud GmbH            | Germany (Hetzner) |
| stun.t-online.de                       | Deutsche Telekom          | Germany           |
| stun.1und1.de                          | IONOS                     | Germany           |
| stun.easybell.de                       | easybell                  | Germany           |
| stun.sipgate.net                       | sipgate                   | AWS anycast       |
| stun.ipfire.org                        | IPFire project            | Germany           |
| stun.linphone.org                      | Belledonne Communications | France (OVH)      |
| stun.voys.nl                           | Voys                      | Netherlands       |
| stun.voipgate.com                      | VoIPGATE                  | Luxembourg        |

No answer from antisip, f.haeder, ekiga or zoiper; stunprotocol.org has no DNS record.

## TURN

- **Bandwidth:** about 52–70 kbit/s per relayed audio pair, which is about 190–250 MB per 8 h night or 6–8 GB per month for a pair relayed every night.
- **How many need it:** about 10–20 % of sessions, up to about 25 % mobile ↔ mobile (CGNAT, DS-Lite).

| Option                                     | Cost                                                             | Privacy                                     |
| ------------------------------------------ | ---------------------------------------------------------------- | ------------------------------------------- |
| Self-hosted eturnal or coturn on an EU VPS | €4–6 per month (20 TB egress), about 1–2 h per month to maintain | We control logging; short-lived credentials |
| Cloudflare Realtime TURN                   | 1000 GB per month free, then $0.05/GB                            | US company; Signal uses it                  |
| Metered Open Relay                         | 20 GB per month free                                             | Canadian, shared credentials; testing only  |
| Free EU community TURN                     | none found                                                       |                                             |

## Signal for comparison

- **Signaling:** offers, answers, ICE updates and hangups are ordinary end-to-end encrypted Signal messages (`CallMessage` with opaque bytes), so the server never sees the SDP.
- **Media:** goes through RingRTC on WebRTC. A direct connection is tried only when you called or the caller is a contact; calls from non-contacts are relayed. "Always relay calls" hides the IP for every call.
- **TURN:** Signal-Server issues TURN credentials from Cloudflare alongside its own TURN URIs.

Sources:

- https://github.com/signalapp/Signal-Android/blob/main/lib/libsignal-service/src/main/protowire/SignalService.proto
- https://signal.org/blog/signal-video-calls/
- https://github.com/signalapp/Signal-Server

## Sources

**Nostr:**

- NIP-01: https://github.com/nostr-protocol/nips/blob/master/01.md
- strfry config: https://github.com/hoytech/strfry/blob/master/strfry.conf

**Prior art:**

- Trystero: https://github.com/dmotz/trystero
- voistr: https://voistr.codeberg.page/

**TURN cost and share:**

- https://developers.cloudflare.com/realtime/turn/faq/
- https://www.metered.ca/tools/openrelay/
- https://www.nojitter.com/video-conferencing/know-where-to-turn-when-deploying-webrtc
- https://openvidu.io/blog/2026/06/09/turn-key-considerations/

**Other mailboxes:**

- ntfy privacy: https://docs.ntfy.sh/privacy/

## Unknowns to test

1. **All-night stability:** do subscriptions survive 8 h? Run a one-night soak on 3 EU relays.
2. **Reconnect bursts:** test a realistic burst (offer, answer and 6–10 candidates in 1 s), including from two clients behind one CGNAT IP.
3. **Tag-only filter:** delivery with a tag filter and no `authors` filter.
4. **Mobile networks:** Android over LTE, IPv6-only carriers with NAT64, and relays behind Cloudflare compared with direct ones.
5. **Relay churn:** monthly re-runs for 3 months; ask 3–4 operators about acceptable use and logging.
6. **Real TURN share for our users:** a local-only, opt-in diagnostic in gate runs (no telemetry).
7. **Retention in other relay software:** khatru and others.
8. **Silent drops:** application-level acknowledgements, because some relays accept events without delivering them.
