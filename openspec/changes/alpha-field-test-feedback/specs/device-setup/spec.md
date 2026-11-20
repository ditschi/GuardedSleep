## ADDED Requirements

### Requirement: Permissions on first start

On Android, the first start SHALL show one permission screen before the role choice that lists every permission either role can need: the microphone (noise detection on the baby device, talking to the child on the parent device), the camera (scanning the pairing code), notifications, full-screen alerts, Do Not Disturb access and background operation. Each row SHALL show its state and one plain sentence on why it is needed.

#### Scenario: Fresh install

- **WHEN** a user opens the Android app for the first time
- **THEN** the permission screen appears before the role choice and lists the microphone, camera, notifications, full-screen alerts, Do Not Disturb access and background operation, each with a reason

#### Scenario: Web client

- **WHEN** the web client starts for the first time
- **THEN** it shows no first-start permission screen and asks for each permission at first use, because browsers grant per use

#### Scenario: Allow all

- **WHEN** the user taps "Allow all"
- **THEN** the runtime permissions are requested in one sequence, the remaining system settings open one by one, and the user can continue with rows left open

#### Scenario: Everything allowed up front

- **WHEN** the user allowed every permission on the first-start screen and later pairs as parent and presses the talk button
- **THEN** no further permission request appears, neither for the camera on the scan screen nor for the microphone on the first talk

### Requirement: No later permission prompts

After the first-start permission screen, the app SHALL ask for a permission again only when that permission is still missing as its feature is used (skipped, refused or revoked), and then only with the same plain reason as on the first-start screen. When the system no longer shows its permission dialog (repeated refusal), the app SHALL open the app's system settings instead. The baby unit SHALL NOT start monitoring without microphone permission.

#### Scenario: Revoked in system settings

- **WHEN** the user revokes the camera in system settings and later opens the scan screen
- **THEN** the scan screen asks for the camera with the reason "to read the pairing code"

#### Scenario: Microphone refused twice, then baby monitoring starts (failure)

- **WHEN** the user refused the microphone twice and starts monitoring as baby device
- **THEN** monitoring does not start without sound; the app explains why and opens the app's system settings

### Requirement: Baby unit permission gate before connecting

Before the baby unit shows a pairing code or starts monitoring, it SHALL check its permissions. While the microphone permission is missing, it SHALL show a clear warning with the reason and an "Allow" action in place of the code and the start action, and SHALL NOT connect. While background operation or Do Not Disturb access is missing, it SHALL show a visible warning with the reason and a fix action before the start, without blocking it.

#### Scenario: Microphone missing when showing the code (failure)

- **WHEN** the baby unit has no microphone permission and the user opens "Show code"
- **THEN** no code is shown and no connection is made; a warning says the baby device cannot hear the child without the microphone and offers "Allow"

#### Scenario: Microphone allowed from the warning

- **WHEN** the user taps "Allow" on that warning and grants the microphone
- **THEN** the code appears without leaving the screen

#### Scenario: Background operation missing

- **WHEN** the baby unit is still battery-optimised and the user starts monitoring
- **THEN** a warning says monitoring may stop in the background and offers the fix, and monitoring can still start

## MODIFIED Requirements

### Requirement: Setup checklist per role

Each role SHALL have one checklist screen, reachable from Settings, whose rows show a state and open the matching system setting: on the baby unit the microphone, background operation (battery optimisation exemption), silence access and a talk-back row that says parent devices can speak through this device, with its switch and volume; on the parent unit notifications, lock-screen and full-screen alerts, Do Not Disturb bypass, the microphone for talking and background operation. On Android, permissions SHALL be requested on the first-start permission screen; the checklist SHALL show what is still missing, with the same plain reason, and SHALL NOT be a mandatory step after the role choice.

#### Scenario: Battery optimisation still on

- **WHEN** the baby unit's app is still battery-optimised
- **THEN** the background row is marked to do and its button opens the system's battery optimisation setting

#### Scenario: Setting up the baby unit

- **WHEN** an adult opens the baby unit's checklist
- **THEN** the talk-back row shows whether parent devices can speak through this device and lets the adult change it

#### Scenario: Role chosen after first start

- **WHEN** the user finished the first-start permission screen and chooses a role
- **THEN** the app goes straight to pairing or monitoring without another permission checklist

### Requirement: Baby device check

The baby unit SHALL check its microphone level, battery, the reachability of its mailboxes (at least one reachable, and which ones), permissions and Do Not Disturb access, and show the result as OK, limited or not recommended with plain explanations. Reachability SHALL be described by what it means for the user ("Connection over the internet: works" or "doesn't work") without a host name in the main line; the host names SHALL stay available in an expandable detail line. When the room's steady level is within about 6 dB of the threshold, the check SHALL say that the current sensitivity may raise alerts for the room's background sound. The result SHALL NOT block monitoring; the user decides.

#### Scenario: Old phone with a weak battery

- **WHEN** the device check finds a low battery that does not charge
- **THEN** it shows "limited" or "not recommended" with the reason, and monitoring can still start

#### Scenario: No mailbox reachable

- **WHEN** the device check reaches none of the pairing's mailboxes
- **THEN** it shows "not recommended", the main line says the connection over the internet doesn't work, and the reason says the devices could not find each other again after a disconnection

#### Scenario: Project server reachable

- **WHEN** the device check reaches the project server
- **THEN** the main line says the connection over the internet works, and the server's host name appears only after expanding the details

#### Scenario: Fan running near the threshold

- **WHEN** a fan keeps the room level 3 dB below the threshold during the check
- **THEN** the check shows "limited" and suggests a less sensitive setting or moving the device

### Requirement: Stopping monitoring on the parent

On the parent unit, "Stop monitoring" SHALL be reachable from the monitor's More sheet, set apart from the other actions, and as a quiet text button at the end of the full-screen connection alerts, and SHALL always ask for confirmation. Opening or cancelling the confirmation SHALL NOT silence or acknowledge any alert. The one exception is the notice that the baby unit stopped on purpose (`link-supervision` "Acknowledging a deliberate stop"): its primary button SHALL say that it also stops monitoring on this device and SHALL stop it without a second confirmation.

#### Scenario: Accidental tap at night

- **WHEN** the parent opens the More sheet and taps "Stop monitoring"
- **THEN** monitoring stops only after the confirmation

#### Scenario: Confirmation over a running alarm (failure)

- **WHEN** the loud alarm sounds and the parent opens the stop confirmation and then taps "Keep monitoring"
- **THEN** the alarm kept sounding the whole time and the alert screen is unchanged

#### Scenario: Baby device stopped on purpose

- **WHEN** the baby device stops monitoring by user action and the parent taps the primary button on the "stopped monitoring" notice
- **THEN** the parent unit stops monitoring and returns to Home

#### Scenario: Baby battery dies (failure)

- **WHEN** the baby device shuts down from an empty battery without sending a stop notice
- **THEN** the parent shows the connection-loss alarm with "I'm checking", offers no button that stops monitoring without confirmation, and keeps escalating

### Requirement: Parent pairing entry

"Set up new devices" on Home SHALL lead to the parent scan screen whenever fewer than the cap of parent pairings are saved; the camera permission SHALL come from the first-start permission screen and be asked there only if still missing. With no saved parent pairing, choosing "Parent device" on the first start SHALL open the scan screen directly; cancelling it SHALL return to Home with "Set up new devices" and SHALL NOT reopen the scan screen until Home is entered again.

#### Scenario: First parent start

- **WHEN** a parent with no saved pairing chooses "Parent device" after the first-start permission screen
- **THEN** the scan screen opens, and the camera starts without a permission dialog if it was allowed on the first-start screen

#### Scenario: Scanning cancelled

- **WHEN** the parent cancels the scan screen with no saved pairing
- **THEN** Home is shown and "Set up new devices" opens the scan screen again
