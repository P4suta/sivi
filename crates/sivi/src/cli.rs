//! The argument contract of the `sivi` binary.

use clap::Parser;

/// Compile Markdown decks to PDF through Typst and refuse any slide its master cannot hold.
#[derive(Clone, Copy, Debug, Parser)]
#[command(name = "sivi", version, about, long_about = None)]
pub struct Arguments {}

/// The exit status of a usage error, which `clap` also reports.
pub const USAGE_EXIT_CODE: u8 = 2;

#[cfg(test)]
mod tests {
    use super::Arguments;
    use clap::{CommandFactory, Parser};

    #[test]
    fn the_argument_contract_is_well_formed() {
        Arguments::command().debug_assert();
        let Arguments {} = Arguments::try_parse_from(["sivi"]).unwrap();
    }

    #[test]
    fn an_unknown_argument_is_a_usage_error() {
        let error = Arguments::try_parse_from(["sivi", "--unknown"]).unwrap_err();
        assert_eq!(error.exit_code(), i32::from(super::USAGE_EXIT_CODE));
    }
}
