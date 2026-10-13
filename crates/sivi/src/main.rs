//! The `sivi` binary.

use clap::Parser;
use sivi::cli::Arguments;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Arguments {} = Arguments::parse();
    ExitCode::SUCCESS
}
