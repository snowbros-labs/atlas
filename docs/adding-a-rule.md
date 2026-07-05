# Adding a rule

This guide walks through adding a new detection to Snowbros Atlas end to end:
its metadata, its implementation, its tests, and the correctness bar every rule
must clear before it ships.

Rules are the heart of the tool, and they are the most common external
contribution. They are also held to a deliberately high standard — a rule that
fires incorrectly is worse than no rule at all, because it erodes trust in every
other finding. Read the [correctness bar](#the-correctness-bar) before you
start; it is not optional.

## Table of contents

- [Mental model](#mental-model)
- [Anatomy of a rule](#anatomy-of-a-rule)
- [Step 1 — write the metadata](#step-1--write-the-metadata)
- [Step 2 — implement the detector](#step-2--implement-the-detector)
- [Step 3 — attach evidence](#step-3--attach-evidence)
- [Step 4 — write the tests](#step-4--write-the-tests)
- [Step 5 — (optional) add a fixer](#step-5-optional-add-a-fixer)
- [The correctness bar](#the-correctness-bar)
- [Checklist before you open a PR](#checklist-before-you-open-a-pr)

## Mental model

Snowbros Atlas behaves like a compiler for engineering issues: **the same
codebase and config in produces the same findings out, every time.** A rule is a
pure function from the analyzed project (files, extracted facts, the import
graph, framework detection) to a set of diagnostics. It must not read the clock,
the network, the environment, or anything else that would make two runs on the
same input disagree.

A rule reports a finding **only when it can prove it** from evidence already in
the analysis. Anything it cannot prove is either not reported or reported at a
lower confidence — never guessed.

## Anatomy of a rule

Every rule is two things that must stay in sync:

| Part | Location | Purpose |
|---|---|---|
| Metadata | `crates/snowbros_rules/rules/<category>/<name>.toml` | Identity, docs, severity, confidence, false-positive guards. Embedded at compile time. |
| Detector | `crates/snowbros_rules/src/...` | The code that inspects the analysis and emits diagnostics. |

The test harness enforces a **1:1 mapping** between rule IDs and metadata files
and checks metadata completeness. A detector without metadata (or vice versa)
fails the build — this is intentional.

A rule ID is `category/name`, for example `graph/no-circular-imports`. The
category is the directory the metadata lives in; the name is the file stem.

## Step 1 — write the metadata

Create `crates/snowbros_rules/rules/<category>/<name>.toml`. Use an existing
rule as a template — for example `graph/no-circular-imports.toml`. A metadata
file declares:

```toml
id = "imports/unresolved-import"
title = "Unresolved import"
severity = "medium"          # high | medium | low | info
confidence = "likely"        # certain | likely | possible
summary = "An import specifier could not be resolved to a file or package."

description = """
Longer prose shown by `sb explain imports/unresolved-import`. Explain what the
rule detects and why it matters, in plain language.
"""

# What concretely triggers the finding.
detection = """
Fires when the resolver exhausts relative, extension/index, and tsconfig-path
resolution for an import specifier without finding a target.
"""

# REQUIRED: the false-positive guards. Be specific and honest.
false_positives = """
Package self-imports that rely on `package.json#main` are a known gap and may
report here; scope them out with `disable = ["imports/*"]` until resolved.
"""

# How to fix, or how to silence if it is a false positive.
remediation = """
Correct the path, add the missing tsconfig path mapping, or disable the rule
for the affected glob.
"""
```

Match the field set of the existing files exactly — the harness validates that
every required field is present and non-empty. `severity` and `confidence` must
use the vocabulary from `snowbros_core` (`Severity`, `Confidence`); no other
values are accepted.

Choosing severity and confidence honestly is part of the job:

- **`certain`** — the finding is a provable structural fact (e.g. a cycle the
  graph literally contains). Reserve this; it is a strong claim.
- **`likely`** — strong evidence, but a rare legitimate pattern could explain
  it.
- **`possible`** — a heuristic signal worth surfacing at low severity.

If you are unsure, pick the lower confidence. Under-claiming preserves trust;
over-claiming destroys it.

## Step 2 — implement the detector

Implement the detector in `crates/snowbros_rules`, following the structure of
the rule nearest to yours in category. A detector receives the analysis context
(extracted file facts, the resolved import graph, framework detection) and
returns diagnostics.

Rules to follow while implementing:

- **Read, never mutate.** Detectors are read-only over the analysis. Side
  effects belong in fixers (step 5), not detectors.
- **No nondeterminism.** Do not iterate a `HashMap`/`HashSet` and push results in
  iteration order. Collect into a `Vec`, then **sort** by a stable key (path,
  then span) before emitting. Two runs must produce byte-identical output.
- **No I/O beyond what the pipeline already did.** No reading files off disk mid
  rule, no network, no time, no environment reads.
- **Respect the framework/context signals** already computed rather than
  re-deriving them.

## Step 3 — attach evidence

**Every diagnostic must carry the concrete chain that produced it.** This is not
decoration — it is the contract that lets a user (and a reviewer) verify the
finding without trusting the tool. Depending on the rule, evidence is:

- the import chain (`A.tsx → lib/b.ts → lib/c.ts`),
- the cycle members,
- the config line that enabled the behavior,
- the span of the offending code.

A finding without evidence is not shippable. If you cannot produce evidence for
a positive, the rule is not ready.

**Secrets are always redacted** to their first four characters, in every output
format and in the cache. If your rule can surface a value that might be a
credential, redact it at the point of construction — never let a raw secret
reach a diagnostic.

## Step 4 — write the tests

At minimum, every rule ships with three tests:

1. **Positive case** — input that must fire, asserting on the rule ID, severity,
   and the evidence content (not just that *a* finding exists).
2. **Negative case** — similar-looking input that must **not** fire.
3. **False-positive guard case** — the specific pattern your metadata's
   `false_positives` section promises not to flag. This test is what keeps the
   guard honest over time.

Additional expectations:

- Prefer table-style fixtures next to the existing rule tests.
- If your rule interacts with the cache, the workspace-level determinism test
  (warm output byte-identical to cold) must still pass — do not special-case it.
- Run the full suite: `cargo test --workspace`.

## Step 5 — (optional) add a fixer

If the finding is mechanically fixable, add a fixer in
`crates/snowbros_cli/src/fixers.rs`. Fixers plan edits as byte-span
replacements, then apply them **only when the file still matches what the
analysis saw**. Requirements:

- **Guarded** — skip (never clobber) a file that drifted since analysis.
- **Idempotent** — running `sb fix` twice produces the same result as running it
  once.
- **Format-preserving** — do not reformat the file; touch only what you must.
- **Conservative** — when in doubt, do not fix. A missed fix is a nuisance; a
  wrong fix is a bug in the user's code.

Add a test proving the fix is applied on a matching file and skipped on a
drifted one.

## The correctness bar

A rule is only mergeable if all five hold:

1. **Deterministic** — same code and config in, same findings out. Sorted
   output, no timestamps, no iteration-order leakage, no external reads.
2. **Provable** — the rule reports only what it can demonstrate from analysis
   facts. Unknowns are reported as unknown (e.g. *unresolved*) or not at all,
   never guessed.
3. **Evidence-backed** — every finding carries its producing chain.
4. **Honestly rated** — severity and confidence reflect the real strength of the
   evidence; false-positive guards are documented and tested.
5. **Secret-safe** — any potentially sensitive value is redacted to four
   characters everywhere.

These mirror the ground rules in [CONTRIBUTING.md](../CONTRIBUTING.md); a rule
is the place they are tested hardest.

## Checklist before you open a PR

- [ ] Metadata TOML added under `rules/<category>/<name>.toml`, all required
      fields present, with a real `false_positives` section.
- [ ] Detector implemented in `snowbros_rules`, read-only and deterministic.
- [ ] Every finding carries an evidence chain and an honest confidence.
- [ ] Positive, negative, and false-positive-guard tests added and passing.
- [ ] `sb explain <your-rule-id>` renders correctly.
- [ ] (If applicable) guarded, idempotent fixer added with tests.
- [ ] `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
      and `cargo test --workspace` all pass.
- [ ] README rules table updated if the rule is user-facing.

Thank you for keeping the bar high. Fewer, trustworthy rules beat many noisy
ones — that discipline is the whole product.
