# Git Guidelines

These guidelines apply to every change made in this repository, whether by a
human contributor or a coding agent. Follow them without exception.

## Branch Policy

- Work only on the currently checked-out branch.
- Never switch branches, create new branches, or delete branches without
  explicit user instruction.

## Working Tree Safety

- Inspect the working tree before making changes.
- Preserve all pre-existing user changes.
- Never overwrite, discard, stage, commit, or push unrelated changes.
- If pre-existing changes could be confused with task changes, stop and ask how
  to proceed.

## Commit Policy

### Atomic Commits

- Commit each completed, discrete logical unit of work.
- Keep commits small enough to review and revert independently.
- Verify the result of each logical unit before committing it.
- If verification fails, fix or revert the task-related change before
  proceeding. Never revert unrelated user changes.
- Do not create empty commits.

### Commit Message Format

Use the Conventional Commits structure for every commit:

    <type>(<scope>): <short summary>

    <body>

    [optional footer(s)]

_Types_

- `feat` — a new feature
- `fix` — a bug fix
- `docs` — documentation-only changes
- `style` — formatting, whitespace, or other non-behavioral changes
- `refactor` — a change that neither fixes a bug nor adds a feature
- `perf` — a performance improvement
- `test` — adding or fixing tests
- `build` — build-system or dependency changes
- `ci` — CI configuration changes
- `chore` — maintenance tasks that do not fit any other type
- `revert` — reverting a previous commit

_Subject line_

- Format: `<type>(<scope>): <short summary>`.
- The scope is optional. Use it when the change is confined to a
  recognizable component, for example `parser`, `auth`, or `docs`.
  Scope is lowercase.
- Use imperative mood in the summary, for example `add`, `fix`,
  `update` — not `added`, `adds`, or `fixed`.
- The entire subject line must not exceed 72 characters.
- Do not end the subject line with a period.
- Do not use emojis unless the user explicitly requests them.
- For a breaking change, append `!` after the type or scope, for
  example `feat(api)!: remove v1 endpoints`.

_Body_

- Separate the subject line from the body with exactly one blank line.
- The body is required and must describe **what changed**. A reader who
  sees only the commit must be able to understand the scope of the
  change.
- For a trivial change (for example a typo fix), a single short body
  line is sufficient; the body must not be omitted entirely.
- Use bullet points or short paragraphs. Wrap lines at approximately 72
  characters for readability.
- Where useful, describe **why** the change was made in addition to what
  changed.
- Do not restate the subject line or narrate obvious implementation
  details.

_Footers_

- Optional. Use footers for breaking changes
  (`BREAKING CHANGE: <description>`), issue references
  (`Refs: #123`), or similar metadata.
- Never include a `Co-authored-by:` trailer that identifies the coding
  agent.

_Examples_

    feat(parser): add support for nested expressions

    - Extend the grammar to allow parenthesized subexpressions
    - Add recursive-descent handling for grouping tokens
    - Add unit tests covering three levels of nesting

    fix(auth): prevent token refresh on expired session

    - Check token expiry before issuing a refresh request
    - Return a 401 response when the session has already expired

    docs: correct broken link in CONTRIBUTING.md

    - Update the anchor target to match the current heading text

_Existing commits_

- Existing commits are not rewritten. This format applies to new
  commits only.

### Identity

- Use the repository's configured Git user identity for commits.
- Do not change Git identity configuration.
- If identity is missing, stop and ask the user.

## Push Policy

- Push once, after the entire assigned task has been completed, verified, and
  committed.
- Do not push intermediate commits.
- Push to the current branch's configured upstream.
- If no upstream is configured, stop and ask the user.
- Before pushing, verify that the outgoing commits contain only authorized task
  changes and that no unrelated changes are included.
- If a required policy placeholder remains unresolved, do not push.

## Prohibited Actions

- Force-pushing, including `--force` and `--force-with-lease`.
- Rebasing shared branches.
- Amending commits that have already been pushed.
- Changing Git configuration.
- Creating tags or releases without explicit instruction.
- Committing or pushing unrelated user changes.
