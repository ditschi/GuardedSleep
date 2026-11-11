# Why GuardedSleep?

> STATUS: UNDER DISCUSSION -- Product positioning. Reference only; binding behaviour lives in [`openspec/specs/`](../../openspec/specs/).

Privacy is our reason to build GuardedSleep, but it is not everyone's reason to use it. This page explains what GuardedSleep does better than the two things parents use today, **for parents who do not care about privacy**:

- **A. an open phone or messenger call** (WhatsApp, Signal, FaceTime) between two phones;
- **B. a dedicated baby monitor** (DECT or radio parent unit, or a Wi-Fi camera).

It also says honestly where each alternative is still simpler. Privacy arguments are in [vision.md](vision.md), the market overview in [competitive-analysis.md](competitive-analysis.md), the event and long-distance scenarios in [personas.md](personas.md) (Persona A) and [use-cases.md](use-cases.md).

Each benefit is tagged **built** (in a spec today), **decided** (in an active change in [`openspec/changes/`](../../openspec/changes/)) or **idea** (in the [backlog](../backlog.md)).

## At a glance

| What parents need                | Phone call               | Baby monitor (DECT/radio) | GuardedSleep                             |
| -------------------------------- | ------------------------ | ------------------------- | ---------------------------------------- |
| Notice noise without listening   | ✗ headset all evening    | ✓                         | ✓ vibration, screen, level meter         |
| Know when the connection is lost | ✗ sounds like silence    | ~ beep when out of range  | ✓ escalating alarm, missed-noise report  |
| Distance                         | unlimited                | ✗ a few rooms             | ✓ unlimited over the internet            |
| Works without internet           | ✗                        | ✓                         | ✓ local Wi-Fi (planned for alpha)        |
| Alerts through silent mode / DND | ✗                        | – own device              | ✓                                        |
| Silent next to the child         | ✗ rings, two-way audio   | ✓                         | ✓ one-way, never makes a sound           |
| Check before you leave the room  | ✗                        | ~                         | ✓ test alert, battery, mic check         |
| Your phone stays free            | ✗ line busy              | ✓                         | ✓                                        |
| Extra hardware                   | none                     | ✗ two devices to charge   | none, an old phone will do               |
| Mobile data                      | continuous stream        | –                         | audio only on noise                      |
| Setup                            | ✓ none                   | ✓ switch on               | install twice, scan a QR code            |
| Talk back to the child           | ✓                        | often ✓                   | ✗ (Phase 2)                              |
| Privacy                          | depends on the messenger | radio can be overheard    | end-to-end encrypted, no server hears it |

✓ yes · ~ partly · ✗ no · – not applicable. Details and the honest downsides follow below.

## The core difference

A call is a **pipe**: it carries sound for as long as it lasts, and the parent has to listen to know anything. A dedicated monitor is a watcher, but **tied to a radio range and to extra hardware**. GuardedSleep is a watcher that runs on the phones you already carry and **reaches as far as the internet does**: the baby unit decides when something happens and tells the parent, and it also tells the parent when it can no longer watch.

## A. Compared with a phone call

### A1. You do not have to listen

- **Call:** you hold the phone to your ear or wear a headset all evening, or you miss the crying. At a party, a dinner or in the garden that does not work.
- **GuardedSleep:** the baby unit runs the noise detection. The parent phone stays in your pocket or on the table and reacts only when the child makes noise: vibration, screen on, full-screen alert over the lock screen, live level meter (**built**, [parent-alerts](../../openspec/specs/parent-alerts/spec.md)). The visual alert stays until the room has been quiet for 2 minutes, so a glance at the screen is enough to see that something happened (**built**). You then decide whether to listen or go and check.
- **Discreet profile:** vibration and screen only, the child's audio only when you tap Listen (**decided**, `add-discreet-alert-profile`, target alpha). Alerts on a smartwatch would complete the party use case (**idea**, backlog `parent-alerts`).

### A2. Silence is not mistaken for "all quiet"

The most important safety argument.

- **Call:** if the call drops (network change, flat battery, the messenger ends a long call, an incoming call), you hear silence -- exactly what a sleeping child sounds like. Nobody tells you.
- **GuardedSleep:** heartbeats run all the time. A lost link escalates on the parent: yellow, orange, then red with vibration, then an audible alarm that bypasses Do Not Disturb (**built**, [link-supervision](../../openspec/specs/link-supervision/spec.md)). "All quiet since {time}" is shown only while connected. After recovery the parent sees how long the child was not monitored and gets a report of noise missed during the outage (**built**, [audio-monitoring](../../openspec/specs/audio-monitoring/spec.md)).

### A3. Alerts get through a muted phone

- **Call:** a parent phone on silent or Do Not Disturb (cinema, meeting, night) means no sound at all.
- **GuardedSleep:** noise alerts play on the alarm stream and get through silent mode and Do Not Disturb (**built**, [parent-alerts](../../openspec/specs/parent-alerts/spec.md)).

### A4. The baby phone never makes a sound

- **Call:** the baby phone still rings for a second call and plays message tones, and the call is two-way: your voice, the party or the TV around you can come out of the phone next to the child.
- **GuardedSleep:** audio flows only from baby to parent. The baby unit never plays audio, vibrates or lights up (**built**, [audio-monitoring](../../openspec/specs/audio-monitoring/spec.md)). It sets a Do Not Disturb rule while monitoring and tells the parent if it is still not silent (**built**).

### A5. You can trust the setup before you leave the room

- **Call:** you find out that the volume was too low, the microphone was covered or the battery was empty when it is too late.
- **GuardedSleep:** a test alert runs the full alert chain with no sound on the baby side (**built**). The parent sees the baby phone's battery and charging state, and a warning if its microphone delivers only silence (**built**).

### A6. Your phone stays your phone

- **Call:** the line is busy all evening; incoming calls interrupt or end the "monitor"; the parent phone decodes audio for hours.
- **GuardedSleep:** no phone line is used. Audio is sent only while there is noise or while you listen; otherwise only small heartbeats flow, which saves mobile data and battery (**built**; a measured data estimate is an **idea**). Hold-to-listen on demand, which stops by itself after 60 minutes, and snooze for 10 or 30 minutes that never silences connection warnings (**built**).

### A7. Network changes do not end monitoring

- **Call:** a switch between Wi-Fi and mobile data often drops the call, and you have to call again by hand.
- **GuardedSleep:** the connection recovers by itself and is supervised while it does (**built**). LAN and internet paths are used in parallel with no manual switch (**decided**, `alpha-connection-model`).

### A8. No phone number, no account

Pairing is a QR scan between the two devices. A babysitter or grandparent needs neither your number nor a messenger account (**built**, [pairing](../../openspec/specs/pairing/spec.md)).

## B. Compared with a dedicated baby monitor

### B1. Unlimited distance over the internet

The biggest practical difference.

- **Dedicated monitor:** DECT and radio units reach a few dozen metres indoors and rarely more than a few hundred outdoors; concrete floors, a second building or a garden end it. Our Persona A compares against "the reach of a hardware monitor" and is let down by exactly this ([personas.md](personas.md)).
- **GuardedSleep:** when there is no shared local network, the phones connect over the internet, either directly or through an encrypted TURN relay on the pairing's own signaling host, with mobile data as a fully valid path (**decided**, `alpha-connection-model`). Distance stops mattering:
  - the wedding reception in the hotel hall while the child sleeps in the room two floors up;
  - the restaurant down the street, the neighbours' garden party, the allotment;
  - a holiday rental where the bedroom is in another building;
  - the child sleeps at the grandparents' house and the parents can still check from home.
- Link supervision (A2) applies on every path, so a long-distance link that degrades is reported, not silently lost.

### B2. No extra device to buy, charge, pack or forget

- **Dedicated monitor:** a parent unit you have to charge, carry and not forget at home; another device on the packing list for every trip.
- **GuardedSleep:** the parent unit is the phone you carry anyway, and the baby unit can be an old phone from the drawer (**built**, Android; web client for other devices, [web-client](../../openspec/specs/web-client/spec.md)). Nothing to buy, so trying it costs nothing.

### B3. Works the same everywhere

- **Dedicated monitor:** range and interference change with every building.
- **GuardedSleep:** at home on the local Wi-Fi, offline without internet in a holiday home or a basement flat (**decided**, `add-offline-lan`, target alpha), and over the internet anywhere else, with the same app and the same pairing.

### B4. Smarter alerts and status

Many hardware monitors show a few level LEDs and beep when out of range. GuardedSleep adds: alerts that get through silent mode and Do Not Disturb (A3), an escalation chain instead of a single beep and a report of the noise you missed during an outage (A2), the baby unit's battery on the parent screen, a microphone-silence warning and a test alert (A5), and snooze that never hides connection problems (A6) (all **built**).

### B5. Not tied to a cloud account (compared with Wi-Fi cameras)

Wi-Fi monitors that reach over the internet usually need a vendor account, a subscription or the vendor's cloud, and stop working when the vendor shuts down the service. GuardedSleep needs no account and can run with your own signaling server or fully offline (**built** for the server, **decided** for offline LAN). The software is open source, so it outlives any one company.

## Where the alternatives are still better

Honest positioning matters for trust:

- **Zero setup (call):** a call works with any two phones. GuardedSleep needs the app on both and a QR pairing once.
- **Single-purpose reliability (hardware):** a dedicated monitor is not affected by phone battery-saving, OS updates, other apps or a phone you need for something else. GuardedSleep has to prove that on the emulator gate and on real hardware before families rely on it.
- **Internet dependency at distance:** beyond the local network GuardedSleep needs mobile coverage or internet on both phones and a reachable signaling server. A radio monitor needs neither, within its range.
- **Platform reach:** the native client is Android only; iPhones use the web client until iOS (`DEFERRED`, Phase 2).
- **Talk-back and extras:** calls and many monitors let you speak to the child (`DEFERRED`, Phase 2); some monitors add a nightlight or a thermometer (**idea**).
- **Maturity:** GuardedSleep is alpha and not yet tested with real families.

## Message for the store listing and website (draft)

> Put your phone in your pocket and enjoy the evening -- in the next room or across town. GuardedSleep listens for you, only buzzes when your child makes noise, and warns you if the connection is lost, so silence really means quiet. No extra hardware, no phone line, no account, no tracking.
