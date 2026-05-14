/**
 * Conventional Commits — used by commitlint (pre-commit commit-msg + CI).
 *
 * The `type-enum` mirrors `@commitlint/config-conventional` plus one project-specific
 * type: `security`. Release Please (see ADR-006 and release-please-config.json) maps
 * these types to CHANGELOG sections:
 *   feat → Added (minor),  fix → Fixed (patch),  perf → Changed (patch),
 *   revert → Removed (patch),  security → Security (patch).
 * `feat!:` / `BREAKING CHANGE:` footer triggers a major (or minor pre-1.0) bump.
 * All other types are hidden from the changelog by default.
 */
export default {
  extends: ["@commitlint/config-conventional"],
  rules: {
    "body-max-line-length": [2, "always", 120],
    "header-max-length": [2, "always", 100],
    "type-enum": [
      2,
      "always",
      [
        "build",
        "chore",
        "ci",
        "docs",
        "feat",
        "fix",
        "perf",
        "refactor",
        "revert",
        "security",
        "style",
        "test",
      ],
    ],
  },
};
