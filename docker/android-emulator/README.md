# Android emulator (Docker, dev-only)

Headless **Android emulator in Docker** used by the native emulator gate (`scripts/poc/local-gate.sh`, AC-1…AC-4 [+AC-10]) without Android Studio on the host.

**Requirements**

- Linux host with **`/dev/kvm`** (BIOS virtualization enabled; `ls -l /dev/kvm`)
- Docker Compose v2
- ~4 GB RAM free per emulator container

**Not supported:** macOS/Windows Docker Desktop (no reliable KVM passthrough). Use a physical device or host emulator there.

## Quick start

From the **repository root**:

```bash
just gate                  # builds the APK, starts emulators + signaling + coturn, runs the gate
DOZE_MINUTES=2 just gate   # quick run
```

Without KVM, `just gate` prints `SKIPPED: gate needs kvm` (a failure under `GS_STRICT=1`). The hybrid browser E2E flow and its `pnpm run emulator:*` / `e2e:hybrid*` scripts were removed in wave 1 ([ADR-020](../../docs/decisions/ADR-020-toolchain-and-development-setup.md)).

## How it works

| Component                    | Role                                                                                                                |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `emulator` service           | AVD `guardedsleep_e2e`, API 35, headless, adb on port **5555**                                                      |
| `10.0.2.2`                   | Emulator reaches host services (Vite `5173`, signaling `18787`, Docker WSS `8443`)                                  |
| `GUARDEDSLEEP_ADB`           | `adb.sh`: runs adb in the container and maps host repo paths to the mount                                           |
| `GUARDEDSLEEP_ADB_REPO_ROOT` | Optional override of the mount path (default `/mnt/workspace`, set in `compose.yaml`); set before starting the gate |

The emulator container is **privileged** and mounts `/dev/kvm` — dev-only, not for production. The repo is mounted read-only at `/mnt/workspace` so `adb install` can read the host-built APK.

## Troubleshooting

- **`/dev/kvm` not found** — enable virtualization in BIOS; on Linux: `sudo modprobe kvm` and add your user to group `kvm`.
- **Boot timeout** — first image build downloads the system image (~1 GB); retry after build completes. Check logs: `docker compose -f docker/android-emulator/compose.yaml logs -f emulator`.
- **Connection failed** — the emulator reaches host services via `10.0.2.2`, not `127.0.0.1`; make sure the signaling server listens on `0.0.0.0`.

## Related

- [android-build](../android-build/README.md) — Gradle APK without host JDK
- [stack](../stack/README.md) — HTTPS/WSS local stack
- [apps/e2e/README.md](../../apps/e2e/README.md) — hybrid test matrix
