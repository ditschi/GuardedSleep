## ADDED Requirements

### Requirement: Brand app icon

The Android launcher icon and the web favicon and PWA icons SHALL show the GuardedSleep logo (crescent, dot and sound wave), derived from one vector master in the repository. The Android icon SHALL be an adaptive icon whose logo lies entirely inside the adaptive-icon safe zone, and SHALL provide a monochrome layer for themed icons. Raster icons SHALL be produced from the master by the build, and the Android vector layers SHALL be checked against the master.

#### Scenario: Circular launcher mask

- **WHEN** the app is installed on a launcher that masks icons to a circle
- **THEN** the whole logo, including both ends of the sound wave, is visible and not clipped

#### Scenario: Themed icons enabled

- **WHEN** the user turns on themed icons on Android 13 or later
- **THEN** the launcher shows the logo shapes in the system's single colour instead of the placeholder or a blank tile

#### Scenario: Same mark on web and Android

- **WHEN** a tester installs the PWA and the Android app on one device
- **THEN** both home-screen icons show the same logo on the same background colour

#### Scenario: Master changed

- **WHEN** the vector master is changed
- **THEN** the next web build produces PWA icons from it without a manual step, and `just lint` fails until the Android icon layers match it again
