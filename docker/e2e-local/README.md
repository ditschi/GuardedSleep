# Host-independent local E2E (Docker)

Runs the **full local test matrix** in one container — no host Node, pnpm, Playwright, or adb required. Only **Docker on Linux**.

## Quick start

```bash
pnpm run e2e:docker          # browser + LAN + hybrid (ws/wss when possible)
pnpm run e2e:docker:browser  # browser baseline only (fast smoke)
pnpm run check:kvm             # diagnose /dev/kvm for hybrid emulator
```

## Why a dedicated container?

| Problem                                 | Fix in this image                             |
| --------------------------------------- | --------------------------------------------- |
| WebRTC timeouts in nested Dev Container | `network_mode: host` — same as CI loopback    |
| Missing Playwright browsers             | Official `mcr.microsoft.com/playwright` image |
| Chromium `/dev/shm` in Docker           | `shm_size: 2gb` + `--disable-dev-shm-usage`   |
| Hybrid stack / emulator                 | Docker socket mount → nested `docker compose` |

## Test tiers

| Tier                | Needs                                          | Command                       |
| ------------------- | ---------------------------------------------- | ----------------------------- |
| Browser E2E (01–16) | Docker + Linux                                 | `pnpm run e2e:docker:browser` |
| + LAN browser       | same                                           | included in `e2e:docker`      |
| Hybrid ws/wss       | `/dev/kvm` on Docker **host** (recommended)    | included in `e2e:docker`      |
| Hybrid without KVM  | `EMULATOR_ALLOW_NO_KVM=1` (default in compose) | very slow software emulator   |

## KVM setup (hybrid Android)

If `pnpm run check:kvm` fails:

1. Enable **VT-x / AMD-V** in BIOS/UEFI.
2. Load modules: `sudo modprobe kvm` and `kvm_intel` or `kvm_amd`.
3. Add user to group: `sudo usermod -aG kvm $USER` (re-login).
4. Inside a VM: enable **nested virtualization**.

Without KVM, browser tests still run; hybrid uses software emulation or is skipped after timeout.

## Environment overrides

```bash
RUN_WSS_STACK=0 pnpm run e2e:docker           # skip Docker WSS stack + wss hybrid
USE_DOCKER_EMULATOR=0 pnpm run e2e:docker      # skip android emulator (browser only)
EMULATOR_ALLOW_NO_KVM=0 pnpm run e2e:docker   # fail fast if no KVM
```

## Related

- [android-emulator](../android-emulator/README.md) — headless AVD
- [stack](../stack/README.md) — HTTPS/WSS for wss hybrid
- [apps/e2e/README.md](../../apps/e2e/README.md) — test catalogue
