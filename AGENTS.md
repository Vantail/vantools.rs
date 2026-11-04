# AGENTS.md

This file governs coding-agent behavior in this repository. It is the central
source of truth for universal agent operating rules. Project-specific build,
test, language, platform, and architecture instructions belong in separate
project-specific documentation or additional agent instruction files.

## Mandatory Pre-Task Protocol

Before executing any task in this repository, you must:

1. Read this `AGENTS.md` completely.
2. Read `docs/GIT_GUIDELINES.md` completely.
3. Read `docs/DESIGN_GUIDELINES.md` completely.
4. Inspect the repository structure and files relevant to the assigned task.
5. Read any applicable project-specific instructions.

If a required guideline file does not exist or cannot be read, stop and ask the
user how to proceed.

## Decision-Making Policy

- Do not make unauthorized product, architecture, scope, or policy decisions.
- Ask the user when requirements are ambiguous, contradictory, security-relevant,
  or require a consequential choice that has not been authorized.
- Make ordinary technical implementation choices only when needed to fulfill
  clear requirements within the assigned scope.
- Do not invent requirements or silently assume user intent.

## Scope Discipline

- Execute only the explicitly assigned task.
- Do not refactor unrelated code, fix unrelated bugs, update unrelated
  documentation, or make opportunistic improvements outside the defined scope.
- If you discover an unrelated issue, report it without changing it.
- Preserve existing user changes. Never overwrite or discard unrelated changes.

## Avoid Overthinking

- Be pragmatic and action-oriented. Do not overanalyze clear, bounded tasks.
- Use the simplest approach that satisfies the requirements and fits the
  existing project conventions.
- Do not create unnecessary planning steps, abstractions, speculative edge
  cases, or hypothetical future-proofing.
- Do not repeatedly reconsider decisions that are already supported by clear
  requirements and evidence.
- Investigate risks and edge cases in proportion to the task's complexity and
  impact. Give security, data-loss, correctness, and explicitly stated
  requirements the attention they warrant.
- If a real ambiguity or consequential decision blocks progress, ask the user.
  Otherwise, proceed with a straightforward implementation.
- Prefer completing the requested work and verifying it over producing
  excessive analysis or discussing alternatives that do not materially improve
  the result.

## Git Identity Policy

- You are a tool, not an author.
- Never add yourself as a co-author in commit messages.
- Never configure Git to use your own name or email.
- All commits must use the repository's already configured Git user identity.
- If Git identity is not configured, stop and inform the user.

## Git Operations

- Follow `docs/GIT_GUIDELINES.md` without exception.
- Never commit or push unrelated changes.
- Never discard existing user changes or rewrite shared history.

## Verification

- Run the relevant available checks and tests required by the task and
  project-specific instructions.
- Do not claim that a check passed unless it was actually run and passed.
- If a check cannot be run, report the reason and its status accurately.
- Before committing or pushing, inspect the diff and confirm it contains only
  authorized changes.

## Communication

- Report progress clearly and concisely.
- Flag blockers immediately.
- Do not proceed past an ambiguity requiring user input.
- At completion, summarize the changes, checks performed, commit status, and
  push status.
