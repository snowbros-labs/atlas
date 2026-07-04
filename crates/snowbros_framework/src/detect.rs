//! The detection engine: a declarative signal table evaluated against
//! [`ProjectFacts`].
//!
//! Confidence policy:
//! - dependency match + config/folder marker → [`Confidence::Certain`]
//! - dependency match alone → [`Confidence::Likely`]
//! - config/folder marker alone → [`Confidence::Possible`]
//!
//! Results are sorted by framework for deterministic output.

use snowbros_core::Confidence;

use crate::facts::ProjectFacts;
use crate::framework::{DetectedFramework, Framework};

/// A framework's detection signals.
struct Signals {
    framework: Framework,
    /// npm package names that positively identify the framework.
    packages: &'static [&'static str],
    /// Root-relative config files / folder markers.
    markers: &'static [&'static str],
}

/// The signal table. Order does not affect results (output is sorted).
const SIGNALS: &[Signals] = &[
    Signals {
        framework: Framework::NextJs,
        packages: &["next"],
        markers: &[
            "next.config.js",
            "next.config.mjs",
            "next.config.ts",
            "app",
            "src/app",
            "pages",
            "src/pages",
        ],
    },
    Signals {
        framework: Framework::React,
        packages: &["react", "react-dom"],
        markers: &[],
    },
    Signals {
        framework: Framework::Vue,
        packages: &["vue"],
        markers: &["vue.config.js"],
    },
    Signals {
        framework: Framework::Nuxt,
        packages: &["nuxt"],
        markers: &["nuxt.config.ts", "nuxt.config.js"],
    },
    Signals {
        framework: Framework::Angular,
        packages: &["@angular/core"],
        markers: &["angular.json"],
    },
    Signals {
        framework: Framework::Svelte,
        packages: &["svelte", "@sveltejs/kit"],
        markers: &["svelte.config.js", "svelte.config.ts"],
    },
    Signals {
        framework: Framework::Solid,
        packages: &["solid-js"],
        markers: &[],
    },
    Signals {
        framework: Framework::Astro,
        packages: &["astro"],
        markers: &["astro.config.mjs", "astro.config.ts"],
    },
    Signals {
        framework: Framework::Express,
        packages: &["express"],
        markers: &[],
    },
    Signals {
        framework: Framework::NestJs,
        packages: &["@nestjs/core"],
        markers: &["nest-cli.json"],
    },
    Signals {
        framework: Framework::Supabase,
        packages: &["@supabase/supabase-js", "@supabase/ssr"],
        markers: &["supabase/config.toml"],
    },
    Signals {
        framework: Framework::Prisma,
        packages: &["prisma", "@prisma/client"],
        markers: &["prisma/schema.prisma"],
    },
    Signals {
        framework: Framework::Drizzle,
        packages: &["drizzle-orm"],
        markers: &["drizzle.config.ts", "drizzle.config.js"],
    },
    Signals {
        framework: Framework::Laravel,
        packages: &[],
        markers: &["artisan", "composer.json"],
    },
    Signals {
        framework: Framework::Django,
        packages: &[],
        markers: &["manage.py"],
    },
];

/// Runs all detectors against the given facts.
///
/// Also reports [`Framework::Node`] when a `package.json` exists and no
/// dedicated runtime framework was found (a plain Node project is still a
/// project).
pub fn detect_frameworks(facts: &ProjectFacts) -> Vec<DetectedFramework> {
    let mut results: Vec<DetectedFramework> = SIGNALS
        .iter()
        .filter_map(|signals| evaluate(signals, facts))
        .collect();

    // Laravel needs both markers to avoid matching every composer project.
    results.retain(|d| {
        d.framework != Framework::Laravel
            || (facts.has_entry("artisan") && facts.has_entry("composer.json"))
    });

    if facts.package_json.is_some() && results.is_empty() {
        results.push(DetectedFramework {
            framework: Framework::Node,
            confidence: Confidence::Likely,
            version: None,
            evidence: vec!["package.json present".to_string()],
        });
    }

    results.sort_by_key(|d| d.framework);
    results
}

/// Evaluates one framework's signals. Returns `None` when nothing matched.
fn evaluate(signals: &Signals, facts: &ProjectFacts) -> Option<DetectedFramework> {
    let mut evidence = Vec::new();
    let mut version = None;

    let mut dep_hit = false;
    if let Some(pkg) = &facts.package_json {
        for name in signals.packages {
            if let Some(v) = pkg.dependency_version(name) {
                dep_hit = true;
                version.get_or_insert_with(|| v.to_string());
                evidence.push(format!("package.json dependency \"{name}\" = \"{v}\""));
            }
        }
    }

    let mut marker_hit = false;
    for marker in signals.markers {
        if facts.has_entry(marker) {
            marker_hit = true;
            evidence.push(format!("found `{marker}`"));
        }
    }

    if evidence.is_empty() {
        return None;
    }

    let confidence = match (dep_hit, marker_hit) {
        (true, true) => Confidence::Certain,
        (true, false) => Confidence::Likely,
        (false, _) => Confidence::Possible,
    };

    Some(DetectedFramework {
        framework: signals.framework,
        confidence,
        version,
        evidence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::PackageJson;

    fn facts_with(deps: &[(&str, &str)], entries: &[&str]) -> ProjectFacts {
        let mut pkg = PackageJson::default();
        for (name, version) in deps {
            pkg.dependencies
                .insert((*name).to_string(), (*version).to_string());
        }
        ProjectFacts {
            package_json: Some(pkg),
            root_entries: entries.iter().map(Into::into).collect(),
        }
    }

    fn find(results: &[DetectedFramework], fw: Framework) -> Option<&DetectedFramework> {
        results.iter().find(|d| d.framework == fw)
    }

    #[test]
    fn nextjs_app_router_is_certain() {
        let facts = facts_with(
            &[("next", "^15.1.0"), ("react", "^19.0.0")],
            &["next.config.ts", "src/app", "package.json"],
        );
        let results = detect_frameworks(&facts);

        let next = find(&results, Framework::NextJs).expect("detects next");
        assert_eq!(next.confidence, Confidence::Certain);
        assert_eq!(next.version.as_deref(), Some("^15.1.0"));
        assert!(next.evidence.len() >= 2);

        let react = find(&results, Framework::React).expect("detects react");
        assert_eq!(react.confidence, Confidence::Likely);
    }

    #[test]
    fn dependency_only_is_likely() {
        let facts = facts_with(&[("express", "^4.19.0")], &["package.json"]);
        let results = detect_frameworks(&facts);
        let express = find(&results, Framework::Express).expect("detects express");
        assert_eq!(express.confidence, Confidence::Likely);
    }

    #[test]
    fn marker_only_is_possible() {
        let facts = ProjectFacts {
            package_json: None,
            root_entries: vec!["manage.py".into()],
        };
        let results = detect_frameworks(&facts);
        let django = find(&results, Framework::Django).expect("detects django");
        assert_eq!(django.confidence, Confidence::Possible);
    }

    #[test]
    fn laravel_requires_both_markers() {
        let only_composer = ProjectFacts {
            package_json: None,
            root_entries: vec!["composer.json".into()],
        };
        assert!(find(&detect_frameworks(&only_composer), Framework::Laravel).is_none());

        let both = ProjectFacts {
            package_json: None,
            root_entries: vec!["composer.json".into(), "artisan".into()],
        };
        assert!(find(&detect_frameworks(&both), Framework::Laravel).is_some());
    }

    #[test]
    fn plain_node_project_falls_back_to_node() {
        let facts = facts_with(&[("lodash", "^4.17.21")], &["package.json"]);
        let results = detect_frameworks(&facts);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].framework, Framework::Node);
    }

    #[test]
    fn no_signals_no_results() {
        let facts = ProjectFacts::default();
        assert!(detect_frameworks(&facts).is_empty());
    }

    #[test]
    fn output_is_sorted_and_deterministic() {
        let facts = facts_with(
            &[
                ("next", "15.0.0"),
                ("react", "19.0.0"),
                ("prisma", "6.0.0"),
                ("@supabase/supabase-js", "2.45.0"),
            ],
            &["next.config.ts", "prisma/schema.prisma"],
        );
        let a = detect_frameworks(&facts);
        let b = detect_frameworks(&facts);
        assert_eq!(a, b);
        let mut sorted = a.clone();
        sorted.sort_by_key(|d| d.framework);
        assert_eq!(a, sorted);
    }
}
