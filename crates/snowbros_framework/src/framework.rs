//! Framework identifiers and detection results.

use std::fmt;

use serde::{Deserialize, Serialize};

use snowbros_core::Confidence;

/// Frameworks and platforms the engine recognizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Framework {
    /// Next.js (React meta-framework).
    NextJs,
    /// React.
    React,
    /// Vue.js.
    Vue,
    /// Nuxt (Vue meta-framework).
    Nuxt,
    /// Angular.
    Angular,
    /// Svelte / SvelteKit.
    Svelte,
    /// SolidJS.
    Solid,
    /// Astro.
    Astro,
    /// Node.js runtime project.
    Node,
    /// Express HTTP framework.
    Express,
    /// NestJS.
    NestJs,
    /// Laravel (PHP).
    Laravel,
    /// Django (Python).
    Django,
    /// Supabase backend platform.
    Supabase,
    /// Prisma ORM.
    Prisma,
    /// Drizzle ORM.
    Drizzle,
}

impl fmt::Display for Framework {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::NextJs => "next.js",
            Self::React => "react",
            Self::Vue => "vue",
            Self::Nuxt => "nuxt",
            Self::Angular => "angular",
            Self::Svelte => "svelte",
            Self::Solid => "solid",
            Self::Astro => "astro",
            Self::Node => "node",
            Self::Express => "express",
            Self::NestJs => "nestjs",
            Self::Laravel => "laravel",
            Self::Django => "django",
            Self::Supabase => "supabase",
            Self::Prisma => "prisma",
            Self::Drizzle => "drizzle",
        };
        f.write_str(s)
    }
}

/// A framework detection result with its supporting evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedFramework {
    /// Which framework was detected.
    pub framework: Framework,
    /// Detection certainty. A dependency match plus a config file is
    /// [`Confidence::Certain`]; a single weak signal is lower.
    pub confidence: Confidence,
    /// Version declared in the manifest, verbatim (e.g. `^15.1.0`), if any.
    pub version: Option<String>,
    /// Human-readable evidence, e.g. `package.json dependency "next"`.
    pub evidence: Vec<String>,
}
