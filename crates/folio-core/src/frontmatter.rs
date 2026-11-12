//! Frontmatter parsing.
//!
//! Agent-authored Markdown frequently starts with a small YAML block. We parse
//! the handful of fields a reader benefits from and remove the block from the
//! document instead of rendering it as a horizontal rule.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrontMatter {
    pub title: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub date: Option<String>,
    pub tags: Vec<String>,
}

/// Split a document into frontmatter and body. The body borrows from `src`.
pub fn parse_frontmatter(src: &str) -> (FrontMatter, &str) {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut lines = LineCursor::new(src);

    let Some(first) = lines.peek_line() else {
        return (FrontMatter::default(), src);
    };
    if !is_fence(first, "---") {
        return (FrontMatter::default(), src);
    }
    lines.next_line();

    let start = lines.offset();
    let mut end = None;
    while let Some((line, line_start, next)) = lines.next_line() {
        if is_fence(line, "---") || is_fence(line, "...") {
            end = Some((line_start, next));
            break;
        }
    }
    let Some((end_start, body_start)) = end else {
        // An unterminated fence is not frontmatter.
        return (FrontMatter::default(), src);
    };

    let block = &src[start..end_start];
    (parse_fields(block), &src[body_start..])
}

fn is_fence(line: &str, fence: &str) -> bool {
    line.trim_end() == fence
}

fn parse_fields(block: &str) -> FrontMatter {
    let mut fm = FrontMatter::default();
    let mut lines = block.lines().peekable();
    while let Some(raw) = lines.next() {
        let line = raw.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if line.starts_with([' ', '\t', '-']) {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        match key.as_str() {
            "title" => fm.title = scalar(value),
            "description" | "summary" | "excerpt" => fm.description = scalar(value),
            "author" | "byline" => fm.author = scalar(value),
            "date" | "created" | "updated" => fm.date = scalar(value),
            "tags" | "keywords" => {
                let mut tags = parse_list(value);
                // YAML block lists:
                //   tags:
                //     - one
                //     - two
                while let Some((next, _)) = lines.peek().map(|l| (*l, 0)) {
                    let item = next.trim_start();
                    if let Some(tag) = item.strip_prefix("- ") {
                        if next.starts_with([' ', '\t']) {
                            if let Some(tag) = scalar(tag.trim()) {
                                tags.push(tag);
                            }
                            lines.next();
                            continue;
                        }
                    }
                    break;
                }
                fm.tags = tags;
            }
            _ => {}
        }
    }
    fm
}

fn scalar(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value == "~" || value.eq_ignore_ascii_case("null") {
        return None;
    }
    let value = strip_quotes(value);
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn strip_quotes(value: &str) -> &str {
    for quote in ['"', '\''] {
        if value.len() >= 2 && value.starts_with(quote) && value.ends_with(quote) {
            return &value[1..value.len() - 1];
        }
    }
    value
}

fn parse_list(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .unwrap_or(trimmed);
    inner
        .split(',')
        .filter_map(|item| scalar(item.trim()))
        .collect()
}

struct LineCursor<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> LineCursor<'a> {
    fn new(src: &'a str) -> Self {
        Self { src, pos: 0 }
    }

    fn offset(&self) -> usize {
        self.pos
    }

    fn peek_line(&self) -> Option<&'a str> {
        if self.pos >= self.src.len() {
            return None;
        }
        let rest = &self.src[self.pos..];
        Some(match rest.find('\n') {
            Some(i) => &rest[..i],
            None => rest,
        })
    }

    fn next_line(&mut self) -> Option<(&'a str, usize, usize)> {
        if self.pos >= self.src.len() {
            return None;
        }
        let start = self.pos;
        let rest = &self.src[start..];
        let (line, next) = match rest.find('\n') {
            Some(i) => (&rest[..i], start + i + 1),
            None => (rest, self.src.len()),
        };
        self.pos = next;
        Some((line, start, next))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_fields() {
        let src = "---\ntitle: \"A quiet document\"\nauthor: Ada\ndate: 2026-01-02\ntags: [design, writing]\ndescription: Notes on reading.\n---\n\n# Heading\n";
        let (fm, body) = parse_frontmatter(src);
        assert_eq!(fm.title.as_deref(), Some("A quiet document"));
        assert_eq!(fm.author.as_deref(), Some("Ada"));
        assert_eq!(fm.date.as_deref(), Some("2026-01-02"));
        assert_eq!(fm.tags, ["design", "writing"]);
        assert_eq!(fm.description.as_deref(), Some("Notes on reading."));
        assert_eq!(body, "\n# Heading\n");
    }

    #[test]
    fn parses_block_tags_and_single_quotes() {
        let src = "---\ntitle: 'Hi'\ntags:\n  - one\n  - 'two'\n---\nBody";
        let (fm, body) = parse_frontmatter(src);
        assert_eq!(fm.title.as_deref(), Some("Hi"));
        assert_eq!(fm.tags, ["one", "two"]);
        assert_eq!(body, "Body");
    }

    #[test]
    fn leaves_unterminated_or_absent_fences_alone() {
        let src = "---\ntitle: Nope\n\n# Still body\n";
        let (fm, body) = parse_frontmatter(src);
        assert_eq!(fm, FrontMatter::default());
        assert_eq!(body, src);

        let src = "# Title\n\n---\ntitle: No\n---\n";
        let (fm, body) = parse_frontmatter(src);
        assert_eq!(fm, FrontMatter::default());
        assert_eq!(body, src);
    }

    #[test]
    fn handles_empty_and_null_values() {
        let src = "---\ntitle:\nauthor: null\ndescription: ~\ntags: []\n---\nx";
        let (fm, _) = parse_frontmatter(src);
        assert!(fm.title.is_none());
        assert!(fm.author.is_none());
        assert!(fm.description.is_none());
        assert!(fm.tags.is_empty());
    }

    #[test]
    fn colons_inside_values_survive() {
        let src = "---\ntitle: Reading: a note\ndescription: https://example.com/x\n---\n";
        let (fm, _) = parse_frontmatter(src);
        assert_eq!(fm.title.as_deref(), Some("Reading: a note"));
        assert_eq!(fm.description.as_deref(), Some("https://example.com/x"));
    }
}
