//! The command table of the interactive interface.
//!
//! Every command the interface offers is one [`Command`] entry in
//! [`COMMANDS`]: its menu label, progress heading and steps, optional
//! result titles, the parameter form it opens, and the function that
//! runs it. The interactive loop reads this table to build the menu and
//! the forms and hands every submission to the shared progress runner,
//! so adding a command means adding one entry and its runner here. The
//! animation, the result screen, and the error presentation are shared
//! and need no per-command code.

use std::path::{Path, PathBuf};

use vantools::common::Error;
use vantools::common::validation::{escape_path, escape_text};
use vantools::pack::PackCommand;
use vantools::tui::progress::{ProgressStep, TaskMessages};
use vantools::unpack::UnpackCommand;
use vantools::verify::VerifyCommand;

/// Width of one form field.
///
/// Twenty-eight characters keeps the widest form, `pack` and `unpack`,
/// at 76 columns, which fits an 80-column terminal.
const FIELD_WIDTH: usize = 28;

/// One progress message of a command: percentage and status text.
type StepSpec = (usize, &'static str);

/// Steps shown while a plugin is packed.
const PACK_STEPS: &[StepSpec] = &[
    (20, "Preparing engine..."),
    (40, "Checking plugin configuration..."),
    (60, "Packing plugin files..."),
    (80, "Finalizing package..."),
    (100, "Ready to roll!"),
];

/// Steps shown while an archive is verified.
const VERIFY_STEPS: &[StepSpec] = &[
    (25, "Popping the hood..."),
    (50, "Inspecting archive structure..."),
    (75, "Checking manifest and entrypoint..."),
    (100, "Roadworthiness confirmed!"),
];

/// Steps shown while an archive is unpacked.
const UNPACK_STEPS: &[StepSpec] = &[
    (20, "Preparing engine..."),
    (40, "Reading archive..."),
    (60, "Unpacking plugin files..."),
    (80, "Installing plugin..."),
    (100, "Ready to roll!"),
];

/// Every command the interactive interface offers, in menu order.
const COMMANDS: [Command; 3] = [
    Command {
        name: "Pack",
        title: "Packing plugin",
        steps: PACK_STEPS,
        success_title: Some("✓ Plugin packed!"),
        failure_title: None,
        segments: &["van pack -s ", " -t ", ""],
        widths: &[FIELD_WIDTH, FIELD_WIDTH],
        runner: run_pack,
    },
    Command {
        name: "Verify",
        title: "Verifying archive",
        steps: VERIFY_STEPS,
        success_title: Some("✓ Archive roadworthy!"),
        failure_title: None,
        segments: &["van verify -s ", ""],
        widths: &[FIELD_WIDTH],
        runner: run_verify,
    },
    Command {
        name: "Unpack",
        title: "Unpacking plugin",
        steps: UNPACK_STEPS,
        success_title: Some("✓ Plugin unpacked!"),
        failure_title: None,
        segments: &["van unpack -s ", " -t ", ""],
        widths: &[FIELD_WIDTH, FIELD_WIDTH],
        runner: run_unpack,
    },
];

/// One command the interactive interface can run.
///
/// The fields hold everything the interface draws for the command, so
/// the menu, the parameter form, the progress animation, and the result
/// screen never need to know which command they are showing.
pub struct Command {
    /// Label drawn in the menu.
    name: &'static str,
    /// Heading drawn above the progress bar.
    title: &'static str,
    /// Steps drawn while the command runs.
    steps: &'static [StepSpec],
    /// Title drawn above a successful result.
    success_title: Option<&'static str>,
    /// Title drawn above a failed result.
    failure_title: Option<&'static str>,
    /// Text segments of the parameter form, in display order.
    segments: &'static [&'static str],
    /// Width of each form field, in display order.
    widths: &'static [usize],
    /// The function that runs the command from submitted field values.
    runner: fn(&[String]) -> Result<String, Error>,
}

impl Command {
    /// Return the menu label.
    pub fn name(&self) -> &'static str {
        return self.name;
    }

    /// Build the progress and result messages of the command.
    ///
    /// A title the command does not set stays unset, so the progress
    /// module draws its generic success or failure title instead.
    pub fn messages(&self) -> TaskMessages<'static> {
        let mut messages = TaskMessages::new(self.title);
        let success = self.success_title;
        if let Some(title) = success {
            messages = messages.with_success_title(title);
        }
        let failure = self.failure_title;
        if let Some(title) = failure {
            messages = messages.with_failure_title(title);
        }
        return messages;
    }

    /// Build the progress steps of the command.
    pub fn steps(&self) -> Vec<ProgressStep> {
        let mut steps: Vec<ProgressStep> = Vec::new();
        for pair in self.steps {
            let percent = pair.0;
            let message = pair.1;
            steps.push(ProgressStep::new(percent, message));
        }
        return steps;
    }

    /// Build the text segments of the command's parameter form.
    pub fn form_segments(&self) -> Vec<String> {
        let mut segments: Vec<String> = Vec::new();
        for segment in self.segments {
            segments.push(String::from(*segment));
        }
        return segments;
    }

    /// Return the width of each parameter form field.
    pub fn form_widths(&self) -> &'static [usize] {
        return self.widths;
    }

    /// Run the command from submitted field values.
    ///
    /// The returned description is the result detail the interface
    /// displays; the returned error is displayed as the failure reason.
    ///
    /// # Errors
    ///
    /// Returns the error the command rejects the values with, and any
    /// error of the underlying operation.
    pub fn run(&self, values: &[String]) -> Result<String, Error> {
        return (self.runner)(values);
    }
}

/// Return every command the interface offers, in menu order.
pub fn all() -> &'static [Command] {
    return &COMMANDS;
}

/// Return the command at `index`, or `None` when no command has it.
pub fn command_at(index: usize) -> Option<&'static Command> {
    return COMMANDS.get(index);
}

/// Run a submitted `pack` form and describe what it produced.
fn run_pack(values: &[String]) -> Result<String, Error> {
    let (source, target) = require_two_fields(values)?;
    reject_dash(source, "source")?;
    reject_dash(target, "target")?;
    let command =
        PackCommand::new(PathBuf::from(source), PathBuf::from(target));
    let report = command.run()?;
    let mut lines: Vec<String> = Vec::new();
    if report.target_created {
        let created = escape_path(Path::new(target));
        lines.push(format!("Target directory created: {created}"));
    }
    let archive = escape_path(&report.archive_path);
    if report.overwrote_existing {
        lines.push(format!("Existing archive overwritten: {archive}"));
    }
    lines.push(format!("Archive: {archive}"));
    lines.push(format!("Files written: {}", report.files_written));
    let ignored = report.ignored_files.len();
    lines.push(format!("Files ignored: {ignored}"));
    return Ok(lines.join("\n"));
}

/// Run a submitted `verify` form and describe what it found.
fn run_verify(values: &[String]) -> Result<String, Error> {
    let source = require_one_field(values)?;
    reject_dash(source, "source")?;
    let command = VerifyCommand::new(PathBuf::from(source));
    let report = command.run()?;
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("Archive: {}", escape_path(&report.source)));
    let name = escape_text(&report.manifest.name);
    let version = escape_text(&report.manifest.version);
    lines.push(format!("Plugin: {name} {version}"));
    lines.push(format!("Entries: {}", report.entry_names.len()));
    return Ok(lines.join("\n"));
}

/// Run a submitted `unpack` form and describe what it installed.
fn run_unpack(values: &[String]) -> Result<String, Error> {
    let (source, target) = require_two_fields(values)?;
    reject_dash(source, "source")?;
    reject_dash(target, "target")?;
    let command =
        UnpackCommand::new(PathBuf::from(source), PathBuf::from(target));
    let report = command.run()?;
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("Archive: {}", escape_path(&report.archive)));
    lines.push(format!("Installed to: {}", escape_path(&report.target)));
    lines.push(format!("Files written: {}", report.files_written));
    return Ok(lines.join("\n"));
}

/// Split the field values of a command that expects exactly two.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the submitted form does not hold
/// exactly two values, which the fixed form composition cannot trigger.
fn require_two_fields(values: &[String]) -> Result<(&str, &str), Error> {
    let first = match values.split_first() {
        Some(value) => value,
        None => return Err(field_count_error()),
    };
    let second = match first.1.split_first() {
        Some(value) => value,
        None => return Err(field_count_error()),
    };
    if !second.1.is_empty() {
        return Err(field_count_error());
    }
    return Ok((first.0.as_str(), second.0.as_str()));
}

/// Return the single field value of a one-field command.
///
/// # Errors
///
/// Returns [`Error::Runtime`] when the submitted form does not hold
/// exactly one value, which the fixed form composition cannot trigger.
fn require_one_field(values: &[String]) -> Result<&str, Error> {
    let first = match values.split_first() {
        Some(value) => value,
        None => return Err(field_count_error()),
    };
    if !first.1.is_empty() {
        return Err(field_count_error());
    }
    return Ok(first.0.as_str());
}

/// Build the error reported when a form hands over the wrong number of
/// field values.
fn field_count_error() -> Error {
    return Error::Runtime(String::from(
        "form returned an unexpected field count",
    ));
}

/// Refuse a field value the CLI parser would read as a flag.
///
/// Such a value would be parsed as an argument instead of as the path
/// the form displays, so the command is not run.
fn reject_dash(value: &str, field: &str) -> Result<(), Error> {
    if value.starts_with('-') {
        return Err(Error::Runtime(format!(
            "the {field} path must not start with a dash"
        )));
    }
    return Ok(());
}

#[cfg(test)]
mod tests {
    use super::{COMMANDS, command_at, run_pack, run_unpack, run_verify};
    use std::path::Path;
    use tempfile::tempdir;
    use vantools::common::Error;

    #[test]
    fn every_command_composes_one_form_line() {
        for command in &COMMANDS {
            let segments = command.form_segments();
            let widths = command.form_widths();
            assert_eq!(segments.len(), widths.len() + 1);
            assert!(!widths.is_empty());
        }
    }

    #[test]
    fn menu_labels_are_unique() {
        let mut names: Vec<&str> = Vec::new();
        for command in &COMMANDS {
            names.push(command.name());
        }
        names.sort();
        names.dedup();
        assert_eq!(names.len(), COMMANDS.len());
    }

    #[test]
    fn every_step_list_is_ordered_and_complete() {
        for command in &COMMANDS {
            let steps = command.steps();
            assert!(!steps.is_empty());
            let mut previous = 0;
            for step in &steps {
                assert!(step.percent > previous);
                previous = step.percent;
            }
            assert_eq!(previous, 100);
        }
    }

    #[test]
    fn every_command_names_its_own_result() {
        for command in &COMMANDS {
            let messages = command.messages();
            assert!(messages.success_title().is_some());
        }
    }

    #[test]
    fn an_unknown_index_has_no_command() {
        assert!(command_at(COMMANDS.len()).is_none());
    }

    #[test]
    fn a_dash_source_is_rejected_before_the_command_runs() {
        let values = vec![String::from("-s"), String::from("out")];
        let error = expect_error(run_pack(&values));
        assert!(
            format!("{error}")
                .contains("source path must not start with a dash")
        );
    }

    #[test]
    fn a_dash_target_is_rejected_before_the_command_runs() {
        let values = vec![String::from("plugin"), String::from("-t")];
        let error = expect_error(run_unpack(&values));
        assert!(
            format!("{error}")
                .contains("target path must not start with a dash")
        );
    }

    #[test]
    fn a_missing_field_is_reported() {
        let error = expect_error(run_verify(&[]));
        assert!(format!("{error}").contains("unexpected field count"));
    }

    #[test]
    fn a_surplus_field_is_reported() {
        let values = vec![String::from("a"), String::from("b")];
        let error = expect_error(run_verify(&values));
        assert!(format!("{error}").contains("unexpected field count"));
    }

    #[test]
    fn packing_describes_the_archive_it_wrote() {
        let target = tempdir();
        let target_dir = match target {
            Ok(value) => value,
            Err(error) => {
                panic!("failed to create a temporary directory: {error}")
            }
        };
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/test-plugin");
        let source_text = match source.to_str() {
            Some(value) => value,
            None => panic!("source path is not valid Unicode"),
        };
        let target_text = match target_dir.path().to_str() {
            Some(value) => value,
            None => panic!("target path is not valid Unicode"),
        };
        let values = vec![String::from(source_text), String::from(target_text)];
        let details = match run_pack(&values) {
            Ok(value) => value,
            Err(error) => panic!("pack failed: {error}"),
        };
        assert!(details.contains("Archive:"));
        assert!(details.contains("Files written: 2"));
    }

    #[test]
    fn verifying_describes_the_plugin_it_checked() {
        let target = tempdir();
        let target_dir = match target {
            Ok(value) => value,
            Err(error) => {
                panic!("failed to create a temporary directory: {error}")
            }
        };
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/test-plugin");
        let source_text = match source.to_str() {
            Some(value) => value,
            None => panic!("source path is not valid Unicode"),
        };
        let target_text = match target_dir.path().to_str() {
            Some(value) => value,
            None => panic!("target path is not valid Unicode"),
        };
        let pack_values =
            vec![String::from(source_text), String::from(target_text)];
        match run_pack(&pack_values) {
            Ok(_) => {}
            Err(error) => panic!("pack failed: {error}"),
        }
        let archive = target_dir.path().join("test-plugin-0.1.0.van");
        let archive_text = match archive.to_str() {
            Some(value) => value,
            None => panic!("archive path is not valid Unicode"),
        };
        let details = match run_verify(&[String::from(archive_text)]) {
            Ok(value) => value,
            Err(error) => panic!("verify failed: {error}"),
        };
        assert!(details.contains("Plugin: test-plugin 0.1.0"));
    }

    /// Unwrap the error of a runner result that must have failed.
    fn expect_error(result: Result<String, Error>) -> Error {
        return match result {
            Ok(value) => panic!("expected the command to fail: {value}"),
            Err(error) => error,
        };
    }
}
