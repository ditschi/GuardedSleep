# Tasks

Coarse tasks; refine before `/opsx:apply` (beta). Depends on `add-rust-signaling-server` and `rebuild-web-client`.

- [ ] 1.1 Android listener on the `relay` crate with lifecycle, locks and opaque NSD
- [ ] 1.2 Private-address cleartext rule: network security config plus one platform check; release-APK assertion keeps the base config restrictive
- [ ] 1.3 Server local mode (code exists from W2-B in `guardedsleep-server`: `--serve-web`, `--local`; specify, security-review and complete it here): static UI and loopback API on 127.0.0.1/::1, Origin/Host checks, LAN signaling port, forwarding for a second PC
- [ ] 1.4 Invite carries the LAN address; user copy and a hotspot guide
- [ ] 1.5 Host simulation and gate scenarios for both topologies; `docs/ci-gates.md`
