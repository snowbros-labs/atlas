//! A snapshot of the project's files for resolution lookups.

use std::collections::BTreeSet;

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

/// Immutable set of root-relative file paths (forward-slashed).
///
/// `BTreeSet` keeps iteration deterministic.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSet {
    paths: BTreeSet<Utf8PathBuf>,
}

impl FileSet {
    /// Whether the exact path exists in the set.
    pub fn contains(&self, path: &Utf8Path) -> bool {
        self.paths.contains(path)
    }

    /// Number of files.
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Iterates paths in sorted order.
    pub fn iter(&self) -> impl Iterator<Item = &Utf8PathBuf> {
        self.paths.iter()
    }
}

impl FromIterator<Utf8PathBuf> for FileSet {
    fn from_iter<T: IntoIterator<Item = Utf8PathBuf>>(iter: T) -> Self {
        Self {
            paths: iter.into_iter().collect(),
        }
    }
}
