//! `snowbros init` — write a starter configuration file.

use std::fs;
use std::path::Path;

use owo_colors::OwoColorize;
use snowbros_core::Config;

/// Writes `snowbros.toml` into the current directory.
///
/// Refuses to overwrite an existing file unless `force` is set.
pub fn run(force: bool) -> Result<(), String> {
    let path = Path::new(Config::FILE_NAME);
    if path.exists() && !force {
        return Err(format!(
            "{} already exists (use --force to overwrite)",
            Config::FILE_NAME
        ));
    }
    fs::write(path, Config::starter_template())
        .map_err(|e| format!("could not write {}: {e}", Config::FILE_NAME))?;
    println!(
        "{} created {}",
        "✓".green().bold(),
        Config::FILE_NAME.bold()
    );
    println!("  next: run `snowbros analyze` (coming in Sprint 4)");
    Ok(())
}
