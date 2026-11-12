//! Markdown → HTML document renderer.
//!
//! The renderer is event driven and produces a single, self-contained HTML
//! fragment plus metadata (outline, word count, frontmatter). It is the only
//! place Markdown is interpreted; the UI never parses Markdown.

use crate::frontmatter::{parse_frontmatter, FrontMatter};
use crate::highlight::{canonical_language, highlight, language_label};
use crate::html::{count_words, escape_html, push_escaped, push_escaped_attr, slugify, truncate};
use crate::paths::{resolve_image, resolve_link, LocalKind, ResolvedImage, ResolvedLink};
use pulldown_cmark::{
    Alignment, BlockQuoteKind, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd,
};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const TRANSPARENT_PIXEL: &str =
    "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7";

#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Directory used to resolve relative links and images.
    pub base_dir: Option<PathBuf>,
    /// Emit host-resolvable data attributes for local paths instead of raw
    /// relative URLs. The desktop app enables this.
    pub localize: bool,
    /// Enable syntax highlighting.
    pub highlight: bool,
    /// Curly quotes, em dashes, ellipses.
    pub smart_punctuation: bool,
    /// Treat single newlines as hard breaks.
    pub hard_breaks: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            base_dir: None,
            localize: false,
            highlight: true,
            smart_punctuation: true,
            hard_breaks: false,
        }
    }
}

impl RenderOptions {
    pub fn for_file(path: &Path) -> Self {
        Self {
            base_dir: path.parent().map(Path::to_path_buf),
            localize: true,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub words: usize,
    pub minutes: usize,
    pub headings: usize,
    pub links: usize,
    pub images: usize,
    pub code_blocks: usize,
    pub code_lines: usize,
    pub tables: usize,
    pub footnotes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Heading {
    pub level: u8,
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentMeta {
    pub title: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub date: Option<String>,
    pub tags: Vec<String>,
    pub outline: Vec<Heading>,
    pub stats: Stats,
    pub has_h1: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Rendered {
    pub html: String,
    pub meta: DocumentMeta,
}

/// Render a Markdown document.
pub fn render(markdown: &str, options: &RenderOptions) -> Rendered {
    let normalized: Cow<'_, str> = if markdown.contains('\r') {
        Cow::Owned(markdown.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Cow::Borrowed(markdown)
    };
    let (frontmatter, body) = parse_frontmatter(normalized.as_ref());
    let mut renderer = Renderer::new(options, &frontmatter);
    renderer.out.reserve(markdown.len() / 2 + 256);
    let mut opts = Options::empty();
    opts.insert(
        Options::ENABLE_TABLES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_FOOTNOTES
            | Options::ENABLE_GFM,
    );
    if options.smart_punctuation {
        opts.insert(Options::ENABLE_SMART_PUNCTUATION);
    }
    let mut parser = Parser::new_ext(body, opts).peekable();
    while let Some(event) = parser.next() {
        if let Event::Start(Tag::Item) = &event {
            renderer.item_is_task = matches!(parser.peek(), Some(Event::TaskListMarker(_)));
        }
        renderer.event(event);
    }
    renderer.finish(markdown.len())
}

struct Renderer<'a> {
    options: &'a RenderOptions,
    out: String,
    frames: Vec<Frame>,
    footnote_html: String,
    footnote_numbers: HashMap<String, usize>,
    footnote_ref_counts: HashMap<String, usize>,
    defined_footnotes: usize,
    slug_counts: HashMap<String, usize>,
    outline: Vec<Heading>,
    stats: Stats,
    meta: DocumentMeta,
    first_h1_text: Option<String>,
    first_para_text: Option<String>,
    para_capture: Option<String>,
    heading_capture: Option<String>,
    has_h1: bool,
    after_title: bool,
    blocks_started: usize,
    table_alignments: Vec<Vec<Alignment>>,
    table_in_head: bool,
    table_cell: usize,
    in_code: bool,
    item_is_task: bool,
}

enum FrameKind {
    Paragraph { image: Option<ImageInline> },
    Heading,
    Link { dest: String, title: String },
    Image { dest: String, title: String },
    Code { info: String },
    Footnote { number: usize },
    RawHtml,
}

struct Frame {
    kind: FrameKind,
    html: String,
    text: String,
}

#[derive(Clone)]
struct ImageInline {
    html: String,
    caption: String,
}

impl<'a> Renderer<'a> {
    fn new(options: &'a RenderOptions, fm: &FrontMatter) -> Self {
        let meta = DocumentMeta {
            title: fm.title.clone(),
            description: fm.description.clone(),
            author: fm.author.clone(),
            date: fm.date.clone(),
            tags: fm.tags.clone(),
            ..DocumentMeta::default()
        };
        Self {
            options,
            out: String::new(),
            frames: Vec::new(),
            footnote_html: String::new(),
            footnote_numbers: HashMap::new(),
            footnote_ref_counts: HashMap::new(),
            defined_footnotes: 0,
            slug_counts: HashMap::new(),
            outline: Vec::new(),
            stats: Stats::default(),
            meta,
            first_h1_text: None,
            first_para_text: None,
            para_capture: None,
            heading_capture: None,
            has_h1: false,
            after_title: false,
            blocks_started: 0,
            table_alignments: Vec::new(),
            table_in_head: false,
            table_cell: 0,
            in_code: false,
            item_is_task: false,
        }
    }

    fn finish(mut self, _source_len: usize) -> Rendered {
        if !self.footnote_html.is_empty() {
            self.out
                .push_str("<section class=\"footnotes\" aria-label=\"Footnotes\">\n");
            self.out
                .push_str("<h2 class=\"footnotes-title\">Notes</h2>\n");
            self.out.push_str(&self.footnote_html);
            self.out.push_str("</section>\n");
        }
        self.stats.footnotes = self.defined_footnotes;
        self.stats.headings = self.outline.len();
        self.stats.minutes = if self.stats.words == 0 {
            0
        } else {
            ((self.stats.words as f64) / 230.0).ceil() as usize
        };
        self.meta.title = self.meta.title.take().or(self.first_h1_text.clone());
        if self.meta.description.is_none() {
            if let Some(text) = self.first_para_text.as_deref() {
                let trimmed = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if !trimmed.is_empty() {
                    self.meta.description = Some(truncate(&trimmed, 200));
                }
            }
        }
        self.meta.outline = self.outline;
        self.meta.stats = self.stats;
        self.meta.has_h1 = self.has_h1;
        if !self.out.ends_with('\n') {
            self.out.push('\n');
        }
        Rendered {
            html: self.out,
            meta: self.meta,
        }
    }

    /// Append a break to an image's alt text. Returns true when handled.
    fn append_alt(&mut self, text: &str) -> bool {
        let is_image = matches!(
            self.frames.last().map(|f| &f.kind),
            Some(FrameKind::Image { .. })
        );
        if is_image {
            if let Some(frame) = self.frames.last_mut() {
                frame.text.push_str(text);
            }
        }
        is_image
    }

    fn write(&mut self, text: &str) {
        match self.frames.last_mut() {
            Some(frame) => frame.html.push_str(text),
            None => self.out.push_str(text),
        }
    }

    fn write_escaped(&mut self, text: &str) {
        match self.frames.last_mut() {
            Some(frame) => push_escaped(&mut frame.html, text),
            None => push_escaped(&mut self.out, text),
        }
    }

    fn pop(&mut self) -> Frame {
        self.frames.pop().expect("balanced pulldown-cmark events")
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.text(&text),
            Event::Code(code) => self.inline_code(&code),
            Event::Html(html) => {
                // Raw HTML is never executed. Blocks are shown as source;
                // inline tags are shown as literal text.
                self.write_escaped(&html);
            }
            Event::InlineHtml(html) => self.write_escaped(&html),
            Event::SoftBreak => {
                if let Some(capture) = &mut self.para_capture {
                    capture.push(' ');
                }
                if let Some(capture) = &mut self.heading_capture {
                    capture.push(' ');
                }
                if self.append_alt(" ") {
                    // inside an image description; nothing to render
                } else if self.options.hard_breaks {
                    self.write("<br />\n");
                } else {
                    self.write("\n");
                }
            }
            Event::HardBreak => {
                if let Some(capture) = &mut self.para_capture {
                    capture.push(' ');
                }
                if let Some(capture) = &mut self.heading_capture {
                    capture.push(' ');
                }
                if !self.append_alt(" ") {
                    self.write("<br />\n");
                }
            }
            Event::Rule => {
                self.after_title = false;
                self.blocks_started += 1;
                self.write("<hr />\n");
            }
            Event::FootnoteReference(label) => self.footnote_reference(&label),
            Event::TaskListMarker(checked) => self.task_marker(checked),
            Event::InlineMath(math) => {
                self.write("<span class=\"math\">$");
                self.write_escaped(&math);
                self.write("$</span>");
            }
            Event::DisplayMath(math) => {
                self.write("<div class=\"math math-display\">$$");
                self.write_escaped(&math);
                self.write("$$</div>\n");
            }
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {
                if self.blocks_started <= 2
                    && self.meta.description.is_none()
                    && self.first_para_text.is_none()
                    && self.para_capture.is_none()
                {
                    self.para_capture = Some(String::new());
                }
                self.blocks_started += 1;
                self.frames.push(Frame {
                    kind: FrameKind::Paragraph { image: None },
                    html: String::new(),
                    text: String::new(),
                });
            }
            Tag::Heading { level, .. } => {
                self.after_title = false;
                self.blocks_started += 1;
                self.heading_capture = Some(String::new());
                let _ = level;
                self.frames.push(Frame {
                    kind: FrameKind::Heading,
                    html: String::new(),
                    text: String::new(),
                });
            }
            Tag::BlockQuote(kind) => {
                self.after_title = false;
                self.blocks_started += 1;
                match kind {
                    Some(kind) => {
                        self.write("<aside class=\"callout\" data-kind=\"");
                        self.write(callout_slug(kind));
                        self.write("\" role=\"note\">\n<p class=\"callout-label\">");
                        self.write(callout_label(kind));
                        self.write("</p>\n");
                    }
                    None => self.write("<blockquote>\n"),
                }
            }
            Tag::CodeBlock(kind) => {
                self.after_title = false;
                self.blocks_started += 1;
                self.in_code = true;
                let info = match kind {
                    CodeBlockKind::Fenced(info) => info.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.frames.push(Frame {
                    kind: FrameKind::Code { info },
                    html: String::new(),
                    text: String::new(),
                });
            }
            Tag::HtmlBlock => {
                self.after_title = false;
                self.blocks_started += 1;
                self.frames.push(Frame {
                    kind: FrameKind::RawHtml,
                    html: String::new(),
                    text: String::new(),
                });
            }
            Tag::List(start) => {
                self.after_title = false;
                self.blocks_started += 1;
                match start {
                    None => self.write("<ul>\n"),
                    Some(1) => self.write("<ol>\n"),
                    Some(start) => {
                        self.write("<ol start=\"");
                        self.write(&start.to_string());
                        self.write("\">\n");
                    }
                }
            }
            Tag::Item => {
                if self.item_is_task {
                    self.write("<li class=\"task-item\">");
                    self.item_is_task = false;
                } else {
                    self.write("<li>");
                }
            }
            Tag::FootnoteDefinition(label) => {
                self.after_title = false;
                self.blocks_started += 1;
                let number = self.footnote_number(&label);
                self.frames.push(Frame {
                    kind: FrameKind::Footnote { number },
                    html: String::new(),
                    text: String::new(),
                });
            }
            Tag::DefinitionList => self.write("<dl>\n"),
            Tag::DefinitionListTitle => self.write("<dt>"),
            Tag::DefinitionListDefinition => self.write("<dd>"),
            Tag::Table(alignments) => {
                self.after_title = false;
                self.blocks_started += 1;
                self.stats.tables += 1;
                self.table_alignments.push(alignments);
                self.write("<div class=\"table-wrap\"><table>\n");
            }
            Tag::TableHead => {
                self.table_in_head = true;
                self.write("<thead>\n");
            }
            Tag::TableRow => {
                self.table_cell = 0;
                self.write("<tr>");
            }
            Tag::TableCell => {
                let index = self.table_cell;
                self.table_cell += 1;
                let class = self
                    .table_alignments
                    .last()
                    .and_then(|a| a.get(index))
                    .copied()
                    .unwrap_or(Alignment::None);
                if self.table_in_head {
                    self.write("<th");
                } else {
                    self.write("<td");
                }
                match class {
                    Alignment::Center => self.write(" class=\"align-center\""),
                    Alignment::Right => self.write(" class=\"align-right\""),
                    _ => {}
                }
                self.write(">");
            }
            Tag::Emphasis => self.write("<em>"),
            Tag::Strong => self.write("<strong>"),
            Tag::Strikethrough => self.write("<del>"),
            Tag::Superscript => self.write("<sup>"),
            Tag::Subscript => self.write("<sub>"),
            Tag::Link {
                dest_url, title, ..
            } => {
                self.frames.push(Frame {
                    kind: FrameKind::Link {
                        dest: dest_url.to_string(),
                        title: title.to_string(),
                    },
                    html: String::new(),
                    text: String::new(),
                });
            }
            Tag::Image {
                dest_url, title, ..
            } => {
                self.frames.push(Frame {
                    kind: FrameKind::Image {
                        dest: dest_url.to_string(),
                        title: title.to_string(),
                    },
                    html: String::new(),
                    text: String::new(),
                });
            }
            Tag::MetadataBlock(_) => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.end_paragraph(),
            TagEnd::Heading(level) => self.end_heading(heading_level(level)),
            TagEnd::BlockQuote(kind) => {
                if kind.is_some() {
                    self.write("</aside>\n");
                } else {
                    self.write("</blockquote>\n");
                }
            }
            TagEnd::CodeBlock => self.end_code_block(),
            TagEnd::HtmlBlock => {
                let frame = self.pop();
                let mut html = String::with_capacity(frame.html.len() + 80);
                html.push_str(
                    "<pre class=\"raw-html\" aria-label=\"Raw HTML, not rendered\"><code>",
                );
                html.push_str(&frame.html);
                html.push_str("</code></pre>\n");
                self.write(&html);
            }
            TagEnd::List(ordered) => {
                self.write(if ordered { "</ol>\n" } else { "</ul>\n" });
            }
            TagEnd::Item => self.write("</li>\n"),
            TagEnd::FootnoteDefinition => self.end_footnote_definition(),
            TagEnd::DefinitionList => self.write("</dl>\n"),
            TagEnd::DefinitionListTitle => self.write("</dt>\n"),
            TagEnd::DefinitionListDefinition => self.write("</dd>\n"),
            TagEnd::Table => {
                self.write("</tbody></table></div>\n");
                self.table_alignments.pop();
            }
            TagEnd::TableHead => {
                self.table_in_head = false;
                self.write("</thead>\n<tbody>\n");
            }
            TagEnd::TableRow => self.write("</tr>\n"),
            TagEnd::TableCell => {
                if self.table_in_head {
                    self.write("</th>");
                } else {
                    self.write("</td>");
                }
            }
            TagEnd::Emphasis => self.write("</em>"),
            TagEnd::Strong => self.write("</strong>"),
            TagEnd::Strikethrough => self.write("</del>"),
            TagEnd::Superscript => self.write("</sup>"),
            TagEnd::Subscript => self.write("</sub>"),
            TagEnd::Link => self.end_link(),
            TagEnd::Image => self.end_image(),
            TagEnd::MetadataBlock(_) => {}
        }
    }

    fn text(&mut self, text: &str) {
        if self.in_code {
            if let Some(frame) = self.frames.last_mut() {
                frame.text.push_str(text);
            }
            return;
        }

        let image = matches!(
            self.frames.last().map(|f| &f.kind),
            Some(FrameKind::Image { .. })
        );
        if image {
            if let Some(frame) = self.frames.last_mut() {
                frame.text.push_str(text);
            }
        } else if let Some(frame) = self.frames.last_mut() {
            push_escaped(&mut frame.html, text);
        } else {
            push_escaped(&mut self.out, text);
        }

        if let Some(capture) = &mut self.heading_capture {
            capture.push_str(text);
        }
        if let Some(capture) = &mut self.para_capture {
            capture.push_str(text);
        }
        self.stats.words += count_words(text);
    }

    fn inline_code(&mut self, code: &str) {
        if self.in_code {
            if let Some(frame) = self.frames.last_mut() {
                frame.text.push_str(code);
            }
            return;
        }
        let image = matches!(
            self.frames.last().map(|f| &f.kind),
            Some(FrameKind::Image { .. })
        );
        if image {
            if let Some(frame) = self.frames.last_mut() {
                frame.text.push_str(code);
            }
        } else {
            self.write("<code>");
            self.write_escaped(code);
            self.write("</code>");
        }
        if let Some(capture) = &mut self.heading_capture {
            capture.push_str(code);
        }
        if let Some(capture) = &mut self.para_capture {
            capture.push_str(code);
        }
        self.stats.words += count_words(code);
    }

    fn end_paragraph(&mut self) {
        let frame = self.pop();
        let FrameKind::Paragraph { image } = frame.kind else {
            return;
        };
        if let Some(capture) = self.para_capture.take() {
            let text = capture.split_whitespace().collect::<Vec<_>>().join(" ");
            if !text.is_empty() {
                self.first_para_text = Some(text);
            }
        }
        let html = frame.html;
        if html.trim().is_empty() {
            return;
        }
        let figure = image.filter(|image| html.trim() == image.html.trim());
        if let Some(image) = figure {
            self.write("<figure class=\"doc-figure\">");
            self.write(&image.html);
            if !image.caption.is_empty() {
                self.write("<figcaption>");
                self.write_escaped(&image.caption);
                self.write("</figcaption>");
            }
            self.write("</figure>\n");
            self.after_title = false;
            return;
        }
        if self.after_title {
            self.write("<p class=\"lead\">");
            self.after_title = false;
        } else {
            self.write("<p>");
        }
        self.write(&html);
        self.write("</p>\n");
    }

    fn end_heading(&mut self, level: u8) {
        let frame = self.pop();
        let text = self.heading_capture.take().unwrap_or_default();
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let base = slugify(&text);
        let id = {
            let count = self.slug_counts.entry(base.clone()).or_insert(0);
            let id = if *count == 0 {
                base
            } else {
                format!("{base}-{count}")
            };
            *count += 1;
            id
        };

        let is_doc_title = level == 1 && !self.has_h1;
        if is_doc_title {
            self.has_h1 = true;
            self.first_h1_text = Some(text.clone());
            self.after_title = true;
        }

        let display = if text.is_empty() {
            "Section"
        } else {
            text.as_str()
        };
        self.outline.push(Heading {
            level,
            id: id.clone(),
            text: display.to_string(),
        });

        self.write("<h");
        self.write(&level.to_string());
        self.write(" id=\"");
        self.write(&id);
        self.write("\"");
        if is_doc_title {
            self.write(" class=\"doc-title\" data-doc-title=\"true\"");
        }
        self.write("><a class=\"heading-anchor\" href=\"#");
        self.write(&id);
        self.write("\" aria-hidden=\"true\" tabindex=\"-1\">#</a>");
        self.write(&frame.html);
        self.write("</h");
        self.write(&level.to_string());
        self.write(">\n");
    }

    fn end_code_block(&mut self) {
        let frame = self.pop();
        self.in_code = false;
        let FrameKind::Code { info } = frame.kind else {
            return;
        };
        let lang = canonical_language(&info);
        let code = frame.text.strip_suffix('\n').unwrap_or(&frame.text);
        self.stats.code_blocks += 1;
        self.stats.code_lines += code.lines().count();

        let body = if self.options.highlight {
            highlight(code, &lang).unwrap_or_else(|| escape_html(code).into_owned())
        } else {
            escape_html(code).into_owned()
        };
        let label = language_label(&lang);
        let mut html = String::with_capacity(body.len() + 360);
        html.push_str("<div class=\"code-block\" data-lang=\"");
        push_escaped_attr(&mut html, &lang);
        html.push_str("\"><div class=\"code-head\"><span class=\"code-lang\">");
        push_escaped(&mut html, &label);
        html.push_str("</span><button class=\"code-copy\" type=\"button\" aria-label=\"Copy code\">Copy</button></div>");
        html.push_str("<pre tabindex=\"0\"><code class=\"language-");
        push_escaped_attr(&mut html, &lang);
        html.push_str("\">");
        html.push_str(&body);
        html.push_str("</code></pre></div>\n");
        self.write(&html);
    }

    fn end_link(&mut self) {
        let frame = self.pop();
        let FrameKind::Link { dest, title } = frame.kind else {
            return;
        };
        self.stats.links += 1;
        let target = resolve_link(
            &dest,
            self.options.base_dir.as_deref(),
            self.options.localize,
        );
        let mut html = String::with_capacity(frame.html.len() + 120);
        match target {
            ResolvedLink::Blocked => {
                html.push_str("<span class=\"link-blocked\" title=\"Blocked link\">");
                html.push_str(&frame.html);
                html.push_str("</span>");
                self.write(&html);
                return;
            }
            ResolvedLink::Anchor(anchor) => {
                html.push_str("<a class=\"anchor-link\" href=\"#");
                push_escaped_attr(&mut html, &anchor);
                html.push_str("\">");
            }
            ResolvedLink::External(url) => {
                html.push_str("<a href=\"");
                push_escaped_attr(&mut html, &url);
                html.push_str("\" data-external=\"true\" rel=\"noopener noreferrer\"");
                if !title.is_empty() {
                    html.push_str(" title=\"");
                    push_escaped_attr(&mut html, &title);
                    html.push('"');
                }
                html.push('>');
            }
            ResolvedLink::Relative(url) => {
                html.push_str("<a href=\"");
                push_escaped_attr(&mut html, &url);
                html.push_str("\">");
            }
            ResolvedLink::Local { path, anchor, kind } => {
                html.push_str("<a href=\"#\" data-open-path=\"");
                push_escaped_attr(&mut html, &path.to_string_lossy());
                html.push('"');
                if let Some(anchor) = anchor {
                    html.push_str(" data-anchor=\"");
                    push_escaped_attr(&mut html, &anchor);
                    html.push('"');
                }
                html.push_str(" data-link-kind=\"");
                html.push_str(match kind {
                    LocalKind::Doc => "doc",
                    LocalKind::File => "file",
                });
                html.push_str("\">");
            }
        }
        html.push_str(&frame.html);
        html.push_str("</a>");
        self.write(&html);
    }

    fn end_image(&mut self) {
        let frame = self.pop();
        let FrameKind::Image { dest, title } = frame.kind else {
            return;
        };
        self.stats.images += 1;
        let alt = frame.text.trim().to_string();
        let caption = if !alt.is_empty() {
            alt.clone()
        } else {
            title.clone()
        };
        let target = resolve_image(
            &dest,
            self.options.base_dir.as_deref(),
            self.options.localize,
        );
        let mut html = String::with_capacity(alt.len() + 220);
        match target {
            ResolvedImage::Blocked => {
                html.push_str("<span class=\"image-blocked\" title=\"Blocked image\">");
                html.push_str(&escape_html(if alt.is_empty() { "Image" } else { &alt }));
                html.push_str("</span>");
                self.write(&html);
                return;
            }
            ResolvedImage::LocalPath(path) => {
                html.push_str("<img class=\"local-image\" src=\"");
                html.push_str(TRANSPARENT_PIXEL);
                html.push_str("\" data-local-image=\"");
                push_escaped_attr(&mut html, &path.to_string_lossy());
                html.push('"');
            }
            ResolvedImage::External(url) => {
                html.push_str("<img src=\"");
                push_escaped_attr(&mut html, &url);
                html.push_str("\" referrerpolicy=\"no-referrer\"");
            }
            ResolvedImage::Data(url) | ResolvedImage::Relative(url) => {
                html.push_str("<img src=\"");
                push_escaped_attr(&mut html, &url);
                html.push('"');
            }
        }
        html.push_str(" alt=\"");
        push_escaped_attr(&mut html, &alt);
        html.push('"');
        if !title.is_empty() {
            html.push_str(" title=\"");
            push_escaped_attr(&mut html, &title);
            html.push('"');
        }
        html.push_str(" loading=\"lazy\" decoding=\"async\" />");

        // A paragraph containing only an image becomes a figure.
        if let Some(parent) = self.frames.last_mut() {
            if let FrameKind::Paragraph { image } = &mut parent.kind {
                if parent.html.trim().is_empty() {
                    *image = Some(ImageInline {
                        html: html.clone(),
                        caption,
                    });
                }
            }
            parent.html.push_str(&html);
        } else {
            self.out.push_str(&html);
        }
    }

    fn footnote_number(&mut self, label: &str) -> usize {
        if let Some(number) = self.footnote_numbers.get(label) {
            return *number;
        }
        let number = self.footnote_numbers.len() + 1;
        self.footnote_numbers.insert(label.to_string(), number);
        number
    }

    fn footnote_reference(&mut self, label: &str) {
        let number = self.footnote_number(label);
        let count = self
            .footnote_ref_counts
            .entry(label.to_string())
            .or_insert(0);
        let ref_id = if *count == 0 {
            format!("fnref-{number}")
        } else {
            format!("fnref-{number}-{count}")
        };
        *count += 1;
        let mut html = String::with_capacity(120);
        html.push_str("<sup class=\"fn-ref\"><a href=\"#fn-");
        html.push_str(&number.to_string());
        html.push_str("\" id=\"");
        html.push_str(&ref_id);
        html.push_str("\" aria-label=\"Footnote ");
        html.push_str(&number.to_string());
        html.push_str("\">");
        html.push_str(&number.to_string());
        html.push_str("</a></sup>");
        self.write(&html);
    }

    fn end_footnote_definition(&mut self) {
        let frame = self.pop();
        let FrameKind::Footnote { number } = frame.kind else {
            return;
        };
        self.defined_footnotes += 1;
        self.footnote_html
            .push_str("<aside class=\"footnote\" id=\"fn-");
        self.footnote_html.push_str(&number.to_string());
        self.footnote_html.push_str("\">\n");
        self.footnote_html.push_str(&frame.html);
        self.footnote_html
            .push_str("<a class=\"fn-back\" href=\"#fnref-");
        self.footnote_html.push_str(&number.to_string());
        self.footnote_html
            .push_str("\" aria-label=\"Back to reference\">↩</a>\n</aside>\n");
    }

    fn task_marker(&mut self, checked: bool) {
        self.write("<span class=\"task-box\" data-checked=\"");
        self.write(if checked { "true" } else { "false" });
        self.write("\" role=\"img\" aria-label=\"");
        self.write(if checked { "Done" } else { "Not done" });
        self.write("\"></span> ");
    }
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn callout_slug(kind: BlockQuoteKind) -> &'static str {
    match kind {
        BlockQuoteKind::Note => "note",
        BlockQuoteKind::Tip => "tip",
        BlockQuoteKind::Important => "important",
        BlockQuoteKind::Warning => "warning",
        BlockQuoteKind::Caution => "caution",
    }
}

fn callout_label(kind: BlockQuoteKind) -> &'static str {
    match kind {
        BlockQuoteKind::Note => "Note",
        BlockQuoteKind::Tip => "Tip",
        BlockQuoteKind::Important => "Important",
        BlockQuoteKind::Warning => "Warning",
        BlockQuoteKind::Caution => "Caution",
    }
}
