# Atlas RFCs

Design documents for significant architectural decisions in Snowbros Atlas. Each
RFC is grounded in shipped code, not aspiration — where an RFC and
`ARCHITECTURE.md` disagree, the RFC wins.

**Before adding a new language or a major analysis capability, read the relevant
RFC first.** Multi-language work follows [RFC 0002](0002-atlas-multi-language-semantic-platform.md).

## Index

| RFC | Title | Status | Scope |
|---|---|---|---|
| [0001](0001-atlas-v0.2-react-nextjs.md) | Atlas v0.2: Semantic Engine, Atlas IR & Deep React / Next.js | Approved (2026-07-06) | v0.2 → v0.5 — introduced Atlas IR, the semantic model, and React/Next.js support |
| [0002](0002-atlas-multi-language-semantic-platform.md) | Atlas as a Multi-Language Semantic Static-Analysis Platform | **Accepted (2026-07-12)** | M3 → v1.0 — one IR + one semantic model + one rule engine across 12 languages; the `LanguageFrontend` abstraction, analysis stages, framework intelligence, language-maturity tiers, and the milestone roadmap |

## Status meanings

- **Draft** — proposal under discussion; not yet cleared for implementation.
- **Approved / Accepted** — design agreed; implementation may proceed at the stated
  milestone.
- **Superseded** — replaced by a later RFC (linked from its header).

## Authoring a new RFC

Number sequentially (`000N-short-slug.md`), open with a status/author/scope header
(mirror an existing RFC), ground every claim in real crate/type names, and add a
row to the index above. Keep RFCs additive with the compat law: frozen rule ids,
output schemas, and cache-format discipline described in the existing RFCs.
