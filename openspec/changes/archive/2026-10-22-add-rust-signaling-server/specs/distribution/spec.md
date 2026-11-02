# Spec Delta

## ADDED Requirements

### Requirement: Hardened signaling image

The signaling image SHALL run as a non-root user on a distroless base with a read-only root filesystem.

#### Scenario: Container compromise

- **WHEN** the server process is compromised
- **THEN** it cannot write to the image's file system or find a shell
