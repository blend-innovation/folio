//! Link and image resolution.
//!
//! Markdown is untrusted input: a document may link to `javascript:` URLs or
//! try to reach outside its directory. This module classifies destinations so
//! the renderer can emit safe markup. Local paths are only resolved when the
//! caller opts in (the desktop app does; CLI HTML output does not).

use percent_encoding::percent_decode_str;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalKind {
    Doc,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedLink {
    /// In-document heading anchor, without the leading `#`.
    Anchor(String),
    /// A web, mail, or phone URL that the OS should handle.
    External(String),
    /// A path on disk with the anchor that should be followed after opening.
    Local {
        path: PathBuf,
        anchor: Option<String>,
        kind: LocalKind,
    },
    /// A relative URL left untouched for portable HTML output.
    Relative(String),
    /// An unsafe or malformed destination.
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedImage {
    External(String),
    /// A local image the host should read and inline (data URL).
    LocalPath(PathBuf),
    /// Already self-contained.
    Data(String),
    Relative(String),
    Blocked,
}

const MARKDOWN_EXTS: &[&str] = &["md", "markdown", "mdown", "mkd", "mkdn", "txt", "text"];
const IMAGE_DATA_PREFIXES: &[&str] = &[
    "data:image/png;base64,",
    "data:image/jpeg;base64,",
    "data:image/jpg;base64,",
    "data:image/gif;base64,",
    "data:image/webp;base64,",
    "data:image/avif;base64,",
];
const MAX_INLINE_IMAGE_BYTES: usize = 8 * 1024 * 1024;

pub fn resolve_link(dest: &str, base_dir: Option<&Path>, localize: bool) -> ResolvedLink {
    let dest = dest.trim();
    if dest.is_empty() {
        return ResolvedLink::Blocked;
    }
    if let Some(fragment) = dest.strip_prefix('#') {
        let decoded = decode_component(fragment);
        return if decoded.is_empty() {
            ResolvedLink::Blocked
        } else {
            ResolvedLink::Anchor(decoded)
        };
    }
    if has_control_chars(dest) {
        return ResolvedLink::Blocked;
    }

    if let Some(scheme) = scheme(dest) {
        return match scheme.as_str() {
            "http" | "https" | "ftp" => ResolvedLink::External(dest.to_string()),
            "mailto" | "tel" => ResolvedLink::External(dest.to_string()),
            _ => ResolvedLink::Blocked,
        };
    }

    if dest.starts_with("//") {
        return ResolvedLink::External(format!("https:{dest}"));
    }

    if !localize {
        return ResolvedLink::Relative(dest.to_string());
    }
    let Some(base) = base_dir else {
        return ResolvedLink::Relative(dest.to_string());
    };

    let (raw_path, anchor) = split_fragment(dest);
    let decoded = decode_component(raw_path);
    if decoded.is_empty() {
        return ResolvedLink::Blocked;
    }
    let path = normalize_path(&resolve_against(base, &decoded));
    let kind = kind_for(&path);
    ResolvedLink::Local { path, anchor, kind }
}

pub fn resolve_image(dest: &str, base_dir: Option<&Path>, localize: bool) -> ResolvedImage {
    let dest = dest.trim();
    if dest.is_empty() {
        return ResolvedImage::Blocked;
    }
    if has_control_chars(dest) {
        return ResolvedImage::Blocked;
    }
    if let Some(prefix) = IMAGE_DATA_PREFIXES.iter().find(|p| {
        dest.get(..p.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(p))
    }) {
        let payload = &dest[prefix.len()..];
        if payload.len() <= MAX_INLINE_IMAGE_BYTES && is_base64_payload(payload) {
            return ResolvedImage::Data(dest.to_string());
        }
        return ResolvedImage::Blocked;
    }

    if let Some(scheme) = scheme(dest) {
        return match scheme.as_str() {
            "http" | "https" => ResolvedImage::External(dest.to_string()),
            _ => ResolvedImage::Blocked,
        };
    }
    if dest.starts_with("//") {
        return ResolvedImage::External(format!("https:{dest}"));
    }
    if !localize {
        return ResolvedImage::Relative(dest.to_string());
    }
    let Some(base) = base_dir else {
        return ResolvedImage::Relative(dest.to_string());
    };

    let (raw_path, _) = split_fragment(dest);
    let decoded = decode_component(raw_path);
    if decoded.is_empty() {
        return ResolvedImage::Blocked;
    }
    ResolvedImage::LocalPath(normalize_path(&resolve_against(base, &decoded)))
}

/// Lowercase the URL scheme when the destination has one. Only ASCII scheme
/// characters are considered, and whitespace/control characters are removed
/// first so `java\nscript:` cannot slip through.
fn scheme(dest: &str) -> Option<String> {
    let compact: String = dest
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && !c.is_ascii_control())
        .collect();
    let colon = compact.find(':')?;
    let candidate = &compact[..colon];
    if candidate.is_empty() || !candidate.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    if !candidate
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
        return None;
    }
    Some(candidate.to_ascii_lowercase())
}

fn has_control_chars(dest: &str) -> bool {
    dest.chars().any(|c| c.is_control())
}

fn decode_component(value: &str) -> String {
    percent_decode_str(value).decode_utf8_lossy().into_owned()
}

fn split_fragment(dest: &str) -> (&str, Option<String>) {
    // Query strings are irrelevant for local files; fragments are useful.
    let (path, fragment) = match dest.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment)),
        None => (dest, None),
    };
    let path = path.split_once('?').map_or(path, |(path, _)| path);
    let anchor = fragment.filter(|f| !f.is_empty()).map(decode_component);
    (path, anchor)
}

fn resolve_against(base: &Path, dest: &str) -> PathBuf {
    let path = Path::new(dest);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

/// Normalize `.` and `..` lexically. The path does not need to exist.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    let mut depth = 0usize;
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => {
                out.push(Path::new(std::path::MAIN_SEPARATOR_STR));
                depth = 0;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if depth > 0 {
                    out.pop();
                    depth -= 1;
                } else if !out.has_root() {
                    out.push("..");
                }
            }
            Component::Normal(part) => {
                out.push(part);
                depth += 1;
            }
        }
    }
    out
}

fn kind_for(path: &Path) -> LocalKind {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some(ext) if MARKDOWN_EXTS.contains(&ext) => LocalKind::Doc,
        _ => LocalKind::File,
    }
}

fn is_base64_payload(payload: &str) -> bool {
    let trimmed = payload.trim_end_matches('=');
    !trimmed.is_empty()
        && trimmed
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'-' || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> PathBuf {
        PathBuf::from("/Users/ada/notes")
    }

    #[test]
    fn allows_web_and_mail_links() {
        assert_eq!(
            resolve_link("https://example.com/a?b=1#c", None, true),
            ResolvedLink::External("https://example.com/a?b=1#c".into())
        );
        assert_eq!(
            resolve_link("mailto:hi@example.com", None, true),
            ResolvedLink::External("mailto:hi@example.com".into())
        );
        assert_eq!(
            resolve_link("//example.com/x", None, true),
            ResolvedLink::External("https://example.com/x".into())
        );
    }

    #[test]
    fn blocks_dangerous_schemes() {
        for url in [
            "javascript:alert(1)",
            "JaVaScRiPt:alert(1)",
            "java\nscript:alert(1)",
            "data:text/html,<script>alert(1)</script>",
            "vbscript:msgbox(1)",
            "file:///etc/passwd",
            "blob:https://x/y",
        ] {
            assert_eq!(
                resolve_link(url, Some(&base()), true),
                ResolvedLink::Blocked,
                "{url}"
            );
        }
    }

    #[test]
    fn resolves_local_docs_and_anchors() {
        let resolved = resolve_link("../other/readme.md#part-2", Some(&base()), true);
        assert_eq!(
            resolved,
            ResolvedLink::Local {
                path: PathBuf::from("/Users/ada/other/readme.md"),
                anchor: Some("part-2".into()),
                kind: LocalKind::Doc,
            }
        );
        assert_eq!(
            resolve_link("notes.txt", Some(&base()), true),
            ResolvedLink::Local {
                path: PathBuf::from("/Users/ada/notes/notes.txt"),
                anchor: None,
                kind: LocalKind::Doc,
            }
        );
        assert_eq!(
            resolve_link("diagram.png", Some(&base()), true),
            ResolvedLink::Local {
                path: PathBuf::from("/Users/ada/notes/diagram.png"),
                anchor: None,
                kind: LocalKind::File,
            }
        );
    }

    #[test]
    fn decodes_spaces_in_local_paths() {
        assert_eq!(
            resolve_link("my%20notes.md", Some(&base()), true),
            ResolvedLink::Local {
                path: PathBuf::from("/Users/ada/notes/my notes.md"),
                anchor: None,
                kind: LocalKind::Doc,
            }
        );
    }

    #[test]
    fn does_not_escape_root_with_parent_segments() {
        let resolved = resolve_link(
            "../../../../etc/passwd",
            Some(Path::new("/Users/ada")),
            true,
        );
        match resolved {
            ResolvedLink::Local { path, .. } => {
                assert_eq!(path, PathBuf::from("/etc/passwd"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn leaves_relative_urls_alone_without_localization() {
        assert_eq!(
            resolve_link("a b.md", Some(&base()), false),
            ResolvedLink::Relative("a b.md".into())
        );
        assert_eq!(
            resolve_image("img/x.png", Some(&base()), false),
            ResolvedImage::Relative("img/x.png".into())
        );
    }

    #[test]
    fn resolves_images() {
        assert_eq!(
            resolve_image("img/x.png?v=2", Some(&base()), true),
            ResolvedImage::LocalPath(PathBuf::from("/Users/ada/notes/img/x.png"))
        );
        assert_eq!(
            resolve_image("https://example.com/x.png", Some(&base()), true),
            ResolvedImage::External("https://example.com/x.png".into())
        );
        assert_eq!(
            resolve_image("data:image/png;base64,iVBORw0KGgo=", None, true),
            ResolvedImage::Data("data:image/png;base64,iVBORw0KGgo=".into())
        );
    }

    #[test]
    fn blocks_unsafe_images() {
        assert_eq!(
            resolve_image("javascript:alert(1)", None, true),
            ResolvedImage::Blocked
        );
        assert_eq!(
            resolve_image("data:image/svg+xml;base64,PHN2Zz4=", None, true),
            ResolvedImage::Blocked
        );
        assert_eq!(
            resolve_image("data:text/html;base64,PHNjcmlwdD4=", None, true),
            ResolvedImage::Blocked
        );
        assert_eq!(
            resolve_image("file:///tmp/x.png", None, true),
            ResolvedImage::Blocked
        );
    }

    #[test]
    fn normalizes_windows_style_segments() {
        let p = normalize_path(Path::new("/a/b/../c/./d"));
        assert_eq!(p, PathBuf::from("/a/c/d"));
    }
}
