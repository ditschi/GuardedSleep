# Development Roles

This directory defines the roles used during concept development and decision-making for GuardedSleep. Each role represents a specific perspective that reviews and contributes to design decisions.

## How Roles Are Used

Roles are invoked by the Cursor agent as sub-agents during concept work. The main agent acts as **Coordinator** and decides which roles are relevant for each task. See the [role-coordination.mdc](../../.cursor/rules/role-coordination.mdc) Cursor rule for the full orchestration logic.

### Automated Coordination

When working on features, technical analysis, or decisions:

1. The coordinator assesses which roles are relevant
2. Sub-agents are spawned with the role definition as context
3. Each role reviews/contributes from its specific perspective
4. The coordinator synthesizes feedback and presents conflicts to the user
5. The user makes the final decision

### Manual Invocation

You can also invoke a role manually by asking:

> "Review [document] from the perspective of [Role Name]"

## Code / infrastructure reviews

For structured checklists that map work items to these roles (e.g. signaling, mobile, Docker, CI), see [docs/reviews/](../reviews/README.md).

## Roles

| Role                         | File                                                           | Focus                                                |
| ---------------------------- | -------------------------------------------------------------- | ---------------------------------------------------- |
| Product Owner                | [product-owner.md](product-owner.md)                           | Feature prioritization, MVP scope, user needs        |
| Security / Privacy Architect | [security-privacy-architect.md](security-privacy-architect.md) | Encryption, data protection, threat modeling         |
| UX / Usability Expert        | [ux-usability-expert.md](ux-usability-expert.md)               | Ease of use, non-technical users, interaction design |
| Technical Architect          | [technical-architect.md](technical-architect.md)               | Platform feasibility, architecture, maintainability  |
| Critical Reviewer            | [critical-reviewer.md](critical-reviewer.md)                   | Devil's advocate, gaps, contradictions, blind spots  |

## Output Format

All roles use a consistent output format:

1. **Findings** as a bullet list, each tagged with severity:
   - `CRITICAL` -- Must be addressed before proceeding
   - `IMPORTANT` -- Should be addressed, may impact success
   - `SUGGESTION` -- Nice to have, can be deferred

2. **Summary Verdict**:
   - `APPROVED` -- No significant concerns
   - `APPROVED WITH CONCERNS` -- Can proceed but issues noted
   - `NEEDS REWORK` -- Significant problems must be resolved first
