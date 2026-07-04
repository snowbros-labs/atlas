//! Error types for core operations.

use std::io;

/// Errors produced by core operations (configuration loading, etc.).
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// Reading a file from disk failed.
    #[error("failed to read `{path}`: {source}")]
    Io {
        /// Path that failed to read.
        path: String,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },

    /// A TOML configuration file could not be parsed.
    #[error("invalid configuration in `{path}`: {source}")]
    ConfigParse {
        /// Path of the offending config file.
        path: String,
        /// Underlying TOML error.
        #[source]
        source: toml::de::Error,
    },
}
