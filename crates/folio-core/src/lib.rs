//! Folio's rendering engine.
//!
//! Pure, host-independent Markdown → HTML rendering with a small theme system.
//! The desktop shell adds file watching and native dialogs; everything that
//! turns Markdown into a readable document lives here.

pub mod frontmatter;
pub mod highlight;
pub mod html;
pub mod markdown;
pub mod paths;
pub mod theme;

pub use frontmatter::{parse_frontmatter, FrontMatter};
pub use markdown::{render, DocumentMeta, Heading, RenderOptions, Rendered, Stats};
pub use theme::{scaffold_theme, Appearance, Theme, ThemeCatalog, ThemeSet};

/// The app version, shared with the shell.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
