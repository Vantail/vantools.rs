# vantools.rs

A command-line toolset for working with `.van` plugin archives, written in
Rust.

The crate is named `vantools` and builds a single binary named `van`.

## Status

Early development. The `pack` command is implemented: it validates a
plugin source directory and writes a `.van` archive. Archive verification
and extraction are not implemented yet.

## Requirements

- A Rust toolchain that supports edition 2024 (Rust 1.85 or newer).
- `rust-toolchain.toml` pins the `stable` channel and requests the
  `rustfmt` and `clippy` components.

## Build

```sh
cargo build
```

The binary is written to `target/debug/van`.

To build an optimized binary:

```sh
cargo build --release
```

The optimized binary is written to `target/release/van`.

To install the binary from a checkout:

```sh
cargo install --path .
```

## Usage

### Top-level options

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

Running `van` without arguments prints the help text to stderr and exits
with code 2. Any first argument other than `--help`, `--version`, or the
`pack` subcommand is rejected as an unknown subcommand. The top-level
help output lists the shared options; run `van pack --help` for the
`pack` usage.

### Pack a plugin

`van pack` builds a `.van` archive from a source directory:

```text
$ van pack --help
Usage: pack -s <source> -t <target>

Options:
  -s, --source <path>  The source directory containing manifest.json and dist/
  -t, --target <path>  The target directory to place the .van archive
  -h, --help           Print this help message
  -v, --version        Print version information
```

The source directory must contain a validated `manifest.json` and a
`dist/` directory. The archive is written to
`<target>/<name>-<version>.van`, where `name` and `version` come from the
manifest. Files outside `dist/` (other than `manifest.json`) are not
included in the archive; the command reports them as ignored.

For example, the repository includes `examples/test-plugin`:

```sh
mkdir -p out
van pack -s examples/test-plugin -t out
```

```text
Successfully created archive: out/test-plugin-0.1.0.van
Files written: 2
Files ignored: 0
```

A missing target directory is created. An existing regular archive at the
destination is overwritten after a warning on stderr. A symlink or other
non-regular entry at the destination is refused.

### Manifest and archive validation

`pack` validates the manifest and source tree before writing an archive.
It rejects:

- a `manifest.json` that is not UTF-8, starts with a byte order mark,
  exceeds 1 MiB, or is not valid JSON;
- a missing, empty, or longer-than-64-character `name`, or a name that is
  not a single path component;
- a missing or invalid SemVer `version`, or a version longer than 64
  characters;
- an `entrypoint` that is not a relative archive path or does not exist
  under `dist/`;
- a missing `dist/` directory, or a source that is a symlink;
- symlinked or non-regular `manifest.json` or `dist/` entries, or
  symlinks and other non-regular files inside `dist/`;
- a `node_modules` directory at any depth;
- archive entry names that are not relative, use backslashes, contain
  empty, `.`, or `..` segments, or are not in Unicode NFC.

The archive contains `manifest.json` at its root and the contents of
`dist/` under the `dist/` prefix, compressed with deflate. The manifest
bytes written to the archive are the exact bytes that passed validation.
See [docs/SPEC.md](docs/SPEC.md) for the complete format.

### Exit codes

| Code | Meaning                                                        |
| ---- | -------------------------------------------------------------- |
| 0    | Success                                                        |
| 1    | Runtime, validation, or `pack` argument error                  |
| 2    | Top-level usage error (missing, unknown, or non-Unicode argument) |

## Dependencies

Runtime dependencies:

- `semver` — SemVer version parsing.
- `serde` (with `derive`) and `serde_json` — manifest parsing.
- `unicode-normalization` — Unicode NFC validation.
- `zip` (deflate only) — archive writing.

The integration tests use `tempfile` to create temporary directories.

## Development

Run the same checks the CI workflow runs:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

`cargo test` runs the integration tests in `tests/`. In particular,
`tests/cli.rs` exercises the compiled binary.

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
| `src/main.rs`               | Binary entry point; routes subcommands and maps errors to exit codes |
| `src/lib.rs`                | Library root exposing the modules               |
| `src/van.rs`                | Top-level argument parsing and help/version output |
| `src/pack.rs`               | `pack` command implementation                   |
| `src/common/cli.rs`         | Argument collection, error reporting, and exit codes |
| `src/common/manifest.rs`    | Manifest parsing and validation                 |
| `src/common/validation.rs`  | Path validation and safe diagnostic rendering   |
| `src/common/walk.rs`        | Iterative directory traversal                   |
| `src/common/output.rs`      | Terminal output helpers                         |
| `src/common/error.rs`       | Unified error type                              |
| `src/common/command.rs`     | `Command` trait                                 |
| `tests/cli.rs`              | Integration tests for the compiled binary       |
| `tests/pack_test.rs`        | Tests for packing and path validation           |
| `tests/manifest_test.rs`    | Tests for manifest validation                   |
| `examples/test-plugin/`     | Sample plugin source used by the tests          |
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
