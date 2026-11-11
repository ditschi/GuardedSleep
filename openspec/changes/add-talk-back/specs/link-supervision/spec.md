# Spec Delta

## MODIFIED Requirements

### Requirement: Sans-IO interface contract

The `Link` SHALL be driven only through serde-JSON inputs and actions (snake_case, `type`-tagged); `LinkInput` and `LinkAction` in `crates/guardedsleep-core/src/link.rs` are the contract for every binding. The core SHALL export `message_keys()` so the i18n check can verify that each key exists in `locales/en.json`. Every local session description SHALL contain exactly two audio m-lines, the monitoring line first and the talk line second, otherwise the core refuses it: the baby's monitoring line SHALL be `sendonly` and its talk line `recvonly`; the parent's monitoring line SHALL be `recvonly` and its talk line `sendonly` or `inactive`. A parent talk line that is `inactive` or rejected SHALL mean that this parent cannot talk on that attempt.

#### Scenario: Baby SDP would receive audio

- **WHEN** the baby platform reports a local description whose first audio m-line is not `sendonly`
- **THEN** the core refuses it and the attempt does not proceed

#### Scenario: Baby SDP would send on the talk line

- **WHEN** the baby platform reports a local description whose second audio m-line is not `recvonly`
- **THEN** the core refuses it and the attempt does not proceed

#### Scenario: Parent SDP would send on the monitoring line

- **WHEN** the parent platform reports a local description whose first audio m-line is not `recvonly`
- **THEN** the core refuses it and the attempt does not proceed

#### Scenario: Parent without talk support

- **WHEN** the parent's answer marks the talk line `inactive`
- **THEN** the attempt verifies as usual and the baby unit denies talk requests on it as not supported
