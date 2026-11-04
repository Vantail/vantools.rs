# vantools.rs

A command-line toolset for working with `.van` plugin archives, written in
Rust.

The crate is named `vantools` and builds a single binary named `van`.

## Status

Early development. The current code implements the Phase 0.1 CLI skeleton
only:

- `van --help` prints usage.
- `van --version` prints the version.

The `pack`, `verify`, and `unpack` subcommands described in the `.van`
specification are not implemented yet.

## Requirements

- A Rust toolchain that supports edition 2024 (Rust 1.85 or newer).
- `rust-toolchain.toml` pins the `stable` channel and requests the
  `rustfmt` and `clippy` components.

## Build

```sh
cargo build
```

The binary is written to `target/debug/van`.

To install the binary from a checkout:

```sh
cargo install --path .
```

## Usage

```text
$ van --help
Usage: van [--help] [--version]

Options:
  --help     Show this help text
  --version  Show the version
```

```text
$ van --version
van 0.1.0
```

Running `van` without arguments prints the help text to stderr. Any
argument other than `--help` or `--version` is rejected as an unknown
subcommand.

### Exit codes

| Code | Meaning                                               |
| ---- | ----------------------------------------------------- |
| 0    | Success                                               |
| 1    | Runtime error                                         |
| 2    | Usage error (missing or unknown subcommand)           |

## Development

Run the same checks the CI workflow runs:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

`cargo test` runs the integration tests in `tests/cli.rs`, which exercise
the compiled binary.

### Coding style

Code in this repository follows an explicit, "boring" Rust style: no
`unsafe`, no `unwrap`/`expect`, no slice indexing, no iterator chains, and
explicit `return` statements. The rules are enforced by the compiler and
the Clippy configuration in `Cargo.toml` and `clippy.toml`, and documented
in [docs/RUST_GUIDELINES.md](docs/RUST_GUIDELINES.md). Formatting is fixed
by `rustfmt.toml`.

## Project layout

| Path                        | Contents                                        |
| --------------------------- | ----------------------------------------------- |
| `src/main.rs`               | Thin binary entry point; maps errors to codes   |
| `src/van.rs`                | Argument parsing and dispatch                   |
| `src/common/`               | Unified error type and the `Subcommand` trait   |
| `tests/cli.rs`              | Integration tests for the compiled binary       |
| `docs/SPEC.md`              | `.van` archive and manifest format              |
| `docs/`                     | Rust, design, and Git guidelines                |
| `.github/workflows/ci.yml`  | Test and lint workflow                          |

## Documentation

- [docs/SPEC.md](docs/SPEC.md) — the `.van` plugin archive format.
- [docs/RUST_GUIDELINES.md](docs/RUST_GUIDELINES.md) — Rust style rules.
- [docs/DESIGN_GUIDELINES.md](docs/DESIGN_GUIDELINES.md) — design principles.
- [docs/GIT_GUIDELINES.md](docs/GIT_GUIDELINES.md) — branch, commit, and
  push rules.
- [AGENTS.md](AGENTS.md) — operating rules for coding agents.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the pull request workflow and
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for community expectations.

## Security

See [SECURITY.md](SECURITY.md) for how to report a vulnerability. Do not
open a public issue for security problems.

## License

MIT. See [LICENSE](LICENSE).
