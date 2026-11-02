# Spec Delta

## Purpose

Prepare each phone for its role, baby unit or parent unit, so that monitoring keeps working in the background, and keep the baby unit dark, locked and silent next to the child. Migrated from ADR-009, ADR-010 (amended 2026-10-21), ADR-018 §2 and the device-management and user-interface feature docs, as built on Android.

## ADDED Requirements

### Requirement: Role choice

At first start the user SHALL choose the role, baby unit or parent unit, from two large choices, and the device SHALL remember it. A device SHALL have exactly one role at a time; choosing a different role SHALL delete the existing pairing. A role SHALL never be changed remotely.

#### Scenario: Second start

- **WHEN** the app starts again on a device that chose a role before
- **THEN** it opens directly in that role without asking again

### Requirement: Setup checklist per role

Each role SHALL have one checklist screen whose rows show a state and open the matching system setting: on the baby unit the microphone, background operation (battery optimisation exemption) and silence access; on the parent unit notifications, lock-screen and full-screen alerts, Do Not Disturb bypass and background operation. Permissions SHALL be requested only when needed, with a plain reason.

#### Scenario: Battery optimisation still on

- **WHEN** the baby unit's app is still battery-optimised
- **THEN** the background row is marked to do and its button opens the system's battery optimisation setting

### Requirement: Baby device check

The baby unit SHALL check its microphone level, battery, signaling reachability, permissions and Do Not Disturb access, and show the result as OK, limited or not recommended with plain explanations. The result SHALL NOT block monitoring; the user decides.

#### Scenario: Old phone with a weak battery

- **WHEN** the device check finds a low battery that does not charge
- **THEN** it shows "limited" or "not recommended" with the reason, and monitoring can still start

### Requirement: Battery safety copy

Baby-unit setup SHALL tell the user not to use phones with swollen or damaged batteries, to place the phone on a stable, non-flammable surface and to follow the manufacturer's charging guidance.

#### Scenario: Setting up the baby unit

- **WHEN** a user sets up a phone as baby unit
- **THEN** the battery safety advice is shown during setup

### Requirement: Baby kiosk

While monitoring, the baby unit SHALL show a dark kiosk screen at minimum brightness that locks itself 30 seconds after the last touch. A touch SHALL wake it at low brightness (about 10 %) to show the status, and unlocking SHALL need a hold-and-slide gesture. The kiosk SHALL never use sound, vibration or haptics.

#### Scenario: Toddler taps the baby phone

- **WHEN** someone taps the locked baby unit
- **THEN** the screen wakes dimly and shows the status, and only hold-and-slide unlocks it

### Requirement: Settings in Basic and Advanced

Basic settings SHALL contain the noise threshold and hold time, alert output and gain, theme (dark by default), language and, on the baby unit, silencing the phone while monitoring. Settings that can break monitoring SHALL live only in Advanced: the signaling URL list (validated as `ws://` or `wss://` URLs), an optional TURN server and the heartbeat interval (5–15 s).

#### Scenario: Invalid server address

- **WHEN** the user enters a signaling URL that is not a `ws://` or `wss://` URL with a host
- **THEN** the URL is rejected with an error and not saved

### Requirement: Stopping monitoring on the parent

On the parent unit, "Stop monitoring" SHALL be reachable only from the menu and SHALL ask for confirmation, so it cannot be triggered by accident.

#### Scenario: Accidental tap at night

- **WHEN** the parent opens the menu and taps "Stop monitoring"
- **THEN** monitoring stops only after the confirmation
