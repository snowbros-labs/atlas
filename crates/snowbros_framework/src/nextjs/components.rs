//! Server vs client rendering classification.

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use super::NextInput;

/// How a module renders in the Next.js App Router.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rendering {
    /// A Server Component — the App-Router default.
    Server,
    /// A Client Component — carries a `"use client"` directive.
    Client,
}

/// Classifies a file's rendering.
///
/// A file is [`Rendering::Client`] when it declares `"use client"`;
/// otherwise it is a Server Component (the App-Router default). This is a
/// **direct** classification. Transitive client propagation — a Server
/// Component that imports a Client Component is not itself a boundary, but
/// a module imported *by* a client tree becomes client — is left to the
/// import-graph rules (the existing `server-only-in-client` BFS already
/// proves that path); it is deliberately not guessed at here.
pub fn classify(path: &Utf8Path, input: &NextInput<'_>) -> Rendering {
    if input.client_files.contains(path) {
        Rendering::Client
    } else {
        Rendering::Server
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn client_directive_makes_client_else_server() {
        let files = vec![
            Utf8PathBuf::from("app/page.tsx"),
            Utf8PathBuf::from("app/counter.tsx"),
        ];
        let mut client = BTreeSet::new();
        client.insert(Utf8PathBuf::from("app/counter.tsx"));
        let exports = BTreeMap::new();
        let input = NextInput {
            files: &files,
            client_files: &client,
            file_exports: &exports,
        };
        assert_eq!(
            classify(Utf8Path::new("app/counter.tsx"), &input),
            Rendering::Client
        );
        assert_eq!(
            classify(Utf8Path::new("app/page.tsx"), &input),
            Rendering::Server
        );
    }
}
