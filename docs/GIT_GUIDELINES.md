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

- Use imperative mood, for example: `Add SECURITY.md`,
  `Fix null check in parser`, `Update dependency versions`.
- Keep the subject line under 72 characters.
- Do not use emojis unless the user explicitly requests them.
- Never include `Co-authored-by:` trailers identifying the coding agent.

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
