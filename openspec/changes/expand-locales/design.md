# Design

## Context

Sources: ADR-021 §2, ADR-020 §7, AGENTS.md §5.2a. Today `locales/` holds en, de, fr, es with 645 keys each and full key parity. Android reads only the `native.*` group (`ANDROID_GROUPS` in `crates/i18n-gen`); the other groups belong to the legacy web client, which `rebuild-web-client` replaces with a greenfield `apps/web` and new keys. The device wording "baby device" / "parent device" was decided on 2026-10-23 (`rebuild-web-client` design.md, open decision A8 of `alpha-ux-flows.md`).

## Decisions

- **Reach over perfection** (owner D2): AI translation now, native-speaker review as an improvement path; Arabic proves RTL early.
- **Critical keys are complete or the locale does not ship**, so an alarm can never mix languages; non-critical keys may fall back.
- **Prompting rules**: keep placeholders and plural categories; keep alert copy short; the generator validates the output.
- **Critical tags in one sidecar file**: `locales/critical.json` lists key patterns (exact keys or `group.*` prefixes) with a length budget per pattern (characters, measured on the rendered `other` case). The catalogues stay plain message JSON, so translators and CSpell see only copy.
  - Pros: one reviewable list of what is safety copy; no churn in 20 catalogues when a tag changes.
  - Cons: a renamed key can drop out of the list; the generator fails on a pattern that matches no `en` key.
- **Review markers per locale file**: each non-English catalogue carries a reserved top-level `"$meta": {"default": "machineTranslated", "reviewed": [<keys>]}`. A value is `reviewed` when its key is listed, else `machineTranslated`. The generator ignores `$meta` for output and fails on a listed key that does not exist. Re-translating a reviewed key removes it from `reviewed`.
- **Device wording rename lives here**: the catalogue rewrite touches every value anyway, so "baby phone" / "parent phone" become "baby device" / "parent device" in `en` and the existing translations before the new locales are generated, and `alpha-ux-flows.md` A8 is closed. Android copy changes through the catalogue only.
- **Unused keys are removed and kept out**: a key that no client and no core message key uses is not translated into 19 languages. `i18n-gen check` fails on such a key.
- **Core message keys live in a `core` group** (owner, 2026-10-23): each `message_keys()` entry `snake_case` is the catalogue key `core.<camelCase>` (`server_unreachable` → `core.serverUnreachable`). Clients look core keys up by that rule instead of hand-kept mappings, and the copy now under unrelated `native.*` names moves there. A core test reads `locales/en.json` and fails on a missing key; `just lint` runs it. Discarded: an explicit mapping table (a second list that drifts, and the mapping is still duplicated per client); matching the camelCase key against the last segment of any key (implicit, ambiguous, and fails today for `pairingKeyMismatch` and `wrongKeyAttempt`).
- **Order**: generator rules and the rename first (in parallel with `rebuild-web-client`); unused-key pruning and translation after the web client has its keys; screenshots last.
- **Discarded alternatives:** Fluent (second format next to ICU on Android); Android XML as the source (no web output); four locales only (smaller reach); inline value objects `{"value", "critical", "marker"}` in every catalogue (heavy churn, noisy for translators and CSpell); per-key markers as a parallel file per locale (two files to keep in step).

## Risks / Trade-offs

- Machine translation errors in safety copy: length budgets and `critical` tags keep alerts short and reviewable; German is owner-reviewed.
- Translation is the bulk of the work and needs the final key set; running it before `rebuild-web-client` group 7 lands would translate keys that are then deleted.
