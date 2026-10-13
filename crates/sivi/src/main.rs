//! The `sivi` binary.

use clap::Parser;
use sivi::args::{self, Arguments};
use sivi::cli::{self, USAGE_EXIT_CODE};
use sivi::output;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments = Arguments::parse();
    let run = cli::run(arguments, &args::environment());
    if !run.stderr.is_empty() {
        output::stderr(&run.stderr);
    }
    if !run.stdout.is_empty()
        && let Err(error) = output::stdout(&run.stdout)
    {
        output::stderr(&format!("error: cannot write standard output: {error}\n"));
        return ExitCode::from(USAGE_EXIT_CODE);
    }
    ExitCode::from(run.code)
}
