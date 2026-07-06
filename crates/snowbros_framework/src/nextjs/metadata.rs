//! Special-file classification and the Metadata API / route-handler
//! surface.

use camino::Utf8Path;

use super::components;
use super::{NextInput, RouteFile, SpecialFile};

/// Source extensions Next.js treats as route modules.
const ROUTE_EXTS: &[&str] = &["js", "jsx", "ts", "tsx", "mjs"];

/// HTTP methods a Route Handler may export.
const HTTP_METHODS: &[&str] = &["GET", "HEAD", "POST", "PUT", "DELETE", "PATCH", "OPTIONS"];

/// Classifies a file as a Next.js special file by its stem, or `None` if
/// it is an ordinary module (component, util, …).
pub fn special_file_kind(path: &Utf8Path) -> Option<SpecialFile> {
    let ext = path.extension()?;
    if !ROUTE_EXTS.contains(&ext) {
        return None;
    }
    match path.file_stem()? {
        "page" => Some(SpecialFile::Page),
        "layout" => Some(SpecialFile::Layout),
        "loading" => Some(SpecialFile::Loading),
        "error" => Some(SpecialFile::Error),
        "global-error" => Some(SpecialFile::GlobalError),
        "template" => Some(SpecialFile::Template),
        "not-found" => Some(SpecialFile::NotFound),
        "default" => Some(SpecialFile::Default),
        "route" => Some(SpecialFile::Route),
        _ => None,
    }
}

/// Builds a [`RouteFile`] for a classified special file, reading its
/// rendering and Metadata/handler surface from the snapshot.
pub fn route_file(path: &Utf8Path, kind: SpecialFile, input: &NextInput<'_>) -> RouteFile {
    let exports = input.file_exports.get(path);
    let has = |name: &str| exports.is_some_and(|e| e.contains(name));

    let http_methods = if kind == SpecialFile::Route {
        exports
            .map(|e| {
                let mut m: Vec<String> = HTTP_METHODS
                    .iter()
                    .filter(|verb| e.contains(**verb))
                    .map(|verb| verb.to_string())
                    .collect();
                m.sort();
                m
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    RouteFile {
        path: path.to_owned(),
        kind,
        rendering: components::classify(path, input),
        has_generate_metadata: has("generateMetadata"),
        has_generate_static_params: has("generateStaticParams"),
        has_metadata_export: has("metadata"),
        http_methods,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;

    #[test]
    fn classifies_special_files() {
        assert_eq!(
            special_file_kind(Utf8Path::new("app/page.tsx")),
            Some(SpecialFile::Page)
        );
        assert_eq!(
            special_file_kind(Utf8Path::new("app/not-found.jsx")),
            Some(SpecialFile::NotFound)
        );
        assert_eq!(
            special_file_kind(Utf8Path::new("app/api/route.ts")),
            Some(SpecialFile::Route)
        );
        // Ordinary modules are not special files.
        assert_eq!(special_file_kind(Utf8Path::new("app/button.tsx")), None);
        // Non-route extensions are ignored.
        assert_eq!(special_file_kind(Utf8Path::new("app/page.css")), None);
    }

    #[test]
    fn route_handler_methods_sorted() {
        use std::collections::{BTreeMap, BTreeSet};
        let files = vec![Utf8PathBuf::from("app/api/route.ts")];
        let client = BTreeSet::new();
        let mut exports = BTreeMap::new();
        exports.insert(
            Utf8PathBuf::from("app/api/route.ts"),
            ["POST", "GET", "notAMethod"]
                .iter()
                .map(|s| s.to_string())
                .collect::<BTreeSet<_>>(),
        );
        let input = NextInput {
            files: &files,
            client_files: &client,
            file_exports: &exports,
        };
        let rf = route_file(
            Utf8Path::new("app/api/route.ts"),
            SpecialFile::Route,
            &input,
        );
        assert_eq!(rf.http_methods, vec!["GET", "POST"]);
    }
}
