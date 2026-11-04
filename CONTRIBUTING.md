# Contributing

Thanks for your interest in improving this project. Contributions of all sizes
are welcome, provided they stay within the scope of the project: a
command-line toolset for working with `.van` plugin archives, written in Rust.

By participating in this project, you agree to follow our
[Code of Conduct](CODE_OF_CONDUCT.md).

## Reporting Bugs

Before opening an issue, search the existing issues to check whether the
problem has already been reported.

When you report a bug, include:

- The version or commit you are using.
- Your operating system, Rust toolchain version, and any relevant platform
  details.
- The exact command or code path that triggered the problem.
- What you expected to happen and what happened instead.
- A minimal reproduction where possible, including any input archives or a
  description of their structure.

## Suggesting Features

Open an issue describing the feature, the problem it solves, and the use case
behind it. Explain how it fits the scope of the project. Do not open a pull
request for a substantial feature before the approach has been discussed in an
issue — this avoids work that cannot be merged.

## Pull Request Workflow

1. **Fork** the repository and clone your fork.
2. **Branch** from the default branch. Use a short, descriptive branch name.
3. **Commit** your changes in small, logical units. Follow the commit
   expectations in [docs/GIT_GUIDELINES.md](docs/GIT_GUIDELINES.md).
4. **Open a pull request** against the default branch. Describe what changed
   and why, reference any related issues, and note how you verified the change.

Keep pull requests focused. Do not mix unrelated changes into one pull request;
unrelated changes make review slower and make a change harder to revert.

## Coding and Design Standards

Follow [docs/DESIGN_GUIDELINES.md](docs/DESIGN_GUIDELINES.md) for coding and
design standards. These are the baseline expectations for readability,
simplicity, maintainability, error handling, testing, and dependency choices.

Follow the existing conventions of the project. If a convention conflicts with
an explicit requirement, or changing it would be a consequential architectural
decision, raise it in an issue before making the change.

## Git Practices

Follow [docs/GIT_GUIDELINES.md](docs/GIT_GUIDELINES.md) for branch, commit, and
push expectations. That document applies to human contributors and coding
agents alike.

## License

This project is released under the MIT License; see [LICENSE](LICENSE).
By submitting a contribution, you agree that your contribution is licensed
under the same terms. You do not need to sign a contributor license agreement.
