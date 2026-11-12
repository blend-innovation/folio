//! A small, dependency-free syntax highlighter.
//!
//! It is intentionally not a full parser. It recognizes the shapes that
//! dominate agent-generated code: comments, strings, numbers, keywords,
//! function names, types and attributes. Highlighting happens once, in Rust,
//! while the document is rendered; the webview only sees ready-made spans.

use crate::html::push_escaped;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lang {
    Rust,
    Ts,
    Tsx,
    Js,
    Jsx,
    Python,
    Bash,
    Go,
    C,
    Cpp,
    Swift,
    Java,
    Kotlin,
    Sql,
    Json,
    Jsonc,
    Css,
    Html,
    Yaml,
    Toml,
    Diff,
}

/// Normalize a fence info string: `{.rust .numberLines}`, `language-ts`, `TS`.
pub fn canonical_language(raw: &str) -> String {
    let raw = raw.trim();
    let raw = raw
        .strip_prefix('{')
        .and_then(|s| s.split('}').next())
        .unwrap_or(raw);
    let first = raw.split_whitespace().next().unwrap_or("");
    let first = first.trim_start_matches('.');
    let first = first
        .strip_prefix("language-")
        .or_else(|| first.strip_prefix("lang-"))
        .unwrap_or(first);
    let lower = first.to_ascii_lowercase();
    match lower.as_str() {
        "" => "text".into(),
        "rs" => "rust".into(),
        "typescript" => "ts".into(),
        "tsx" => "tsx".into(),
        "javascript" | "mjs" | "cjs" => "js".into(),
        "jsx" => "jsx".into(),
        "py" => "python".into(),
        "sh" | "shell" | "zsh" | "console" | "shell-session" => "bash".into(),
        "yml" => "yaml".into(),
        "htm" | "xml" | "svg" => "html".into(),
        "c++" | "hpp" | "cc" | "cxx" => "cpp".into(),
        "csharp" | "cs" => "csharp".into(),
        "golang" => "go".into(),
        "patch" => "diff".into(),
        "plain" | "plaintext" | "txt" | "log" | "output" | "md" | "markdown" => "text".into(),
        other => other.into(),
    }
}

/// Human-readable label shown in the code block header.
pub fn language_label(lang: &str) -> String {
    match lang {
        "" | "text" => "Text".into(),
        "rust" => "Rust".into(),
        "ts" => "TypeScript".into(),
        "tsx" => "TSX".into(),
        "js" => "JavaScript".into(),
        "jsx" => "JSX".into(),
        "python" => "Python".into(),
        "bash" => "Shell".into(),
        "json" | "jsonc" => "JSON".into(),
        "css" | "scss" | "less" => "CSS".into(),
        "html" => "HTML".into(),
        "go" => "Go".into(),
        "sql" => "SQL".into(),
        "yaml" => "YAML".into(),
        "toml" => "TOML".into(),
        "diff" => "Diff".into(),
        "c" => "C".into(),
        "cpp" => "C++".into(),
        "swift" => "Swift".into(),
        "java" => "Java".into(),
        "kotlin" => "Kotlin".into(),
        "csharp" => "C#".into(),
        "graphql" => "GraphQL".into(),
        "prisma" => "Prisma".into(),
        other => {
            let mut chars = other.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => "Text".into(),
            }
        }
    }
}

/// Highlight `code`. Returns `None` when the language is plain or unknown, in
/// which case the caller should just escape the code.
pub fn highlight(code: &str, lang: &str) -> Option<String> {
    let lang = canonical_language(lang);
    let parsed = parse_lang(&lang)?;
    if lang == "diff" {
        return Some(highlight_diff(code));
    }
    if lang == "html" {
        return Some(highlight_markup(code));
    }
    if matches!(lang.as_str(), "css" | "scss" | "less") {
        return Some(highlight_css(code));
    }
    if matches!(lang.as_str(), "json" | "jsonc") {
        return Some(highlight_json(code, lang == "jsonc"));
    }
    if lang == "yaml" {
        return Some(highlight_yaml(code));
    }
    if lang == "toml" {
        return Some(highlight_toml(code));
    }
    Some(highlight_code(code, parsed))
}

fn parse_lang(lang: &str) -> Option<Lang> {
    Some(match lang {
        "rust" => Lang::Rust,
        "ts" => Lang::Ts,
        "tsx" => Lang::Tsx,
        "js" => Lang::Js,
        "jsx" => Lang::Jsx,
        "python" => Lang::Python,
        "bash" => Lang::Bash,
        "go" => Lang::Go,
        "c" => Lang::C,
        "cpp" => Lang::Cpp,
        "swift" => Lang::Swift,
        "java" => Lang::Java,
        "kotlin" => Lang::Kotlin,
        "sql" => Lang::Sql,
        "json" => Lang::Json,
        "jsonc" => Lang::Jsonc,
        "css" | "scss" | "less" => Lang::Css,
        "html" => Lang::Html,
        "yaml" => Lang::Yaml,
        "toml" => Lang::Toml,
        "diff" => Lang::Diff,
        _ => return None,
    })
}

fn span(out: &mut String, class: &str, text: &str) {
    if text.is_empty() {
        return;
    }
    out.push_str("<span class=\"tok-");
    out.push_str(class);
    out.push_str("\">");
    push_escaped(out, text);
    out.push_str("</span>");
}

fn plain(out: &mut String, text: &str) {
    push_escaped(out, text);
}

struct Spec {
    line_comments: &'static [&'static str],
    block_comment: Option<(&'static str, &'static str)>,
    hash_comments: bool,
    quotes: &'static [char],
    backtick: bool,
    triple_quotes: bool,
    rust_raw: bool,
    bash_vars: bool,
}

fn spec_for(lang: Lang) -> Spec {
    let c_like = Spec {
        line_comments: &["//"],
        block_comment: Some(("/*", "*/")),
        hash_comments: false,
        quotes: &['"', '\''],
        backtick: false,
        triple_quotes: false,
        rust_raw: false,
        bash_vars: false,
    };
    match lang {
        Lang::Rust => Spec {
            rust_raw: true,
            ..c_like
        },
        Lang::Ts | Lang::Tsx | Lang::Js | Lang::Jsx => Spec {
            backtick: true,
            ..c_like
        },
        Lang::Python => Spec {
            line_comments: &[],
            block_comment: None,
            hash_comments: true,
            quotes: &['"', '\''],
            triple_quotes: true,
            ..c_like
        },
        Lang::Bash => Spec {
            line_comments: &[],
            block_comment: None,
            hash_comments: true,
            quotes: &['"', '\''],
            backtick: true,
            bash_vars: true,
            ..c_like
        },
        Lang::Go => Spec {
            backtick: true,
            ..c_like
        },
        Lang::Swift => Spec {
            triple_quotes: true,
            ..c_like
        },
        Lang::Sql => Spec {
            line_comments: &["--"],
            block_comment: Some(("/*", "*/")),
            hash_comments: false,
            quotes: &['\'', '"'],
            ..c_like
        },
        _ => c_like,
    }
}

fn highlight_code(code: &str, lang: Lang) -> String {
    let spec = spec_for(lang);
    let mut out = String::with_capacity(code.len() * 2);
    let bytes = code.as_bytes();
    let mut pos = 0usize;

    if code.starts_with("#!") {
        let end = code.find('\n').unwrap_or(code.len());
        span(&mut out, "meta", &code[..end]);
        pos = end;
    }

    while pos < code.len() {
        let rest = &code[pos..];
        let ch = rest.chars().next().expect("non-empty");

        if ch.is_whitespace() {
            plain(&mut out, ch.encode_utf8(&mut [0; 4]));
            pos += ch.len_utf8();
            continue;
        }

        // Comments.
        if spec.hash_comments && ch == '#' && hash_starts_comment(code, pos) {
            let end = code[pos..].find('\n').map_or(code.len(), |i| pos + i);
            span(&mut out, "com", &code[pos..end]);
            pos = end;
            continue;
        }
        let mut matched_comment = false;
        for marker in spec.line_comments {
            if rest.starts_with(marker) {
                let end = code[pos..].find('\n').map_or(code.len(), |i| pos + i);
                span(&mut out, "com", &code[pos..end]);
                pos = end;
                matched_comment = true;
                break;
            }
        }
        if matched_comment {
            continue;
        }
        if let Some((open, close)) = spec.block_comment {
            if rest.starts_with(open) {
                let end = rest
                    .find(close)
                    .map(|i| pos + i + close.len())
                    .unwrap_or(code.len());
                span(&mut out, "com", &code[pos..end]);
                pos = end;
                continue;
            }
        }

        // Strings, including raw and triple-quoted forms.
        if spec.rust_raw {
            if let Some(end) = scan_rust_raw(code, pos) {
                span(&mut out, "str", &code[pos..end]);
                pos = end;
                continue;
            }
        }
        if spec.triple_quotes && (rest.starts_with("\"\"\"") || rest.starts_with("'''")) {
            let quote = &rest[..3];
            let mut end = pos + 3;
            while end < code.len() {
                if code[end..].starts_with(quote) {
                    end += 3;
                    break;
                }
                if code.as_bytes()[end] == b'\\' {
                    end += 1;
                }
                end += char_len_at(code, end);
            }
            span(&mut out, "str", &code[pos..end.min(code.len())]);
            pos = end.min(code.len());
            continue;
        }
        if spec.quotes.contains(&ch) {
            // Rust/Go distinguish lifetimes and raw strings from char literals.
            if ch == '\'' && matches!(lang, Lang::Rust | Lang::Go) {
                if let Some(end) = scan_char_literal(code, pos) {
                    span(&mut out, "str", &code[pos..end]);
                    pos = end;
                } else {
                    // A lifetime, e.g. 'a. Highlight the name, leave the quote.
                    plain(&mut out, "'");
                    let end = scan_ident_end(code, pos + 1);
                    if end > pos + 1 {
                        span(&mut out, "typ", &code[pos + 1..end]);
                    }
                    pos = end;
                }
                continue;
            }
            let end = scan_quoted(code, pos, ch, false);
            span(&mut out, "str", &code[pos..end]);
            pos = end;
            continue;
        }
        if spec.backtick && ch == '`' {
            let end = scan_quoted(code, pos, '`', true);
            span(&mut out, "str", &code[pos..end]);
            pos = end;
            continue;
        }

        // Shell variables.
        if spec.bash_vars && ch == '$' {
            let end = scan_shell_var(code, pos);
            if end > pos + 1 {
                span(&mut out, "var", &code[pos..end]);
                pos = end;
                continue;
            }
        }

        // Attributes / decorators.
        if ch == '@' && !rest.starts_with("@media") {
            let end = scan_ident_end(code, pos + 1);
            if end > pos + 1 {
                span(&mut out, "attr", &code[pos..end]);
                pos = end;
                continue;
            }
        }
        if ch == '#' && rest.starts_with("#[") && matches!(lang, Lang::Rust) {
            let end = code[pos..].find(']').map_or(code.len(), |i| pos + i + 1);
            span(&mut out, "attr", &code[pos..end]);
            pos = end;
            continue;
        }

        // Numbers.
        if ch.is_ascii_digit() {
            let end = scan_number(code, pos, false);
            span(&mut out, "num", &code[pos..end]);
            pos = end;
            continue;
        }

        // Identifiers, keywords, types, function names.
        if is_ident_start(ch) {
            let end = scan_ident_end(code, pos);
            let word = &code[pos..end];
            let next = next_non_space(bytes, end);
            let class = if is_keyword(lang, word) {
                "kw"
            } else if is_builtin_type(lang, word) {
                "typ"
            } else if next == Some(b'(') {
                "fn"
            } else if word.chars().next().is_some_and(|c| c.is_uppercase()) {
                "typ"
            } else if next == Some(b'!') {
                "fn"
            } else {
                ""
            };
            if class.is_empty() {
                plain(&mut out, word);
            } else {
                span(&mut out, class, word);
            }
            pos = end;
            continue;
        }

        plain(&mut out, ch.encode_utf8(&mut [0; 4]));
        pos += ch.len_utf8();
    }

    out
}

fn hash_starts_comment(code: &str, pos: usize) -> bool {
    if pos == 0 {
        return true;
    }
    let prev = code[..pos].chars().next_back();
    prev.is_some_and(|c| c.is_whitespace() || matches!(c, ';' | '(' | '`'))
}

fn char_len_at(s: &str, pos: usize) -> usize {
    s[pos..].chars().next().map_or(1, char::len_utf8)
}

fn scan_quoted(code: &str, start: usize, quote: char, multiline: bool) -> usize {
    let bytes = code.as_bytes();
    let mut pos = start + quote.len_utf8();
    while pos < code.len() {
        let ch = code[pos..].chars().next().unwrap();
        if ch == quote {
            return pos + ch.len_utf8();
        }
        if ch == '\\' {
            pos += 1;
            if pos < code.len() {
                pos += char_len_at(code, pos);
            }
            continue;
        }
        if ch == '\n' && !multiline {
            return pos;
        }
        let _ = bytes;
        pos += ch.len_utf8();
    }
    code.len()
}

fn scan_char_literal(code: &str, start: usize) -> Option<usize> {
    let rest = &code[start + 1..];
    let mut chars = rest.char_indices();
    let (_, first) = chars.next()?;
    if first.is_whitespace() || first.is_alphanumeric() {
        return None;
    }
    if first == '\\' {
        let mut end = start + 1;
        let mut count = 0;
        while end < code.len() && count < 12 {
            let ch = code[end..].chars().next()?;
            end += ch.len_utf8();
            count += 1;
            if ch == '\'' {
                return Some(end);
            }
        }
        return None;
    }
    let after = start + 1 + first.len_utf8();
    if code[after..].starts_with('\'') {
        Some(after + 1)
    } else {
        None
    }
}

fn scan_rust_raw(code: &str, start: usize) -> Option<usize> {
    let rest = &code[start..];
    let after_prefix = if rest.starts_with("br") {
        2
    } else if rest.starts_with('r') || rest.starts_with("b\"") {
        1
    } else {
        return None;
    };
    let bytes = code.as_bytes();
    let mut hashes = 0usize;
    let mut pos = start + after_prefix;
    while pos < code.len() && bytes[pos] == b'#' {
        hashes += 1;
        pos += 1;
    }
    if pos >= code.len() || bytes[pos] != b'"' {
        return None;
    }
    pos += 1;
    let closing: String = std::iter::once('"')
        .chain(std::iter::repeat_n('#', hashes))
        .collect();
    let end = code[pos..]
        .find(&closing)
        .map_or(code.len(), |i| pos + i + closing.len());
    Some(end)
}

fn scan_shell_var(code: &str, start: usize) -> usize {
    let rest = &code[start + 1..];
    if rest.starts_with('{') {
        if let Some(end) = rest.find('}') {
            return start + 1 + end + 1;
        }
        return start + 1;
    }
    scan_ident_end(code, start + 1)
}

fn scan_ident_end(code: &str, start: usize) -> usize {
    let mut end = start;
    for (i, ch) in code[start..].char_indices() {
        if i == 0 {
            if !is_ident_start(ch) {
                return start;
            }
        } else if !is_ident_continue(ch) {
            break;
        }
        end = start + i + ch.len_utf8();
    }
    end
}

fn is_ident_start(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_' || (!ch.is_ascii() && ch.is_alphanumeric())
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn next_non_space(bytes: &[u8], mut pos: usize) -> Option<u8> {
    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    bytes.get(pos).copied()
}

fn scan_number(code: &str, start: usize, allow_units: bool) -> usize {
    let bytes = code.as_bytes();
    let mut pos = start;
    if pos + 2 <= code.len() && bytes[pos] == b'0' {
        let radix = bytes.get(pos + 1).copied();
        if matches!(radix, Some(b'x' | b'X' | b'b' | b'B' | b'o' | b'O')) {
            pos += 2;
            while pos < code.len() && (bytes[pos].is_ascii_hexdigit() || bytes[pos] == b'_') {
                pos += 1;
            }
            return take_numeric_suffix(code, pos);
        }
    }
    let mut seen_dot = false;
    while pos < code.len() {
        let b = bytes[pos];
        if b.is_ascii_digit() || b == b'_' {
            pos += 1;
        } else if b == b'.' && !seen_dot && bytes.get(pos + 1).is_some_and(u8::is_ascii_digit) {
            seen_dot = true;
            pos += 1;
        } else if matches!(b, b'e' | b'E') && !seen_dot {
            let mut exp = pos + 1;
            if matches!(bytes.get(exp), Some(b'+' | b'-')) {
                exp += 1;
            }
            if bytes.get(exp).is_some_and(u8::is_ascii_digit) {
                pos = exp + 1;
                while pos < code.len() && bytes[pos].is_ascii_digit() {
                    pos += 1;
                }
                break;
            } else {
                break;
            }
        } else {
            break;
        }
    }
    if allow_units {
        while pos < code.len() && (bytes[pos].is_ascii_alphabetic() || bytes[pos] == b'%') {
            pos += 1;
        }
    }
    take_numeric_suffix(code, pos)
}

fn take_numeric_suffix(code: &str, pos: usize) -> usize {
    let bytes = code.as_bytes();
    let mut end = pos;
    while end < code.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        // Only consume short type suffixes such as f32, u64, n, px.
        if end - pos >= 4 {
            break;
        }
        end += 1;
    }
    end
}

fn is_keyword(lang: Lang, word: &str) -> bool {
    match lang {
        Lang::Rust => matches!(
            word,
            "as" | "async"
                | "await"
                | "break"
                | "const"
                | "continue"
                | "crate"
                | "dyn"
                | "else"
                | "enum"
                | "extern"
                | "false"
                | "fn"
                | "for"
                | "if"
                | "impl"
                | "in"
                | "let"
                | "loop"
                | "match"
                | "mod"
                | "move"
                | "mut"
                | "pub"
                | "ref"
                | "return"
                | "self"
                | "Self"
                | "static"
                | "struct"
                | "super"
                | "trait"
                | "true"
                | "type"
                | "unsafe"
                | "use"
                | "where"
                | "while"
                | "yield"
        ),
        Lang::Ts | Lang::Tsx | Lang::Js | Lang::Jsx => matches!(
            word,
            "as" | "async"
                | "await"
                | "break"
                | "case"
                | "catch"
                | "class"
                | "const"
                | "continue"
                | "debugger"
                | "default"
                | "delete"
                | "do"
                | "else"
                | "enum"
                | "export"
                | "extends"
                | "false"
                | "finally"
                | "for"
                | "from"
                | "function"
                | "get"
                | "if"
                | "implements"
                | "import"
                | "in"
                | "instanceof"
                | "interface"
                | "let"
                | "new"
                | "null"
                | "of"
                | "private"
                | "protected"
                | "public"
                | "readonly"
                | "return"
                | "set"
                | "static"
                | "super"
                | "switch"
                | "this"
                | "throw"
                | "true"
                | "try"
                | "type"
                | "typeof"
                | "undefined"
                | "var"
                | "void"
                | "while"
                | "with"
                | "yield"
        ),
        Lang::Python => matches!(
            word,
            "and"
                | "as"
                | "assert"
                | "async"
                | "await"
                | "break"
                | "class"
                | "continue"
                | "def"
                | "del"
                | "elif"
                | "else"
                | "except"
                | "False"
                | "finally"
                | "for"
                | "from"
                | "global"
                | "if"
                | "import"
                | "in"
                | "is"
                | "lambda"
                | "nonlocal"
                | "None"
                | "not"
                | "or"
                | "pass"
                | "raise"
                | "return"
                | "True"
                | "try"
                | "while"
                | "with"
                | "yield"
        ),
        Lang::Bash => matches!(
            word,
            "case"
                | "do"
                | "done"
                | "elif"
                | "else"
                | "esac"
                | "fi"
                | "for"
                | "function"
                | "if"
                | "in"
                | "select"
                | "then"
                | "until"
                | "while"
                | "time"
                | "local"
                | "export"
                | "readonly"
                | "declare"
                | "return"
                | "exit"
                | "true"
                | "false"
        ),
        Lang::Go => matches!(
            word,
            "break"
                | "case"
                | "chan"
                | "const"
                | "continue"
                | "default"
                | "defer"
                | "else"
                | "fallthrough"
                | "false"
                | "for"
                | "func"
                | "go"
                | "goto"
                | "if"
                | "import"
                | "interface"
                | "map"
                | "nil"
                | "package"
                | "range"
                | "return"
                | "select"
                | "struct"
                | "switch"
                | "true"
                | "type"
                | "var"
        ),
        Lang::Sql => matches!(
            word.to_ascii_lowercase().as_str(),
            "all"
                | "alter"
                | "and"
                | "any"
                | "as"
                | "asc"
                | "avg"
                | "between"
                | "by"
                | "case"
                | "constraint"
                | "count"
                | "create"
                | "default"
                | "delete"
                | "desc"
                | "distinct"
                | "drop"
                | "else"
                | "end"
                | "exists"
                | "foreign"
                | "from"
                | "full"
                | "group"
                | "having"
                | "in"
                | "index"
                | "inner"
                | "insert"
                | "into"
                | "is"
                | "join"
                | "key"
                | "left"
                | "like"
                | "limit"
                | "max"
                | "min"
                | "not"
                | "null"
                | "offset"
                | "on"
                | "or"
                | "order"
                | "outer"
                | "primary"
                | "references"
                | "right"
                | "select"
                | "set"
                | "sum"
                | "table"
                | "then"
                | "union"
                | "unique"
                | "update"
                | "values"
                | "when"
                | "where"
                | "with"
        ),
        Lang::C | Lang::Cpp | Lang::Swift | Lang::Java | Lang::Kotlin => matches!(
            word,
            "abstract"
                | "as"
                | "async"
                | "await"
                | "boolean"
                | "break"
                | "case"
                | "catch"
                | "char"
                | "class"
                | "const"
                | "continue"
                | "data"
                | "default"
                | "defer"
                | "delete"
                | "do"
                | "double"
                | "else"
                | "enum"
                | "extends"
                | "extern"
                | "false"
                | "final"
                | "finally"
                | "float"
                | "for"
                | "func"
                | "fun"
                | "function"
                | "guard"
                | "if"
                | "implements"
                | "import"
                | "in"
                | "instanceof"
                | "int"
                | "interface"
                | "internal"
                | "is"
                | "let"
                | "long"
                | "namespace"
                | "new"
                | "nil"
                | "null"
                | "object"
                | "open"
                | "operator"
                | "override"
                | "package"
                | "private"
                | "protected"
                | "public"
                | "readonly"
                | "return"
                | "sealed"
                | "short"
                | "static"
                | "struct"
                | "super"
                | "switch"
                | "template"
                | "this"
                | "throw"
                | "throws"
                | "true"
                | "try"
                | "typealias"
                | "typename"
                | "unsigned"
                | "using"
                | "val"
                | "var"
                | "virtual"
                | "void"
                | "volatile"
                | "where"
                | "while"
                | "yield"
        ),
        _ => false,
    }
}

fn is_builtin_type(lang: Lang, word: &str) -> bool {
    let common = matches!(
        word,
        "i8" | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "f32"
            | "f64"
            | "str"
            | "String"
            | "Vec"
            | "Option"
            | "Result"
            | "Box"
            | "Rc"
            | "Arc"
            | "HashMap"
            | "BTreeMap"
            | "HashSet"
            | "BTreeSet"
            | "Self"
    );
    if common {
        return true;
    }
    match lang {
        Lang::Ts | Lang::Tsx => matches!(
            word,
            "string"
                | "number"
                | "boolean"
                | "unknown"
                | "any"
                | "never"
                | "object"
                | "bigint"
                | "symbol"
                | "Record"
                | "Array"
                | "Promise"
                | "Map"
                | "Set"
                | "Partial"
                | "Pick"
                | "Omit"
                | "Readonly"
        ),
        Lang::Python => matches!(
            word,
            "str"
                | "int"
                | "float"
                | "bool"
                | "list"
                | "dict"
                | "set"
                | "tuple"
                | "bytes"
                | "Any"
                | "Optional"
                | "Union"
                | "List"
                | "Dict"
                | "Tuple"
                | "Iterable"
        ),
        Lang::Go => matches!(
            word,
            "int"
                | "int8"
                | "int16"
                | "int32"
                | "int64"
                | "uint"
                | "uint8"
                | "uint16"
                | "uint32"
                | "uint64"
                | "float32"
                | "float64"
                | "string"
                | "bool"
                | "byte"
                | "rune"
                | "error"
                | "any"
        ),
        Lang::Bash => matches!(word, "echo" | "printf" | "read" | "cd" | "source"),
        _ => false,
    }
}

fn highlight_json(code: &str, comments: bool) -> String {
    let mut out = String::with_capacity(code.len() * 2);
    let mut pos = 0;
    while pos < code.len() {
        let rest = &code[pos..];
        let ch = rest.chars().next().unwrap();
        if ch.is_whitespace() {
            plain(&mut out, ch.encode_utf8(&mut [0; 4]));
            pos += ch.len_utf8();
            continue;
        }
        if comments && rest.starts_with("//") {
            let end = code[pos..].find('\n').map_or(code.len(), |i| pos + i);
            span(&mut out, "com", &code[pos..end]);
            pos = end;
            continue;
        }
        if comments && rest.starts_with("/*") {
            let end = rest.find("*/").map_or(code.len(), |i| pos + i + 2);
            span(&mut out, "com", &code[pos..end]);
            pos = end;
            continue;
        }
        if ch == '"' {
            let end = scan_quoted(code, pos, '"', false);
            let after = next_non_space(code.as_bytes(), end);
            if after == Some(b':') {
                span(&mut out, "prop", &code[pos..end]);
            } else {
                span(&mut out, "str", &code[pos..end]);
            }
            pos = end;
            continue;
        }
        if ch.is_ascii_digit() || (ch == '-' && rest[1..].starts_with(|c: char| c.is_ascii_digit()))
        {
            let end = scan_number(code, pos + usize::from(ch == '-'), false);
            span(&mut out, "num", &code[pos..end.max(pos + 1)]);
            pos = end.max(pos + 1);
            continue;
        }
        if is_ident_start(ch) {
            let end = scan_ident_end(code, pos);
            let word = &code[pos..end];
            if matches!(word, "true" | "false" | "null") {
                span(&mut out, "kw", word);
            } else {
                plain(&mut out, word);
            }
            pos = end;
            continue;
        }
        plain(&mut out, ch.encode_utf8(&mut [0; 4]));
        pos += ch.len_utf8();
    }
    out
}

fn highlight_markup(code: &str) -> String {
    let mut out = String::with_capacity(code.len() * 2);
    let bytes = code.as_bytes();
    let mut pos = 0;
    while pos < code.len() {
        let rest = &code[pos..];
        if rest.starts_with("<!--") {
            let end = rest.find("-->").map_or(code.len(), |i| pos + i + 3);
            span(&mut out, "com", &code[pos..end]);
            pos = end;
            continue;
        }
        if rest.starts_with("<!") {
            let end = rest.find('>').map_or(code.len(), |i| pos + i + 1);
            span(&mut out, "meta", &code[pos..end]);
            pos = end;
            continue;
        }
        if bytes[pos] == b'<' {
            let closing = rest.starts_with("</");
            let name_start = pos + if closing { 2 } else { 1 };
            if name_start >= code.len() {
                plain(&mut out, "<");
                pos += 1;
                continue;
            }
            let name_end = scan_tag_name(code, name_start);
            if name_end == name_start {
                plain(&mut out, "<");
                pos += 1;
                continue;
            }
            span(&mut out, "tag", &code[pos..name_start]);
            span(&mut out, "tag", &code[name_start..name_end]);
            pos = name_end;
            while pos < code.len() {
                let ch = code[pos..].chars().next().unwrap();
                if ch.is_whitespace() || ch == '>' || ch == '/' {
                    plain(&mut out, ch.encode_utf8(&mut [0; 4]));
                    pos += ch.len_utf8();
                    if ch == '>' {
                        break;
                    }
                    continue;
                }
                if ch == '=' {
                    plain(&mut out, "=");
                    pos += 1;
                    // Attribute value, possibly quoted.
                    while pos < code.len() && code.as_bytes()[pos].is_ascii_whitespace() {
                        plain(&mut out, " ");
                        pos += 1;
                    }
                    if pos < code.len() && matches!(code.as_bytes()[pos], b'"' | b'\'') {
                        let quote = code.as_bytes()[pos] as char;
                        let end = scan_quoted(code, pos, quote, false);
                        span(&mut out, "str", &code[pos..end]);
                        pos = end;
                    }
                    continue;
                }
                let end = scan_attr_end(code, pos);
                if end == pos {
                    plain(&mut out, ch.encode_utf8(&mut [0; 4]));
                    pos += ch.len_utf8();
                } else {
                    span(&mut out, "attr", &code[pos..end]);
                    pos = end;
                }
            }
            continue;
        }
        // Text between tags, including entities.
        let end = code[pos..].find('<').map_or(code.len(), |i| pos + i);
        plain(&mut out, &code[pos..end]);
        pos = end;
    }
    out
}

fn scan_tag_name(code: &str, start: usize) -> usize {
    let mut end = start;
    for (i, ch) in code[start..].char_indices() {
        if i == 0 {
            if !ch.is_ascii_alphabetic() && ch != '_' && ch != ':' {
                break;
            }
        } else if !(ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '.')) {
            break;
        }
        end = start + i + ch.len_utf8();
    }
    end
}

fn scan_attr_end(code: &str, start: usize) -> usize {
    let mut end = start;
    for (i, ch) in code[start..].char_indices() {
        if i == 0 {
            if !(ch.is_ascii_alphabetic() || matches!(ch, '_' | ':' | '@' | '[' | '(')) {
                break;
            }
        } else if !(ch.is_ascii_alphanumeric()
            || matches!(ch, '-' | '_' | ':' | '.' | '@' | '[' | ']' | '(' | ')'))
        {
            break;
        }
        end = start + i + ch.len_utf8();
    }
    end
}

fn highlight_css(code: &str) -> String {
    let mut out = String::with_capacity(code.len() * 2);
    let mut pos = 0;
    let mut in_block = 0usize;
    while pos < code.len() {
        let rest = &code[pos..];
        let ch = rest.chars().next().unwrap();
        if ch.is_whitespace() {
            plain(&mut out, ch.encode_utf8(&mut [0; 4]));
            pos += ch.len_utf8();
            continue;
        }
        if rest.starts_with("/*") {
            let end = rest.find("*/").map_or(code.len(), |i| pos + i + 2);
            span(&mut out, "com", &code[pos..end]);
            pos = end;
            continue;
        }
        if matches!(ch, '"' | '\'') {
            let end = scan_quoted(code, pos, ch, true);
            span(&mut out, "str", &code[pos..end]);
            pos = end;
            continue;
        }
        if ch == '@' {
            let end = scan_ident_end(code, pos + 1);
            if end > pos + 1 {
                span(&mut out, "kw", &code[pos..end]);
                pos = end;
                continue;
            }
        }
        if ch == '{' {
            in_block += 1;
            plain(&mut out, "{");
            pos += 1;
            continue;
        }
        if ch == '}' {
            in_block = in_block.saturating_sub(1);
            plain(&mut out, "}");
            pos += 1;
            continue;
        }
        if ch == '#' {
            let end = scan_hex_color(code, pos);
            if end > pos + 1 {
                span(&mut out, "num", &code[pos..end]);
                pos = end;
                continue;
            }
        }
        if ch.is_ascii_digit() {
            let end = scan_number(code, pos, true);
            span(&mut out, "num", &code[pos..end]);
            pos = end;
            continue;
        }
        if in_block > 0 && is_ident_start(ch) {
            let end = scan_ident_end(code, pos);
            let word = &code[pos..end];
            let next = next_non_space(code.as_bytes(), end);
            if next == Some(b':') {
                span(&mut out, "prop", word);
            } else if next == Some(b'(') {
                span(&mut out, "fn", word);
            } else if is_css_keyword(word) {
                span(&mut out, "kw", word);
            } else {
                plain(&mut out, word);
            }
            pos = end;
            continue;
        }
        if in_block == 0 && is_ident_start(ch) {
            // Selector: type/class/id names.
            let end = scan_ident_end(code, pos);
            plain(&mut out, &code[pos..end]);
            pos = end;
            continue;
        }
        plain(&mut out, ch.encode_utf8(&mut [0; 4]));
        pos += ch.len_utf8();
    }
    out
}

fn scan_hex_color(code: &str, start: usize) -> usize {
    let bytes = code.as_bytes();
    let mut pos = start + 1;
    while pos < code.len() && bytes[pos].is_ascii_hexdigit() && pos - start <= 8 {
        pos += 1;
    }
    if pos > start + 1 {
        pos
    } else {
        start
    }
}

fn is_css_keyword(word: &str) -> bool {
    matches!(
        word,
        "inherit"
            | "initial"
            | "unset"
            | "none"
            | "auto"
            | "block"
            | "inline"
            | "flex"
            | "grid"
            | "absolute"
            | "relative"
            | "fixed"
            | "sticky"
            | "bold"
            | "italic"
            | "transparent"
            | "currentColor"
            | "var"
            | "calc"
            | "important"
    )
}

fn highlight_yaml(code: &str) -> String {
    let mut out = String::with_capacity(code.len() * 2);
    for line in code.split_inclusive('\n') {
        let content = line.strip_suffix('\n').unwrap_or(line);
        highlight_yaml_line(&mut out, content);
        if line.ends_with('\n') {
            plain(&mut out, "\n");
        }
    }
    out
}

fn highlight_yaml_line(out: &mut String, line: &str) {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    plain(out, indent);

    if trimmed.is_empty() {
        return;
    }
    if trimmed.starts_with('#') {
        span(out, "com", trimmed);
        return;
    }
    if trimmed.starts_with("---") || trimmed.starts_with("...") {
        span(out, "meta", trimmed);
        return;
    }
    // Block list item.
    if let Some(rest) = trimmed.strip_prefix("- ") {
        plain(out, "- ");
        highlight_yaml_scalar(out, rest);
        return;
    }
    if trimmed == "-" {
        plain(out, "-");
        return;
    }
    if let Some(pos) = find_unquoted(trimmed, ':') {
        let key = trimmed[..pos].trim_end();
        if !key.is_empty() && key.len() < 80 {
            span(out, "prop", key);
            plain(out, &trimmed[key.len()..=pos]);
            highlight_yaml_scalar(out, trimmed[pos + 1..].trim_start());
            return;
        }
    }
    highlight_yaml_scalar(out, trimmed);
}

fn highlight_yaml_scalar(out: &mut String, text: &str) {
    if text.is_empty() {
        return;
    }
    if matches!(
        text.to_ascii_lowercase().as_str(),
        "true" | "false" | "null" | "yes" | "no" | "on" | "off" | "~"
    ) {
        span(out, "kw", text);
        return;
    }
    let mut pos = 0;
    while pos < text.len() {
        let rest = &text[pos..];
        let ch = rest.chars().next().unwrap();
        if ch == '#' && (pos == 0 || text.as_bytes()[pos - 1].is_ascii_whitespace()) {
            span(out, "com", rest);
            return;
        }
        if matches!(ch, '"' | '\'') {
            let end = scan_quoted(text, pos, ch, false);
            span(out, "str", &text[pos..end]);
            pos = end;
            continue;
        }
        if ch == '$' && rest.starts_with("${") {
            let end = rest.find('}').map_or(text.len(), |i| pos + i + 1);
            span(out, "var", &text[pos..end]);
            pos = end;
            continue;
        }
        if ch.is_ascii_digit() {
            let end = scan_number(text, pos, true);
            span(out, "num", &text[pos..end]);
            pos = end;
            continue;
        }
        if matches!(ch, '&' | '*' | '!') && rest.chars().nth(1).is_some_and(is_ident_start) {
            let end = scan_ident_end(text, pos + 1);
            span(out, "attr", &text[pos..end]);
            pos = end;
            continue;
        }
        plain(out, ch.encode_utf8(&mut [0; 4]));
        pos += ch.len_utf8();
    }
}

fn find_unquoted(text: &str, needle: char) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (i, ch) in text.char_indices() {
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                }
            }
            None => {
                if matches!(ch, '"' | '\'') {
                    quote = Some(ch);
                } else if ch == needle {
                    return Some(i);
                }
            }
        }
    }
    None
}

fn highlight_toml(code: &str) -> String {
    let mut out = String::with_capacity(code.len() * 2);
    for line in code.split_inclusive('\n') {
        let content = line.strip_suffix('\n').unwrap_or(line);
        let trimmed = content.trim_start();
        let indent = &content[..content.len() - trimmed.len()];
        plain(&mut out, indent);
        if trimmed.is_empty() {
            if line.ends_with('\n') {
                plain(&mut out, "\n");
            }
            continue;
        }
        if trimmed.starts_with('#') {
            span(&mut out, "com", trimmed);
        } else if trimmed.starts_with('[') {
            let end = trimmed.find(']').map_or(trimmed.len(), |i| i + 1);
            span(&mut out, "prop", &trimmed[..end]);
            let rest = &trimmed[end..];
            highlight_toml_value(&mut out, rest);
        } else if let Some(pos) = find_unquoted(trimmed, '=') {
            let key = trimmed[..pos].trim_end();
            if !key.is_empty() {
                span(&mut out, "prop", key);
                plain(&mut out, &trimmed[key.len()..=pos]);
                highlight_toml_value(&mut out, trimmed[pos + 1..].trim_start());
            } else {
                highlight_toml_value(&mut out, trimmed);
            }
        } else {
            highlight_toml_value(&mut out, trimmed);
        }
        if line.ends_with('\n') {
            plain(&mut out, "\n");
        }
    }
    out
}

fn highlight_toml_value(out: &mut String, text: &str) {
    let mut pos = 0;
    while pos < text.len() {
        let rest = &text[pos..];
        let ch = rest.chars().next().unwrap();
        if ch == '#' {
            span(out, "com", rest);
            return;
        }
        if rest.starts_with("\"\"\"") || rest.starts_with("'''") {
            let quote = &rest[..3];
            let end = rest[3..]
                .find(quote)
                .map_or(text.len(), |i| pos + 3 + i + 3);
            span(out, "str", &text[pos..end]);
            pos = end;
            continue;
        }
        if matches!(ch, '"' | '\'') {
            let end = scan_quoted(text, pos, ch, false);
            span(out, "str", &text[pos..end]);
            pos = end;
            continue;
        }
        if matches!(ch, '&' | '*') {
            let end = scan_ident_end(text, pos + 1);
            span(out, "attr", &text[pos..end]);
            pos = end;
            continue;
        }
        if ch.is_ascii_digit() {
            let end = scan_number(text, pos, true);
            span(out, "num", &text[pos..end]);
            pos = end;
            continue;
        }
        if is_ident_start(ch) {
            let end = scan_ident_end(text, pos);
            let word = &text[pos..end];
            if matches!(word, "true" | "false" | "inf" | "nan" | "True" | "False") {
                span(out, "kw", word);
            } else {
                plain(out, word);
            }
            pos = end;
            continue;
        }
        plain(out, ch.encode_utf8(&mut [0; 4]));
        pos += ch.len_utf8();
    }
}

fn highlight_diff(code: &str) -> String {
    let mut out = String::with_capacity(code.len() * 2);
    for line in code.split_inclusive('\n') {
        let content = line.strip_suffix('\n').unwrap_or(line);
        let class = if content.starts_with("+++")
            || content.starts_with("---")
            || content.starts_with("diff ")
            || content.starts_with("index ")
            || content.starts_with("@@")
        {
            "meta"
        } else if content.starts_with('+') {
            "ins"
        } else if content.starts_with('-') {
            "del"
        } else {
            ""
        };
        if class.is_empty() {
            plain(&mut out, content);
        } else {
            span(&mut out, class, content);
        }
        if line.ends_with('\n') {
            plain(&mut out, "\n");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html(code: &str, lang: &str) -> String {
        highlight(code, lang).unwrap_or_else(|| crate::html::escape_html(code).into_owned())
    }

    #[test]
    fn normalizes_fence_labels() {
        assert_eq!(canonical_language("{.rust .numberLines}"), "rust");
        assert_eq!(canonical_language("language-ts"), "ts");
        assert_eq!(canonical_language("TypeScript"), "ts");
        assert_eq!(canonical_language(""), "text");
        assert_eq!(language_label("ts"), "TypeScript");
        assert_eq!(language_label("unknownthing"), "Unknownthing");
    }

    #[test]
    fn highlights_rust() {
        let out = html("fn main() { let x: u32 = 42; // hi\n}", "rust");
        assert!(out.contains("tok-kw\">fn</span>"));
        assert!(out.contains("tok-fn\">main</span>"));
        assert!(out.contains("tok-typ\">u32</span>"));
        assert!(out.contains("tok-num\">42</span>"));
        assert!(out.contains("tok-com\">// hi</span>"));
    }

    #[test]
    fn highlights_rust_raw_strings_and_lifetimes() {
        let out = html(r##"let s = r#"a < b"#; fn f<'a>(x: &'a str) {}"##, "rust");
        assert!(out.contains(r##"tok-str">r#"a &lt; b"#</span>"##));
        assert!(out.contains("'<span class=\"tok-typ\">a</span>"));
    }

    #[test]
    fn highlights_typescript_functions_and_templates() {
        let out = html(
            "const add = (a: number) => a + 1; const s = `x ${add(1)}`;",
            "ts",
        );
        assert!(out.contains("tok-kw\">const</span>"));
        assert!(out.contains("tok-typ\">number</span>"));
        assert!(out.contains("tok-str\">`x ${add(1)}`</span>"));
    }

    #[test]
    fn highlights_python_triple_strings() {
        let out = html(
            "def f(x):\n    \"\"\"doc <x>\"\"\"\n    return x # note",
            "python",
        );
        assert!(out.contains("tok-kw\">def</span>"));
        assert!(out.contains(r#"tok-str">"""doc &lt;x&gt;"""</span>"#));
        assert!(out.contains("tok-com\"># note</span>"));
    }

    #[test]
    fn escapes_everything_in_code() {
        for (code, lang) in [
            ("<script>alert(1)</script>", "html"),
            ("SELECT '<script>' FROM t;", "sql"),
            ("{\"a\": \"<x>\"}", "json"),
            ("echo \"<x>\"", "bash"),
        ] {
            let out = html(code, lang);
            assert!(!out.contains("<script>"), "{lang}: {out}");
            assert!(!out.contains("<x>"), "{lang}: {out}");
        }
    }

    #[test]
    fn highlights_json_keys_and_literals() {
        let out = html("{\"key\": 1, \"str\": \"ok\", \"flag\": true}", "json");
        assert!(out.contains(r#"tok-prop">"key"</span>"#));
        assert!(out.contains(r#"tok-prop">"str"</span>"#));
        assert!(out.contains(r#"tok-str">"ok"</span>"#));
        assert!(out.contains("tok-kw\">true</span>"));
    }

    #[test]
    fn highlights_markup_attributes() {
        let out = html("<a href=\"x\" data-ID='y'>hi</a>", "html");
        assert!(out.contains("tok-tag\">&lt;</span>"));
        assert!(out.contains("tok-tag\">a</span>"));
        assert!(out.contains("tok-attr\">href</span>"));
        assert!(out.contains(r#"tok-str">"x"</span>"#));
    }

    #[test]
    fn highlights_css_properties() {
        let out = html("a { color: #fff; margin: 1.5rem; }", "css");
        assert!(out.contains("tok-prop\">color</span>"));
        assert!(out.contains("tok-num\">#fff</span>"));
        assert!(out.contains("tok-num\">1.5rem</span>"));
    }

    #[test]
    fn highlights_yaml_and_toml() {
        let y = html("title: \"Hello\"\nlist:\n  - one\nflag: true\n", "yaml");
        assert!(y.contains("tok-prop\">title</span>"));
        assert!(y.contains(r#"tok-str">"Hello"</span>"#));
        assert!(y.contains("tok-kw\">true</span>"));

        let t = html("[package]\nname = \"folio\"\nversion = 1.0\n", "toml");
        assert!(t.contains("tok-prop\">[package]</span>"));
        assert!(t.contains(r#"tok-str">"folio"</span>"#));
    }

    #[test]
    fn highlights_diff_lines() {
        let out = html("--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new\n same", "diff");
        assert!(out.contains("tok-meta\">--- a/x</span>"));
        assert!(out.contains("tok-ins\">+new</span>"));
        assert!(out.contains("tok-del\">-old</span>"));
    }

    #[test]
    fn unknown_language_is_plain() {
        assert!(highlight("fn main() {}", "brainfuck").is_none());
        assert!(highlight("fn main() {}", "text").is_none());
    }

    #[test]
    fn does_not_panic_on_pathological_input() {
        for code in [
            "'",
            "r",
            "r#",
            "r#\"unterminated",
            "\"\"\"",
            "0x",
            "/*",
            "`",
            "@",
            "#!/bin/sh\n",
        ] {
            for lang in [
                "rust", "ts", "python", "bash", "json", "html", "css", "yaml", "toml",
            ] {
                let _ = highlight(code, lang);
            }
        }
    }
}
