//! Escaping of untrusted SBOM text before it reaches a terminal, a CI log or
//! a Markdown renderer.

use std::fmt::Write as _;

/// Longest value shown in text and Markdown output, in characters. JSON
/// output is not truncated.
pub const MAX_DISPLAY_CHARS: usize = 200;

/// Characters that can change how surrounding text is displayed: C0 and C1
/// controls (including ESC and CSI), DEL, bidirectional overrides and
/// isolates, zero-width characters and line/paragraph separators.
pub fn is_unsafe_char(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{061C}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'..='\u{202E}'
                | '\u{2060}'..='\u{2069}'
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFB}'
        )
}

/// Makes a value safe to print: unsafe characters become `\u{XXXX}`,
/// backslashes are doubled so escapes cannot be forged, and long values
/// are truncated.
pub fn display(value: &str) -> String {
    let mut out = String::with_capacity(value.len().min(MAX_DISPLAY_CHARS * 2));
    for (count, c) in value.chars().enumerate() {
        if count == MAX_DISPLAY_CHARS {
            out.push_str("...");
            break;
        }
        if c == '\\' {
            out.push_str("\\\\");
        } else if is_unsafe_char(c) {
            let _ = write!(out, "\\u{{{:04x}}}", c as u32);
        } else {
            out.push(c);
        }
    }
    out
}

/// Sanitizes a multi-line message from this program (such as a usage
/// error) line by line, keeping its line breaks.
pub fn message(text: &str) -> String {
    text.lines()
        .map(display_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn display_line(line: &str) -> String {
    line.chars()
        .map(|c| {
            if is_unsafe_char(c) && c != '\t' {
                format!("\\u{{{:04x}}}", c as u32)
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// A Markdown code span for a GitHub-flavored table cell. Inside a code
/// span, Markdown, HTML, links and `@mentions` are not interpreted. Pipes
/// are escaped so the value cannot end the table cell.
pub fn markdown_code(value: &str) -> String {
    let text = display(value).replace('|', "\\|");
    let mut longest = 0;
    let mut run = 0;
    for c in text.chars() {
        if c == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let fence = "`".repeat(longest + 1);
    // One space on each side is stripped by the renderer, which lets the
    // value start or end with a backtick.
    format!("{fence} {text} {fence}")
}

/// Escapes characters in serialized JSON that `serde_json` leaves as-is but
/// a terminal could act on (DEL, C1 controls, bidi and zero-width
/// characters). Such characters can only occur inside JSON strings, where
/// `\uXXXX` is an exact equivalent, so the data is unchanged.
pub fn json_output(serialized: &str) -> String {
    let mut out = String::with_capacity(serialized.len());
    for c in serialized.chars() {
        if is_unsafe_char(c) && !matches!(c, '\n' | ' ' | '\t' | '\r') {
            // Everything in the unsafe set is in the Basic Multilingual Plane.
            let _ = write!(out, "\\u{:04x}", c as u32);
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_terminal_controls() {
        assert_eq!(display("a\u{1b}[31mb"), "a\\u{001b}[31mb");
        assert_eq!(display("x\u{9b}y"), "x\\u{009b}y");
        assert_eq!(display("evil\u{202e}txt"), "evil\\u{202e}txt");
        assert_eq!(display("line\nbreak"), "line\\u{000a}break");
        assert_eq!(display("back\\slash"), "back\\\\slash");
    }

    #[test]
    fn truncates_long_values() {
        let long = "a".repeat(500);
        let shown = display(&long);
        assert_eq!(shown.len(), MAX_DISPLAY_CHARS + 3);
        assert!(shown.ends_with("..."));
    }

    #[test]
    fn markdown_code_spans_neutralize_markup() {
        assert_eq!(markdown_code("a|b"), "` a\\|b `");
        assert_eq!(markdown_code("x`y"), "`` x`y ``");
        assert_eq!(markdown_code("@team"), "` @team `");
    }

    #[test]
    fn json_output_escapes_without_changing_data() {
        let value = serde_json::json!({"n": "a\u{7f}b\u{9b}c\u{202e}d\u{1b}e"});
        let escaped = json_output(&serde_json::to_string(&value).unwrap());
        assert!(escaped.is_ascii(), "{escaped}");
        let back: serde_json::Value = serde_json::from_str(&escaped).unwrap();
        assert_eq!(back, value);
    }
}
