# Decisions, roles and documentation conventions

Detail behind [AGENTS.md §6](../../AGENTS.md). If the two disagree, AGENTS.md and the specs win.

## Presenting options

When there are several reasonable approaches, present them as a checklist and always end with a recommendation:

```markdown
### Options for [Topic]

- [ ] **Option A: [Name]**
      Pros: ...
      Cons: ...
- [ ] **Option B: [Name]**
      Pros: ...
      Cons: ...

**Recommendation:** Option [X] because [reasoning].
```

Be honest about trade-offs. Skip the checklist when there is one reasonable approach or the detail does not
affect architecture or user experience.

When the user picks or rejects an option:

- Selected: record it in the OpenSpec change's `design.md` (Decisions, with the rationale). If nothing is built
  and no change exists yet, add a `PLANNED` item to [`docs/backlog.md`](../backlog.md).
- Rejected: keep it under "Discarded alternatives" in `design.md`, or in the `REJECTED` items of the backlog,
  with the reason and what triggered it (role review, user decision, technical infeasibility). Never delete it.

## Role coordination

Roles are defined in [`docs/roles/`](../roles/README.md). For non-trivial work the main agent acts as
Coordinator and asks: would a team lead call a meeting for this? New features, architecture decisions, MVP
scoping, security-relevant and significant UX changes: coordinate roles. Typos, formatting and small
clarifications: handle directly. Unsure: ask the user.

| Task type                                      | Minimum roles                           |
| ---------------------------------------------- | --------------------------------------- |
| New feature design                             | Product Owner, Technical Architect      |
| Feature with security implications             | + Security/Privacy Architect            |
| Feature with user-facing changes               | + UX/Usability Expert                   |
| Architecture or technical decision             | Technical Architect, Security Architect |
| Any decision being finalized (OpenSpec change) | + Critical Reviewer                     |
| MVP scoping, prioritization                    | Product Owner, Critical Reviewer        |

Each role sub-agent gets its definition from `docs/roles/<role>.md`, the document under review and a clear task
("Review [document] from the perspective of [Role]; follow the output format of your role"). Reviews run in
parallel, design iterations sequentially. Agents without sub-agents list the role perspectives they applied in
the PR description.

### Critical Review Gate

The Critical Reviewer is mandatory when a role's output changes the architecture, shifts the project scope,
contradicts a prior decision, introduces a new constraint (hardware, platform, licence), changes the security
model or redefines a core concept. When in doubt, trigger it.

For `CRITICAL` and `IMPORTANT` findings:

1. Route them back to the originating role.
2. The originator answers each one: accept, refute with evidence, or propose an alternative.
3. A refutation gets a second pass from the Critical Reviewer (accept or escalate).
4. Unresolved disagreements go to the user as an options checklist (above). The user has final authority;
   roles are advisors.
5. Only `SUGGESTION` findings may be noted without the loop.

The coordinator summarises feedback by severity, names conflicts between roles explicitly and documents the
outcome in the relevant files.

## Documentation conventions

- **Language:** English for specs, changes, backlog, concept notes, reviews and guides (AGENTS.md §5.2a).
  Translated UI strings live only in `locales/*.json`.
- **Where things live:** current behaviour in `openspec/specs/<capability>/spec.md`; decided work in
  `openspec/changes/<id>/`; history in `openspec/changes/archive/`; everything else in `docs/backlog.md`;
  background in `docs/concept/` and `docs/technical/` (non-normative).
- **Status tags** on backlog items and reference docs: `IDEA` (captured, not evaluated), `UNDER DISCUSSION`,
  `PLANNED` (decided, not started, with a target; backlog only), `DECIDED` (reference docs; decided behaviour
  goes into a change), `DEFERRED` (reason and target phase), `REJECTED` (reason).
- **User feature suggestions:** write them to `docs/backlog.md` under the matching capability (add a heading if
  none fits) as `IDEA`, with no phase unless the user decides one. When it is to be built, propose an OpenSpec
  change and delete the backlog item.
- **Changes:** `/opsx:propose` creates the files from the `spec-driven` schema; `openspec/config.yaml` adds the
  project rules (tier, out of scope, Pros / Cons / Recommendation, discarded alternatives, implementer briefs).
