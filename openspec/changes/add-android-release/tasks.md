# Tasks

Coarse tasks; refine before `/opsx:apply`. Depends on `split-android-modules` (derived versionCode).

- [ ] 1.1 Release workflow on `android-v*` tags in the protected `release` environment
- [ ] 1.2 Alpha signing; generate the direct-distribution key offline (owner) and write the rotation-lineage plan
- [ ] 1.3 Publish to a Play testing track and as a GitHub Releases APK
- [ ] 1.4 Extend the release-APK inspection: trackers, permissions, backup exclusion, `debuggable=false`
- [ ] 1.5 Store texts; `docs/ci-gates.md`; dry-run a release
- [ ] 1.6 Release smoke: install the R8 release APK on the emulator and pass AC-1 (pairing) and AC-2 (audio) before the first release APK ships
