//! Argument parsing and dispatch for the `pack` command.

use std::path::{Path, PathBuf};

use vantools::common::Error;
use vantools::common::cli::CliCommand;
use vantools::common::validation::escape_path;
use vantools::pack::{PackCommand, PackReport};
use vantools::tui::log::{log_info, log_ok, log_warn};
use vantools::tui::progress::show_progress;

use super::parser;
use super::parser::CliAction;

/// Name of the subcommand, used in its usage errors.
const COMMAND_NAME: &str = "pack";

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
    let parsed = parser::parse_args(args, COMMAND_NAME, true)?;
    return match parsed.action {
        CliAction::Help => {
            let metadata = PackCommand::new(PathBuf::new(), PathBuf::new());
            parser::print_help(metadata.help())
        }
        CliAction::Version => parser::print_version(),
        CliAction::Run => run_pack_command(parsed.source, parsed.target),
    };
}

/// Resolve the required flags and run the `pack` command.
///
/// The source flag is reported as missing before the target flag, the
/// same order the flat resolution used. The progress bar runs after the
/// flags were accepted, so the help and version output stays untouched.
fn run_pack_command(
    source: Option<PathBuf>,
    target: Option<PathBuf>,
) -> Result<(), Error> {
    let resolved_source = match source {
        Some(value) => value,
        None => return Err(parser::missing_arguments(COMMAND_NAME)),
    };
    let resolved_target = match target {
        Some(value) => value,
        None => return Err(parser::missing_arguments(COMMAND_NAME)),
    };
    let command = PackCommand::new(resolved_source, resolved_target.clone());
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
