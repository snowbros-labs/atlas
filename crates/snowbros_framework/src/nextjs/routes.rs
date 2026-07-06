//! Route-tree assembly and segment parsing.

use std::collections::BTreeMap;

use camino::{Utf8Path, Utf8PathBuf};

use super::metadata::{route_file, special_file_kind};
use super::{AppRoute, NextInput, PagesRoute, RouteFile, Segment, SegmentKind};

/// Builds the App-Router route tree: every directory that holds at least
/// one special file becomes an [`AppRoute`] with its segments classified.
/// Sorted by directory (the `BTreeMap` key order).
pub fn build_app_routes(base: &Utf8Path, input: &NextInput<'_>) -> Vec<AppRoute> {
    let mut by_dir: BTreeMap<Utf8PathBuf, Vec<RouteFile>> = BTreeMap::new();

    for file in input.files {
        if !file.starts_with(base) || file == base {
            continue;
        }
        let Some(kind) = special_file_kind(file) else {
            continue;
        };
        let dir = file.parent().map(Utf8Path::to_owned).unwrap_or_default();
        by_dir
            .entry(dir)
            .or_default()
            .push(route_file(file, kind, input));
    }

    by_dir
        .into_iter()
        .map(|(dir, mut files)| {
            files.sort_by(|a, b| a.path.cmp(&b.path));
            let segments = parse_segments(base, &dir);
            AppRoute {
                dir,
                segments,
                files,
            }
        })
        .collect()
}

/// Parses the segments of an App-Router directory below the app root.
fn parse_segments(base: &Utf8Path, dir: &Utf8Path) -> Vec<Segment> {
    dir.strip_prefix(base)
        .map(|rest| {
            rest.components()
                .map(|c| parse_segment(c.as_str()))
                .collect()
        })
        .unwrap_or_default()
}

/// Classifies one directory-name segment. Order of checks matters:
/// intercepting before route group (both parenthesized), optional
/// catch-all before catch-all before plain dynamic.
pub fn parse_segment(raw: &str) -> Segment {
    let kind;
    let name;

    if let Some(inner) = raw.strip_prefix("[[...").and_then(|s| s.strip_suffix("]]")) {
        kind = SegmentKind::OptionalCatchAll;
        name = inner.to_string();
    } else if let Some(inner) = raw.strip_prefix("[...").and_then(|s| s.strip_suffix(']')) {
        kind = SegmentKind::CatchAll;
        name = inner.to_string();
    } else if let Some(inner) = raw.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        kind = SegmentKind::Dynamic;
        name = inner.to_string();
    } else if let Some(rest) = intercepting_rest(raw) {
        kind = SegmentKind::Intercepting;
        name = rest.to_string();
    } else if let Some(inner) = raw.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        kind = SegmentKind::RouteGroup;
        name = inner.to_string();
    } else if let Some(slot) = raw.strip_prefix('@') {
        kind = SegmentKind::ParallelSlot;
        name = slot.to_string();
    } else {
        kind = SegmentKind::Static;
        name = raw.to_string();
    }

    Segment {
        raw: raw.to_string(),
        kind,
        name,
    }
}

/// If `raw` begins with an intercepting marker (`(.)`, `(..)`, `(...)`,
/// or a chain like `(..)(..)`), returns the intercepted segment name that
/// follows the marker(s).
fn intercepting_rest(raw: &str) -> Option<&str> {
    let mut rest = raw;
    let mut matched = false;
    for marker in ["(...)", "(..)", "(.)"] {
        while let Some(stripped) = rest.strip_prefix(marker) {
            rest = stripped;
            matched = true;
        }
    }
    if matched {
        Some(rest)
    } else {
        None
    }
}

/// Builds the (coarse) Pages-Router model: every route module under the
/// pages base, flagged for API location and dynamic segments.
pub fn build_pages_routes(base: &Utf8Path, input: &NextInput<'_>) -> Vec<PagesRoute> {
    let api_dir = base.join("api");
    let mut routes: Vec<PagesRoute> = input
        .files
        .iter()
        .filter(|f| f.starts_with(base) && *f != base && special_file_kind(f).is_none())
        .filter(|f| is_page_module(f))
        .map(|f| PagesRoute {
            path: f.clone(),
            is_api: f.starts_with(&api_dir),
            is_dynamic: pages_is_dynamic(f),
        })
        .collect();
    routes.sort_by(|a, b| a.path.cmp(&b.path));
    routes
}

/// Whether a segment kind carries a URL parameter.
fn is_dynamic(kind: SegmentKind) -> bool {
    matches!(
        kind,
        SegmentKind::Dynamic | SegmentKind::CatchAll | SegmentKind::OptionalCatchAll
    )
}

/// Whether a Pages-Router file is dynamic — a `[param]` marker in any
/// parent directory or in the filename stem (`pages/blog/[slug].tsx`).
fn pages_is_dynamic(path: &Utf8Path) -> bool {
    let dir_dynamic = path
        .parent()
        .into_iter()
        .flat_map(|p| p.components())
        .any(|c| is_dynamic(parse_segment(c.as_str()).kind));
    let file_dynamic = path
        .file_stem()
        .is_some_and(|s| is_dynamic(parse_segment(s).kind));
    dir_dynamic || file_dynamic
}

/// Whether a Pages-Router file is a route module (`.js`/`.ts`/… with a
/// route extension). Files in `pages/` are routes by filename, unlike the
/// App Router's special-file convention.
fn is_page_module(path: &Utf8Path) -> bool {
    matches!(path.extension(), Some("js" | "jsx" | "ts" | "tsx" | "mjs"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(raw: &str) -> (SegmentKind, String) {
        let s = parse_segment(raw);
        (s.kind, s.name)
    }

    #[test]
    fn parses_the_full_segment_taxonomy() {
        assert_eq!(seg("dashboard"), (SegmentKind::Static, "dashboard".into()));
        assert_eq!(seg("[id]"), (SegmentKind::Dynamic, "id".into()));
        assert_eq!(seg("[...slug]"), (SegmentKind::CatchAll, "slug".into()));
        assert_eq!(
            seg("[[...slug]]"),
            (SegmentKind::OptionalCatchAll, "slug".into())
        );
        assert_eq!(
            seg("(marketing)"),
            (SegmentKind::RouteGroup, "marketing".into())
        );
        assert_eq!(seg("@team"), (SegmentKind::ParallelSlot, "team".into()));
    }

    #[test]
    fn parses_intercepting_markers() {
        assert_eq!(seg("(.)photo"), (SegmentKind::Intercepting, "photo".into()));
        assert_eq!(
            seg("(..)photo"),
            (SegmentKind::Intercepting, "photo".into())
        );
        assert_eq!(
            seg("(...)photo"),
            (SegmentKind::Intercepting, "photo".into())
        );
        assert_eq!(
            seg("(..)(..)photo"),
            (SegmentKind::Intercepting, "photo".into())
        );
        // A plain group is not intercepting.
        assert_eq!(seg("(shop)"), (SegmentKind::RouteGroup, "shop".into()));
    }

    #[test]
    fn route_group_excluded_from_url_but_kept_as_segment() {
        let segs = parse_segments(
            Utf8Path::new("app"),
            Utf8Path::new("app/(marketing)/blog/[slug]"),
        );
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0].kind, SegmentKind::RouteGroup);
        assert_eq!(segs[1].kind, SegmentKind::Static);
        assert_eq!(segs[2].kind, SegmentKind::Dynamic);
    }
}
