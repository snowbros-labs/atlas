//! Router detection: which router a project uses, `src/` layout, and
//! middleware. All pure functions over the file inventory.

use camino::{Utf8Path, Utf8PathBuf};

use super::RouterKind;

/// App-Router base directory (`app` or `src/app`), whichever a file lives
/// under. `app` takes precedence when both somehow appear.
pub fn app_base(files: &[Utf8PathBuf]) -> Option<Utf8PathBuf> {
    base_dir(files, "app")
}

/// Pages-Router base directory (`pages` or `src/pages`).
pub fn pages_base(files: &[Utf8PathBuf]) -> Option<Utf8PathBuf> {
    base_dir(files, "pages")
}

/// Finds `<name>` or `src/<name>` as a directory that at least one file
/// sits inside.
fn base_dir(files: &[Utf8PathBuf], name: &str) -> Option<Utf8PathBuf> {
    let plain = Utf8PathBuf::from(name);
    let nested = Utf8PathBuf::from("src").join(name);
    let under = |base: &Utf8Path| files.iter().any(|f| f.starts_with(base) && f != base);
    if under(&plain) {
        Some(plain)
    } else if under(&nested) {
        Some(nested)
    } else {
        None
    }
}

/// Determines the router kind from the detected bases. `None` when neither
/// router is present.
pub fn router_kind(app: Option<&Utf8Path>, pages: Option<&Utf8Path>) -> Option<RouterKind> {
    match (app.is_some(), pages.is_some()) {
        (true, true) => Some(RouterKind::Mixed),
        (true, false) => Some(RouterKind::App),
        (false, true) => Some(RouterKind::Pages),
        (false, false) => None,
    }
}

/// The middleware file, if present — `middleware.{ts,js}` or
/// `src/middleware.{ts,js}`.
pub fn middleware(files: &[Utf8PathBuf]) -> Option<Utf8PathBuf> {
    const CANDIDATES: &[&str] = &[
        "middleware.ts",
        "middleware.js",
        "src/middleware.ts",
        "src/middleware.js",
    ];
    CANDIDATES
        .iter()
        .find(|c| files.iter().any(|f| f == **c))
        .map(Utf8PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(list: &[&str]) -> Vec<Utf8PathBuf> {
        list.iter().map(Utf8PathBuf::from).collect()
    }

    #[test]
    fn detects_app_base_plain_and_src() {
        assert_eq!(
            app_base(&paths(&["app/page.tsx"])),
            Some(Utf8PathBuf::from("app"))
        );
        assert_eq!(
            app_base(&paths(&["src/app/page.tsx"])),
            Some(Utf8PathBuf::from("src/app"))
        );
        assert_eq!(app_base(&paths(&["lib/util.ts"])), None);
    }

    #[test]
    fn router_kind_matrix() {
        let app = Utf8PathBuf::from("app");
        let pages = Utf8PathBuf::from("pages");
        assert_eq!(router_kind(Some(&app), None), Some(RouterKind::App));
        assert_eq!(router_kind(None, Some(&pages)), Some(RouterKind::Pages));
        assert_eq!(
            router_kind(Some(&app), Some(&pages)),
            Some(RouterKind::Mixed)
        );
        assert_eq!(router_kind(None, None), None);
    }

    #[test]
    fn finds_middleware() {
        assert_eq!(
            middleware(&paths(&["app/page.tsx", "src/middleware.ts"])),
            Some(Utf8PathBuf::from("src/middleware.ts"))
        );
        assert_eq!(middleware(&paths(&["app/page.tsx"])), None);
    }

    #[test]
    fn bare_app_dir_without_children_is_not_a_base() {
        // The directory entry itself must not count as being "under" it.
        assert_eq!(app_base(&paths(&["app"])), None);
    }
}
