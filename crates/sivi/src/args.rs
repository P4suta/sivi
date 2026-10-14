//! The argument contract of the `sivi` binary, and the one place that reads its environment.

use clap::{Args, Parser, Subcommand, ValueEnum};
use sivi_core::palette_rules::{Harmony, Mode};
use sivi_core::syntax_theme::Variant;
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
    /// Font commands: the catalog of Google Fonts families and the library they install into.
    Fonts {
        #[command(subcommand)]
        command: FontsCommand,
    },
    /// Lists the code schemes the binary carries, which a theme names under `[code] theme`.
    CodeThemes {
        /// Only dark or only light schemes.
        #[arg(long, value_enum)]
        variant: Option<VariantChoice>,
        #[command(flatten)]
        rendering: Rendering,
    },
    /// Grows a palette from a seed color or a carried palette name, as theme text; without a seed, lists the carried palettes.
    Palette {
        /// A color as `#rrggbb`, or the name of a carried palette.
        seed: Option<String>,
        /// Light or dark paper.
        #[arg(long, value_enum)]
        mode: Option<ModeChoice>,
        /// How the accents relate to the seed on the hue circle.
        #[arg(long, value_enum)]
        harmony: Option<HarmonyChoice>,
        #[command(flatten)]
        rendering: Rendering,
    },
}

/// Which paper a palette sits on.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ModeChoice {
    Light,
    Dark,
}

impl ModeChoice {
    /// The mode of the core's palettes.
    #[must_use]
    pub const fn mode(self) -> Mode {
        match self {
            Self::Light => Mode::Light,
            Self::Dark => Mode::Dark,
        }
    }
}

/// How the accents of a palette relate to its seed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum HarmonyChoice {
    Mono,
    Analogous,
    Complementary,
    Split,
    Triadic,
}

impl HarmonyChoice {
    /// The harmony of the core's palettes.
    #[must_use]
    pub const fn harmony(self) -> Harmony {
        match self {
            Self::Mono => Harmony::Mono,
            Self::Analogous => Harmony::Analogous,
            Self::Complementary => Harmony::Complementary,
            Self::Split => Harmony::Split,
            Self::Triadic => Harmony::Triadic,
        }
    }
}

/// Which schemes `code-themes` lists.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum VariantChoice {
    Dark,
    Light,
}

impl VariantChoice {
    /// The variant of the core's schemes.
    #[must_use]
    pub const fn variant(self) -> Variant {
        match self {
            Self::Dark => Variant::Dark,
            Self::Light => Variant::Light,
        }
    }
}

#[derive(Clone, Debug, Subcommand)]
pub enum FontsCommand {
    /// Lists the families of the catalog with their state: bundled, installed, or available.
    List {
        /// Only families covering this writing system, such as `japanese`.
        #[arg(long)]
        script: Option<String>,
        /// Only bundled and installed families.
        #[arg(long)]
        installed: bool,
        #[command(flatten)]
        rendering: Rendering,
    },
    /// Installs families from Google Fonts into the library, checking every digest the catalog pins.
    Install {
        /// The families, as `sivi fonts list` names them.
        #[arg(required = true)]
        families: Vec<String>,
        #[command(flatten)]
        rendering: Rendering,
    },
    /// Prints the directory installed fonts live in.
    Dir,
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
/// The variable that names the data directory outright.
pub const SIVI_HOME: &str = "SIVI_HOME";
/// The data base of Unix desktops.
pub const XDG_DATA_HOME: &str = "XDG_DATA_HOME";
/// The home directory, under which `.local/share` is the conventional data base.
pub const HOME: &str = "HOME";
/// The per-user data base of Windows.
pub const LOCALAPPDATA: &str = "LOCALAPPDATA";

/// What the binary read from its environment, once, before any decision.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Environment {
    pub source_date_epoch: Option<String>,
    pub no_color: bool,
    pub stderr_is_terminal: bool,
    pub sivi_home: Option<String>,
    pub xdg_data_home: Option<String>,
    pub home: Option<String>,
    pub local_app_data: Option<String>,
    /// Whether the host is Windows, where `LOCALAPPDATA` is the data base.
    pub windows: bool,
}

/// The value of a variable as text, when it is set.
fn variable(name: &str) -> Option<String> {
    std::env::var_os(name).map(|value| value.to_string_lossy().into_owned())
}

/// Reads the environment the binary honors.
#[must_use]
pub fn environment() -> Environment {
    Environment {
        source_date_epoch: variable(SOURCE_DATE_EPOCH),
        no_color: std::env::var_os(NO_COLOR).is_some_and(|value| !value.is_empty()),
        stderr_is_terminal: std::io::stderr().is_terminal(),
        sivi_home: variable(SIVI_HOME),
        xdg_data_home: variable(XDG_DATA_HOME),
        home: variable(HOME),
        local_app_data: variable(LOCALAPPDATA),
        windows: cfg!(windows),
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
    use super::{Arguments, ColorChoice, Command, Environment, FontsCommand, Format, ThemeCommand};
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
        let fonts =
            Arguments::try_parse_from(["sivi", "fonts", "install", "Noto Sans JP", "Inter"])
                .unwrap();
        let Command::Fonts {
            command: FontsCommand::Install { families, .. },
        } = fonts.command
        else {
            panic!("a font installation");
        };
        assert_eq!(families, vec!["Noto Sans JP", "Inter"]);
        Arguments::try_parse_from(["sivi", "fonts", "install"]).unwrap_err();
        let listing =
            Arguments::try_parse_from(["sivi", "fonts", "list", "--script", "japanese"]).unwrap();
        let Command::Fonts {
            command: FontsCommand::List { script, .. },
        } = listing.command
        else {
            panic!("a font listing");
        };
        assert_eq!(script.as_deref(), Some("japanese"));
    }

    #[test]
    fn an_unknown_argument_is_a_usage_error() {
        let error = Arguments::try_parse_from(["sivi", "--unknown"]).unwrap_err();
        assert_eq!(error.exit_code(), i32::from(crate::cli::USAGE_EXIT_CODE));
    }

    #[test]
    fn color_follows_the_choice_then_the_terminal_and_no_color() {
        let terminal = Environment {
            stderr_is_terminal: true,
            ..Environment::default()
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
