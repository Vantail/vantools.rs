# vantools.rs

A command-line toolset for working with `.van` plugin archives, written in
Rust.

The crate is named `vantools` and builds a single binary named `van`.

## Status

Early development. The `pack` command validates a plugin source
directory and writes a `.van` archive, `verify` checks an existing
archive, its manifest, and its layout without extracting it, and
`unpack` validates an archive and extracts it into
`<target>/<name>-<version>`.

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
`pack`, `unpack`, and `verify` subcommands is rejected as an unknown
subcommand. The top-level help output lists the shared options; run
`van pack --help`, `van unpack --help`, or `van verify --help` for the
subcommand usage.

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

### Verify an archive

`van verify` checks an existing `.van` archive without extracting it:

```text
$ van verify --help
Usage: verify -s <source>

Options:
  -s, --source <path>  The .van archive to verify (a regular file)
  -h, --help           Print this help message
  -v, --version        Print version information
```

It validates every entry name with the same path rules `pack` applies,
rejects symlink entries and `node_modules` at any depth, validates the
archived `manifest.json` with the same rules as `pack`, and requires the
`dist/` directory and the manifest entrypoint to exist in the archive.
The archive path must be a regular file, so a symbolic link or other
non-regular file is refused.

For example, verifying the archive produced above:

```sh
van verify -s out/test-plugin-0.1.0.van
```

```text
Verifying archive: test-plugin-0.1.0.van
Check passed: Paths and ZIP structure
Check passed: manifest.json is present and valid
Check passed: dist/ and entrypoint (app.ts) are present
manifest.json
{
  "entrypoint": "app.ts",
  "name": "test-plugin",
  "version": "0.1.0"
}

Archive Contents
    .
    ├── dist/
    │   └── app.ts
    └── manifest.json

Archive is valid.
```

An archive that fails a check is reported on stderr with exit code 1, and
no report is printed.

### Unpack an archive

`van unpack` validates an existing `.van` archive and extracts it:

```text
$ van unpack --help
Usage: unpack -s <source> -t <target>

Options:
  -s, --source <path>  The .van archive to extract
  -t, --target <path>  The target directory to extract into
  -h, --help           Print this help message
  -v, --version        Print version information
```

The archive is validated with the same inspection `van verify` runs, so
an invalid archive fails before anything is written. A valid archive is
extracted into `<target>/<name>-<version>`, with the archived
`manifest.json` at the top of that directory and the payload below its
`dist/` prefix. The target directory is created when it is missing, and
an existing `<name>-<version>` entry, including a symbolic link, is
refused instead of overwritten. The archive path itself must be a regular
file. Files are written into a temporary directory in the target and
moved into place with one rename, so the plugin directory appears
complete or not at all; a failed run removes its temporary directory.

For example, extracting the archive produced above:

```sh
van unpack -s out/test-plugin-0.1.0.van -t out
```

```text
Check passed: Paths and ZIP structure
Check passed: manifest.json is present and valid
Check passed: dist/ and entrypoint (app.ts) are present
Extracting archive: out/test-plugin-0.1.0.van
Successfully extracted archive: out/test-plugin-0.1.0
Files written: 2
```

The run above creates `out/test-plugin-0.1.0/manifest.json` and
`out/test-plugin-0.1.0/dist/app.ts`. Extracting the same archive into the
same target again fails because the plugin directory already exists.

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

`verify` and `unpack` apply the same manifest and path rules to an
existing archive, and additionally require `dist/` and the manifest
entrypoint to exist inside it. `verify` only reports on the archive;
`unpack` extracts it after the checks pass. Both commands require the
archive path to be a regular file.

Untrusted values echoed in diagnostics are rendered as a single line:
control characters are escaped, and argument and manifest values are
truncated after 64 characters.

The archive contains `manifest.json` at its root and the contents of
`dist/` under the `dist/` prefix, compressed with deflate. The manifest
bytes written to the archive are the exact bytes that passed validation.
See [docs/SPEC.md](docs/SPEC.md) for the complete format.

### Exit codes

| Code | Meaning                                                           |
| ---- | ----------------------------------------------------------------- |
| 0    | Success                                                           |
| 1    | Runtime, validation, or `pack`/`unpack`/`verify` argument error   |
| 2    | Top-level usage error (missing, unknown, or non-Unicode argument) |

## Dependencies

Runtime dependencies:

- `semver` — SemVer version parsing.
- `serde` (with `derive`) and `serde_json` — manifest parsing.
- `unicode-normalization` — Unicode NFC validation.
- `zip` (deflate only) — archive reading and writing.
- `same-file` (Windows only) — comparing file identities while
  validating paths.

The integration tests use `tempfile` to create temporary directories.

## Development

Run the same checks the CI workflow runs:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs the tests on Linux, macOS, and Windows, runs the lint checks on
Ubuntu, and runs a RustSec audit with `rustsec/audit-check`.

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

| Path | Contents |
| --- | --- |
| `src/main.rs` | Binary entry point; routes subcommands and maps errors to exit codes |
| `src/lib.rs` | Library root exposing the modules |
| `src/van.rs` | Top-level argument parsing and help/version output |
| `src/cli/mod.rs` | Terminal interaction modules used by the binary |
| `src/cli/parser.rs` | Shared flag scanning, usage errors, and stdout output |
| `src/cli/pack.rs` | `pack` argument parsing and dispatch |
| `src/cli/unpack.rs` | `unpack` argument parsing and dispatch |
| `src/cli/verify.rs` | `verify` argument parsing and dispatch |
| `src/cli/output.rs` | Line writing that reports stream failures |
| `src/cli/verify_report.rs` | Rendered output of a completed verification |
| `src/cli/tui/` | Logo, tagged log lines, progress bar, and tree rendering |
| `src/pack/mod.rs` | `pack` command and its source validation |
| `src/pack/writer.rs` | ZIP writing and atomic archive replacement |
| `src/unpack/mod.rs` | `unpack` command and its destination handling |
| `src/unpack/writer.rs` | Archive extraction and atomic directory installation |
| `src/verify/mod.rs` | `verify` command and its report |
| `src/common/mod.rs` | Shared module root and the `Subcommand` trait |
| `src/common/archive.rs` | Archive opening, validation, and manifest reading |
| `src/common/cli.rs` | Argument collection, error reporting, exit codes, `CliCommand` |
| `src/common/manifest.rs` | Manifest parsing and validation |
| `src/common/validation.rs` | Path validation and safe diagnostic rendering |
| `src/common/walk.rs` | Iterative directory traversal |
| `src/common/error.rs` | Unified error type |
| `tests/cli.rs` | Integration tests for the compiled binary |
| `tests/pack_test.rs` | Tests for packing and path validation |
| `tests/unpack_test.rs` | Tests for archive extraction |
| `tests/manifest_test.rs` | Tests for manifest validation |
| `tests/van_tests.rs` | Tests for unknown-subcommand diagnostics |
| `tests/main_tests.rs` | Tests for error reporting and exit-code mapping |
| `tests/verify_test.rs` | Tests for archive verification and its report |
| `examples/test-plugin/` | Sample plugin source used by the tests |
| `docs/SPEC.md` | `.van` archive and manifest format |
| `docs/` | Rust, design, and Git guidelines |
| `.github/workflows/ci.yml` | Test and lint workflow |

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
