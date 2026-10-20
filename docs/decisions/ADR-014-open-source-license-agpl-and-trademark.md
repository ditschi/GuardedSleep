# ADR-014: Open-source license (AGPL-3.0) and trademark policy

- Status: DECIDED
- Date: 2026-07-07
- Deciders: Product owner (user)

> **Partially superseded (2026-10-20):** goal 3 and the package names in the Decision section. F-Droid main repository is `DEFERRED` ([ADR-021](ADR-021-offline-lan-topologies-locales-and-distribution.md) §3); packages `@guardedsleep/protocol` and `@guardedsleep/signaling-server` are replaced by Rust crates ([ADR-020](ADR-020-toolchain-and-development-setup.md) §5–§6). The AGPL-3.0-or-later license and trademark policy are unchanged.

## Context

Before the first public source and APK distribution we must choose a license. Goals:

1. **Allow** self-hosting and operating signaling infrastructure (including paid hosting).
2. **Discourage** closed-source commercial fork apps sold as proprietary products.
3. Remain compatible with **F-Droid** (F-Droid main deferred, see ADR-021 §3) and transparent privacy review where possible.

A **non-commercial (NC)** license would forbid many commercial uses outright but is **not** OSI-approved, complicates F-Droid, and is a poor fit for infrastructure operators.

## Options Considered

### Option A: MIT or Apache-2.0

- **Pros**: Maximum adoption; simple compliance; Play/F-Droid friendly.
- **Cons**: Permits proprietary forks and resale without source disclosure — conflicts with the product owner’s intent.

### Option B: GPL-3.0 (device copyleft only)

- **Pros**: Strong copyleft on distributed apps; F-Droid friendly.
- **Cons**: Weaker on **modified signaling servers** offered as a network service (no “network copyleft” unless users receive the app over the network from that server in AGPL sense — signaling is separate).

### Option C: AGPL-3.0-or-later + trademark policy

- **Pros**: Copyleft on distributed apps **and** on modified AGPL-covered programs used interactively over a network; aligns with self-hosted signaling; F-Droid accepts AGPL; makes proprietary commercial forks costly (must publish source).
- **Cons**: Does **not** legally forbid selling copies — GPL family explicitly allows charging for distribution; mitigation is source disclosure + separate **trademark** rules for the product name.

### Option D: Custom non-commercial license

- **Pros**: Could ban resale of derivative apps explicitly.
- **Cons**: Not OSI-open-source; F-Droid friction; custom legal maintenance; infrastructure “commercial hosting” needs careful carve-outs.

## Decision

Adopt **Option C**:

1. **License:** [GNU Affero General Public License v3.0 or later](https://www.gnu.org/licenses/agpl-3.0.html) for the entire monorepo (`LICENSE` at repository root applies to `@guardedsleep/mobile`, `@guardedsleep/signaling-server`, `@guardedsleep/protocol` (superseded, see ADR-020 §5), Docker images built from this source, and documentation where copyright applies).
2. **Trademark:** Publish [`docs/legal/trademark.md`](../legal/trademark.md) — AGPL does not grant use of the **GuardedSleep** product name or official branding for third-party store listings; forks must use distinct naming.

**Clarification (product owner intent):** Operating **signaling infrastructure** (even commercially) is acceptable. Shipping a **derivative baby-monitor app** as a proprietary closed-source product is the outcome we want to prevent — AGPL requires source for distributed derivatives; trademark policy reduces passing off as the official app.

## Consequences

### Positive

- Clear legal frame before first APK / GitHub Release.
- Signaling operators who modify AGPL code must offer corresponding source.
- Consistent with privacy-first, auditable codebase narrative.

### Negative

- Contributors and downstream packagers must understand AGPL network and distribution obligations.
- Selling AGPL-compliant source-visible forks remains legally possible (unattractive for closed business models).

### Risks

- **Risk:** Trademark policy is not a substitute for trademark registration in all jurisdictions. **Mitigation:** Distinct official releases; register mark when budget allows.
- **Risk:** Some enterprises avoid AGPL. **Mitigation:** Acceptable for a consumer privacy baby monitor; dual-licensing only if a concrete partner requires it later.

## Discarded alternatives

- **MIT / Apache-2.0** — rejected: too permissive for proprietary resale of fork apps.
- **Custom NC license** — rejected: F-Droid / FOSS ecosystem friction; owner accepts AGPL + trademark instead.
