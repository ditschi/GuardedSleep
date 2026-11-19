## MODIFIED Requirements

### Requirement: Brand app icon

The Android launcher icon and the web favicon and PWA icons SHALL show the GuardedSleep night-crescent mark (a sage crescent with a warm cream breath wave, separated by a dark gap) on the night tile, derived from one vector master in the repository. The Android icon SHALL be an adaptive icon whose mark lies entirely inside the adaptive-icon safe zone, and SHALL provide a monochrome layer for themed icons in which wave and crescent stay separated. At 16 px the favicon SHALL show the crescent alone. Raster icons SHALL be produced from the master by the build, and the Android vector layers SHALL be checked against the master.

#### Scenario: Circular launcher mask

- **WHEN** the app is installed on a launcher that masks icons to a circle
- **THEN** the whole mark, including both ends of the wave, is visible and not clipped

#### Scenario: Dark home screen at night

- **WHEN** a parent opens a dark home screen at night
- **THEN** the GuardedSleep icon shows a dark tile with the crescent and the warm wave, not a bright light tile

#### Scenario: Themed icons enabled

- **WHEN** the user turns on themed icons on Android 13 or later
- **THEN** the launcher shows the crescent and the wave in the system's single colour, separated by a visible gap

#### Scenario: Same mark on web and Android

- **WHEN** a tester installs the PWA and the Android app on one device
- **THEN** both home-screen icons show the same mark on the same night tile

#### Scenario: Browser tab at 16 px

- **WHEN** the web client is open in a browser tab that shows a 16 px favicon
- **THEN** the tab shows the crescent alone, and larger icons show the full mark

#### Scenario: Master changed

- **WHEN** the vector master is changed
- **THEN** the next web build produces PWA icons from it without a manual step, and `just lint` fails until the Android icon layers match it again

## ADDED Requirements

### Requirement: Role notification icons

The Android app's notifications SHALL use GuardedSleep small icons instead of system icons: a crescent while the device runs as the baby device, and a crescent with listening bars while it runs as the parent device, alerts included. Each SHALL be a single-colour silhouette that stays legible at 24 dp.

#### Scenario: Baby device monitoring

- **WHEN** a device runs as the baby device and its monitoring notification is shown
- **THEN** the status bar shows the GuardedSleep crescent, not a microphone or alarm-clock system icon

#### Scenario: Alert on the parent device

- **WHEN** the parent device raises a noise or link-loss alert at night
- **THEN** the alert notification's small icon is the parent glyph (crescent with listening bars), so the parent recognises the app in the notification shade
