# Spec Delta

## MODIFIED Requirements

### Requirement: Parent ambient bar

While monitoring is started, every parent screen, including Settings and open sheets, SHALL show a full-width status bar at the top that renders the core's link state and escalation level. The connected state SHALL be neutral: the surface colour with a green dot, a link symbol and words that name the baby device, so that a coloured bar always means a change.

#### Scenario: Normal night

- **WHEN** the link is connected
- **THEN** the bar uses the surface colour with a green dot and a link symbol and reads "Connected" with the baby device's connection name

#### Scenario: Settings opened while monitoring

- **WHEN** the parent opens Settings or the More sheet while monitoring
- **THEN** the status bar stays visible and keeps showing the current state

### Requirement: Ambient bar per level

The bar SHALL show level 0 in blue with a refresh symbol, saying "Connecting" in the core state `connecting` and "Reconnecting" in `reconnecting`; level 1 in orange with a 2 px outline, a broken-link symbol and "Can't reach … · Still trying" ("Not connected yet" while `connecting`); level 2 and above in red, slowly pulsing, with a broken-link and exclamation symbol and the outage duration; a stopped peer in grey with a pause symbol; a link that needs the user in red with the reason.

#### Scenario: Short drop

- **WHEN** a link that was connected in this run has been down for 10 seconds
- **THEN** the bar is blue with a refresh symbol and says it is reconnecting, without a timer

#### Scenario: Outage passes level 1

- **WHEN** the link has been down for 25 seconds
- **THEN** the bar is orange with a broken-link symbol and says the baby device cannot be reached and the app is still trying

#### Scenario: First connection not answered

- **WHEN** a link that never connected in this run has been running for 25 seconds
- **THEN** the bar is orange and says "Not connected yet", not blue

#### Scenario: Outage passes level 2

- **WHEN** the link has been down for 70 seconds
- **THEN** the bar is red, pulses slowly and shows how long there has been no connection

### Requirement: Plain copy

User-facing status copy SHALL NOT contain IP addresses, port numbers, protocol terms (such as WebSocket, DTLS, ICE or socket errors) or internal terms (escalation level numbers, "level", "link", "peer"). Stages SHALL be named by what the device does. A link that needs the user SHALL say what to do in plain words: pair again, or update one of the devices.

#### Scenario: Protocol mismatch

- **WHEN** the link fails with `update_required`
- **THEN** the parent reads that one of the devices needs a GuardedSleep update, without any technical term

#### Scenario: No level numbers

- **WHEN** the link reaches level 3
- **THEN** the parent reads "Loud alarm sounding" and "Still no connection", and no level number or the word "level" appears

### Requirement: Restored notice

After recovery from level 1 or above, the parent SHALL show a card that the link is connected again, how long the baby was not monitored (duration in words) and the missed-sound result, until the parent taps OK, and SHALL post a notification with the same duration. The card SHALL NOT dismiss itself; a new outage SHALL replace it.

#### Scenario: Recovery after 90 seconds

- **WHEN** the link recovers after a 90-second outage
- **THEN** the parent sees "Connected again" with "Monitoring was interrupted for 1 min 30 s", and the card stays until OK is tapped

#### Scenario: Incomplete report

- **WHEN** the link recovers and the missed-events report is not complete
- **THEN** the card says it is not known whether there were sounds, and shows no count

#### Scenario: New drop while the card is shown

- **WHEN** the "Connected again" card is shown and the link drops again
- **THEN** the card disappears and the bar shows the new outage

### Requirement: Baby unit edge indicator

The baby unit SHALL code the link state by shape, not hue alone: a short solid bar when connected, no mark at level 0, a dashed top edge at level 1, and a full frame with a slow pulse (static with reduced motion) from level 2. Every kiosk mark colour SHALL have a relative luminance at most that of `#5A1414`, and the baby unit SHALL never use sound or vibration for any connection state.

#### Scenario: Parent phone goes away at night

- **WHEN** the parent device has been unreachable for 2 minutes
- **THEN** the baby unit shows a dim full frame with a slow pulse and stays silent

#### Scenario: Still trying

- **WHEN** the parent device has been unreachable for 30 seconds
- **THEN** the baby unit shows a dashed top edge, distinguishable from the fine and the lost marks without colour

### Requirement: Status is never colour alone

Every status SHALL combine colour with a symbol and a text label. Pulsing SHALL become static when the system asks for reduced motion. Screen-reader announcements of status changes SHALL follow "State announcements".

#### Scenario: Colour-blind parent

- **WHEN** a parent who cannot tell red from green looks at the bar
- **THEN** the symbol and text alone tell whether the link is connected or lost

## ADDED Requirements

### Requirement: Baby status line

While monitoring, the baby kiosk SHALL always show one dim status line (at least 4.5:1 against black at kiosk brightness) with the state in words and the connection name, such as "Listening · Travel cot" or "Parent device not responding — still trying". During a granted talk only the talk indicator SHALL show.

#### Scenario: Adult glances at the tablet

- **WHEN** an adult looks at the locked baby kiosk while it is connected
- **THEN** the status line says it is listening and names the connection, without a touch

#### Scenario: Talk running

- **WHEN** the parent talks to the baby device
- **THEN** the status line and the edge mark are hidden and only the talk indicator shows

### Requirement: Baby needs-attention mark

When the baby unit's battery is low or critical or its charger was pulled while monitoring, the kiosk SHALL add a separate small square next to the bar and say "Needs attention" with the reason in the status line, without sound, vibration or extra brightness.

#### Scenario: Charger pulled

- **WHEN** someone unplugs the baby tablet during monitoring
- **THEN** the kiosk shows the bar plus the square and "Needs attention: not charging", and stays silent

### Requirement: Baby set-up confirmation

From the first verified contact with a parent device, for at most 60 seconds, and on every touch-wake, the baby kiosk SHALL show a larger confirmation at the touch-wake brightness that names the parent connection and that it is listening ("Connected to the parent device" when no name is known), and then dim back to the status line.

#### Scenario: Grandparent sets up the tablet

- **WHEN** a user starts monitoring on the baby device and the parent device connects
- **THEN** the kiosk shows "Connected to Anna's phone · Listening" from that moment for 60 seconds before it dims

#### Scenario: Parent not there yet

- **WHEN** monitoring starts on the baby device and no parent device has connected
- **THEN** no confirmation is shown and the kiosk stays at minimum brightness with its status line

### Requirement: Full-screen connection alert

From level 2 the parent SHALL show the shared full-screen alert template (`parent-alerts`) with a label of what the phone does, a headline, the outage counter, one sentence with the next stage and its clock time, and, until acknowledged, "I'm checking" as the only filled, pinned action. While acknowledged it SHALL stay red, say "Alarm paused", show when sound returns and have no filled action.

#### Scenario: One minute without connection

- **WHEN** the link reaches level 2
- **THEN** the screen says "Your phone is vibrating", shows the counter and says when the loud alarm starts if there is still no connection

#### Scenario: Parent is checking (failure)

- **WHEN** the parent taps "I'm checking" at 03:12 and the link stays down
- **THEN** the screen stays red, says sound returns at 03:17, and at 03:17 the loud alarm starts

### Requirement: Troubleshooter presentation

The connection alert SHALL render the core's cause hint as one plain cause line with the first step visible, "Try now" and further steps folded behind "More steps". Before the first connection of a run it SHALL also offer "Wrong connection?", the way back to the saved connections. "Stop monitoring" SHALL be a quiet text button at the end of the content.

#### Scenario: Baby device offline

- **WHEN** the cause hint is `peer_absent` at level 2
- **THEN** the card says the baby device isn't online, shows the first step and "Try now", and the other steps stay folded until "More steps" is opened

#### Scenario: Hotel Wi-Fi blocks the media

- **WHEN** the cause hint is `media_blocked`
- **THEN** the card says the connection is blocked on this network and suggests mobile data

### Requirement: Leaving from the alarm needs confirmation

From level 2, "Wrong connection?" and "Stop monitoring" SHALL both open the stop confirmation of `device-setup` "Stopping monitoring on the parent", which silences and acknowledges nothing. Only at levels 0 and 1 MAY "Wrong connection?" stop monitoring with one tap.

#### Scenario: Wrong connection on the red screen (failure)

- **WHEN** a link that never connected reaches level 2 and the parent taps "Wrong connection?"
- **THEN** the stop confirmation opens, vibration and the soft tone continue, and monitoring stops only after the confirmation

#### Scenario: Wrong connection while still blue

- **WHEN** a link that never connected has run for 10 seconds and the parent taps "Wrong connection?"
- **THEN** monitoring stops and the saved connections are shown, without a confirmation

### Requirement: State announcements

Screen readers SHALL hear each stage change once: politely for level 0 and 1, assertively from level 2, for a stopped peer, a link that needs the user and a silent microphone. Counters and countdowns SHALL NOT be in a live region and SHALL be labelled in words. On a full-screen alert, focus SHALL move to the headline and return on close.

#### Scenario: Counter ticking

- **WHEN** a screen reader user stays on the no-connection screen for a minute
- **THEN** the stage was announced once and the ticking counter is not announced, but reads "1 minute 12 seconds" when focused
