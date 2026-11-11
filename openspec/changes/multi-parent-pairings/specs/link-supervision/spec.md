# Spec Delta

## MODIFIED Requirements

### Requirement: Never give up while monitoring

The `Link` SHALL keep retrying while monitoring is on, except where the baby hub releases a link while another parent link is trusted (join window, grace period). From escalation level 2 it SHALL surface a troubleshooter while retrying, with the cause hint `server_unreachable` (no mailbox opens), `peer_absent` (at least one mailbox open, no current `here` or room membership of the peer), or `media_blocked` (offer and answer done, no verification after 3 attempts). "Try now" SHALL reset the backoff.

#### Scenario: Peer never arrives

- **WHEN** mailboxes are open but the other device announces no session past level 2
- **THEN** the parent sees the troubleshooter with hint `peer_absent` while the link keeps retrying

#### Scenario: Project server unreachable

- **WHEN** the project server is the only mailbox and it cannot be reached
- **THEN** the parent sees the troubleshooter with hint `server_unreachable` and the link keeps retrying it on the normal backoff

### Requirement: Deliberate stop is not an outage

`stop` SHALL send a stop notice over the DataChannel and over every open mailbox before closing. On an authenticated stop notice for the current session the peer link SHALL close the verified attempt and go to `peer_stopped`: the parent raises one non-repeating alert, the baby shows a calm grey state, and no escalation runs. The link SHALL then reopen its mailboxes and wait until the peer announces a new session that confirms; on the baby unit, the hub SHALL instead release the link at once when another parent link is trusted.

#### Scenario: Baby is stopped on purpose

- **WHEN** the baby user stops monitoring
- **THEN** the parent shows one "baby unit stopped monitoring" alert and does not escalate

#### Scenario: One of two parents stops

- **WHEN** parent B stops monitoring while parent A's link is trusted
- **THEN** the baby unit releases B's link at once and A stays connected

### Requirement: Process death

The parent foreground service SHALL be sticky and resume monitoring from the persisted, encrypted pairing chosen for the session. When the OS kills the baby process, the baby SHALL post a "tap to resume monitoring" notification where possible, and the parent SHALL escalate as usual meanwhile. When the baby resumes, links whose parent was trusted in the interrupted session SHALL start in the grace period instead of a new join window. Onboarding SHALL state that the parent phone is a single point of failure for alerting.

#### Scenario: OS kills the baby process

- **WHEN** Android kills the baby process and cannot restart the microphone service from the background
- **THEN** the parent escalates through levels 1–3 until the baby app is reopened

#### Scenario: Resume with two parents

- **WHEN** the baby resumes after process death and parent A reconnects within 20 s while parent B is still in backoff
- **THEN** B's link stays open for the 5-minute grace period and B reconnects without a rejoin

## ADDED Requirements

### Requirement: Baby hub

While monitoring, the baby unit SHALL run one core `Link` per saved parent pairing, at most 2, under one sans-IO core baby hub that owns every rule between them. Platforms SHALL feed and execute the hub's link-tagged inputs and actions and SHALL NOT start, stop or prioritise links on their own. A parent link counts as trusted only while its attempt is pin-proven and alive.

#### Scenario: Two saved parents

- **WHEN** a baby unit with two saved parent pairings starts monitoring
- **THEN** the hub creates two links, each with its own room and mailboxes

### Requirement: Released link

The hub SHALL release a link by closing its attempt and mailboxes without a stop notice, so the parent treats it as an outage and escalates on its own phone. A released link SHALL open its mailboxes for 60 seconds every 10 minutes, so its parent can reconnect without help; a parent that connects in that time SHALL be accepted as on rejoin.

#### Scenario: Release sends no stop notice

- **WHEN** the hub releases parent B's link
- **THEN** no stop notice is sent, and B's link escalates as a lost link

#### Scenario: Parent returns by itself

- **WHEN** parent B's network returns 8 minutes after its link was released
- **THEN** B reconnects at the next 60-second opening, within 10 minutes

### Requirement: Join window

The join window SHALL open when the baby unit starts monitoring and close at the later of 2 minutes after the start and 2 minutes after the first parent link became trusted. When it closes, every link that has not been trusted in this session SHALL be released. While no parent link is trusted, no link SHALL be released, released links SHALL reopen, and the next trusted link SHALL start a new join window.

#### Scenario: Second parent stays away

- **WHEN** parent A is trusted 30 s after the start and parent B never connects
- **THEN** B's link is released 2 min 30 s after the start

#### Scenario: Nobody connects

- **WHEN** no parent is trusted within 2 minutes
- **THEN** both links keep waiting until a parent is trusted

#### Scenario: Late second parent

- **WHEN** parent B starts monitoring 3 hours after the window closed
- **THEN** B connects at the next 60-second opening of its released link

### Requirement: Parent grace period

When a trusted parent link goes down while another parent link is trusted, the hub SHALL let it recover for 5 minutes and then release it. When the last trusted parent link goes down, the hub SHALL NOT release any link.

#### Scenario: One parent's phone leaves

- **WHEN** both parents are connected and parent B's phone loses the network for 6 minutes
- **THEN** B's link is released after 5 minutes, parent A stays connected, and B's own link escalates the outage on B's phone

#### Scenario: Back within the grace period

- **WHEN** parent B's phone returns after 3 minutes
- **THEN** B's link verifies a new attempt and both parents are connected again

#### Scenario: Last parent drops

- **WHEN** parent A is the only connected parent and loses the network
- **THEN** the baby unit keeps retrying A's link, and B's link reopens

### Requirement: Rejoin

The hub SHALL open every released link's mailboxes for 2 minutes on `reopen_join`, accepted from the unlocked baby unit's UI or as a message from a trusted parent link, at most once every 10 seconds. With no released link it SHALL do nothing.

#### Scenario: Remote rejoin

- **WHEN** parent A asks the baby unit to let the other parent rejoin
- **THEN** B's link opens its mailboxes for 2 minutes and B can connect without touching the baby unit

### Requirement: Shared noise gate and event ids

The hub SHALL run one noise gate and one alert log for all parent links and SHALL give every noise event a per-session id carried in each `noise_alert`. A rejoining parent SHALL receive the missed events since its link was released.

#### Scenario: Same event on both parents

- **WHEN** the room gets loud while both parents are connected
- **THEN** both receive a noise alert with the same event id

#### Scenario: Missed events after rejoin

- **WHEN** parent B rejoins after its link was released and a noise event happened meanwhile
- **THEN** B receives that event as missed
