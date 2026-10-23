# Tasks

Coarse tasks; refine before `/opsx:apply`. Depends on `add-rust-signaling-server` and, for the web part, `rebuild-web-client`.

- [ ] 1.1 Research and vet initial STUN candidates against the criteria in `design.md`; record operator, country, policy link, vetting date
- [ ] 1.2 Data file for the list; generator or loader for Android and web
- [ ] 1.3 Android: Advanced setting with default "Your signaling server", list choice and manual `host:port` entry, warning dialog
- [ ] 1.4 Web: the same setting in `apps/web`
- [ ] 1.5 Copy and warning text in all shipped locales (critical keys)
- [ ] 1.6 Tests: no third-party STUN request without opt-in; Google STUN never present (lint check); non-blocking reachability check for list entries
