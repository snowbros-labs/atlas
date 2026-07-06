//! Next.js structural rules, read from the [`NextProjectModel`].
//!
//! M0 proof rules for the Next.js project model: they reason purely over
//! the structured route model the engine builds — no re-scanning, no
//! textual matching — so they are deterministic by construction.
//!
//! [`NextProjectModel`]: snowbros_framework::nextjs::NextProjectModel

use snowbros_core::{Confidence, Diagnostic, Evidence, Severity};
use snowbros_framework::nextjs::{Rendering, RouterKind, SpecialFile};

use crate::context::AnalysisContext;
use crate::registry::Rule;
use crate::rules::file_location;

/// `next/mixed-router` — the project uses both the App Router and the
/// Pages Router. Usually a half-finished migration; the routers have
/// different data-fetching and rendering models and are easy to confuse.
pub struct MixedRouter;

impl Rule for MixedRouter {
    fn id(&self) -> &'static str {
        "next/mixed-router"
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        let Some(model) = ctx.next_model else {
            return Vec::new();
        };
        if model.router != RouterKind::Mixed {
            return Vec::new();
        }
        // Anchor at the first Pages-Router file (the migration target).
        // Mixed guarantees at least one exists; fall back defensively.
        let location = match model.pages_routes.first() {
            Some(route) => file_location(route.path.clone()),
            None => file_location("pages"),
        };
        vec![Diagnostic::new(
            self.id(),
            "Mixed App and Pages routers",
            "This project uses both the App Router (`app/`) and the Pages \
             Router (`pages/`). Mixing routers is supported during migration \
             but doubles the routing surface and its rendering rules — finish \
             the migration to a single router.",
            "nextjs",
            Severity::Low,
            Confidence::Certain,
            location,
        )
        .with_evidence(Evidence::note(format!(
            "{} App-Router route(s) and {} Pages-Router file(s) coexist",
            model.app_routes.len(),
            model.pages_routes.len()
        )))]
    }
}

/// `next/client-metadata-ignored` — a `page`/`layout` that is a Client
/// Component (`"use client"`) yet exports the Metadata API. Next.js
/// **ignores** `metadata` / `generateMetadata` in Client Components, so the
/// SEO tags silently never render.
pub struct ClientMetadataIgnored;

impl Rule for ClientMetadataIgnored {
    fn id(&self) -> &'static str {
        "next/client-metadata-ignored"
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        let Some(model) = ctx.next_model else {
            return Vec::new();
        };
        let mut diagnostics = Vec::new();
        for route in &model.app_routes {
            for file in &route.files {
                let metadata_bearing = file.has_metadata_export || file.has_generate_metadata;
                let ui_file = matches!(file.kind, SpecialFile::Page | SpecialFile::Layout);
                if file.rendering == Rendering::Client && metadata_bearing && ui_file {
                    let which = if file.has_generate_metadata {
                        "generateMetadata"
                    } else {
                        "metadata"
                    };
                    diagnostics.push(
                        Diagnostic::new(
                            self.id(),
                            "Metadata ignored in Client Component",
                            format!(
                                "`{}` is a Client Component (\"use client\") but exports \
                                 `{which}`. Next.js ignores the Metadata API in Client \
                                 Components — move it to a Server Component.",
                                file.path
                            ),
                            "nextjs",
                            Severity::Medium,
                            Confidence::Certain,
                            file_location(file.path.clone()),
                        )
                        .with_evidence(Evidence::note(format!(
                            "`{}` carries \"use client\" and exports `{which}`",
                            file.path
                        ))),
                    );
                }
            }
        }
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ContextInputs;
    use snowbros_framework::nextjs::{self, NextInput};
    use snowbros_graph::SemanticGraph;
    use std::collections::{BTreeMap, BTreeSet};

    use camino::Utf8PathBuf;

    fn paths(list: &[&str]) -> Vec<Utf8PathBuf> {
        let mut v: Vec<Utf8PathBuf> = list.iter().map(Utf8PathBuf::from).collect();
        v.sort();
        v
    }

    fn run_rule<R: Rule>(
        rule: R,
        files: &[Utf8PathBuf],
        client: &BTreeSet<Utf8PathBuf>,
        exports: &BTreeMap<Utf8PathBuf, BTreeSet<String>>,
    ) -> Vec<Diagnostic> {
        let model = nextjs::build(NextInput {
            files,
            client_files: client,
            file_exports: exports,
        });
        let g = SemanticGraph::new();
        let ctx = AnalysisContext::new(
            &g,
            BTreeMap::new(),
            ContextInputs {
                next_model: model.as_ref(),
                ..ContextInputs::default()
            },
        );
        rule.run(&ctx)
    }

    #[test]
    fn mixed_router_flagged() {
        let files = paths(&["app/page.tsx", "pages/about.tsx"]);
        let diags = run_rule(MixedRouter, &files, &BTreeSet::new(), &BTreeMap::new());
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("both"));
    }

    #[test]
    fn single_router_not_flagged() {
        let files = paths(&["app/page.tsx"]);
        let diags = run_rule(MixedRouter, &files, &BTreeSet::new(), &BTreeMap::new());
        assert!(diags.is_empty());
    }

    #[test]
    fn client_metadata_flagged() {
        let files = paths(&["app/page.tsx"]);
        let mut client = BTreeSet::new();
        client.insert(Utf8PathBuf::from("app/page.tsx"));
        let mut exports = BTreeMap::new();
        exports.insert(
            Utf8PathBuf::from("app/page.tsx"),
            ["default", "metadata"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        );
        let diags = run_rule(ClientMetadataIgnored, &files, &client, &exports);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("Metadata API"));
    }

    #[test]
    fn server_metadata_not_flagged() {
        let files = paths(&["app/page.tsx"]);
        let mut exports = BTreeMap::new();
        exports.insert(
            Utf8PathBuf::from("app/page.tsx"),
            ["default", "metadata"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        );
        // No "use client" → Server Component → metadata honored.
        let diags = run_rule(ClientMetadataIgnored, &files, &BTreeSet::new(), &exports);
        assert!(diags.is_empty());
    }
}
