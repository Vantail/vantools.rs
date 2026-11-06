//! Argument parsing and dispatch for the `pack` command.

use std::io;
use std::path::{Path, PathBuf};

use vantools::common::Error;
use vantools::common::cli::{CliCommand, version_line};
use vantools::common::validation::{escape_path, escape_text};
use vantools::pack::{PackCommand, PackReport};

use super::output::write_line;
use super::tui::log::{log_info, log_ok, log_warn};
use super::tui::logo::print_logo;
use super::tui::progress::show_progress;

/// Source flag of the `pack` subcommand.
const SOURCE_FLAG: &str = "-s";

/// Target flag of the `pack` subcommand.
const TARGET_FLAG: &str = "-t";

/// Long form of the source flag of the `pack` subcommand.
const SOURCE_LONG_FLAG: &str = "--source";

/// Long form of the target flag of the `pack` subcommand.
const TARGET_LONG_FLAG: &str = "--target";

/// Short flag that requests help for the `pack` subcommand.
const HELP_FLAG: &str = "-h";

/// Long flag that requests help for the `pack` subcommand.
const HELP_LONG_FLAG: &str = "--help";

/// Short flag that requests the version of the `pack` subcommand.
const VERSION_FLAG: &str = "-v";

/// Long flag that requests the version of the `pack` subcommand.
const VERSION_LONG_FLAG: &str = "--version";

/// A `pack` invocation resolved into an action to execute.
///
/// The argument scan stops at the first `--help` or `--version` flag
/// without examining later arguments, so the resolution of a scan that
/// stopped early must be distinguishable from a fully parsed run.
enum PackRequest {
    /// Run the command with the parsed flag values.
    Run {
        /// Value of the source flag, when given.
        source: Option<PathBuf>,
        /// Value of the target flag, when given.
        target: Option<PathBuf>,
    },
    /// Print the help text and exit successfully.
    Help,
    /// Print the version banner and exit successfully.
    Version,
}

/// Parse the arguments following `pack` and run the command.
///
/// Help and version flags are answered with exit code 0. Unknown or
/// incomplete arguments produce a [`Error::CliError`] and exit code 1.
///
/// # Errors
///
/// Returns [`Error::CliError`] when an argument is unknown or a required
/// flag value is missing, and any error of the `pack` command.
pub fn run_cli(args: &[String]) -> Result<(), Error> {
    let request = parse_pack_request(args)?;
    return execute_pack_request(request);
}

/// Scan the arguments and parse the flags into a request.
///
/// The scan stops at the first `--help` or `--version` flag, so later
/// arguments are not examined and a stopped-early request must stay
/// distinguishable from a fully parsed run. A repeated `-s`/`-t`
/// replaces the earlier value, as before.
fn parse_pack_request(arguments: &[String]) -> Result<PackRequest, Error> {
    let mut source: Option<PathBuf> = None;
    let mut target: Option<PathBuf> = None;
    let mut remaining = arguments.iter();
    loop {
        let next = remaining.next();
        let argument = match next {
            None => break,
            Some(value) => value.as_str(),
        };
        if argument == HELP_FLAG || argument == HELP_LONG_FLAG {
            return Ok(PackRequest::Help);
        }
        if argument == VERSION_FLAG || argument == VERSION_LONG_FLAG {
            return Ok(PackRequest::Version);
        }
        let is_source = argument == SOURCE_FLAG || argument == SOURCE_LONG_FLAG;
        if is_source {
            source = Some(parse_flag_value(&mut remaining)?);
            continue;
        }
        let is_target = argument == TARGET_FLAG || argument == TARGET_LONG_FLAG;
        if is_target {
            target = Some(parse_flag_value(&mut remaining)?);
            continue;
        }
        return Err(unknown_argument(argument));
    }
    return Ok(PackRequest::Run {
        source,
        target,
    });
}

/// Consume the value that follows a `-s`/`-t` flag.
///
/// # Errors
///
/// Returns [`Error::CliError`] when the flag is the last argument and
/// no value follows it.
fn parse_flag_value<'a, I>(remaining: &mut I) -> Result<PathBuf, Error>
where
    I: Iterator<Item = &'a String>,
{
    let value = match remaining.next() {
        Some(value) => value,
        None => return Err(missing_arguments()),
    };
    return Ok(PathBuf::from(value));
}

/// Execute a parsed `pack` request.
fn execute_pack_request(request: PackRequest) -> Result<(), Error> {
    return match request {
        PackRequest::Help => print_pack_help(),
        PackRequest::Version => print_version(),
        PackRequest::Run {
            source,
            target,
        } => run_pack_command(source, target),
    };
}

/// Print the full help text of the `pack` command to stdout.
fn print_pack_help() -> Result<(), Error> {
    let metadata = PackCommand::new(PathBuf::new(), PathBuf::new());
    return print_line(metadata.help());
}

/// Resolve the required flags and run the `pack` command.
///
/// The source flag is reported as missing before the target flag, the
/// same order the flat resolution used. The banner and the progress bar
/// run after the flags were accepted, so the help and version output
/// stays untouched.
fn run_pack_command(
    source: Option<PathBuf>,
    target: Option<PathBuf>,
) -> Result<(), Error> {
    let resolved_source = match source {
        Some(value) => value,
        None => return Err(missing_arguments()),
    };
    let resolved_target = match target {
        Some(value) => value,
        None => return Err(missing_arguments()),
    };
    let command = PackCommand::new(resolved_source, resolved_target.clone());
    print_logo();
    show_progress();
    let report = command.run()?;
    print_report(&report, &resolved_target);
    return Ok(());
}

/// Print the report of a completed pack.
///
/// The lines keep the order the command produced them before the
/// reports existed: the notice about a created target directory, the
/// warning about an existing archive, then the success summary. Every
/// interpolated path is escaped like any other diagnostic.
fn print_report(report: &PackReport, target: &Path) {
    if report.target_created {
        log_info("created target directory", &escape_path(target));
    }
    if report.overwrote_existing {
        log_warn(
            "Warning: Overwriting existing archive",
            &escape_path(&report.archive_path),
        );
    }
    log_ok("Successfully created archive", &escape_path(&report.archive_path));
    log_info("Files written", &report.files_written.to_string());
    let ignored_count = report.ignored_files.len();
    log_info("Files ignored", &ignored_count.to_string());
    for relative in &report.ignored_files {
        log_info("Ignored path", &escape_path(relative));
    }
    return;
}

/// Build the error for an argument the `pack` command does not accept.
fn unknown_argument(argument: &str) -> Error {
    let rendered = escape_text(argument);
    return Error::CliError(format!(
        "Unknown argument: {rendered}. Run 'van pack --help' for usage."
    ));
}

/// Build the error for a `pack` invocation without source and target.
fn missing_arguments() -> Error {
    return Error::CliError(String::from(
        "Missing required arguments. Run 'van pack --help' for usage.",
    ));
}

/// Print one line to stdout.
fn print_line(text: &str) -> Result<(), Error> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    return write_line(&mut handle, text, "output");
}

/// Print the version banner (`van <crate-version>`) to stdout.
fn print_version() -> Result<(), Error> {
    let line: String = version_line();
    return print_line(&line);
}
