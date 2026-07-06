# RFC 0001 — Atlas v0.2: Semantic Engine, Atlas IR & Deep React / Next.js

> **Status:** Approved (2026-07-06) — implementation may begin at M0. **Rev 2** (adds Atlas IR).
> **Author:** Engineering
> **Target:** Atlas v0.2 → v0.5 (v0.2 scoped in detail, v0.3+ in outline)
> **Supersedes design intent in:** `ARCHITECTURE.md` §15, §18 (aspirational; this RFC is grounded in shipped v0.1.1 code)

---

## 0. Summary

Atlas v0.1.1 ships a **deterministic, file-level** analysis engine (12 real crates,
tree-sitter JS/TS/JSX/TSX, an import graph, a shallow framework signal table, 11 rules,
incremental cache, auto-fixer, LSP, five output formats).

v0.2 moves Atlas from **file-level observation** to a **semantic project model**, on top of a
new **language-agnostic intermediate representation (Atlas IR)** — without breaking a single
existing rule id, output schema, or CLI/LSP contract.

**Two foundational decisions define v0.2:**

1. **The symbol graph is the v0.2 foundation, not a v0.3 feature.** Deep React/TS rules are
   structurally impossible against file-level facts; they need per-symbol, per-component,
   per-hook resolution. We build the semantic layer first, then harvest rules on top.

2. **Atlas IR becomes a first-class layer.** Rules must not understand tree-sitter nodes.
   Rules understand *Atlas concepts* — `Function`, `Call`, `Import`, `Class`, `Symbol`. Every
   language parser **lowers** its AST into the same IR, so `react/large-component` (v0.2) and
   a future `python/large-function` (v0.4) share infrastructure instead of duplicating
   per-language node-walking. IR is designed in M0, implemented as the minimal subset the
   first rules need, and grown per milestone.

**IR placement (refinement of the approved diagram).** The approved sketch was
`Semantic → IR`. This RFC places **IR *below* semantic**: parsers *lower* to IR, and the
semantic layer resolves and enriches *over IR*. This makes symbol resolution itself
language-agnostic (it reads `ir::Symbol`, not JS-specific nodes), which is the whole point
of a shared IR — and it mirrors rust-analyzer's HIR, where name resolution runs on the
lowered representation, not the raw syntax tree. Framework meaning (React/Next) layers on top
of the resolved IR.

---

## 1. Atlas as a compiler

The target architecture reads as a compiler pipeline — recognizable, robust, extensible:

```
Scanner        discover files
   ↓
Tree-sitter    parse to language AST (error-tolerant)
   ↓
Lowering       AST → Atlas IR        ← per-language, thin
   ↓
Semantic       resolve symbols, references, scopes; enrich (React/Next/TS)
   ↓
Graph Builder  populate symbol/type/call nodes & edges
   ↓
Rule Engine    read IR + semantic model → diagnostics   ← never touches tree-sitter
   ↓
Fix Engine     deterministic auto-fix (existing)
   ↓
LSP / Outputs  JSON · SARIF · HTML · Markdown · terminal
```

Everything above the Lowering line is per-language and thin. Everything below is shared —
each new language pays for a parser + a lowering pass, and inherits every language-agnostic
rule for free.

---

## 2. Principles (non-negotiable, carried from v0.1)

1. **Deterministic.** Same codebase → identical findings, byte-for-byte. Sorted output,
   `BTreeMap`/`BTreeSet`, no wall-clock, no map-iteration order in output.
2. **No AI in the analysis path.** AI never decides a diagnostic.
3. **No heuristic without evidence.** Every diagnostic carries ≥1 evidence entry.
4. **Reproducible.** No network during analysis; content-addressed cache.
5. **Accuracy > quantity.** "Unresolved" ≠ "guessed"; FP guards documented in metadata.
6. **Additive-only compatibility.** New capability never changes an existing rule id or
   output field. (See §9.)

---

## 3. Current architecture (grounded, as-shipped v0.1.1)

**Pipeline (`snowbros_engine::pipeline`):**
`scan → parse → resolve → detect frameworks → build graph → build context → run rules → report → emit`

**Real crates:** `core` (645) · `scanner` (197) · `parser` (1284, file-level `FileFacts`) ·
`resolver` (789) · `framework` (547, signal table) · `graph` (444) · `cache` (402) ·
`rules` (493, 11 rules) · `output` (919) · `engine` (305) · `lsp` (263) · `cli` (551).

**Stub crates (empty 7-line `lib.rs`):** `architecture` · `deps` · `performance` ·
`plugin` · `security`.

**The two structural gaps v0.2 closes:**

1. **The graph is file-level.** `snowbros_graph::model` *already declares*
   `NodeKind::Symbol { name, symbol_kind }` and `EdgeKind::{Contains, Calls, TypeRef}` —
   nothing populates them. Nodes today are `File` and `Package` only. **The type system is
   ready; the producer is missing.**
2. **Parser facts are whole-file.** `FileFacts` has no notion of a component, hook call,
   prop, scope, local reference, or TS declaration shape — exactly what React/TS rules need.

---

## 4. Atlas IR — `snowbros_ir` (new crate)

The language-agnostic substrate. Each parser lowers its tree-sitter AST into IR; everything
downstream reads IR.

### 4.1 Design constraints

- **Language-agnostic node set.** Concepts every target language shares.
- **Stable ids.** `path#kind#name@span`, sorted — same scheme unifies IR nodes and graph
  symbol nodes, so incremental cache keys are stable across re-parse.
- **Serde-serializable.** IR is cacheable; warm re-analysis re-derives identical IR.
- **Lossy by design.** IR keeps what rules reason about, not every syntactic detail. A rule
  needing raw syntax (rare) can still reach the tree-sitter node via a back-reference span,
  but the default surface is IR.

### 4.2 Node set — grown per milestone

**v0.2.0 minimal subset (what the first rules and the symbol graph need):**

```
Module    { path, imports, symbols }
Import    { source, names, span }
Symbol    { name, kind, span, exported }        // fn | class | const | arrow | type…
Function  { name, params, body_span, is_async, returns_jsx }
Class     { name, members, span }
Call      { callee, arg_count, span }
Reference { name, span }                          // a use of a binding
```

**Added in M2 (React):** JSX affordances on `Function` (hook call list, prop set,
`memo`/`forwardRef`/`lazy` wrappers, context create/use sites).

**Added in M3 (TypeScript):** `Interface { members }`, `TypeAlias`, `Enum`, `Namespace`,
`Generic` params, `TypeRef`.

**Deferred (Phase-agnostic, land when a rule needs them):** `Loop`, `Condition`, `Block`
— control-flow IR arrives with complexity/maintainability rules, not before.

### 4.3 Relationship to the semantic layer

- **Lowering** (per-language, lives in `parser`): tree-sitter AST → IR. Thin, mechanical.
- **`snowbros_ir`**: the node types + id scheme + serde. No logic.
- **`snowbros_semantic`**: resolution (symbol tables, references, scopes) + framework
  enrichment, computed **over IR**. Emits the enriched model and populates the graph's
  symbol nodes/edges. Resolution is language-agnostic; enrichment (React/Next) is not.
- **Rules**: read IR + semantic model. Never tree-sitter.

**Shared-infrastructure payoff:** `maintainability/large-function` reads
`ir::Function.body_span` length — works for any language that lowers to IR.
`react/large-component` = semantic flags an IR `Function` as a component
(`returns_jsx && capitalized`); the rule reads the same IR `Function`. One day
`python/large-function` reuses the identical size logic.

---

## 5. Phase → Milestone mapping

| Milestone | Ships in | Delivers | Depends on |
|---|---|---|---|
| **M0 — IR + Semantic Foundation** | **v0.2.0** | `snowbros_ir`, lowering, `snowbros_semantic`, symbol graph, Next.js project model, 2–5 proof rules | parser, graph, resolver |
| **M1 — React Intelligence** (Phase 2) | **v0.2.1** | Component/hook/context model, ~12 rules | **M0 (hard)** |
| **M2 — TypeScript Intelligence** (Phase 3) | v0.3 | Symbol/type/call graph, ~7 rules | **M0 (hard)** |
| **M3 — Architecture Intelligence** (Phase 4) | v0.3 | Boundaries, layers, forbidden imports, monorepo, ~6 rules | import graph, M0 |
| **M4 — Rule Engine Expansion** (Phase 5) | continuous | ~50 rules, categories, maturity gating | M0–M3 |
| **M5 — Performance** (Phase 6) | continuous | IR/symbol-layer cache, parallel parse, interning, DHAT | M0 |
| **M6 — Plugin Foundation** (Phase 7) | v0.5 | deterministic read-only plugin API (native → WASM) | M0, stable IR |

> **Note:** Phase 1 (Next.js project model) folds into **M0/v0.2.0** — it proves the semantic
> architecture end-to-end with real, shippable value. React (Phase 2) becomes its own
> release because it is far larger than it looks (hooks alone are substantial), and splitting
> it gets users value sooner.

**Release cadence (approved):**

```
v0.2.0  Semantic Engine + Atlas IR + Next.js project model + proof rules
  ↓
v0.2.1  React intelligence
  ↓
v0.3    TypeScript semantics + architecture
  ↓
v0.4    Python + Go  (parser + lowering only; inherit language-agnostic rules)
  ↓
v0.5    Plugins (native → WASM/Extism)
  ↓
v1.0    Stable multi-language engineering-intelligence platform
```

M4 (rules) and M5 (perf) are **continuous tracks**, not standalone releases.

---

## 6. Milestones in detail

### M0 — IR + Semantic Foundation → v0.2.0

**Goal:** prove the architecture end-to-end. Pure additive layer; zero behavior change to the
existing 11 rules.

- **`snowbros_ir`** (new): the v0.2.0 node subset (§4.2), stable id scheme, serde.
- **`snowbros_parser`**: a lowering pass tree-sitter → IR for JS/TS/JSX/TSX. `FileFacts`
  **stays as-is** (existing rules keep reading it); IR is produced alongside. `FileFacts`
  becomes a derived view over IR in a later milestone — **no big-bang migration.**
- **`snowbros_semantic`** (new): symbol table + reference resolution over IR; populate
  `NodeKind::Symbol` + `EdgeKind::{Contains, Exports, Calls}` in `snowbros_graph`.
- **`snowbros_framework/nextjs/`**: project model (see §7 layout) — router kind
  (App/Pages/Mixed), route tree (dynamic · catch-all · route groups · parallel ·
  intercepting), special files (layout/loading/error/template/not-found/page/route),
  middleware, metadata API, `generateStaticParams`/`generateMetadata`, server↔client graph.
- **`snowbros_cache`**: fingerprint IR + symbol facts (format v4 → v5, versioned, clean
  invalidation).
- **Proof rules (2–5):** symbol-level `typescript/unused-export`,
  `typescript/duplicate-declaration`, and 1–3 Next.js project-model rules
  (e.g. `next/metadata-in-client-component`, `next/route-page-collision`).

**Exit criteria:** `sb model --format json` emits the route tree deterministically; graph DOT
renders symbol nodes; warm re-analysis re-derives identical IR + symbol ids; existing 11 rules
produce byte-identical output; `sb graph --symbols` works.

### M1 — React Intelligence (Phase 2) → v0.2.1 — depends on M0

- IR gains React affordances (§4.2). Semantic component model: components, props, hooks,
  contexts, `memo`/`forwardRef`/`lazy`/`Suspense`/`ErrorBoundary`, children, hierarchy.
- Metric computation hosted in the now-populated **`snowbros_performance`** crate.
- **~12 rules:** `react/large-component`, `react/too-many-props`, `react/context-abuse`,
  `react/duplicate-hook`, `react/unused-state`, `react/unused-effect`,
  `react/infinite-effect-loop`, `react/missing-dependency-array`,
  `react/unnecessary-rerender`, `react/expensive-inline-object`,
  `react/expensive-inline-function`, `react/prop-drilling`.

**Determinism:** effect-dependency and re-render rules are AST/scope-derived, not speculative
— a missing dep is proven by a referenced-but-undeclared binding; an inline-object prop by an
object/arrow literal in JSX attribute position. Unprovable → not flagged; uncertain rules ship
in **nursery** (off by default).

### M2 — TypeScript Intelligence (Phase 3) → v0.3 — depends on M0

- IR gains type-level nodes (§4.2). Symbol/type/call graph over `snowbros_semantic`.
- **~7 rules:** `typescript/unused-export`, `typescript/dead-code`,
  `typescript/duplicate-interface`, `typescript/circular-type-reference`,
  `imports/broken-path-alias`, `typescript/duplicate-declaration`,
  `typescript/unreachable-symbol`.

### M3 — Architecture Intelligence (Phase 4) → v0.3

- Populate **`snowbros_architecture`** + **`snowbros_deps`**. Boundary DSL in
  `snowbros.toml` (reimplemented in Rust, dependency-cruiser-inspired). Layer violations,
  forbidden imports, shared-folder misuse, monorepo package graph, dependency health.
- **~6 rules:** `architecture/layer-violation`, `architecture/forbidden-import`,
  `architecture/circular-feature-dependency`, `architecture/shared-folder-misuse`,
  `imports/cross-package-deep-import`, `architecture/orphan-module`.

### M4 — Rule Engine Expansion (Phase 5) → continuous

- Categories: `architecture` · `performance` · `react` · `nextjs` · `typescript` ·
  `imports` · `maintainability`. Reach **~50 rules**. Maturity gating in TOML metadata:
  `nursery` (off by default) → `preview` → `stable`.

### M5 — Performance (Phase 6) → continuous

- IR/symbol-layer incremental cache (v5), parallel parsing (rayon, deterministic sorted
  merge), string interning (`lasso`), DHAT on a generated 1M-line fixture.
  **No nondeterministic optimization.** Targets: cold < 200ms/<10k files, incremental < 50ms,
  < 1GB/1M LOC.

### M6 — Plugin Foundation (Phase 7) → v0.5

- Populate **`snowbros_plugin`**. Deterministic, **read-only** API: receives **IR +
  symbol graph**, returns diagnostics, never mutates analysis. Native trait API first, WASM
  (Extism) sandbox second. IR is exactly the stable surface plugins target — a plugin never
  sees tree-sitter, so the plugin API is language-agnostic from day one.

---

## 7. Crate decisions (Deliverables 2 & 3)

**Framework submodule layout (approved — extend, do not create `snowbros_nextjs`):**

```
snowbros_framework/src/
    lib.rs
    detect.rs          # existing signal table
    facts.rs
    framework.rs
    nextjs/
        mod.rs
        detector.rs    # router kind, mixed detection
        routes.rs      # route tree: dynamic, catch-all, groups, parallel, intercepting
        metadata.rs    # Metadata API, generateMetadata, generateStaticParams
        components.rs   # server/client classification over the import graph
    react/             # (M1)
    vue/  svelte/       # (future)
```

**New crates:**

- `snowbros_ir` — language-agnostic IR node types + stable id scheme. *(M0)*
- `snowbros_semantic` — symbol/component/type model over IR; the wedge. *(M0)*

**Extend:** `snowbros_parser` (lowering to IR) · `snowbros_graph` (populate symbol
nodes/edges) · `snowbros_framework` (nextjs submodule) · `snowbros_core` (`ProjectModel`,
`RuleCategory`, `RuleMaturity`, boundary config) · `snowbros_cache` (v5) · `snowbros_rules`
(categories + maturity) · `snowbros_output` (optional `project_model` section).

**Populate (stub → real):** `snowbros_performance` (M1) · `snowbros_architecture` (M3) ·
`snowbros_deps` (M3) · `snowbros_plugin` (M6).

**Stays stub:** `snowbros_security` (secrets already in parser; taint is post-v0.3).

---

## 8. Dependency diagram (Deliverable 4)

```
                         cli ── lsp
                          │      │
                          └──── engine ────────────── output
                                  │
         ┌──────────┬─────────────┼───────────┬────────────┐
       rules   architecture   performance    deps        (plugin, v0.5)
         │          │              │           │
         └──────────┴──────┬───────┴───────────┘
                           │
                        graph ───────────── semantic  ★NEW (the wedge)
                           │                   │
                           │                   │  (resolves & enriches over IR)
                           │        ┌──────────┼──────────┐
                           │     parser     resolver   framework
                           │        │ (lowers)  │           │
                           │        └───────────┼───────────┤
                           │                    │           │
                           └──────────────── ir ★NEW ───────┘
                                             │  (language-agnostic substrate)
                                          scanner
                                             │
                                           core   ← foundation, depended on by all
```

New edges v0.2 introduces: `parser/resolver/framework/semantic → ir`,
`semantic → {parser, resolver, framework}`, `graph → semantic`,
`rules/architecture/performance/deps/plugin → {ir, semantic}`. All additive; no existing edge
removed or reversed.

---

## 9. Compatibility guarantees (Deliverable 9)

**Locked — unchanged in v0.2/v0.3:**

- Existing **11 rule ids**, severities, confidences.
- **JSON / SARIF / HTML** shapes byte-stable for unchanged inputs; `project_model` is a new
  **optional** top-level key.
- **CLI:** existing subcommands/flags unchanged; new surface additive (`sb model`,
  `sb graph --symbols`, category filters).
- **LSP + VS Code:** still per-file `publishDiagnostics`; extension needs **zero changes**.
- **Cache:** v4 → v5 invalidates cleanly (versioned); worst case one cold re-analysis.
- **`FileFacts`:** stays as-is through v0.2.0; existing rules untouched. IR runs alongside;
  `FileFacts`-as-IR-view is a later, internal migration.

---

## 10. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Scope creep (Rome's downfall) | Milestones ship independently; v0.2 split into 0.2.0 / 0.2.1. |
| IR over-engineering / premature generality | Minimal subset in M0; grow per milestone only when a rule needs a node. |
| Dual `FileFacts` + IR drift | `FileFacts` frozen in v0.2.0; single migration later, snapshot-guarded. |
| False-positive overload (React) | Provable-only detection; nursery gating; documented FP guards. |
| Symbol/IR id instability breaks cache | Ids keyed `path#kind#name@span`, sorted; CI determinism test (warm == cold). |
| Monorepo / multi-`package.json` (v0.1 gap) | Addressed in M3 `snowbros_deps`. |
| Perf regression from IR/semantic layer | M5 continuous; CI benchmark fails on >10% regression. |

---

## 11. What this RFC does NOT do

- No new languages in v0.2 (Python/Go are v0.4 — parser + lowering only).
- No taint/deep security (post-v0.3).
- No AI anywhere in the analysis path.
- No refactor of the pipeline, `Rule` trait, or output schemas beyond additive extension.
- No WASM plugins in v0.2 (native plugin API is v0.5's first step).

---

## 12. Approved decisions & first work

**Approved:**
1. Symbol graph pulled into v0.2 as the M0 foundation. ✅
2. Extend `snowbros_framework` with a `nextjs/` submodule; no `snowbros_nextjs`. ✅
3. Split v0.2 → **v0.2.0** (semantic + IR + Next.js + proof rules) / **v0.2.1** (React). ✅
4. **Atlas IR** added as a first-class layer (`snowbros_ir`), placed *below* semantic
   (parsers lower to IR; semantic resolves over IR). ✅ *(placement refined from the approved
   diagram — see §0.)*

**First unit of work:** M0 / v0.2.0, in order —
`snowbros_ir` (node subset + ids) → `snowbros_parser` lowering → `snowbros_semantic`
resolution + graph population → `snowbros_framework/nextjs/` project model → 2–5 proof
rules → `sb model` + `sb graph --symbols`.
```
