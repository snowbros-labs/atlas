//! `sb` binary entry point — short alias for `snowbros`.

use std::process::ExitCode;

fn main() -> ExitCode {
    snowbros_cli::run()
}
