# Design Guidelines

These are language-agnostic, principle-based guidelines for the code in this
repository. They are a baseline: project-specific requirements take precedence,
and they never replace the requirements of a specific task, platform, or
component.

The goal is simple, readable, maintainable code. Complexity is justified when
it solves a real problem or makes the overall design easier to understand,
safer, or easier to maintain — not merely because a language or framework
permits it.

## Readability Over Cleverness

Prefer straightforward, easy-to-follow code over clever, dense, or
unnecessarily intricate solutions. Names should communicate intent. A future
maintainer should be able to understand the code without having to mentally
unpack avoidable tricks.

## Simplicity Over Complexity

Choose the simplest solution that reliably satisfies the actual requirements.
Do not use a language feature, design pattern, abstraction, or advanced
technique merely because the language makes it possible.

## Maintainability Over Sophistication

Optimize for code that the project's developers can read, debug, modify, and
maintain over time. Do not make code more sophisticated at the cost of making
routine maintenance harder.

## Explicit Over Implicit

Prefer clear control flow and visible behavior over hidden side effects,
surprising conventions, or mechanisms that obscure what the code does.

## Single Responsibility

Functions and modules should have a focused responsibility. Split them when
doing so materially improves clarity and maintainability; do not apply
mechanical rules without judgment.

## No Unnecessary Abstractions

Do not introduce extra interfaces, layers, frameworks, generic systems, or
indirection without a concrete benefit for the current requirements. Avoid
speculative extensibility and designing for hypothetical future needs.

## Pragmatic Use of Language Features

Modern language features and advanced techniques are appropriate when they make
the solution clearer, safer, or materially easier to maintain. Do not use them
solely to demonstrate language capability or sophistication.

## Error Handling

Fail explicitly. Do not silently swallow errors. Provide useful diagnostic
information without exposing secrets.

## Testing

Add or update tests for new or changed behavior where the project has an
applicable test setup. Tests should be deterministic and isolated where
practical.

## Documentation

Document public interfaces where appropriate. Explain contracts and non-obvious
behavior rather than narrating obvious implementation details.

## Dependency Discipline

Avoid adding dependencies for trivial functionality that can be implemented
clearly and safely with the standard library. Consider maintenance, security,
compatibility, and platform implications before adding a dependency.

## Configuration

Keep environment-specific or deploy-time configuration out of hardcoded source
values. Use environment variables or dedicated configuration files where
appropriate, and do not commit secrets.

## Consistency

Follow existing project conventions unless they conflict with explicit
requirements or create a material correctness or security issue. Ask before
making a consequential architectural change.
