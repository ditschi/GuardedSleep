# GuardedSleep trademark policy

> STATUS: DECIDED — companion to [ADR-014](../../openspec/changes/archive/2026-07-07-adr-014-open-source-license-agpl-and-trademark/design.md).

The software in this repository is licensed under the **GNU Affero General Public License v3.0 or later** ([`LICENSE`](../../LICENSE)). The license grants broad rights to use, modify, and redistribute the **code**, subject to AGPL conditions (including source availability for distributed derivatives and for network-facing AGPL-covered services you modify).

The **GuardedSleep** name, logo, and product positioning are **not** granted by the AGPL. This policy clarifies acceptable use of project branding.

## What you may do (without separate permission)

- Run, study, modify, and redistribute the **software** under AGPL-3.0-or-later terms.
- **Self-host** the signaling server and web stack (see `docker/stack/`).
- **Operate infrastructure** (hosting signaling or the static web UI) for yourself or others, including for a fee, provided AGPL obligations are met when you distribute or offer modified versions over a network.
- State that your deployment is **“based on GuardedSleep”** or **“powered by GuardedSleep”** in factual, non-misleading documentation (e.g. README, ops runbook).

## What requires permission (not granted by default)

- Using **GuardedSleep**, **GuardedSleep – Privacy Baby Monitor**, or confusingly similar names as the **primary product name** of a **forked or derivative app** distributed to end users (e.g. Play Store listing title).
- Using official **logos, icons, or store graphics** from this repository in a way that implies **endorsement** or an **official** release when it is not built or published by the GuardedSleep maintainers.
- Implying that a third-party build is the **official** GuardedSleep app when it is not.

Forks and derivatives should use a **distinct name** and branding. AGPL still requires corresponding source for distributed binaries.

## Official releases

Builds published by the GuardedSleep maintainers (e.g. GitHub Releases artifacts, tagged `web-v*` or `android-v*`, documented Docker images on GHCR under the project namespace) are the **reference** distributions. Third-party builds are welcome under AGPL but must not be presented as official without agreement.

## Contact

For trademark or branding questions, open a GitHub discussion or issue in the project repository (contact details to be added when the public org/repo URL is final).
