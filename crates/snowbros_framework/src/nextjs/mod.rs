//! Next.js project intelligence.
//!
//! Turns a project's file inventory into a structured [`NextProjectModel`]
//! — router kind, the App-Router route tree (with every dynamic / catch-all
//! / route-group / parallel / intercepting segment classified), special
//! files, route handlers, the Metadata API surface, and per-file
//! server/client rendering.
//!
//! Like the rest of this crate, the model is a **pure function over a
//! deterministic snapshot** ([`NextInput`]) — no filesystem access here —
//! so it is reproducible and unit-testable without a real project on disk.
//! The engine builds the snapshot from the scanner + lowered IR and calls
//! [`build`].
//!
//! Submodules:
//! - [`detector`] — router kind, `src/` layout, middleware;
//! - [`routes`] — segment parsing and route-tree assembly;
//! - [`metadata`] — special-file classification and the Metadata API /
//!   route-handler surface;
//! - [`components`] — server vs client rendering.

pub mod components;
pub mod detector;
pub mod metadata;
pub mod routes;

use std::collections::{BTreeMap, BTreeSet};

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

pub use components::Rendering;

/// The deterministic snapshot the model is built from.
///
/// Every field is project-relative and sorted by the caller; the builder
/// never touches disk.
#[derive(Debug, Clone, Copy)]
pub struct NextInput<'a> {
    /// All project-relative source file paths.
    pub files: &'a [Utf8PathBuf],
    /// Files carrying a top-of-file `"use client"` directive.
    pub client_files: &'a BTreeSet<Utf8PathBuf>,
    /// Exported names per file — the Metadata API and route-handler
    /// surface is read from these.
    pub file_exports: &'a BTreeMap<Utf8PathBuf, BTreeSet<String>>,
}

/// Which Next.js router(s) a project uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouterKind {
    /// App Router only (`app/`).
    App,
    /// Pages Router only (`pages/`).
    Pages,
    /// Both routers present — a migration in progress.
    Mixed,
}

/// One classified path segment of an App-Router route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    /// The directory name as written, e.g. `[slug]` or `(marketing)`.
    pub raw: String,
    /// What kind of segment it is.
    pub kind: SegmentKind,
    /// The meaningful name: the parameter for dynamic segments, the group
    /// name for route groups, the slot for parallel routes, otherwise the
    /// literal text.
    pub name: String,
}

/// The App-Router segment taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentKind {
    /// A literal path segment, e.g. `dashboard`.
    Static,
    /// A dynamic segment, `[id]`.
    Dynamic,
    /// A catch-all segment, `[...slug]`.
    CatchAll,
    /// An optional catch-all segment, `[[...slug]]`.
    OptionalCatchAll,
    /// A route group, `(marketing)` — organizational, absent from the URL.
    RouteGroup,
    /// A parallel-route slot, `@team`.
    ParallelSlot,
    /// An intercepting-route marker, `(.)`, `(..)`, `(..)(..)`, `(...)`.
    Intercepting,
}

/// A Next.js special file recognized by the framework.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpecialFile {
    /// `page` — a route's UI, makes the segment publicly routable.
    Page,
    /// `layout` — shared shell wrapping a segment and its children.
    Layout,
    /// `loading` — Suspense fallback for the segment.
    Loading,
    /// `error` — error boundary for the segment (always a Client Component).
    Error,
    /// `global-error` — root error boundary.
    GlobalError,
    /// `template` — like layout but re-mounted per navigation.
    Template,
    /// `not-found` — 404 UI.
    NotFound,
    /// `default` — parallel-route fallback.
    Default,
    /// `route` — a Route Handler (HTTP API endpoint).
    Route,
}

/// A special file within an App-Router directory, with its analysis flags.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteFile {
    /// Project-relative path.
    pub path: Utf8PathBuf,
    /// Which special file it is.
    pub kind: SpecialFile,
    /// How it renders (Client when it carries `"use client"`).
    pub rendering: Rendering,
    /// Exports `generateMetadata`.
    pub has_generate_metadata: bool,
    /// Exports `generateStaticParams`.
    pub has_generate_static_params: bool,
    /// Exports a static `metadata` object.
    pub has_metadata_export: bool,
    /// For `route` handlers: the HTTP methods exported (`GET`, `POST`, …),
    /// sorted. Empty for non-handlers.
    pub http_methods: Vec<String>,
}

/// An App-Router directory that contains at least one special file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppRoute {
    /// The directory, project-relative (e.g. `app/(marketing)/blog/[slug]`).
    pub dir: Utf8PathBuf,
    /// Its path segments below the app root, classified.
    pub segments: Vec<Segment>,
    /// Special files in the directory, sorted by path.
    pub files: Vec<RouteFile>,
}

/// A Pages-Router file. The Pages Router is modeled coarsely — its route
/// intelligence is deliberately lighter than the App Router's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PagesRoute {
    /// Project-relative path.
    pub path: Utf8PathBuf,
    /// Whether it lives under `pages/api/` (an API route).
    pub is_api: bool,
    /// Whether any segment is dynamic (`[id]` / `[...slug]`).
    pub is_dynamic: bool,
}

/// The structured Next.js model for a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NextProjectModel {
    /// Which router(s) the project uses.
    pub router: RouterKind,
    /// Whether routes live under `src/` (`src/app`, `src/pages`).
    pub uses_src_dir: bool,
    /// The middleware file, if present.
    pub middleware: Option<Utf8PathBuf>,
    /// App-Router routes, sorted by directory.
    pub app_routes: Vec<AppRoute>,
    /// Pages-Router files, sorted by path.
    pub pages_routes: Vec<PagesRoute>,
}

/// Builds the Next.js model from a project snapshot.
///
/// Returns `None` when the project has neither an App nor a Pages router
/// directory — it is not a routed Next.js app.
pub fn build(input: NextInput<'_>) -> Option<NextProjectModel> {
    let app_base = detector::app_base(input.files);
    let pages_base = detector::pages_base(input.files);
    let router = detector::router_kind(app_base.as_deref(), pages_base.as_deref())?;

    let uses_src_dir = app_base
        .as_ref()
        .or(pages_base.as_ref())
        .map(|b| b.starts_with("src"))
        .unwrap_or(false);

    let app_routes = app_base
        .as_ref()
        .map(|base| routes::build_app_routes(base, &input))
        .unwrap_or_default();
    let pages_routes = pages_base
        .as_ref()
        .map(|base| routes::build_pages_routes(base, &input))
        .unwrap_or_default();

    Some(NextProjectModel {
        router,
        uses_src_dir,
        middleware: detector::middleware(input.files),
        app_routes,
        pages_routes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input<'a>(
        files: &'a [Utf8PathBuf],
        client: &'a BTreeSet<Utf8PathBuf>,
        exports: &'a BTreeMap<Utf8PathBuf, BTreeSet<String>>,
    ) -> NextInput<'a> {
        NextInput {
            files,
            client_files: client,
            file_exports: exports,
        }
    }

    fn paths(list: &[&str]) -> Vec<Utf8PathBuf> {
        let mut v: Vec<Utf8PathBuf> = list.iter().map(Utf8PathBuf::from).collect();
        v.sort();
        v
    }

    #[test]
    fn non_next_project_is_none() {
        let files = paths(&["src/index.ts", "src/util.ts"]);
        let client = BTreeSet::new();
        let exports = BTreeMap::new();
        assert!(build(input(&files, &client, &exports)).is_none());
    }

    #[test]
    fn app_router_detected_with_segments() {
        let files = paths(&[
            "app/layout.tsx",
            "app/page.tsx",
            "app/(marketing)/blog/[slug]/page.tsx",
            "app/dashboard/[...rest]/page.tsx",
            "middleware.ts",
        ]);
        let client = BTreeSet::new();
        let exports = BTreeMap::new();
        let model = build(input(&files, &client, &exports)).unwrap();

        assert_eq!(model.router, RouterKind::App);
        assert!(!model.uses_src_dir);
        assert_eq!(
            model.middleware.as_deref(),
            Some(Utf8PathBuf::from("middleware.ts").as_path())
        );

        // Route for app/(marketing)/blog/[slug].
        let blog = model
            .app_routes
            .iter()
            .find(|r| r.dir == "app/(marketing)/blog/[slug]")
            .unwrap();
        let kinds: Vec<SegmentKind> = blog.segments.iter().map(|s| s.kind).collect();
        assert_eq!(
            kinds,
            vec![
                SegmentKind::RouteGroup,
                SegmentKind::Static,
                SegmentKind::Dynamic
            ]
        );

        let rest = model
            .app_routes
            .iter()
            .find(|r| r.dir == "app/dashboard/[...rest]")
            .unwrap();
        assert_eq!(rest.segments.last().unwrap().kind, SegmentKind::CatchAll);
    }

    #[test]
    fn mixed_router_detected() {
        let files = paths(&["app/page.tsx", "pages/about.tsx"]);
        let client = BTreeSet::new();
        let exports = BTreeMap::new();
        let model = build(input(&files, &client, &exports)).unwrap();
        assert_eq!(model.router, RouterKind::Mixed);
    }

    #[test]
    fn src_dir_layout_flagged() {
        let files = paths(&["src/app/page.tsx"]);
        let client = BTreeSet::new();
        let exports = BTreeMap::new();
        let model = build(input(&files, &client, &exports)).unwrap();
        assert!(model.uses_src_dir);
        assert_eq!(model.router, RouterKind::App);
    }

    #[test]
    fn metadata_and_client_flags_populated() {
        let files = paths(&["app/page.tsx", "app/counter.tsx", "app/api/users/route.ts"]);
        let mut client = BTreeSet::new();
        client.insert(Utf8PathBuf::from("app/counter.tsx"));
        let mut exports = BTreeMap::new();
        exports.insert(
            Utf8PathBuf::from("app/page.tsx"),
            ["default", "generateMetadata", "generateStaticParams"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        );
        exports.insert(
            Utf8PathBuf::from("app/api/users/route.ts"),
            ["GET", "POST"].iter().map(|s| s.to_string()).collect(),
        );
        let model = build(input(&files, &client, &exports)).unwrap();

        let page = model
            .app_routes
            .iter()
            .flat_map(|r| &r.files)
            .find(|f| f.kind == SpecialFile::Page)
            .unwrap();
        assert!(page.has_generate_metadata);
        assert!(page.has_generate_static_params);
        assert_eq!(page.rendering, Rendering::Server);

        let handler = model
            .app_routes
            .iter()
            .flat_map(|r| &r.files)
            .find(|f| f.kind == SpecialFile::Route)
            .unwrap();
        assert_eq!(handler.http_methods, vec!["GET", "POST"]);
    }

    #[test]
    fn pages_router_api_and_dynamic() {
        let files = paths(&[
            "pages/index.tsx",
            "pages/blog/[slug].tsx",
            "pages/api/hello.ts",
        ]);
        let client = BTreeSet::new();
        let exports = BTreeMap::new();
        let model = build(input(&files, &client, &exports)).unwrap();
        assert_eq!(model.router, RouterKind::Pages);
        let api = model.pages_routes.iter().find(|r| r.is_api).unwrap();
        assert_eq!(api.path, "pages/api/hello.ts");
        let dynamic = model.pages_routes.iter().find(|r| r.is_dynamic).unwrap();
        assert_eq!(dynamic.path, "pages/blog/[slug].tsx");
    }

    #[test]
    fn model_is_deterministic() {
        let files = paths(&[
            "app/page.tsx",
            "app/blog/[slug]/page.tsx",
            "app/(shop)/cart/page.tsx",
        ]);
        let client = BTreeSet::new();
        let exports = BTreeMap::new();
        let a = serde_json::to_string(&build(input(&files, &client, &exports)).unwrap()).unwrap();
        let b = serde_json::to_string(&build(input(&files, &client, &exports)).unwrap()).unwrap();
        assert_eq!(a, b);
    }
}
