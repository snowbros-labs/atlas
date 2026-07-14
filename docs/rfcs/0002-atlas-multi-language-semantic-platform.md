# RFC 0002 — Atlas as a Multi-Language Semantic Static-Analysis Platform

- **Status:** Accepted (2026-07-12) — implementation begins at M3 with the
  `LanguageFrontend` extraction (§2, §8.2.1). All future multi-language work
  follows this RFC.
- **Author:** Lead architect
- **Supersedes:** none. Extends [RFC 0001](0001-atlas-v0.2-react-nextjs.md).
- **Scope:** long-term architecture (M3 → v1.0) for 12 target languages.
- **Guiding constraint:** every language meets the TypeScript engine's quality
  bar. Determinism, accuracy over quantity, zero avoidable false positives,
  evidence-based diagnostics. We do **not** trade correctness for coverage.

---

## 0. Where Atlas actually is today (the honest baseline)

Everything below builds on the real code, not the aspirational `ARCHITECTURE.md`.
The current pipeline is:

```
Scanner → Tree-sitter parse → Lower (Atlas IR) → Semantic → Graph → Rules → Fix → LSP/Outputs
```

Concrete facts this RFC treats as load-bearing invariants:

- **Atlas IR** (`snowbros_ir`) is a rust-analyzer-style HIR: one `ir::Module` per
  file, holding `imports`, `symbols`, `calls`, `references`. `SymbolKind` today
  is `Function | Class | Interface | TypeAlias | Enum | Const | Let | Var |
  Unknown`, each carrying kind-specific data. **This is the lowering target that
  makes multi-language possible** — rules already read `Function`/`Call`/`Import`,
  not tree-sitter nodes.
- **Symbol ids are stable and content-addressed**: `path#kind#name@startByte-endByte`.
  Cache keys depend on them. Any new `SymbolKind::tag()` value is a cache-format
  change.
- **Semantic layer** (`snowbros_semantic`) resolves over IR: intra- and cross-file
  call edges (`resolved_call_edges`), type refs, React roles. Resolution is
  conservative — member/aliased/default calls are left unresolved rather than
  guessed. That conservatism *is* the zero-FP policy in code.
- **Graph** (`snowbros_graph`) has `NodeKind::{File, Module, Symbol, Package}` and
  `EdgeKind::{Imports, Exports, Contains, Calls, TypeRef, DependsOn}`. The symbol
  graph is kept **separate** from the rule graph so `sb graph` DOT and
  node-count-sensitive rules stay stable.
- **Rules** read `AnalysisContext` (`ctx.semantic`, `ctx.import_bindings`,
  `ctx.next_model`). 22 rules. Rule↔metadata is 1:1 and harness-enforced.
  New categories are **not** added to `MONITORED_CATEGORIES` until deliberate, so
  clean-project scorecards stay byte-identical.
- **Compat law:** additive-only. Existing rule ids, JSON/SARIF/HTML/LSP shapes are
  frozen; cache bumps do clean invalidation (currently v8).

The multi-language strategy is therefore **not** "write 12 engines." It is: keep
one IR + one semantic model + one rule engine, and add *frontends* that lower each
language into that IR. This is the Clang/LLVM and rust-analyzer lesson — the
diversity lives at the edges, the value lives in the shared middle.

---

## 1. Universal Semantic IR

### 1.1 Design principle: three tiers, not one flat model

Trying to make one struct express Rust traits, C++ templates, Go interfaces, and
Python duck typing produces a model that is wrong for all of them. Instead the IR
is layered by **universality**:

1. **Core IR (universal).** Present in every language, identical semantics. This is
   the current `ir::Module` set, generalized. Rules that only touch Core IR run on
   all 12 languages for free.
2. **Extension facets (family-universal).** Concepts shared by a *family* of
   languages (e.g. "nominal type with heritage" for Java/C#/TS/Dart; "trait-like
   interface implementation" for Rust/Go/Java). Modeled as optional facets hung off
   Core nodes, not new node types.
3. **Language attributes (language-specific).** Escape hatch: a typed, namespaced
   side-table (`lang::rust::*`, `lang::cpp::*`). Never invented speculatively —
   added only when a shipping rule needs it. Keeps the Core IR from bloating.

The rule: **a concept enters Core IR only if at least 3 target languages share its
semantics closely enough that a single rule reading it is correct for all of them.**
Otherwise it is a facet or a language attribute. This is the gate that prevents the
"lowest common denominator mush" failure mode.

### 1.2 Universal vs. language-specific — the concept map

| Concept | Tier | Rationale |
|---|---|---|
| **Symbol** (named declaration + span + exported flag) | Core | Every language has named declarations. Already exists. |
| **Module** (one file) | Core | Universal unit of lowering. Already exists. |
| **Package** | Core node (`NodeKind::Package`) | Universal, but *discovery* is per-language (§2). |
| **Import / Export** | Core | Universal linkage. Semantics of "export" vary (see below) → resolved by frontend, not Core. |
| **Function** (params, is_async, body_span) | Core | Universal. `is_async` is `Option`-like: absent where the language has no async. |
| **Call** (callee text, arg_count, in_symbol) | Core | Universal. Resolution is the semantic layer's job, per-language callee syntax normalized by frontend. |
| **Reference** (bare identifier use) | Core | Universal, drives reachability/dead-code. |
| **Class / Struct** | Core `Class` + `struct` facet | Class and struct are the same Core node (named type with members). "Value semantics" (C/C++/Rust/Go struct vs. reference class) is a **facet flag**, not a separate node. |
| **Interface / Trait / Protocol** | Core `Interface` + `impl` facet | TS interface, Java/C# interface, Go interface, Rust trait, Swift/Dart protocol collapse to one Core node: "named contract of members." *How it is satisfied* differs → facet. |
| **Inheritance / heritage** | Core edge (`EdgeKind::TypeRef` today; add `Extends`/`Implements`) | Universal as a graph edge. Single vs. multiple inheritance, `extends` vs `implements` vs `impl Trait for` → edge subtype + facet. Note the existing `InterfaceData.extends` already separates heritage from member refs *because their cycle semantics differ* — that instinct generalizes. |
| **Enum** | Core `Enum` + variant-payload facet | C enum (ints), Rust/Swift enum (sum type with payloads), Java enum (singleton objects) share the "named closed set" core; payloads are a facet. |
| **Generics / Templates** | Facet on Symbol (`generics: Vec<TypeParam>`) | Java/C#/TS/Rust/Dart generics are *checked* parametric polymorphism → one facet. **C++ templates are different** (unchecked, instantiation-time, SFINAE, specialization) → C++ language attribute, not the shared generics facet. Modeling them as "the same" would be the classic false-equivalence bug. |
| **Visibility** | Core enum `{Public, Private, Protected, Internal, Crate, Package, Module, FileLocal}` | Superset enum; each frontend maps its keywords in. A cross-language "visibility violation" rule reads this uniformly. |
| **Async** | Core flag + facet | `is_async` already on `FunctionData`. Colored-function semantics (JS Promise, Rust `Future`, C# `Task`, Python coroutine, Dart `Future`) captured as a facet `async_model` for rules that care about await-correctness. |
| **Closures / Lambdas** | Core `Function` with `is_anonymous` + `captures` facet | A lambda is a Function that captures its environment. Capture analysis (by-ref/by-value/move) is a facet; C++ capture lists and Rust `move` closures populate it, GC languages leave it empty. |
| **Traits (Rust) / mixins (Dart)** | `Interface` Core + `default_methods` facet | Trait with default methods = interface that carries implementations. Facet holds the provided-method set. |
| **Macros** | Language attribute + **opaque-expansion marker** | Rust `macro_rules!`/proc-macros, C/C++ preprocessor, C# source generators. **Not universal, actively dangerous.** Modeled as: (a) a Symbol of kind `Macro`, and (b) a marker on any span whose content is macro-generated. Rules must be able to *suppress* themselves on macro-generated spans to preserve zero-FP. See §1.4. |

### 1.3 What stays out of Core IR (deliberately)

- **Full type inference results.** The IR records *type references by name* and
  declared annotations, not inferred types. Real inference is per-language and lives
  in the frontend's type-resolution phase, feeding *facts* back (e.g. "this call
  resolves to symbol X") rather than a universal type lattice. A universal type
  system is a 10-year project and a false-positive factory; we don't build one.
- **Control-flow and data-flow graphs.** These are *derived* on demand by the
  analysis-stage machinery (§5) for the rules that request them, not stored in Core
  IR. Keeps the IR small and cache-friendly.

### 1.4 Macros, codegen, and the zero-FP contract

The single biggest source of false positives across C/C++/Rust/C# is analyzing
generated code as if a human wrote it. The IR carries an explicit **provenance**
bit on every node: `Provenance::{Source, MacroExpansion, Generated}`. The rule
engine's default policy: **rules that emit diagnostics against a symbol suppress
themselves when the symbol's provenance is not `Source`, unless the rule explicitly
opts in.** This is enforced at the engine level, once, so no individual rule author
can accidentally violate it. It is the generalization of the existing "unresolved ≠
guessed" discipline.

### 1.5 IR evolution policy

Per RFC 0001's proven approach: the IR grows **only** per-milestone, only the nodes
that milestone's shipping rules need. No speculative modeling. Every IR addition is
additive (new optional facet / new `SymbolKind` variant), cache version bumps do
clean invalidation, and `SymbolKind::tag()` values are frozen once shipped.

---

## 2. Language Frontends

A **frontend** = { parser adapter, lowering pass, name resolver, type resolver,
project discovery, build-config detection, framework metadata hook }. All frontends
implement one trait so the engine is language-agnostic:

```rust
/// Every language plugs in by producing Atlas IR + resolution facts.
/// The engine never sees tree-sitter or language syntax past this boundary.
pub trait LanguageFrontend {
    fn language(&self) -> Language;
    /// File extensions / shebangs / heuristics this frontend claims.
    fn matches(&self, path: &Utf8Path, first_bytes: &[u8]) -> bool;
    /// Parse + lower one file to Core IR (+ facets). No cross-file work here.
    fn lower(&self, source: &str, path: &Utf8Path) -> LowerResult;
    /// Resolve imports/exports/heritage across the workspace file set.
    fn resolve(&self, modules: &ModuleSet, project: &ProjectModel) -> ResolutionFacts;
    /// Discover projects/packages and build config from the workspace root.
    fn discover(&self, root: &Utf8Path, files: &FileIndex) -> Vec<ProjectModel>;
    /// Optional: framework detectors this frontend enables (§3).
    fn frameworks(&self) -> &[Box<dyn FrameworkDetector>] { &[] }
}
```

**Parser strategy default: Tree-sitter.** It gives error-recovery (essential for
editor/LSP use on incomplete code), incremental reparse, uniform node API, and we
already depend on it. We deviate only where Tree-sitter demonstrably can't meet the
quality bar. Below, "TS-grammar" = tree-sitter grammar exists and is production-grade.

### Per-language frontends

**1–4. TypeScript / JavaScript / React / Next.js** — *shipped / in progress.*
Parser: TS-grammar (`tree-sitter-typescript`, `-javascript`). Name resolution:
relative + ext/index probing + tsconfig `paths` (JSONC + `extends`). Type
resolution: declared annotations + structural cross-file symbol resolution (no
full inference — deliberately). Framework metadata: `snowbros_framework::nextjs`
(router, routes, metadata, server/client boundary), React roles in semantic.
Incremental: IR rides the file cache (xxh3 + mtime), warm==cold proven. Project
discovery: `package.json` + `tsconfig.json`. Build config: tsconfig `paths`,
`baseUrl`, `extends`. **Known gaps to close (from project memory):** monorepo
multi-`package.json`, package-based tsconfig `extends`, deeper `extends` chains in
cache fingerprint.

**5. Python.** Parser: TS-grammar (`tree-sitter-python`). Name resolution: the hard
part — Python resolution is *dynamic*, so we resolve what is statically evident
(explicit `import`/`from ... import`, `__all__`, module-relative packages via
`__init__.py`) and mark the rest unresolved rather than guess. Type resolution:
PEP 484 annotations + `.pyi` stubs when present; **no runtime type inference.**
Absence of annotations ⇒ conservative "unknown," never a guessed type. Framework
metadata: Django (apps, models, URL conf), FastAPI (routers, dependencies),
Flask. Incremental: file cache. Project discovery: `pyproject.toml` /
`setup.cfg` / `requirements.txt`; virtualenv detection for dependency presence.
Build config: `pyproject.toml` (tool tables), `mypy.ini` for declared strictness.
*Risk:* dynamic imports, metaclasses, monkey-patching — mitigated by conservatism.

**6. Java.** Parser: TS-grammar (`tree-sitter-java`) for the IR. Name/type
resolution: Java is nominally typed and *resolvable*, which is its strength — full
package/classpath resolution. This is where we may **supplement** tree-sitter: to
resolve symbols against compiled dependencies (jars) we read classfile signatures
(no bytecode execution), because source-only analysis can't see library types.
Project discovery: Maven (`pom.xml`) / Gradle (`build.gradle[.kts]`) reactor;
multi-module aware. Build config: source roots, dependency coordinates, Java
release level. Framework metadata: Spring Boot (§3). Incremental: file cache +
classpath fingerprint. *Risk:* annotation processing / Lombok generate code →
provenance marker (§1.4).

**7. Go.** Parser: TS-grammar (`tree-sitter-go`). Name/type resolution: Go's
package model is clean and static — resolve via module path (`go.mod`) and package
directories. Interface satisfaction is *structural* (no `implements` keyword) →
computed by the resolver into `Implements` edges, a genuine cross-language-reusable
fact. Project discovery: `go.mod` (module + `go` version), `go.work` workspaces.
Build config: build tags/constraints (must honor for correct dead-code). Framework
metadata: standard `net/http`, and detectors for common routers. Incremental: file
cache; Go's fast compile model maps well to per-package invalidation. *Risk:* build
tags gating files — handled in discovery.

**8. Rust.** Parser: TS-grammar (`tree-sitter-rust`) for IR; we are a Rust shop so
this is the best-understood frontend. Name/type resolution: module tree (`mod`),
`use` paths, crate graph from `Cargo.toml`/`Cargo.lock`. Traits → `Interface` Core
node + `impl` facet; `impl Trait for T` → `Implements` edges. Project discovery:
Cargo workspace (`[workspace] members`), which Atlas itself uses — we dogfood.
Build config: features, target cfg. Framework metadata: async runtimes (tokio),
web frameworks (axum/actix) as detectors. Incremental: file cache. **Macros are the
risk** — `macro_rules!` and proc-macros generate code we can't see from syntax.
Policy: mark macro-invocation spans `MacroExpansion` provenance; rules suppress
on them. We do **not** attempt macro expansion in v1.0.

**9. C#.** Parser: TS-grammar (`tree-sitter-c-sharp`). Name/type resolution:
nominal, resolvable; namespaces + assembly references. Project discovery: `.csproj`
/ `.sln` (MSBuild). Build config: target frameworks, `LangVersion`, nullable
context (drives null-safety rules correctly). Framework metadata: ASP.NET Core
(§3). *Risk:* source generators + partial classes → provenance + partial-merge in
lowering. Incremental: file cache + assembly-ref fingerprint.

**10. C.** Parser: **here Tree-sitter alone is insufficient** and we justify a
deviation. C semantics are defined *after preprocessing*; analyzing pre-preprocessor
text yields false positives (conditional compilation, macro-defined symbols). We
integrate a **preprocessor pass** (libclang's or a vendored preprocessor) to obtain
translation-unit-accurate token streams, then lower. `#include` graph + macro
provenance are first-class. Project discovery: `compile_commands.json` (the
compilation database — the only reliable source of include paths + defines),
CMake/Make fallback. Build config: include dirs, `-D` defines, standard level.
*Risk:* without `compile_commands.json` we degrade to best-effort and say so
(evidence-based honesty). Incremental: TU-level cache keyed on the command +
include closure.

**11. C++.** As C, plus templates. Parser: Tree-sitter for structure but **type
resolution requires a real C++ frontend** (libclang) for anything template- or
overload-sensitive — Tree-sitter cannot resolve overload sets or instantiate
templates, and pretending otherwise breaks zero-FP. Strategy: **two-speed**. Core
IR (symbols, includes, functions, calls, classes) from Tree-sitter for fast, broad,
syntax-level rules; libclang-backed resolution *opt-in* for the type-aware and
interprocedural rules that need it (declared via rule maturity stage, §5). Templates
are a C++ language attribute, never the shared generics facet (§1.2). Project
discovery/build config: `compile_commands.json` primary. This is the hardest
frontend and is intentionally last (§7).

**12. Dart / Flutter.** Parser: TS-grammar (`tree-sitter-dart`). Name/type
resolution: nominal, sound null-safety — resolvable. Project discovery:
`pubspec.yaml`, package config. Build config: SDK constraints, null-safety mode.
Framework metadata: Flutter widget tree (§3). Incremental: file cache. *Risk:*
code generation (`build_runner`, `*.g.dart`) → `Generated` provenance; freezed/json
generated files suppressed by default.

### 2.1 Frontend quality gate (non-negotiable)

A language does not ship until its frontend passes the **same** gate TypeScript
passed:

1. IR round-trips deterministically (serialize == reserialize).
2. Warm cache output is byte-identical to cold.
3. At least one real-world repo dogfooded with a documented false-positive count,
   and every FP either fixed or documented in rule metadata with a guard.
4. Name resolution is *conservative*: unresolved is a first-class state, never a
   guess.
5. Framework/codegen provenance wired so generated code is not analyzed as source.

This is why we do **not** rush languages: the gate, not the grammar, is the work.

### 2.2 Language maturity contract (the stability tiers)

The §2.1 gate is pass/fail for *release*, but "released" is not binary in a
12-language engine — a language can have production parsing while its data-flow
rules are still nursery. Users, contributors, release notes, and the roadmap all
need one word that says exactly how far a language has come. Every language
declares one **maturity tier**, surfaced in `sb languages`, the docs matrix, and
LSP server capabilities. A language may only advance a tier after meeting the
tier's bar *and* the §2.1 gate for every capability that tier implies.

| Tier | Analysis stages available (§5) | What Atlas promises | What it does **not** promise |
|---|---|---|---|
| **Experimental** | AST only (stage 1) | Files parse; syntax-level rules run; symbol *outline* may exist | No cross-file resolution, no semantic accuracy guarantee. Findings are best-effort. |
| **Preview** | + Semantic + Type-aware (stages 2–3) | IR + symbol graph + name/type resolution; a *subset* of rules run and are held to the zero-FP bar | Not all rule families run; call-graph / data-flow rules absent |
| **Stable** | + Call graph (stage 4) | Meets Atlas' full accuracy guarantee for every rule that ships for the language; conservative resolution; dogfooded FP count documented | Interprocedural / whole-program rules may still be limited |
| **Enterprise** | + Control flow + Data flow + Interprocedural (stages 5–7) | The complete semantic engine: call graph, CFG, data flow, cross-function analysis. This is the TypeScript bar. | — |

Rules governing the ladder:

- **A tier claim is a contract.** "Python is Stable" means every rule that runs on
  Python meets the zero-avoidable-FP bar, verified by dogfood — not that all rules
  run. This prevents the "Python is supported" overclaim when only parsing exists.
- **Tiers advance per capability, monotonically.** A language never loses stages on
  upgrade; a stage only becomes claimable once its rules pass the gate on a real repo
  in that language.
- **Maturity composes with rule maturity.** A rule can be `stable` at a language's
  call-graph tier while its data-flow-powered superset stays `nursery` until the
  language reaches Enterprise. The two axes are independent: *language* tier = which
  stages are trustworthy here; *rule* maturity = how proven this rule is anywhere.
- **Target tiers by milestone (§7):** M3 lands Python at **Preview→Stable**; M4
  brings Go/Rust/Java to **Stable**, targeting **Enterprise** as data-flow lands;
  M5 takes C#/Dart to **Stable/Enterprise**; v1.0 ships C/C++ at **Preview→Stable**
  (Enterprise gated on libclang-backed resolution). TypeScript is the reference
  **Enterprise** language today.

---

## 3. Framework Intelligence

### 3.1 Integration model — detectors produce facets, never fork the IR

Framework knowledge integrates through **detectors** that read the already-built IR
+ resolution facts and attach a typed, namespaced **project/framework model** (the
existing `next_model` pattern in `AnalysisContext`, generalized). Detectors:

- **never** change Core IR or `SymbolKind`;
- run *after* lowering + semantic, *before* rules;
- attach evidence + confidence (mirroring the existing `snowbros_framework`
  evidence model);
- expose their model on `AnalysisContext` as `ctx.frameworks: FrameworkModels`,
  keyed by framework, `Option`-typed so non-framework code and legacy tests are
  unaffected.

A framework-specific rule reads `ctx.frameworks.react` etc.; a generic rule ignores
it. This is exactly how `next/*` rules work today and it has kept the shared model
clean — we scale the same seam.

### 3.2 React

- **Components / Hooks:** already modeled — `ReactRole::{Component, Hook}` in
  semantic via `role_of`. Extend with **props** (from the first param's destructure
  + type ref), **context** (`createContext` symbols + `useContext` call sites →
  provider/consumer edges), **refs**, **memo/forwardRef** wrappers.
- **Hooks correctness:** Rules-of-Hooks already partially shipped
  (`hook-in-non-component`). Full version needs control-flow (conditional/loop hook
  calls) — declared as a CFG-stage rule (§5). `exhaustive-deps` needs the dependency
  array + closure capture facet.
- **Context:** modeled as symbol-graph edges Provider→Consumer, enabling a "context
  used without provider in tree" rule (interprocedural stage).
- **Suspense / Server Components:** the `"use client"` / `"use server"` directive is
  already parsed; Server Components are the default in App Router. Model each
  component's **environment** (`Server | Client`) — this already exists in
  `NextProjectModel.rendering`. Suspense boundaries = JSX `<Suspense>` sites →
  facet on the component subtree.

### 3.3 Next.js

Largely shipped in `snowbros_framework::nextjs` (RFC 0001): App + Pages router,
route tree (dynamic/catch-all/groups/parallel/intercepting), special files,
middleware, metadata API, `generateStaticParams`/`generateMetadata`, server/client
via import graph. Remaining first-class targets:

- **API routes / Route Handlers:** already have `http_methods`; add request/response
  typing hooks for handler-signature rules.
- **Server Actions:** `"use server"` functions crossing the client boundary — model
  as boundary-crossing edges; enables "Server Action called from disallowed context"
  and "non-serializable arg to Server Action" (data-flow stage).
- **Client/Server boundary:** already the strongest area (`server-only-in-client`,
  `private-env-in-client` ship today). Generalize the boundary as a graph partition
  so new rules read one model.

### 3.4 Flutter

- **Widgets:** a Dart class extending `StatelessWidget`/`StatefulWidget` → detector
  tags `WidgetRole`. Widget composition = the `build()` method's returned tree →
  a **widget tree facet** (analogous to React's JSX return).
- **State management:** detect `setState`, and popular libs (Provider, Riverpod,
  Bloc) as detectors → enables "setState in build", "state mutation outside
  setState", "missing dispose".
- **Build methods:** `build(BuildContext)` is the analog of React render — same
  class of rules (expensive work in build, missing keys in lists).

### 3.5 Spring Boot / FastAPI / Django / ASP.NET (backend frameworks)

All four share a **request-handler + dependency-injection + data-mapping** shape, so
they get a **shared backend-framework facet** with per-framework detectors:

- **Spring Boot:** `@RestController`/`@Service`/`@Repository`/`@Autowired` →
  bean graph + endpoint map. Enables N+1 (repository call in a loop), missing
  `@Transactional`, endpoint-without-auth.
- **FastAPI:** router decorators + `Depends()` → endpoint + dependency graph.
- **Django:** apps, models (ORM), URL conf, views → model/query graph for N+1 and
  migration-safety rules.
- **ASP.NET Core:** controllers/minimal APIs + DI container → endpoint + service
  graph.

The *facet* is shared ("HTTP endpoint," "injected dependency," "data query"), the
*detector* is per-framework. A single "endpoint missing authorization" or "query in
a loop (N+1)" rule then runs across Spring, FastAPI, Django, and ASP.NET by reading
the shared facet — the payoff of the whole architecture.

### 3.6 Why this doesn't break the shared model

Because framework knowledge is **read-only over the IR and additive on the side**.
No detector can alter symbol ids, cache keys, Core IR, or existing rule inputs.
Frameworks are new *evidence*, never new *substrate*. If every detector were deleted
tomorrow, the generic engine would still produce identical generic diagnostics.

---

## 4. Cross-Language Rule Engine

### 4.1 The core idea

A rule is a function over the **shared IR + graph + facets**, not over source text.
Because TS, Python, Go, etc. all lower into the same `Symbol`/`Call`/`Import`/graph
model, a rule written against that model runs on every language whose frontend
populates the inputs it reads. The rule declares its inputs; the engine runs it on
exactly the languages that supply them.

```rust
pub trait Rule {
    fn meta(&self) -> &RuleMeta;                 // id, severity, confidence, category
    fn stage(&self) -> Stage;                     // minimum analysis stage (§5)
    fn requires(&self) -> Requirements;           // e.g. CallGraph | Exports | ControlFlow
    fn languages(&self) -> LanguageSupport;       // All, or an explicit allow-list
    fn check(&self, cx: &AnalysisContext, sink: &mut DiagnosticSink);
}
```

`languages() == All` is the goal; a rule narrows only when a concept genuinely
differs. The engine skips a rule on any file whose frontend didn't produce the
required inputs — so a call-graph rule simply doesn't fire on a language without a
resolved call graph yet, rather than producing garbage.

### 4.2 The example rules, and how one implementation spans languages

| Rule | Reads (shared inputs) | Cross-language because… |
|---|---|---|
| **Unused symbol** | `Symbol.exported`, `Reference`s, cross-file import bindings | "declared, never referenced, not exported" is identical over IR in every language. Already shipped for TS (`unreachable-symbol`). |
| **Circular dependency** | `EdgeKind::Imports` graph + Tarjan SCC (already in `snowbros_graph`) | An import cycle is a graph property; the graph is language-neutral. `no-circular-imports` already runs on the shared graph. |
| **Dead code** | Reachability from entry points over `Calls` + `Exports` edges | Reachability is graph math; entry-point *discovery* is per-frontend (main, exported API, framework handlers) — one small per-language hook, one shared rule body. |
| **Large functions** | `FunctionData.body_span` (line count) / CFG node count | Pure AST/semantic metric, universal. Thresholds configurable. |
| **Layer violations** | `Imports` edges + user layer config (path globs → layers) | "module in layer A imports layer B" is a graph+config rule, language-independent. |
| **Dependency violations** | `DependsOn` edges + policy (allow/deny lists, version constraints) | Package graph is shared; per-language discovery feeds it. |
| **Duplicate implementations** | Structural fingerprint of `FunctionData`/`ClassData` members + normalized body hash | Fingerprinting normalized IR is language-neutral; the current `duplicate-declaration` is the seed. Cross-language dup within a repo is a bonus. |
| **Missing error handling** | Async/`Result`-like facets + CFG: a fallible call whose error path is neither handled nor propagated | Needs the `async_model`/error-model facet + data-flow stage; the *rule logic* ("fallible result ignored") is shared, the *facet* per-family (Rust `Result`, Go `error` return, JS Promise rejection, C# exception, Python raise). |
| **Performance issues** | Framework facets + loop CFG (e.g. N+1: data query inside a loop over `Calls`) | Reads the shared backend-facet (§3.5) → one N+1 rule across Django/Spring/FastAPI/ASP.NET. |

### 4.3 The discipline that keeps this honest

- A cross-language rule ships for a new language only after being **dogfooded on a
  real repo in that language** with FP count documented — the §2.1 gate applies to
  rules-on-languages, not just frontends.
- Where a concept *almost* matches but has a language-specific FP trap (e.g. Go's
  blank identifier `_`, Python's `__all__` re-exports, C# partial classes), the trap
  is handled in the **frontend's lowering** (so the IR the rule sees is already
  correct) — never patched into the shared rule with language `if`-ladders. This is
  the load-bearing rule that stops the shared engine from rotting into a pile of
  per-language special cases.

---

## 5. Rule Maturity Levels (analysis stages)

Every rule declares the **minimum analysis stage** it needs. The engine builds
stages lazily and in order, and only builds a stage if some enabled rule requires
it — so a repo analyzed with only AST-stage rules never pays for data-flow. This is
both a performance lever and a correctness contract (a rule can't accidentally read
a graph that wasn't built for its inputs).

| Stage | What's available | Built from | Example rules |
|---|---|---|---|
| **1. AST** | Tree-sitter syntax tree, per file | parse | naming conventions, syntax smells, `no-eval`, large-file |
| **2. Semantic** | Atlas IR: symbols, imports, exports, references, resolved names | lowering + name resolution | unused-export, duplicate-declaration, unresolved-import |
| **3. Type-aware** | declared/annotated types + type-ref edges + heritage | type resolution | circular-type-reference, interface/heritage rules, null-safety (where language has it) |
| **4. Call graph** | resolved `Calls` edges (intra + cross-file) | semantic call resolution | unreachable-symbol, recursion, dead-code (call-based) |
| **5. Control flow** | per-function CFG (built on demand) | CFG builder over IR | rules-of-hooks (conditional/loop), unreachable-branch, missing-return |
| **6. Data flow** | def-use, taint, nullability along CFG | DFG over CFG | missing error handling, taint/security, null-deref, unused-assignment |
| **7. Interprocedural** | summaries propagated across call graph | function summaries + fixpoint | cross-function taint, "context used without provider," Server-Action arg flow |

Design rules for the stage machine:

- **Monotonic dependency:** stage *n* may read stages `< n`, never `> n`. Enforced by
  the `requires()`/`stage()` contract in the trait.
- **Lazy + cached:** CFG/DFG are derived, not stored in IR; cached per-function keyed
  on the function's IR hash, so unchanged functions skip rebuild (extends the
  existing warm-cache invariant into the analysis stages).
- **Determinism at every stage:** stages iterate `BTreeMap`/sorted collections
  (already the semantic layer's convention). Interprocedural fixpoint uses a fixed
  visitation order (reverse-topological over the call graph, SCCs handled by the
  existing Tarjan machinery) so results are identical run-to-run.
- **Stage availability gates language support:** e.g. C++ interprocedural rules only
  run when libclang resolution is available; otherwise the rule is skipped, not
  wrong. Rule maturity (`nursery`/`preview`/`stable`) composes with stage: a
  correct-but-narrow rule can be `stable` at call-graph stage while its
  data-flow-powered superset stays `nursery`.

---

## 6. Performance

Targets: large monorepos (millions of LoC), interactive LSP latency, and
**bit-for-bit determinism** — the two are in tension and the design resolves it by
making parallelism *order-independent* and caching *content-addressed*.

- **Parallelism.** File-level lowering and per-function CFG/DFG are embarrassingly
  parallel → `rayon` (already a dependency) with a **deterministic reduce**: workers
  produce results into per-file slots, then the engine merges in sorted order. No
  diagnostic ordering ever depends on thread scheduling. Cross-file phases (name
  resolution, call-graph, interprocedural fixpoint) run on the assembled, sorted
  module set. This preserves the proven "warm output byte-identical to cold"
  invariant under multi-core.
- **Incremental cache.** Extend the existing xxh3 + mtime + config-fingerprint cache.
  Cache granularity tiers: (a) parsed tree + IR per file (exists, currently v8);
  (b) per-function CFG/DFG keyed on function IR hash; (c) resolution facts keyed on
  the *import-closure* fingerprint of a file (so a file re-resolves only when its
  transitive imports' signatures change, not on every edit). Interprocedural
  summaries cached per function, invalidated by call-graph delta.
- **Symbol indexing.** A workspace symbol index (name → declaring symbol ids) built
  once, updated incrementally, so cross-file resolution is a lookup not a scan. Keyed
  by interned strings.
- **Interning.** Intern module paths, symbol names, and type names into a global
  interner → `u32` handles. Cuts memory and makes graph/edge maps cache-friendly and
  hashing cheap. Ids stay stable (the interner is deterministic given sorted input).
- **Arena allocation.** IR nodes and CFG/DFG nodes for a file live in a per-file
  arena, freed as a unit when the file's cache entry is evicted. Avoids per-node
  alloc churn; improves locality for the graph walks that dominate analysis time.
- **Memory optimization.** IR is `serde`-serializable and disk-backed via the cache,
  so the resident set can be bounded: keep hot files' IR in memory, spill cold IR to
  the on-disk cache, reload on demand. Symbol graph stored as CSR-style adjacency
  (indices into interned-id vectors) rather than pointer-chasing `petgraph` for the
  large-repo path.
- **Workspace indexing.** Project discovery (§2) runs once to build a file index +
  project models; `notify`-based watch (already shipped, 300ms debounce, delta-only)
  updates only affected files and their import-closure. LSP reuses the same warm
  index — editor latency = incremental delta, not full re-analysis.

Determinism guardrails, restated because they constrain every optimization above:
sorted iteration everywhere output-visible, no timestamps in output, no
thread-order dependence, interner seeded deterministically, floating point avoided
in any scoring path. These are already project conventions; the perf work must not
break them, and the "warm == cold, byte-identical" e2e test is the gate.

---

## 7. Roadmap

Sequencing principle: **each milestone ships a complete, user-visible capability**
(per [[atlas-release-philosophy]]) and each unlocks the substrate the next needs.
Languages are ordered by *resolvability* (how statically analyzable they are) and
*ecosystem leverage*, hardest-to-analyze last. We add a language only after its
frontend passes the §2.1 gate.

### M3 — Frontend abstraction + Python (v0.4)
- **Languages:** Python (TS shipped).
- **Frameworks:** FastAPI, Django detectors (facet-based §3.5).
- **Stages:** formalize the `LanguageFrontend` trait; extract the current TS engine
  behind it (proves the abstraction on a known-good language before adding a new
  one). Add CFG builder (stage 5) — Python's dynamism makes CFG the right first
  investment.
- **Rule families:** the generic core (unused-symbol, circular-dep, dead-code,
  large-function, layer/dep violations) made language-neutral and running on
  Python + TS. First backend-framework rules (N+1, endpoint-auth) on the shared
  facet.
- **Risks:** Python's dynamic resolution → conservative resolver, dogfood FP gate.
  Refactoring TS behind the trait without regressing 22 shipped rules → additive,
  golden-output tests.
- **Why first:** Python is high-leverage, has a production tree-sitter grammar, and
  forces the frontend abstraction + generic-rule genericization that every later
  language reuses. Low frontend risk, high architectural payoff.

### M4 — The resolvable statically-typed trio: Go + Rust + Java (v0.4.x → v0.5)
- **Languages:** Go, Rust, Java.
- **Frameworks:** Spring Boot; Go `net/http`; Rust axum/tokio detectors.
- **Stages:** data-flow (stage 6) for error-handling rules; `Implements` edge
  computation (Go structural + Rust `impl` + Java `implements`).
- **Rule families:** missing-error-handling (Go `error` returns, Rust `Result`,
  Java exceptions via shared error-model facet), trait/interface-satisfaction rules,
  full dead-code (these languages resolve cleanly so dead-code is high-confidence).
- **Risks:** Rust macros (provenance suppression), Java classpath/jar resolution
  (classfile signature reading), Go build tags. All mitigated by the frontend gate.
- **Why here:** these three are the *most* statically resolvable languages → highest
  accuracy per unit effort, and they validate the type-aware + data-flow stages on
  sound type systems before we face C/C++'s unsoundness. Rust also lets us dogfood
  Atlas on Atlas.

### M5 — Interprocedural + C# and Dart/Flutter (v0.5.x)
- **Languages:** C#, Dart/Flutter.
- **Frameworks:** ASP.NET Core, Flutter (widgets/state/build), full React context +
  Server Actions (needs interprocedural + data-flow now available).
- **Stages:** interprocedural (stage 7) — summaries + fixpoint over the call graph.
- **Rule families:** cross-function taint/security, context-without-provider,
  Server-Action arg-flow, Flutter state-management rules, nullability
  (C# nullable context, Dart sound null-safety — both give us *declared* nullability
  to check accurately).
- **Risks:** C# source generators + partial classes, Dart codegen — provenance
  markers. Interprocedural determinism at scale — fixed visitation order + SCC
  handling.
- **Why here:** C# and Dart are resolvable (like M4) so they're low-risk languages,
  but the *frameworks* (ASP.NET, Flutter, advanced React) demand interprocedural
  analysis — so this milestone pairs "easy languages" with "hard analysis" to
  de-risk the stage-7 engine before the hardest languages.

### v1.0 — C and C++
- **Languages:** C, then C++.
- **Frameworks:** none required; systems-level rule families instead.
- **Stages:** two-speed frontend (Tree-sitter fast path + opt-in libclang resolution
  for type-aware/interprocedural rules); preprocessor/TU integration;
  `compile_commands.json` ingestion.
- **Rule families:** include-cycle, macro-safety (bounded), resource/ownership
  smells at syntax+call-graph level; type-aware rules gated on libclang availability.
- **Risks:** the largest in the whole roadmap — preprocessor accuracy, template
  resolution, overload sets, build-database dependence. This is *why* C/C++ is last:
  they need every stage built and proven on 10 other languages first, plus an
  external frontend (libclang) that the pure-Tree-sitter languages didn't require.
- **Why last / why v1.0:** shipping C/C++ at the same quality bar is the hardest
  claim in static analysis. Doing it *after* the stage machinery, interning/arena
  perf work, and interprocedural engine are battle-tested is the only way to hit the
  zero-avoidable-FP bar. Reaching it is what earns the v1.0 label: "12 languages,
  one semantic engine, one quality bar."

**Cross-milestone invariants:** additive-only compat (frozen rule ids / output
schemas), cache version bumps do clean invalidation, every language passes the §2.1
gate before release, and every release bundles substrate + the rules that use it
(never substrate alone).

---

## 8. Architecture Review — current Atlas, and what to change (only if it preserves determinism, accuracy, performance, maintainability)

### 8.1 What is already right (keep, don't touch)

- **The IR wedge exists and is honest.** `snowbros_ir` as a small, milestone-grown
  HIR that rules read instead of tree-sitter is *exactly* the Clang/rust-analyzer
  factoring. This is the single most important decision already made — it is what
  makes 12 languages tractable. Do not "enrich" it speculatively.
- **Conservative resolution as policy-in-code.** Leaving member/aliased/default calls
  unresolved rather than guessing is the zero-FP contract expressed as code. Keep it
  as the default for every new frontend.
- **Separation of the symbol graph from the rule graph.** Prevents new nodes from
  perturbing `sb graph` output and node-count-sensitive rules. This seam must be
  preserved as we add edge kinds.
- **Content-addressed stable ids + warm==cold determinism test.** This is the
  regression net for everything; every perf and language change must keep it green.
- **Additive compat + harness-enforced rule↔metadata 1:1.** Cheap discipline, huge
  payoff. Keep.

### 8.2 Recommended changes (each justified against the four constraints)

1. **Introduce the `LanguageFrontend` trait now, before the second language.**
   *Maintainability:* refactoring one engine into "engine + frontends" with two
   languages is a nightmare; with one it's a clean extraction. *Determinism/accuracy/
   perf:* neutral if done as pure refactor with golden-output tests. **Do this in M3,
   as its own step, TS as the first implementor.** Risk if deferred: the abstraction
   gets shaped by whatever the second language happened to need, not by principle.

2. **Add a formal `Stage`/`Requirements` contract to the `Rule` trait (§5).**
   *Correctness:* stops a rule reading a graph that wasn't built. *Performance:* lets
   the engine skip building stages no enabled rule needs. Currently rules implicitly
   assume `ctx.semantic` is present; make the dependency explicit and engine-checked.
   Low complexity, high safety — worth it.

3. **Add `Provenance` to IR nodes (§1.4) before any language with macros/codegen
   (Rust in M4, C/C++/C# later).** *Accuracy:* this is the single highest-value
   defense against the FP class that has historically sunk C/C++ analyzers. Cheap to
   add (one enum on nodes), enforced once in the engine. Add it in M3 even though the
   first consumer is M4, because retrofitting provenance after rules exist is error-
   prone.

4. **Promote visibility + heritage to Core (superset visibility enum; `Extends`/
   `Implements` edge kinds alongside the existing `TypeRef`).** *Maintainability:*
   cross-language layer/inheritance rules need one uniform model; the existing
   `InterfaceData.extends` already proves the "heritage is separate from member refs"
   instinct — generalize it rather than re-derive per language.

5. **Interning + workspace symbol index (§6) — schedule for M4/M5, not M3.**
   *Performance:* essential at monorepo scale, but premature before multiple large
   frontends exist to justify the memory-model complexity. Adding it too early
   violates "avoid unnecessary complexity." Gate it behind a real large-repo fixture
   + DHAT profiling (already on the project's backlog).

6. **Fill the five stub crates (`architecture`, `deps`, `performance`, `plugin`,
   `security`) *only as their rule families land*, not preemptively.** They are
   correctly empty today. `architecture`/`deps` get bodies in M3 (layer/dep-violation
   rules), `security`/`performance` in M4–M5 (data-flow taint, N+1), `plugin` at v0.5+
   (WASM/Extism reading IR read-only). Empty crates are fine; speculative crate
   contents are debt.

### 8.3 What to explicitly *not* do (avoid unnecessary complexity)

- **Do not build a universal type system / type lattice.** It's a multi-year FP
  factory. Record declared/annotated types + resolved-symbol facts; infer only what
  each language's frontend can soundly infer.
- **Do not attempt macro/template expansion in v1.0.** Mark provenance, suppress,
  and say so in evidence. Honest under-coverage beats confident wrong answers.
- **Do not merge framework knowledge into Core IR.** Detectors stay read-only side
  models. The moment a `next/*` concept leaks into `SymbolKind`, the shared model is
  compromised.
- **Do not add languages faster than the §2.1 gate allows.** The gate is the
  product. A 6th mediocre language is worth less than a 5th excellent one.

### 8.4 One-paragraph verdict

The current architecture is already on the correct trajectory — the IR wedge,
conservative resolution, deterministic caching, and additive compat are precisely
the decisions the Clang/rust-analyzer/Semgrep lineage would endorse. The evolution
to 12 languages is **not** a rewrite; it is (a) extracting the `LanguageFrontend`
trait, (b) making analysis stages + rule requirements explicit, (c) adding provenance
and a superset visibility/heritage model to Core IR, and (d) building each frontend
behind the same quality gate TypeScript already passed. Sequence the languages by
resolvability (Python → Go/Rust/Java → C#/Dart → C/C++), pair easy languages with
hard analysis stages to de-risk the engine, and let the quality gate — never the
grammar availability — set the pace. That is how Atlas becomes the highest-quality
semantic analysis engine for the languages that matter, rather than the one that
supports the most languages soonest.
