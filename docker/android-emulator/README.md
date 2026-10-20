# Android emulator (Docker, dev-only)

Headless **Android emulator in Docker** for hybrid E2E (`Android baby + Chromium parent`) without Android Studio on the host.

**Requirements**

- Linux host with **`/dev/kvm`** (BIOS virtualization enabled; `ls -l /dev/kvm`)
- Docker Compose v2
- ~4 GB RAM free for the emulator container
- Repo dependencies installed (`pnpm install`) for Playwright hybrid tests

**Not supported:** macOS/Windows Docker Desktop (no reliable KVM passthrough). Use a physical device or host emulator there.

## Quick start

From the **repository root**:

```bash
pnpm run emulator:up
pnpm run emulator:wait
```

Hybrid tests use adb **inside** the container (no host `adb` required):

```bash
export GUARDEDSLEEP_ADB=docker/android-emulator/adb.sh
export GUARDEDSLEEP_DOCKER_GRADLE=1
export GUARDEDSLEEP_HYBRID_E2E=1
export GUARDEDSLEEP_HYBRID_LAN=1

pnpm run e2e:hybrid:lan-ws
```

Or run the full local orchestrator (starts emulator automatically when no adb device is present):

```bash
pnpm run e2e:docker:auto
```

## How it works

| Component                    | Role                                                                                                            |
| ---------------------------- | --------------------------------------------------------------------------------------------------------------- |
| `emulator` service           | AVD `guardedsleep_e2e`, API 35, headless, adb on port **5555**                                                  |
| `10.0.2.2`                   | Emulator reaches host services (Vite `5173`, signaling `18787`, Docker WSS `8443`)                              |
| `GUARDEDSLEEP_ADB`           | `adb.sh`: runs adb in the container and maps host repo paths to the mount                                       |
| `GUARDEDSLEEP_ADB_REPO_ROOT` | Optional override of the mount path (default `/mnt/workspace`, set in `compose.yaml`); set before `emulator:up` |
| `GUARDEDSLEEP_DOCKER_GRADLE` | Builds debug APK via `docker/android-build`                                                                     |

The emulator container is **privileged** and mounts `/dev/kvm` — dev-only, not for production. The repo is mounted read-only at `/mnt/workspace` so `adb install` can read the host-built APK.

## Commands

```bash
pnpm run emulator:up       # build + start detached
pnpm run emulator:wait     # wait for boot
pnpm run emulator:down     # stop container
pnpm run emulator:status   # ps + adb devices

pnpm run e2e:docker:auto   # browser E2E + docker emulator hybrid (ws + optional wss)
```

## Troubleshooting

- **`/dev/kvm` not found** — enable virtualization in BIOS; on Linux: `sudo modprobe kvm` and add your user to group `kvm`.
- **Boot timeout** — first image build downloads the system image (~1 GB); retry after build completes. Check logs: `docker compose -f docker/android-emulator/compose.yaml logs -f emulator`.
- **Connection failed on baby** — ensure dev stack listens on `0.0.0.0` (Playwright `dev:local` / published Docker ports). Emulator uses `http://10.0.2.2:5173`, not `127.0.0.1`.
- **WSS hybrid** — start Docker stack first (`pnpm run stack:lan`) or use `pnpm run e2e:docker:auto` which starts it for the wss step.

## Related

- [android-build](../android-build/README.md) — Gradle APK without host JDK
- [stack](../stack/README.md) — HTTPS/WSS local stack
- [apps/e2e/README.md](../../apps/e2e/README.md) — hybrid test matrix
