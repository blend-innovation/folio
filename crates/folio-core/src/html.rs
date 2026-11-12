//! HTML escaping and small text helpers.
//!
//! Everything that reaches the DOM passes through here. Escaping is
//! deliberately unconditional: raw HTML in Markdown is never executed.

use std::borrow::Cow;

/// Escape text content (`&`, `<`, `>`).
pub fn escape_html(s: &str) -> Cow<'_, str> {
    if !s.as_bytes().iter().any(|b| matches!(b, b'&' | b'<' | b'>')) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 16);
    push_escaped(&mut out, s);
    Cow::Owned(out)
}

/// Append escaped text content to `out`.
pub fn push_escaped(out: &mut String, s: &str) {
    let bytes = s.as_bytes();
    let mut start = 0;
    for (i, b) in bytes.iter().enumerate() {
        let replacement = match b {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            _ => continue,
        };
        // The input is valid UTF-8 and these are ASCII bytes, so the
        // boundaries are always safe.
        out.push_str(&s[start..i]);
        out.push_str(replacement);
        start = i + 1;
    }
    out.push_str(&s[start..]);
}

/// Append a value escaped for a double-quoted HTML attribute.
pub fn push_escaped_attr(out: &mut String, s: &str) {
    let bytes = s.as_bytes();
    let mut start = 0;
    for (i, b) in bytes.iter().enumerate() {
        let replacement = match b {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'"' => "&quot;",
            b'\'' => "&#39;",
            b'\n' => "&#10;",
            b'\r' => "&#13;",
            b'\t' => "&#9;",
            _ => continue,
        };
        out.push_str(&s[start..i]);
        out.push_str(replacement);
        start = i + 1;
    }
    out.push_str(&s[start..]);
}

/// GitHub-style heading slug: lowercase, letters/numbers kept, punctuation
/// dropped, whitespace runs collapsed into single hyphens.
pub fn slugify(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    let mut pending_dash = false;
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            pending_dash = false;
            for lower in ch.to_lowercase() {
                slug.push(lower);
            }
        } else if ch == '-' || ch.is_whitespace() {
            pending_dash = true;
        } else if ch == '_' {
            pending_dash = false;
            slug.push('_');
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "section".to_string()
    } else {
        slug
    }
}

/// Truncate on a character boundary for descriptions.
pub fn truncate(text: &str, max_chars: usize) -> String {
    let mut out = String::with_capacity(text.len().min(max_chars * 2));
    for (i, ch) in text.chars().enumerate() {
        if i >= max_chars {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out.trim().to_string()
}

/// Count words in prose. A word is a whitespace-delimited token containing at
/// least one alphanumeric character (so "—" and "```" are not words).
pub fn count_words(text: &str) -> usize {
    text.split_whitespace()
        .filter(|token| token.chars().any(char::is_alphanumeric))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_text_and_attributes() {
        let mut out = String::new();
        push_escaped(&mut out, "a & b < c > d \"q\"");
        assert_eq!(out, "a &amp; b &lt; c &gt; d \"q\"");

        out.clear();
        push_escaped_attr(&mut out, "\"'<&>\n");
        assert_eq!(out, "&quot;&#39;&lt;&amp;&gt;&#10;");
    }

    #[test]
    fn escapes_unicode_without_splitting() {
        let mut out = String::new();
        push_escaped(&mut out, "héllo <世界> & 🍃");
        assert_eq!(out, "héllo &lt;世界&gt; &amp; 🍃");
    }

    #[test]
    fn slugs_headings() {
        assert_eq!(slugify("Hello, World!"), "hello-world");
        assert_eq!(slugify("  Multiple   spaces  "), "multiple-spaces");
        assert_eq!(slugify("C++ & Rust"), "c-rust");
        assert_eq!(slugify("Café — Notes"), "café-notes");
        assert_eq!(slugify("笔记 / 第二章"), "笔记-第二章");
        assert_eq!(slugify("snake_case"), "snake_case");
        assert_eq!(slugify("!!!"), "section");
        assert_eq!(slugify("---"), "section");
        assert_eq!(slugify("A — B"), "a-b");
    }

    #[test]
    fn truncates_on_char_boundaries() {
        assert_eq!(truncate("abcdef", 3), "abc…");
        assert_eq!(truncate("ab", 3), "ab");
        assert_eq!(truncate("笔记", 1), "笔…");
    }

    #[test]
    fn counts_words() {
        assert_eq!(count_words("hello world"), 2);
        assert_eq!(count_words("hello,  world! -- 123 🍃"), 3);
        assert_eq!(count_words("    \n\t "), 0);
    }
}
