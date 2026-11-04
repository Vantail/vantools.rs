# Rust Guidelines

These guidelines define the coding style and tooling configuration for
Rust code in this repository. They are enforced by the compiler,
Clippy, and rustfmt where technically possible, and by convention
otherwise.

The intent is **explicit, boring, readable Rust**. The code must be
understandable by maintainers and automated agents without requiring
deep idiomatic Rust experience. Clever, terse, or idiomatic-only
constructs are deliberately avoided.

## Rationale

Rust is chosen for its compile-time safety guarantees. However,
idiomatic Rust can be dense and difficult to read for non-experts. This
repository deliberately trades idiomaticity for readability while
retaining the safety guarantees. The borrow checker remains fully
active: readability is achieved by restricting the _style_, not by
weakening the _safety_.

## Enforced by the Compiler

The following rules are enforced by the compiler and by local
`cargo`/`clippy` invocations. Code that violates them does not compile,
or `cargo clippy` fails.

- **No `unsafe`.** `unsafe_code = "forbid"` at crate level.
- **All public items documented.** `missing_docs = "deny"`.
- **No `unwrap()` or `expect()`.** `clippy::unwrap_used` and
  `clippy::expect_used` are denied.
- **No slice indexing.** `clippy::indexing_slicing` is denied. Use
  `.get()` and handle the `Option`, or iterate.
- **No iterator chains.** The following `Iterator` methods are
  disallowed via `clippy.toml`: `map`, `filter`, `filter_map`,
  `flat_map`, `fold`, `reduce`, `sum`, `product`, `any`, `all`,
  `find`, `find_map`, `position`, `collect`, `for_each`,
  `try_for_each`, `count`, `last`, `nth`, `step_by`. Use explicit
  `for` loops.
- **Explicit `return`.** `clippy::implicit_return` is denied.
- **Short functions.** `clippy::too_many_lines` with a threshold of 60
  lines.
- **Exhaustive structs and enums.** `clippy::exhaustive_structs` and
  `clippy::exhaustive_enums` are denied.
- **Formatting.** `rustfmt` with `max_width = 80` and
  `use_small_heuristics = "Off"`.

## Conventions (Not Compiler-Enforced)

These rules are expected but not enforced by the compiler. Reviewers
and agents must follow them.

- **Use `Ok(())` and not `Ok(_)` for unit results.** Write the unit
  value explicitly.
- **Prefer `match` over `?` for error propagation.** `?` is permitted,
  but explicit `match` is preferred for readability.
- **Prefer `match` over `unwrap_or`, `ok_or`, or similar
  combinators.** Keep both branches visible.
- **Annotate types on function signatures and struct fields
  explicitly.** Local variable types are inferred; annotate only when
  inference is ambiguous (e.g., `Vec::new()`, `HashMap::new()`,
  `Default::default()`).
- **Prefer `String::from("...")` over `"...".to_string()`.** Both are
  acceptable, but `String::from` makes the allocation visible.
- **Use `#[allow(...)]` locally only with a comment explaining why.**
  Do not silence lints globally.

## Extending the Rules

- To add a new disallowed `Iterator` method, add it to
  `disallowed-methods` in `clippy.toml`.
- To tighten a lint, change its level in `Cargo.toml` under
  `[lints.clippy]`.
- If a Clippy suggestion conflicts with the explicit style, add the
  lint name to the `allow` list in `Cargo.toml` under `[lints.clippy]`,
  with a comment explaining the conflict.
- Do not add new `allow` entries for lints that do not conflict with
  the explicit style.

## References

- See `docs/DESIGN_GUIDELINES.md` for language-agnostic design
  principles.
- See `docs/GIT_GUIDELINES.md` for Git practices.
- Rust lints: https://doc.rust-lang.org/rustc/lints/
- Clippy lints: https://rust-lang.github.io/rust-clippy/stable/
- rustfmt configuration: https://rust-lang.github.io/rustfmt/
