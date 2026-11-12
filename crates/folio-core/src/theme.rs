//! Themes are data, not code.
//!
//! Each theme is a small JSON document of design tokens. Folio validates every
//! value and turns the tokens into CSS custom properties, so a custom theme
//! cannot smuggle arbitrary CSS into the app.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Light,
    Dark,
}

impl Appearance {
    pub fn as_str(self) -> &'static str {
        match self {
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Colors {
    pub canvas: String,
    pub surface: String,
    pub surface_muted: String,
    pub ink: String,
    pub muted: String,
    pub subtle: String,
    pub accent: String,
    pub accent_strong: String,
    pub accent_soft: String,
    pub accent_line: String,
    pub warm: String,
    pub warm_soft: String,
    pub rule: String,
    pub selection: String,
    pub shadow: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Syntax {
    pub comment: String,
    pub string: String,
    pub number: String,
    pub keyword: String,
    pub function: String,
    pub type_name: String,
    pub property: String,
    pub tag: String,
    pub attribute: String,
    pub punctuation: String,
    pub operator: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fonts {
    pub body: String,
    pub display: String,
    pub mono: String,
}

impl Default for Fonts {
    fn default() -> Self {
        Self {
            body: "\"Manrope\", ui-sans-serif, -apple-system, sans-serif".into(),
            display: "\"Newsreader\", ui-serif, \"New York\", Georgia, serif".into(),
            mono: "ui-monospace, \"SF Mono\", Menlo, monospace".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Layout {
    pub measure: String,
    pub radius: String,
    pub body_size: String,
    pub line_height: f32,
    pub title_size: String,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            measure: "42rem".into(),
            radius: "14px".into(),
            body_size: "17px".into(),
            line_height: 1.72,
            title_size: "3.35rem".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Theme {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub appearance: Appearance,
    /// Theme to use in automatic mode when the system appearance is the
    /// opposite of this theme's appearance.
    #[serde(default)]
    pub counterpart: Option<String>,
    pub colors: Colors,
    #[serde(default)]
    pub fonts: Fonts,
    #[serde(default)]
    pub layout: Layout,
    pub syntax: Syntax,
}

impl Theme {
    pub fn validate(&self) -> Result<(), String> {
        validate_id(&self.id)?;
        if self.name.trim().is_empty() || self.name.chars().count() > 60 {
            return Err("name must be 1–60 characters".into());
        }
        if self.name.chars().any(char::is_control) {
            return Err("name contains control characters".into());
        }
        if self.description.chars().count() > 300 {
            return Err("description must be at most 300 characters".into());
        }
        if self.description.chars().any(char::is_control) {
            return Err("description contains control characters".into());
        }
        if let Some(counterpart) = &self.counterpart {
            validate_id(counterpart).map_err(|e| format!("counterpart {e}"))?;
            if counterpart == &self.id {
                return Err("counterpart cannot be the theme itself".into());
            }
        }

        let c = &self.colors;
        for (label, value) in [
            ("colors.canvas", &c.canvas),
            ("colors.surface", &c.surface),
            ("colors.surfaceMuted", &c.surface_muted),
            ("colors.ink", &c.ink),
            ("colors.muted", &c.muted),
            ("colors.subtle", &c.subtle),
            ("colors.accent", &c.accent),
            ("colors.accentStrong", &c.accent_strong),
            ("colors.accentSoft", &c.accent_soft),
            ("colors.accentLine", &c.accent_line),
            ("colors.warm", &c.warm),
            ("colors.warmSoft", &c.warm_soft),
            ("colors.rule", &c.rule),
            ("colors.selection", &c.selection),
            ("colors.shadow", &c.shadow),
        ] {
            validate_color(label, value)?;
        }
        for (label, value) in [
            ("syntax.comment", &self.syntax.comment),
            ("syntax.string", &self.syntax.string),
            ("syntax.number", &self.syntax.number),
            ("syntax.keyword", &self.syntax.keyword),
            ("syntax.function", &self.syntax.function),
            ("syntax.typeName", &self.syntax.type_name),
            ("syntax.property", &self.syntax.property),
            ("syntax.tag", &self.syntax.tag),
            ("syntax.attribute", &self.syntax.attribute),
            ("syntax.punctuation", &self.syntax.punctuation),
            ("syntax.operator", &self.syntax.operator),
        ] {
            validate_color(label, value)?;
        }

        validate_font("fonts.body", &self.fonts.body)?;
        validate_font("fonts.display", &self.fonts.display)?;
        validate_font("fonts.mono", &self.fonts.mono)?;

        validate_length(
            "layout.measure",
            &self.layout.measure,
            18.0,
            110.0,
            &["rem", "em", "ch", "px"],
        )?;
        validate_length(
            "layout.radius",
            &self.layout.radius,
            0.0,
            1.8,
            &["px", "rem"],
        )?;
        validate_length(
            "layout.bodySize",
            &self.layout.body_size,
            0.7,
            1.7,
            &["px", "rem"],
        )?;
        validate_length(
            "layout.titleSize",
            &self.layout.title_size,
            1.7,
            5.6,
            &["rem", "px", "em"],
        )?;
        if !(1.2..=2.2).contains(&self.layout.line_height) {
            return Err("layout.lineHeight must be between 1.2 and 2.2".into());
        }
        Ok(())
    }

    /// Serialize the theme as CSS custom properties.
    pub fn css(&self) -> String {
        let mut css = String::with_capacity(1800);
        css.push_str(":root[data-theme=\"");
        css.push_str(&self.id);
        css.push_str("\"] {\n");
        let colors = [
            ("--f-canvas", &self.colors.canvas),
            ("--f-surface", &self.colors.surface),
            ("--f-surface-muted", &self.colors.surface_muted),
            ("--f-ink", &self.colors.ink),
            ("--f-muted", &self.colors.muted),
            ("--f-subtle", &self.colors.subtle),
            ("--f-accent", &self.colors.accent),
            ("--f-accent-strong", &self.colors.accent_strong),
            ("--f-accent-soft", &self.colors.accent_soft),
            ("--f-accent-line", &self.colors.accent_line),
            ("--f-warm", &self.colors.warm),
            ("--f-warm-soft", &self.colors.warm_soft),
            ("--f-rule", &self.colors.rule),
            ("--f-selection", &self.colors.selection),
            ("--f-shadow-color", &self.colors.shadow),
        ];
        for (name, value) in colors {
            css.push_str("  ");
            css.push_str(name);
            css.push_str(": ");
            css.push_str(value.trim());
            css.push_str(";\n");
        }
        let syntax = [
            ("--f-syntax-comment", &self.syntax.comment),
            ("--f-syntax-string", &self.syntax.string),
            ("--f-syntax-number", &self.syntax.number),
            ("--f-syntax-keyword", &self.syntax.keyword),
            ("--f-syntax-function", &self.syntax.function),
            ("--f-syntax-type", &self.syntax.type_name),
            ("--f-syntax-property", &self.syntax.property),
            ("--f-syntax-tag", &self.syntax.tag),
            ("--f-syntax-attribute", &self.syntax.attribute),
            ("--f-syntax-punctuation", &self.syntax.punctuation),
            ("--f-syntax-operator", &self.syntax.operator),
        ];
        for (name, value) in syntax {
            css.push_str("  ");
            css.push_str(name);
            css.push_str(": ");
            css.push_str(value.trim());
            css.push_str(";\n");
        }
        for (name, value) in [
            ("--f-font-body", &self.fonts.body),
            ("--f-font-display", &self.fonts.display),
            ("--f-font-mono", &self.fonts.mono),
            ("--f-measure", &self.layout.measure),
            ("--f-radius", &self.layout.radius),
            ("--f-body-size", &self.layout.body_size),
            ("--f-title-size", &self.layout.title_size),
        ] {
            css.push_str("  ");
            css.push_str(name);
            css.push_str(": ");
            css.push_str(value.trim());
            css.push_str(";\n");
        }
        css.push_str("  --f-line-height: ");
        css.push_str(&format_float(self.layout.line_height));
        css.push_str(";\n}\n");
        css
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThemeCatalog {
    pub themes: Vec<Theme>,
    pub css: String,
    pub default_id: String,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ThemeSet {
    themes: Vec<Theme>,
    errors: Vec<String>,
}

impl ThemeSet {
    /// The six themes that ship with the app.
    pub fn builtin() -> Self {
        Self {
            themes: builtin_themes().clone(),
            errors: Vec::new(),
        }
    }

    /// Load additional `*.json` themes from directories. Invalid themes are
    /// skipped and reported; they never break startup.
    pub fn with_user_dirs(mut self, dirs: &[PathBuf]) -> Self {
        let mut seen: BTreeSet<String> = self.themes.iter().map(|t| t.id.clone()).collect();
        for dir in dirs {
            let entries = match std::fs::read_dir(dir) {
                Ok(entries) => entries,
                Err(_) => continue,
            };
            let mut files: Vec<PathBuf> = entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case("json"))
                })
                .collect();
            files.sort();
            for path in files {
                match load_theme(&path) {
                    Ok(theme) => {
                        if seen.contains(&theme.id) {
                            self.errors.push(format!(
                                "{}: theme id \"{}\" is already in use",
                                path.display(),
                                theme.id
                            ));
                        } else {
                            seen.insert(theme.id.clone());
                            self.themes.push(theme);
                        }
                    }
                    Err(error) => self.errors.push(format!("{}: {error}", path.display())),
                }
            }
        }
        self
    }

    pub fn all(&self) -> &[Theme] {
        &self.themes
    }

    pub fn get(&self, id: &str) -> Option<&Theme> {
        self.themes.iter().find(|theme| theme.id == id)
    }

    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    pub fn default_theme(&self) -> &Theme {
        self.get(DEFAULT_LIGHT)
            .or_else(|| self.themes.first())
            .expect("the built-in theme set is never empty")
    }

    pub fn css(&self) -> String {
        let mut css = String::with_capacity(self.themes.len() * 1800);
        for theme in &self.themes {
            css.push_str(&theme.css());
        }
        css
    }

    pub fn catalog(&self) -> ThemeCatalog {
        ThemeCatalog {
            themes: self.themes.clone(),
            css: self.css(),
            default_id: self.default_theme().id.clone(),
            errors: self.errors.clone(),
        }
    }
}

pub const DEFAULT_LIGHT: &str = "blend";
pub const DEFAULT_DARK: &str = "blend-dark";

fn builtin_themes() -> &'static Vec<Theme> {
    static THEMES: OnceLock<Vec<Theme>> = OnceLock::new();
    THEMES.get_or_init(|| {
        let sources = [
            include_str!("../themes/blend.json"),
            include_str!("../themes/blend-dark.json"),
            include_str!("../themes/editorial.json"),
            include_str!("../themes/editorial-dark.json"),
            include_str!("../themes/studio.json"),
            include_str!("../themes/studio-dark.json"),
        ];
        sources
            .iter()
            .map(|source| {
                let theme: Theme =
                    serde_json::from_str(source).expect("built-in theme is valid JSON");
                theme
                    .validate()
                    .unwrap_or_else(|e| panic!("built-in theme {} is invalid: {e}", theme.id));
                theme
            })
            .collect()
    })
}

fn load_theme(path: &Path) -> Result<Theme, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let theme: Theme = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    theme.validate()?;
    Ok(theme)
}

/// A ready-to-edit theme document, based on Blend.
pub fn scaffold_theme(id: &str) -> Result<String, String> {
    validate_id(id)?;
    let mut theme = builtin_themes()[0].clone();
    theme.id = id.to_string();
    theme.name = title_case(id);
    theme.description = format!("A custom Folio theme called {}.", theme.name);
    theme.counterpart = None;
    serde_json::to_string_pretty(&theme).map_err(|e| e.to_string())
}

pub fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > 48 {
        return Err("id must be 1–48 characters".into());
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("id may only contain lowercase letters, digits, and hyphens".into());
    }
    if !id.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit()) {
        return Err("id must start with a lowercase letter or digit".into());
    }
    if id.ends_with('-') {
        return Err("id cannot end with a hyphen".into());
    }
    Ok(())
}

fn title_case(id: &str) -> String {
    id.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn validate_color(label: &str, value: &str) -> Result<(), String> {
    let value = value.trim();
    let valid = if let Some(hex) = value.strip_prefix('#') {
        matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit())
    } else if let Some(inner) = value
        .strip_prefix("rgb(")
        .and_then(|v| v.strip_suffix(')'))
        .or_else(|| {
            value
                .strip_prefix("rgba(")
                .and_then(|v| v.strip_suffix(')'))
        })
    {
        !inner.is_empty()
            && inner
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '%' | ' ' | '/' | '+' | '-'))
    } else {
        value == "transparent"
    };
    if valid {
        Ok(())
    } else {
        Err(format!(
            "{label} must be a hex color, rgb()/rgba() value, or transparent"
        ))
    }
}

fn validate_font(label: &str, value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 160 {
        return Err(format!("{label} must be 1–160 characters"));
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("url(")
        || trimmed
            .chars()
            .any(|c| matches!(c, ';' | '{' | '}' | '<' | '>' | '@' | '\\' | '\n' | '\r'))
    {
        return Err(format!("{label} contains unsafe characters"));
    }
    if !trimmed.chars().all(|c| {
        c.is_alphanumeric()
            || c.is_whitespace()
            || matches!(c, '"' | '\'' | ',' | '-' | '_' | '.' | '+')
    }) {
        return Err(format!("{label} contains unsupported characters"));
    }
    Ok(())
}

fn validate_length(
    label: &str,
    value: &str,
    min: f32,
    max: f32,
    units: &[&str],
) -> Result<(), String> {
    let value = value.trim();
    let split = value
        .find(|c: char| c.is_ascii_alphabetic())
        .unwrap_or(value.len());
    let (number, unit) = (&value[..split], &value[split..]);
    let number: f32 = number
        .parse()
        .map_err(|_| format!("{label} must be a number followed by a unit"))?;
    if !units.contains(&unit) {
        return Err(format!("{label} must use one of: {}", units.join(", ")));
    }
    // Compare in rem-equivalents so px values are held to the same scale.
    let normalized = if unit == "px" { number / 16.0 } else { number };
    if !(min..=max).contains(&normalized) {
        return Err(format!("{label} is outside the supported range"));
    }
    Ok(())
}

fn format_float(value: f32) -> String {
    let mut text = format!("{value}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_valid_and_ordered() {
        let set = ThemeSet::builtin();
        assert_eq!(set.all().len(), 6);
        assert_eq!(set.default_theme().id, "blend");
        assert_eq!(set.get("blend").unwrap().appearance, Appearance::Light);
        assert_eq!(
            set.get("blend").unwrap().counterpart.as_deref(),
            Some("blend-dark")
        );
        for theme in set.all() {
            theme.validate().unwrap();
        }
    }

    #[test]
    fn css_contains_tokens_and_is_scoped() {
        let set = ThemeSet::builtin();
        let css = set.css();
        assert!(css.contains(":root[data-theme=\"blend\"]"));
        assert!(css.contains("--f-canvas: #FAF9F6;"));
        assert!(css.contains("--f-font-display:"));
        assert!(css.contains("--f-syntax-keyword:"));
        assert!(!css.contains("</style>"));
        assert_eq!(css.matches(":root[data-theme=").count(), set.all().len());
    }

    #[test]
    fn rejects_injection_attempts() {
        let mut theme = ThemeSet::builtin().get("blend").unwrap().clone();
        theme.colors.canvas = "#fff; } body { display:none".into();
        assert!(theme.validate().is_err());

        theme.colors.canvas = "red".into();
        assert!(theme.validate().is_err());

        theme.colors.canvas = "#FAF9F6".into();
        theme.fonts.body = "Manrope; } body { background: url(//evil)".into();
        assert!(theme.validate().is_err());

        theme.fonts.body = "Manrope, serif".into();
        theme.layout.measure = "calc(100vw)".into();
        assert!(theme.validate().is_err());

        theme.layout.measure = "42rem".into();
        theme.layout.line_height = 9.0;
        assert!(theme.validate().is_err());
        assert!(theme.validate().is_err());
    }

    #[test]
    fn rejects_bad_ids() {
        for id in ["", "Blend", "a b", "x_1", "-x", "x-", "ä"] {
            assert!(validate_id(id).is_err(), "{id}");
        }
        assert!(validate_id("my-theme-2").is_ok());
    }

    #[test]
    fn scaffold_roundtrips() {
        let text = scaffold_theme("custom-paper").unwrap();
        let theme: Theme = serde_json::from_str(&text).unwrap();
        theme.validate().unwrap();
        assert_eq!(theme.id, "custom-paper");
        assert_eq!(theme.name, "Custom Paper");
        assert!(scaffold_theme("Bad Id").is_err());
    }

    #[test]
    fn user_theme_loading_reports_errors_without_failing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("good.json"),
            serde_json::to_string_pretty(&{
                let mut t = ThemeSet::builtin().get("blend").unwrap().clone();
                t.id = "good".into();
                t.name = "Good".into();
                t
            })
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("bad.json"),
            r#"{"id":"bad","name":"Bad","appearance":"light","colors":{},"syntax":{}}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("ignored.txt"), "nope").unwrap();

        let set = ThemeSet::builtin().with_user_dirs(&[dir.path().to_path_buf()]);
        assert!(set.get("good").is_some());
        assert_eq!(set.errors().len(), 1);
        assert!(set.errors()[0].contains("bad.json"));
    }

    #[test]
    fn formatting_is_stable() {
        assert_eq!(format_float(1.720), "1.72");
        assert_eq!(format_float(2.0), "2");
        assert_eq!(format_float(1.5), "1.5");
    }
}
