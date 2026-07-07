//! Fix planning and application for `snowbros fix`.
//!
//! Design:
//! - **Planning is pure**: diagnostics in, [`PlannedFix`]es out. Only
//!   deterministic edits are planned; anything else is counted as
//!   not auto-fixable.
//! - **Application is guarded**: every edit verifies its expectation
//!   against the current file content (line still declares the
//!   variable, dependency still present, span bytes unchanged) and is
//!   skipped — never guessed — when the file has drifted.
//! - **Formatting-preserving**: edits are textual surgery on the
//!   original bytes (line removal, byte-range replacement); files are
//!   never reformatted or reserialized.
//! - **Idempotent**: applied fixes remove the finding, so a second run
//!   plans nothing.
//!
//! Adding a fixer: emit a `SuggestedFix` (with `target` for
//! rule-specific fixes) from the rule, then map the rule id to an
//! [`Edit`] in [`plan`].

use std::collections::HashMap;
use std::fs;

use camino::{Utf8Path, Utf8PathBuf};

use snowbros_core::Diagnostic;

/// One concrete edit to one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// Replace a byte range with new text (generic span fix).
    ReplaceBytes {
        /// Start byte (inclusive).
        start: usize,
        /// End byte (exclusive).
        end: usize,
        /// Text the diagnostic's span expects to find there — the edit
        /// is skipped if the file drifted.
        expect: Option<String>,
        /// Replacement text.
        replacement: String,
    },
    /// Delete one whole line (1-based). Skipped unless the line still
    /// contains `expect`.
    DeleteLine {
        /// 1-based line number.
        line: u32,
        /// Substring the line must contain.
        expect: String,
    },
    /// Remove one entry from the `dependencies` object of a
    /// package.json, preserving all surrounding formatting.
    RemoveDependency {
        /// Package name.
        name: String,
    },
}

/// A fix ready to apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFix {
    /// File the edit applies to (root-relative).
    pub file: Utf8PathBuf,
    /// Rule that produced the fix.
    pub rule_id: String,
    /// Human description (from the diagnostic's suggested fix).
    pub description: String,
    /// The edit itself.
    pub edit: Edit,
}

/// Result of planning: fixes plus the count of findings with no
/// deterministic fix.
#[derive(Debug, Default)]
pub struct Plan {
    /// Fixes to apply, in deterministic order.
    pub fixes: Vec<PlannedFix>,
    /// Findings that have no auto-fix.
    pub unfixable: usize,
}

/// Plans fixes for a set of diagnostics.
///
/// `root` is used to read the current span bytes for generic
/// replacements, so application can verify the file has not drifted
/// since analysis.
pub fn plan(root: &Utf8Path, diagnostics: &[Diagnostic]) -> Plan {
    let mut plan = Plan::default();
    let mut contents: HashMap<&Utf8PathBuf, Option<String>> = HashMap::new();
    for d in diagnostics {
        let Some(fix) = &d.suggested_fix else {
            plan.unfixable += 1;
            continue;
        };
        let edit = match (d.rule_id.as_str(), &fix.target, &fix.replacement) {
            ("env/unused-env-var", Some(name), _) => Some(Edit::DeleteLine {
                line: d.location.span.start.line,
                expect: name.clone(),
            }),
            ("deps/unused-dependency", Some(name), _) => {
                Some(Edit::RemoveDependency { name: name.clone() })
            }
            // Generic path: a span substitution with a real byte range.
            (_, _, Some(replacement)) if d.location.span.end_byte > d.location.span.start_byte => {
                let start = d.location.span.start_byte as usize;
                let end = d.location.span.end_byte as usize;
                // Capture the current span bytes so application can
                // verify the file has not drifted since analysis.
                let expect = contents
                    .entry(&d.location.file)
                    .or_insert_with(|| fs::read_to_string(root.join(&d.location.file)).ok())
                    .as_deref()
                    .and_then(|text| {
                        (end <= text.len()
                            && text.is_char_boundary(start)
                            && text.is_char_boundary(end))
                        .then(|| text[start..end].to_string())
                    });
                Some(Edit::ReplaceBytes {
                    start,
                    end,
                    expect,
                    replacement: replacement.clone(),
                })
            }
            _ => None,
        };
        match edit {
            Some(edit) => plan.fixes.push(PlannedFix {
                file: d.location.file.clone(),
                rule_id: d.rule_id.clone(),
                description: fix.description.clone(),
                edit,
            }),
            None => plan.unfixable += 1,
        }
    }
    // Deterministic application order: by file, then bottom-up within
    // the file so earlier edits never shift later positions.
    plan.fixes.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then_with(|| edit_pos(&b.edit).cmp(&edit_pos(&a.edit)))
    });
    plan
}

/// Position key for bottom-up ordering within a file.
fn edit_pos(edit: &Edit) -> (u32, usize) {
    match edit {
        Edit::ReplaceBytes { start, .. } => (u32::MAX, *start),
        Edit::DeleteLine { line, .. } => (*line, 0),
        Edit::RemoveDependency { .. } => (0, 0),
    }
}

/// Outcome of applying a plan.
#[derive(Debug, Default)]
pub struct ApplyOutcome {
    /// Number of fixes written.
    pub applied: usize,
    /// Files that changed.
    pub files_changed: Vec<Utf8PathBuf>,
    /// Fixes skipped, with reasons.
    pub skipped: Vec<(PlannedFix, String)>,
}

/// Applies planned fixes under `root`. With `dry_run`, nothing is
/// written but the outcome reflects what would happen.
pub fn apply(root: &Utf8Path, fixes: &[PlannedFix], dry_run: bool) -> ApplyOutcome {
    let mut outcome = ApplyOutcome::default();
    let mut i = 0;
    while i < fixes.len() {
        // Take the run of fixes for one file.
        let file = &fixes[i].file;
        let mut j = i;
        while j < fixes.len() && &fixes[j].file == file {
            j += 1;
        }
        let batch = &fixes[i..j];
        i = j;

        let abs = root.join(file);
        let Ok(original) = fs::read_to_string(&abs) else {
            for fix in batch {
                outcome
                    .skipped
                    .push((fix.clone(), "file unreadable".to_string()));
            }
            continue;
        };

        let mut text = original.clone();
        let mut applied_here: Vec<&PlannedFix> = Vec::new();
        for fix in batch {
            match apply_edit(&text, &fix.edit) {
                Ok(new_text) => {
                    text = new_text;
                    applied_here.push(fix);
                    outcome.applied += 1;
                }
                Err(reason) => outcome.skipped.push((fix.clone(), reason)),
            }
        }

        if !applied_here.is_empty() && text != original {
            if !dry_run && fs::write(&abs, &text).is_err() {
                // Roll the counters back: nothing was persisted. Only
                // the fixes that were applied in memory become skips;
                // ones that already failed their guard keep their
                // original skip reason.
                outcome.applied -= applied_here.len();
                for fix in applied_here {
                    outcome
                        .skipped
                        .push((fix.clone(), "write failed".to_string()));
                }
                continue;
            }
            outcome.files_changed.push(file.clone());
        }
    }
    outcome
}

/// Applies one edit to text, or explains why it cannot be applied.
fn apply_edit(text: &str, edit: &Edit) -> Result<String, String> {
    match edit {
        Edit::ReplaceBytes {
            start,
            end,
            expect,
            replacement,
        } => {
            if *end > text.len() || !text.is_char_boundary(*start) || !text.is_char_boundary(*end) {
                return Err("span out of bounds (file changed since analysis)".to_string());
            }
            if let Some(expect) = expect {
                if &text[*start..*end] != expect {
                    return Err("span content changed since analysis".to_string());
                }
            }
            let mut out = String::with_capacity(text.len());
            out.push_str(&text[..*start]);
            out.push_str(replacement);
            out.push_str(&text[*end..]);
            Ok(out)
        }
        Edit::DeleteLine { line, expect } => {
            let lines: Vec<&str> = text.split_inclusive('\n').collect();
            let idx = (*line as usize).checked_sub(1).ok_or("invalid line")?;
            let Some(content) = lines.get(idx) else {
                return Err("line no longer exists".to_string());
            };
            if !content.contains(expect.as_str()) {
                return Err(format!("line {line} no longer declares `{expect}`"));
            }
            let mut out = String::with_capacity(text.len());
            for (k, l) in lines.iter().enumerate() {
                if k != idx {
                    out.push_str(l);
                }
            }
            Ok(out)
        }
        Edit::RemoveDependency { name } => remove_dependency(text, name)
            .ok_or_else(|| format!("`{name}` not found in dependencies")),
    }
}

/// Removes `"name": "…"` from the `dependencies` object, preserving all
/// other bytes. Handles the trailing comma when the removed entry was
/// the last one.
fn remove_dependency(text: &str, name: &str) -> Option<String> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let dep_open = lines.iter().position(|l| l.contains("\"dependencies\""))?;

    // Find the closing brace of the dependencies object.
    let mut depth: i32 = 0;
    let mut dep_close = None;
    for (i, line) in lines.iter().enumerate().skip(dep_open) {
        for ch in line.chars() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        dep_close = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        if dep_close.is_some() {
            break;
        }
    }
    let dep_close = dep_close?;

    let needle = format!("\"{name}\"");
    let target = (dep_open + 1..dep_close).find(|&i| lines[i].trim_start().starts_with(&needle))?;

    let removed_had_comma = lines[target].trim_end().ends_with(',');
    let mut out: Vec<String> = lines.iter().map(|s| (*s).to_string()).collect();
    out.remove(target);

    if !removed_had_comma && target > dep_open + 1 {
        // Removed the last entry: the previous entry's trailing comma
        // must go too, or the JSON breaks.
        let prev = &mut out[target - 1];
        if let Some(pos) = prev.rfind(',') {
            if prev[pos + 1..].trim().is_empty() {
                prev.replace_range(pos..=pos, "");
            }
        }
    }
    Some(out.concat())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PKG: &str = "{\n  \"name\": \"demo\",\n  \"dependencies\": {\n    \"lodash\": \"^4.17.21\",\n    \"react\": \"^19.0.0\",\n    \"zod\": \"^3.23.0\"\n  },\n  \"devDependencies\": {\n    \"lodash\": \"^4.17.21\"\n  }\n}\n";

    #[test]
    fn remove_middle_dependency_preserves_rest() {
        let out = remove_dependency(PKG, "react").unwrap();
        assert!(!out.contains("\"react\""));
        assert!(out.contains("\"lodash\": \"^4.17.21\","));
        assert!(out.contains("\"zod\": \"^3.23.0\"\n"));
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["dependencies"].as_object().unwrap().len(), 2);
    }

    #[test]
    fn remove_last_dependency_strips_previous_comma() {
        let out = remove_dependency(PKG, "zod").unwrap();
        assert!(!out.contains("\"zod\""));
        // react no longer has a trailing comma.
        assert!(out.contains("\"react\": \"^19.0.0\"\n"));
        serde_json::from_str::<serde_json::Value>(&out).unwrap();
    }

    #[test]
    fn remove_only_dependency_leaves_empty_object() {
        let pkg = "{\n  \"dependencies\": {\n    \"lodash\": \"^4.17.21\"\n  }\n}\n";
        let out = remove_dependency(pkg, "lodash").unwrap();
        serde_json::from_str::<serde_json::Value>(&out).unwrap();
        assert!(out.contains("\"dependencies\""));
    }

    #[test]
    fn only_touches_dependencies_section() {
        // lodash also in devDependencies — must survive.
        let out = remove_dependency(PKG, "lodash").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(parsed["dependencies"].get("lodash").is_none());
        assert!(parsed["devDependencies"].get("lodash").is_some());
    }

    #[test]
    fn missing_dependency_is_none() {
        assert!(remove_dependency(PKG, "notthere").is_none());
    }

    #[test]
    fn delete_line_with_guard() {
        let env = "API_KEY=a\nGHOST=b\nLAST=c\n";
        let out = apply_edit(
            env,
            &Edit::DeleteLine {
                line: 2,
                expect: "GHOST".into(),
            },
        )
        .unwrap();
        assert_eq!(out, "API_KEY=a\nLAST=c\n");

        // Drifted file: guard refuses.
        let err = apply_edit(
            env,
            &Edit::DeleteLine {
                line: 1,
                expect: "GHOST".into(),
            },
        )
        .unwrap_err();
        assert!(err.contains("no longer declares"));
    }

    #[test]
    fn replace_bytes_respects_bounds() {
        let src = "const x = eval(a);";
        let out = apply_edit(
            src,
            &Edit::ReplaceBytes {
                start: 10,
                end: 17,
                expect: Some("eval(a)".into()),
                replacement: "JSON.parse(a)".into(),
            },
        )
        .unwrap();
        assert_eq!(out, "const x = JSON.parse(a);");

        let err = apply_edit(
            src,
            &Edit::ReplaceBytes {
                start: 10,
                end: 17,
                expect: Some("other()".into()),
                replacement: "x".into(),
            },
        )
        .unwrap_err();
        assert!(err.contains("changed since analysis"));
    }

    #[test]
    fn plan_captures_span_bytes_as_guard() {
        use snowbros_core::{Confidence, Severity, SourceLocation, SuggestedFix};
        use snowbros_core::{Position, Span};

        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap();
        fs::write(root.join("a.ts"), "const x = eval(a);").unwrap();

        let diag = Diagnostic::new(
            "security/no-eval",
            "t",
            "m",
            "security",
            Severity::Critical,
            Confidence::Certain,
            SourceLocation::new(
                "a.ts",
                Span::new(Position::new(1, 11), Position::new(1, 18), 10, 17),
            ),
        )
        .with_fix(SuggestedFix {
            description: "d".into(),
            replacement: Some("JSON.parse(a)".into()),
            target: None,
        });

        let plan = plan(root, &[diag]);
        assert_eq!(plan.fixes.len(), 1);
        match &plan.fixes[0].edit {
            Edit::ReplaceBytes { expect, .. } => {
                assert_eq!(expect.as_deref(), Some("eval(a)"));
            }
            other => panic!("expected ReplaceBytes, got {other:?}"),
        }

        // Drift the file: application must refuse the edit.
        fs::write(root.join("a.ts"), "const x = safe(a);").unwrap();
        let outcome = apply(root, &plan.fixes, false);
        assert_eq!(outcome.applied, 0);
        assert_eq!(outcome.skipped.len(), 1);
        assert!(outcome.skipped[0].1.contains("changed since analysis"));
    }

    #[test]
    fn idempotent_line_delete() {
        let env = "GHOST=b\n";
        let once = apply_edit(
            env,
            &Edit::DeleteLine {
                line: 1,
                expect: "GHOST".into(),
            },
        )
        .unwrap();
        // Second application fails the guard instead of deleting the
        // wrong line.
        assert!(apply_edit(
            &once,
            &Edit::DeleteLine {
                line: 1,
                expect: "GHOST".into(),
            },
        )
        .is_err());
    }
}
