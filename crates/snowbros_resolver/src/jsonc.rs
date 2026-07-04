//! Minimal JSONC preprocessor: strips `//` and `/* */` comments and
//! trailing commas so `tsconfig.json` (which is JSONC in practice) can be
//! parsed with a strict JSON parser.
//!
//! String-aware: comment markers inside string literals are preserved.

/// Converts JSONC text to strict JSON.
pub fn to_json(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                // Copy the entire string literal, honoring escapes.
                out.push('"');
                i += 1;
                while i < bytes.len() {
                    let c = bytes[i];
                    out.push(c as char);
                    i += 1;
                    if c == b'\\' && i < bytes.len() {
                        out.push(bytes[i] as char);
                        i += 1;
                    } else if c == b'"' {
                        break;
                    }
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                // Line comment: skip to end of line (keep the newline).
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                // Block comment: skip past `*/`.
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
            }
            b',' => {
                // Trailing comma: drop if the next non-whitespace,
                // non-comment char closes an object/array.
                if closes_next(bytes, i + 1) {
                    i += 1;
                } else {
                    out.push(',');
                    i += 1;
                }
            }
            c => {
                out.push(c as char);
                i += 1;
            }
        }
    }
    out
}

/// Whether the next significant character at/after `pos` is `}` or `]`.
fn closes_next(bytes: &[u8], mut pos: usize) -> bool {
    while pos < bytes.len() {
        match bytes[pos] {
            b' ' | b'\t' | b'\r' | b'\n' => pos += 1,
            b'/' if pos + 1 < bytes.len() && bytes[pos + 1] == b'/' => {
                while pos < bytes.len() && bytes[pos] != b'\n' {
                    pos += 1;
                }
            }
            b'/' if pos + 1 < bytes.len() && bytes[pos + 1] == b'*' => {
                pos += 2;
                while pos + 1 < bytes.len() && !(bytes[pos] == b'*' && bytes[pos + 1] == b'/') {
                    pos += 1;
                }
                pos = (pos + 2).min(bytes.len());
            }
            b'}' | b']' => return true,
            _ => return false,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_line_and_block_comments() {
        let jsonc = r#"{
  // line comment
  "a": 1, /* block */ "b": 2
}"#;
        let parsed: serde_json::Value = serde_json::from_str(&to_json(jsonc)).unwrap();
        assert_eq!(parsed["a"], 1);
        assert_eq!(parsed["b"], 2);
    }

    #[test]
    fn strips_trailing_commas() {
        let jsonc = r#"{ "arr": [1, 2, 3,], "obj": { "x": 1, }, }"#;
        let parsed: serde_json::Value = serde_json::from_str(&to_json(jsonc)).unwrap();
        assert_eq!(parsed["arr"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn preserves_slashes_and_commas_in_strings() {
        let jsonc = r#"{ "url": "https://x.dev/a,b", "path": "c:\\dir" }"#;
        let parsed: serde_json::Value = serde_json::from_str(&to_json(jsonc)).unwrap();
        assert_eq!(parsed["url"], "https://x.dev/a,b");
        assert_eq!(parsed["path"], "c:\\dir");
    }

    #[test]
    fn plain_json_passes_through() {
        let json = r#"{"a":[1,2],"b":{"c":true}}"#;
        assert_eq!(to_json(json), json);
    }
}
