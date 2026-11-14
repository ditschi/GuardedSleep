# Tasks

Coarse tasks; refine before `/opsx:apply`. Depends on `split-android-modules` (derived versionCode).

- [ ] 1.1 Release workflow on `android-v*` tags in the protected `release` environment
- [ ] 1.2 Alpha signing; generate the direct-distribution key offline (owner) and write the rotation-lineage plan
- [ ] 1.3 Publish to a Play testing track and as a GitHub Releases APK
- [ ] 1.4 Extend the release-APK inspection: trackers, permissions, backup exclusion, `debuggable=false`; make `release-check.sh` parse `<uses-permission>` elements instead of grepping every permission string (it currently lists `android.permission.DUMP`, a profileinstaller receiver protection attribute), and drop this change's duplicate permission allow-list requirement, which `split-android-modules` already archived
- [ ] 1.5 Store texts; `docs/ci-gates.md`; dry-run a release
- [ ] 1.6 Release smoke: install the R8 release APK on the emulator and pass AC-1 (pairing) and AC-2 (audio) before the first release APK ships
- [ ] 1.7 Release tag check: require a green strict gate run (incl. the AC-12 UI journey of `gate-ui-journeys`) on the tagged commit; until then it is a manual item of `docs/contributing/manual-release-testing.md`
