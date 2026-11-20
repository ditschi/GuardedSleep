## MODIFIED Requirements

### Requirement: Invite shown by the baby unit

The baby unit SHALL show the invite as a QR code (inverted) and as copyable text. While the code is shown, the app SHALL NOT raise the screen brightness above its level before the code appeared; it MAY cap it at 50 %. Only "Make code brighter", tapped by the user, MAY raise it. The parent unit SHALL accept the invite by scanning it or by pasting the text; on Android the camera permission SHALL come from the first-start permission screen and be requested on the scan screen only if still missing, and the web client SHALL request it when scanning starts. The invite SHALL be valid for 5 minutes with a visible countdown and a "Show new code" action, and each new code SHALL carry a fresh PSK.

#### Scenario: Code expires

- **WHEN** the parent scans a code whose expiry has passed
- **THEN** pairing is rejected as expired and the parent is told to show a new code on the baby unit and to check the date and time of both devices

#### Scenario: Parent clock ahead at the first pairing

- **WHEN** the parent's wall clock is 10 minutes ahead of the baby's and it scans a fresh invite with a 5-minute expiry
- **THEN** the parent refuses the code as expired, stores nothing, and suggests checking the date and time

#### Scenario: Pairing in a dark room

- **WHEN** the baby unit's screen is at 10 % auto brightness and the code is shown
- **THEN** the screen stays at 10 % until the user taps "Make code brighter"

#### Scenario: Camera allowed on first start

- **WHEN** the parent allowed the camera on the first-start screen and opens the scan screen
- **THEN** the camera starts without a permission dialog
