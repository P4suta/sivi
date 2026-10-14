//! The commands of the binary: each turns its arguments into one build or check, renders the result, and decides the exit status.

use crate::args::{Arguments, Command, Environment, Format, Rendering, ThemeCommand};
use crate::build::{self, Outcome, Request, Sink};
use crate::code_themes;
use crate::library;
use crate::palette;
use crate::render::{self, Sources};
use crate::themes;
use crate::ui;
use sivi_core::calendar;
use sivi_core::diagnostic::{Code, Diagnostic, Severity, has_error};
use sivi_typst::UtcTimestamp;
use std::path::{Path, PathBuf};

/// The exit status of a run without an error diagnostic.
pub const SUCCESS_EXIT_CODE: u8 = 0;
/// The exit status of a run that reported an error, or a warning under `--deny-warnings`.
pub const DIAGNOSTIC_EXIT_CODE: u8 = 1;
/// The exit status of a usage error or an input that could not be read, which `clap` also reports.
pub const USAGE_EXIT_CODE: u8 = 2;
/// The exit status of a defect of sivi or of its adapters.
pub const INTERNAL_EXIT_CODE: u8 = 3;

/// The extension of the default output beside the deck.
const PDF_EXTENSION: &str = "pdf";
/// The output path that means standard output.
const STDOUT_PATH: &str = "-";

/// What a run leaves for the streams and the shell.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Run {
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub code: u8,
}

impl Run {
    fn usage(message: &str) -> Self {
        Self {
            stdout: Vec::new(),
            stderr: format!("error: {message}\n"),
            code: USAGE_EXIT_CODE,
        }
    }
}

/// How one command renders and judges its diagnostics.
#[derive(Clone, Copy, Debug)]
struct Setting<'a> {
    rendering: &'a Rendering,
    environment: &'a Environment,
}

/// Whether a code names a defect of sivi rather than of the deck or the theme.
#[must_use]
pub fn is_internal(code: Code) -> bool {
    code == Code::Internal || code.as_str().starts_with("E09")
}

/// The exit status a list of diagnostics earns.
#[must_use]
pub fn exit_code(diagnostics: &[Diagnostic], deny_warnings: bool) -> u8 {
    if diagnostics
        .iter()
        .any(|diagnostic| is_internal(diagnostic.code))
    {
        INTERNAL_EXIT_CODE
    } else if has_error(diagnostics)
        || (deny_warnings
            && diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity() == Severity::Warning))
    {
        DIAGNOSTIC_EXIT_CODE
    } else {
        SUCCESS_EXIT_CODE
    }
}

/// The PDF creation date `SOURCE_DATE_EPOCH` fixes, or none when it is unset.
///
/// # Errors
///
/// Returns why the variable's value is not a date the PDF can hold.
pub fn timestamp(environment: &Environment) -> Result<Option<UtcTimestamp>, String> {
    let Some(text) = &environment.source_date_epoch else {
        return Ok(None);
    };
    let seconds: i64 = text.trim().parse().map_err(|_invalid| {
        format!("SOURCE_DATE_EPOCH is `{text}`, not a whole number of seconds")
    })?;
    let civil = calendar::civil(seconds).ok_or_else(|| {
        format!("SOURCE_DATE_EPOCH is {seconds}, outside the dates a PDF can hold")
    })?;
    let year = i32::try_from(civil.year)
        .map_err(|_beyond| format!("SOURCE_DATE_EPOCH names the year {}", civil.year))?;
    Ok(Some(UtcTimestamp {
        year,
        month: civil.month,
        day: civil.day,
        hour: civil.hour,
        minute: civil.minute,
        second: civil.second,
    }))
}

/// Where `build` writes: the given output, `-` for standard output, or `deck.pdf` beside the deck.
#[must_use]
pub fn sink(deck: &Path, output: Option<&Path>) -> Sink {
    match output {
        Some(path) if path.as_os_str() == STDOUT_PATH => Sink::Stdout,
        Some(path) => Sink::File(path.to_path_buf()),
        None => Sink::File(default_output(deck)),
    }
}

/// The default output of a deck, beside it.
#[must_use]
pub fn default_output(deck: &Path) -> PathBuf {
    deck.with_extension(PDF_EXTENSION)
}

fn rendered(outcome: &Outcome, deck: &Path, setting: Setting<'_>) -> Run {
    let deck_path = deck.display().to_string();
    let sources = Sources {
        deck_path: &deck_path,
        deck_text: &outcome.deck_text,
        theme_label: &outcome.theme_label,
        theme_text: &outcome.theme_text,
    };
    let code = exit_code(&outcome.diagnostics, setting.rendering.deny_warnings);
    match setting.rendering.format {
        Format::Human => Run {
            stdout: outcome
                .product
                .as_ref()
                .and_then(|product| product.bytes.clone())
                .unwrap_or_default(),
            stderr: render::human(
                &outcome.diagnostics,
                &sources,
                setting.rendering.color.styled(setting.environment),
            ),
            code,
        },
        Format::Json => Run {
            stdout: render::json(&outcome.diagnostics, &sources, outcome.product.as_ref())
                .into_bytes(),
            stderr: String::new(),
            code,
        },
    }
}

fn deck_run(deck: &Path, sink: Sink, theme: Option<&str>, setting: Setting<'_>) -> Run {
    if sink == Sink::Stdout && setting.rendering.format == Format::Json {
        return Run::usage("`-o -` and `--format json` both write to standard output; choose one");
    }
    let timestamp = match timestamp(setting.environment) {
        Ok(timestamp) => timestamp,
        Err(message) => return Run::usage(&message),
    };
    let request = Request {
        deck,
        theme,
        sink,
        timestamp,
        fonts_dir: library::fonts_dir(setting.environment),
    };
    let outcome = build::run(&request);
    if let Some(failure) = &outcome.failure {
        return Run::usage(failure);
    }
    rendered(&outcome, deck, setting)
}

fn theme_run(theme: &str, setting: Setting<'_>) -> Run {
    let choice = themes::choose(Some(theme), None, Path::new("."));
    let fonts_dir = library::fonts_dir(setting.environment);
    let (label, text, diagnostics) = match themes::load(&choice, fonts_dir.as_deref()) {
        Ok((loaded, warnings)) => (loaded.label, loaded.text, warnings),
        Err(diagnostics) => (String::from(theme), String::new(), diagnostics),
    };
    let outcome = Outcome {
        deck_text: String::new(),
        theme_text: text,
        theme_label: label,
        diagnostics,
        product: None,
        failure: None,
    };
    rendered(&outcome, Path::new(theme), setting)
}

/// Runs the command the arguments name.
#[must_use]
pub fn run(arguments: Arguments, environment: &Environment) -> Run {
    match arguments.command {
        Command::Build {
            deck,
            output,
            theme,
            rendering,
        } => {
            let sink = sink(&deck, output.as_deref());
            let setting = Setting {
                rendering: &rendering,
                environment,
            };
            deck_run(&deck, sink, theme.as_deref(), setting)
        }
        Command::Check {
            deck,
            theme,
            rendering,
        } => {
            let setting = Setting {
                rendering: &rendering,
                environment,
            };
            deck_run(&deck, Sink::Nothing, theme.as_deref(), setting)
        }
        Command::Theme {
            command: ThemeCommand::Check { theme, rendering },
        } => theme_run(
            &theme,
            Setting {
                rendering: &rendering,
                environment,
            },
        ),
        Command::Theme {
            command: ThemeCommand::List { rendering },
        } => themes::listing(rendering),
        Command::Fonts { command } => library::run(command, environment),
        Command::CodeThemes { variant, rendering } => {
            code_themes::run(variant, rendering, environment)
        }
        Command::Ui { deck } => ui::run(deck.as_deref(), environment),
        Command::Palette {
            seed,
            mode,
            harmony,
            rendering,
        } => {
            let request = seed.map(|seed| palette::Request {
                seed,
                mode,
                harmony,
            });
            palette::run(request.as_ref(), rendering, environment)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DIAGNOSTIC_EXIT_CODE, INTERNAL_EXIT_CODE, SUCCESS_EXIT_CODE, exit_code, sink, timestamp,
    };
    use crate::args::Environment;
    use crate::build::Sink;
    use sivi_core::diagnostic::{Code, Diagnostic};
    use sivi_typst::UtcTimestamp;
    use std::path::{Path, PathBuf};

    fn diagnostic(code: Code) -> Diagnostic {
        Diagnostic::new(code, String::from("x"))
    }

    #[test]
    fn the_exit_code_follows_the_worst_diagnostic_and_the_warning_policy() {
        assert_eq!(exit_code(&[], false), SUCCESS_EXIT_CODE);
        assert_eq!(
            exit_code(&[diagnostic(Code::EmptySlide)], false),
            SUCCESS_EXIT_CODE
        );
        assert_eq!(
            exit_code(&[diagnostic(Code::EmptySlide)], true),
            DIAGNOSTIC_EXIT_CODE
        );
        assert_eq!(
            exit_code(&[diagnostic(Code::VerticalOverflow)], false),
            DIAGNOSTIC_EXIT_CODE
        );
        assert_eq!(
            exit_code(
                &[
                    diagnostic(Code::VerticalOverflow),
                    diagnostic(Code::MarkerMissing)
                ],
                false
            ),
            INTERNAL_EXIT_CODE
        );
        assert_eq!(
            exit_code(&[diagnostic(Code::Internal)], false),
            INTERNAL_EXIT_CODE
        );
    }

    #[test]
    fn the_timestamp_comes_from_the_epoch_variable_alone() {
        assert_eq!(timestamp(&Environment::default()), Ok(None));
        let fixed = Environment {
            source_date_epoch: Some(String::from("951782400")),
            ..Environment::default()
        };
        assert_eq!(
            timestamp(&fixed),
            Ok(Some(UtcTimestamp {
                year: 2000,
                month: 2,
                day: 29,
                hour: 0,
                minute: 0,
                second: 0,
            }))
        );
        let bad = Environment {
            source_date_epoch: Some(String::from("soon")),
            ..Environment::default()
        };
        assert!(timestamp(&bad).unwrap_err().contains("not a whole number"));
        let far = Environment {
            source_date_epoch: Some(String::from("-5")),
            ..Environment::default()
        };
        assert!(timestamp(&far).unwrap_err().contains("outside"));
    }

    #[test]
    fn the_sink_is_the_output_the_standard_stream_or_the_pdf_beside_the_deck() {
        let deck = Path::new("talks/deck.md");
        assert_eq!(
            sink(deck, None),
            Sink::File(PathBuf::from("talks/deck.pdf"))
        );
        assert_eq!(sink(deck, Some(Path::new("-"))), Sink::Stdout);
        assert_eq!(
            sink(deck, Some(Path::new("out/x.pdf"))),
            Sink::File(PathBuf::from("out/x.pdf"))
        );
    }
}
