//! `snowbros` binary entry point.

use std::process::ExitCode;

fn main() -> ExitCode {
    snowbros_cli::run()
}
