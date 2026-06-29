//! Doc-comment rendering: shift embedded headings beneath the item's own
//! heading and strip hidden doctest lines, with a light line-based pass.

/// Render a doc comment for inclusion beneath a title at heading level
/// `base_level`. Embedded headings are shifted down by `base_level` so they nest
/// correctly; hidden doctest lines (`# ...`) inside code blocks are removed and
/// escaped `##` lines are unescaped.
pub fn render_docs(docs: &str, base_level: usize) -> String {
    let mut out = String::new();
    let mut fence: Option<String> = None;

    for line in docs.lines() {
        let trimmed = line.trim_start();

        match &fence {
            // Inside a fenced code block.
            Some(marker) => {
                if trimmed.starts_with(marker.as_str()) {
                    fence = None;
                    push_line(&mut out, line);
                } else if let Some(unescaped) = unescape_doctest(line) {
                    push_line(&mut out, &unescaped);
                } else if !is_hidden_doctest(line) {
                    push_line(&mut out, line);
                }
            }
            // Normal prose.
            None => {
                if let Some(marker) = opening_fence(trimmed) {
                    fence = Some(marker);
                    push_line(&mut out, line);
                } else if let Some((level, rest)) = heading(trimmed) {
                    let shifted = (level + base_level).min(6);
                    push_line(&mut out, &format!("{} {rest}", "#".repeat(shifted)));
                } else {
                    push_line(&mut out, line);
                }
            }
        }
    }

    out.trim_end().to_string()
}

fn push_line(out: &mut String, line: &str) {
    out.push_str(line);
    out.push('\n');
}

/// The fence marker (```` ``` ```` or `~~~`, possibly longer) if `trimmed` opens
/// a code block, else `None`.
fn opening_fence(trimmed: &str) -> Option<String> {
    for ch in ['`', '~'] {
        let count = trimmed.chars().take_while(|&c| c == ch).count();
        if count >= 3 {
            return Some(ch.to_string().repeat(count));
        }
    }
    None
}

/// A hidden doctest line is `#` alone or `#` followed by a space (rustdoc hides
/// these). `#[attr]` is not hidden.
fn is_hidden_doctest(line: &str) -> bool {
    let t = line.trim_start();
    t == "#" || t.starts_with("# ")
}

/// `## ...` inside a doctest is an escaped line that should display with one `#`
/// removed. Returns the unescaped line if applicable.
fn unescape_doctest(line: &str) -> Option<String> {
    let indent_len = line.len() - line.trim_start().len();
    let (indent, t) = line.split_at(indent_len);
    if t.starts_with("##") {
        Some(format!("{indent}{}", &t[1..]))
    } else {
        None
    }
}

/// If `trimmed` is an ATX heading (`#`..`######` then a space), return its level
/// and the text after the hashes.
fn heading(trimmed: &str) -> Option<(usize, &str)> {
    let level = trimmed.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&level) && trimmed[level..].starts_with(' ') {
        Some((level, trimmed[level + 1..].trim_start()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_passes_through() {
        assert_eq!(render_docs("Hello world.", 1), "Hello world.");
    }

    #[test]
    fn headings_are_shifted() {
        let docs = "# Examples\nsome text";
        // Item title is an h1, so an embedded h1 becomes h2.
        assert_eq!(render_docs(docs, 1), "## Examples\nsome text");
    }

    #[test]
    fn headings_clamp_at_six() {
        assert_eq!(render_docs("###### Deep", 3), "###### Deep");
    }

    #[test]
    fn hidden_doctest_lines_stripped() {
        let docs = "```\n# use std::io;\nlet x = 1;\n```";
        assert_eq!(render_docs(docs, 1), "```\nlet x = 1;\n```");
    }

    #[test]
    fn attributes_in_code_are_kept() {
        let docs = "```\n#[derive(Debug)]\nstruct S;\n```";
        assert_eq!(render_docs(docs, 1), "```\n#[derive(Debug)]\nstruct S;\n```");
    }

    #[test]
    fn escaped_hash_is_unescaped() {
        let docs = "```\n## not hidden\n```";
        assert_eq!(render_docs(docs, 1), "```\n# not hidden\n```");
    }

    #[test]
    fn heading_inside_code_is_not_shifted() {
        let docs = "```\n# hidden\nlet a = 1;\n```";
        // The `# hidden` is a hidden doctest line, not a heading.
        assert_eq!(render_docs(docs, 2), "```\nlet a = 1;\n```");
    }
}
