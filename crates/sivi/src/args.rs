//! The argument contract of the `sivi` binary, and the one place that reads its environment.

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::io::IsTerminal as _;
use std::path::PathBuf;

/// Compile Markdown decks to PDF through Typst and refuse any slide its master cannot hold.
#[derive(Clone, Debug, Parser)]
#[command(name = "sivi", version, about, long_about = None)]
pub struct Arguments {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    /// Compiles a deck to a PDF, or reports why it cannot.
    Build {
        /// The deck, a Markdown file.
        deck: PathBuf,
        /// Where to write the PDF; `-` writes it to standard output.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// The theme to use instead of the deck's `theme:`: a built-in name or a directory.
        #[arg(long)]
        theme: Option<String>,
        #[command(flatten)]
        rendering: Rendering,
    },
    /// Compiles a deck as `build` does and writes nothing.
    Check {
        /// The deck, a Markdown file.
        deck: PathBuf,
        /// The theme to use instead of the deck's `theme:`: a built-in name or a directory.
        #[arg(long)]
        theme: Option<String>,
        #[command(flatten)]
        rendering: Rendering,
    },
    /// Theme commands.
    Theme {
        #[command(subcommand)]
        command: ThemeCommand,
    },
}

#[derive(Clone, Debug, Subcommand)]
pub enum ThemeCommand {
    /// Validates a theme directory or a built-in theme by name.
    Check {
        /// A built-in theme name, or a directory holding `theme.toml`.
        theme: String,
        #[command(flatten)]
        rendering: Rendering,
    },
}

/// How diagnostics are written and judged, which every command shares.
#[derive(Args, Clone, Copy, Debug, Default)]
pub struct Rendering {
    /// How diagnostics are written.
    #[arg(long, value_enum, default_value_t = Format::Human)]
    pub format: Format,
    /// Exits with status 1 when a warning is reported.
    #[arg(long)]
    pub deny_warnings: bool,
    /// Whether diagnostics use color.
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto)]
    pub color: ColorChoice,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum Format {
    /// Diagnostics for a person, on standard error.
    #[default]
    Human,
    /// One JSON document on standard output.
    Json,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum ColorChoice {
    /// Color when standard error is a terminal and `NO_COLOR` is unset.
    #[default]
    Auto,
    Always,
    Never,
}

/// The variable that fixes the PDF creation date, in seconds since the Unix epoch.
pub const SOURCE_DATE_EPOCH: &str = "SOURCE_DATE_EPOCH";
/// The variable that disables color when set to anything but the empty string.
pub const NO_COLOR: &str = "NO_COLOR";

/// What the binary read from its environment, once, before any decision.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Environment {
    pub source_date_epoch: Option<String>,
    pub no_color: bool,
    pub stderr_is_terminal: bool,
}

/// Reads the environment the binary honors.
#[must_use]
pub fn environment() -> Environment {
    Environment {
        source_date_epoch: std::env::var_os(SOURCE_DATE_EPOCH)
            .map(|value| value.to_string_lossy().into_owned()),
        no_color: std::env::var_os(NO_COLOR).is_some_and(|value| !value.is_empty()),
        stderr_is_terminal: std::io::stderr().is_terminal(),
    }
}

impl ColorChoice {
    /// Whether diagnostics are styled under `environment`.
    #[must_use]
    pub const fn styled(self, environment: &Environment) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => environment.stderr_is_terminal && !environment.no_color,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Arguments, ColorChoice, Command, Environment, Format, ThemeCommand};
    use clap::{CommandFactory, Parser};

    #[test]
    fn the_argument_contract_is_well_formed() {
        Arguments::command().debug_assert();
        let parsed = Arguments::try_parse_from([
            "sivi", "build", "deck.md", "-o", "-", "--format", "json", "--theme", "plain",
        ])
        .unwrap();
        let Command::Build {
            deck,
            output,
            theme,
            rendering,
        } = parsed.command
        else {
            panic!("a build");
        };
        assert_eq!(deck.to_str(), Some("deck.md"));
        assert_eq!(output.as_deref().and_then(|path| path.to_str()), Some("-"));
        assert_eq!(theme.as_deref(), Some("plain"));
        assert_eq!(rendering.format, Format::Json);
        assert!(!rendering.deny_warnings);
        let checked = Arguments::try_parse_from(["sivi", "theme", "check", "themes/mine"]).unwrap();
        let Command::Theme {
            command: ThemeCommand::Check { theme: target, .. },
        } = checked.command
        else {
            panic!("a theme check");
        };
        assert_eq!(target, "themes/mine");
    }

    #[test]
    fn an_unknown_argument_is_a_usage_error() {
        let error = Arguments::try_parse_from(["sivi", "--unknown"]).unwrap_err();
        assert_eq!(error.exit_code(), i32::from(crate::cli::USAGE_EXIT_CODE));
    }

    #[test]
    fn color_follows_the_choice_then_the_terminal_and_no_color() {
        let terminal = Environment {
            source_date_epoch: None,
            no_color: false,
            stderr_is_terminal: true,
        };
        assert!(ColorChoice::Auto.styled(&terminal));
        assert!(!ColorChoice::Auto.styled(&Environment {
            no_color: true,
            ..terminal.clone()
        }));
        assert!(!ColorChoice::Auto.styled(&Environment::default()));
        assert!(ColorChoice::Always.styled(&Environment::default()));
        assert!(!ColorChoice::Never.styled(&terminal));
    }
}
