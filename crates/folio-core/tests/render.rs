use folio_core::markdown::RenderOptions;
use folio_core::render;
use std::path::PathBuf;

fn r(md: &str) -> folio_core::Rendered {
    render(md, &RenderOptions::default())
}

fn localized(md: &str) -> folio_core::Rendered {
    render(
        md,
        &RenderOptions {
            base_dir: Some(PathBuf::from("/Users/ada/notes")),
            localize: true,
            ..RenderOptions::default()
        },
    )
}

#[test]
fn headings_become_typed_outline_entries_with_stable_ids() {
    let doc = r("# The Quiet Web\n\nIntro.\n\n## Part One\n\n### Detail\n\n## Part One\n");
    assert_eq!(doc.meta.title.as_deref(), Some("The Quiet Web"));
    assert!(doc.meta.has_h1);
    assert!(doc
        .html
        .contains("<h1 id=\"the-quiet-web\" class=\"doc-title\" data-doc-title=\"true\">"));
    assert!(doc.html.contains("<h2 id=\"part-one\">"));
    assert!(doc.html.contains("<h2 id=\"part-one-1\">"));
    assert!(doc.html.contains("class=\"heading-anchor\""));
    assert_eq!(doc.meta.outline.len(), 4);
    assert_eq!(doc.meta.outline[0].text, "The Quiet Web");
    assert_eq!(doc.meta.outline[2].id, "detail");
    assert_eq!(doc.meta.stats.headings, 4);
}

#[test]
fn title_is_followed_by_a_lead_paragraph() {
    let doc = r("# Title\n\nFirst paragraph.\n\nSecond paragraph.");
    assert!(doc.html.contains("<p class=\"lead\">First paragraph.</p>"));
    assert!(doc.html.contains("<p>Second paragraph.</p>"));
}

#[test]
fn frontmatter_sets_metadata_and_is_not_rendered() {
    let doc = r("---\ntitle: A Quiet Document\nauthor: Ada\ndate: 2026-02-01\ntags: [notes, design]\ndescription: A short note.\n---\n\n# Body Title\n\nHello.\n");
    assert_eq!(doc.meta.title.as_deref(), Some("A Quiet Document"));
    assert_eq!(doc.meta.author.as_deref(), Some("Ada"));
    assert_eq!(doc.meta.date.as_deref(), Some("2026-02-01"));
    assert_eq!(doc.meta.tags, ["notes", "design"]);
    assert_eq!(doc.meta.description.as_deref(), Some("A short note."));
    assert!(!doc.html.contains("A Quiet Document"));
    assert!(!doc.html.contains("<hr"));
    assert_eq!(doc.meta.outline[0].text, "Body Title");
}

#[test]
fn description_falls_back_to_the_first_paragraph() {
    let doc = r("# Title\n\nA calm opening line with *emphasis* and `code`.\n\nLater.");
    assert_eq!(
        doc.meta.description.as_deref(),
        Some("A calm opening line with emphasis and code.")
    );
}

#[test]
fn tables_have_alignment_classes_and_scroll_wrapper() {
    let doc = r("| Left | Center | Right |\n|:-----|:------:|------:|\n| a | b | 12 |\n");
    assert!(doc.html.contains("<div class=\"table-wrap\"><table>"));
    assert!(doc.html.contains("<thead>"));
    assert!(doc.html.contains("<th>Left</th>"));
    assert!(doc.html.contains("<th class=\"align-center\">Center</th>"));
    assert!(doc.html.contains("<th class=\"align-right\">Right</th>"));
    assert!(doc.html.contains("<td class=\"align-right\">12</td>"));
    assert!(doc.html.contains("</tbody></table></div>"));
    assert_eq!(doc.meta.stats.tables, 1);
}

#[test]
fn task_lists_render_checkboxes() {
    let doc = r("- [x] shipped\n- [ ] polish\n- plain\n");
    assert!(doc
        .html
        .contains("class=\"task-box\" data-checked=\"true\""));
    assert!(doc
        .html
        .contains("class=\"task-box\" data-checked=\"false\""));
    assert!(doc.html.contains("<li class=\"task-item\">"));
    assert!(!doc.html.contains(":has"));
}

#[test]
fn ordered_lists_keep_their_start_number() {
    let one = r("1. first\n2. second\n");
    assert!(one.html.contains("<ol>"));
    let three = r("3. third\n4. fourth\n");
    assert!(three.html.contains("<ol start=\"3\">"));
    assert!(three.html.contains("</ol>"));
}

#[test]
fn gfm_callouts_become_asides_without_the_marker() {
    let doc = r("> [!NOTE]\n> A note with **weight**.\n\n> [!WARNING]\n> Careful.\n\n> [!TIP]\n> Try this.\n\n> [!IMPORTANT]\n> Read me.\n\n> [!CAUTION]\n> Stop.\n");
    for (kind, label) in [
        ("note", "Note"),
        ("warning", "Warning"),
        ("tip", "Tip"),
        ("important", "Important"),
        ("caution", "Caution"),
    ] {
        assert!(
            doc.html.contains(&format!("data-kind=\"{kind}\"")),
            "{kind}"
        );
        assert!(
            doc.html
                .contains(&format!("class=\"callout-label\">{label}</p>")),
            "{kind}"
        );
    }
    assert!(!doc.html.contains("[!NOTE]"));
    assert!(doc.html.contains("<strong>weight</strong>"));
    assert_eq!(doc.html.matches("</aside>").count(), 5);
}

#[test]
fn plain_blockquotes_stay_quotes() {
    let doc = r("> A plain quote.\n");
    assert!(doc.html.contains("<blockquote>"));
    assert!(doc.html.contains("<p>A plain quote.</p>"));
    assert!(!doc.html.contains("callout"));
}

#[test]
fn code_blocks_have_headers_highlighting_and_copy_buttons() {
    let doc = r("```rust\nfn main() { let x = 1; }\n```\n");
    assert!(doc
        .html
        .contains("<div class=\"code-block\" data-lang=\"rust\">"));
    assert!(doc.html.contains("<span class=\"code-lang\">Rust</span>"));
    assert!(doc.html.contains("class=\"code-copy\""));
    assert!(doc.html.contains("<code class=\"language-rust\">"));
    assert!(doc.html.contains("tok-kw\">fn</span>"));
    assert!(doc.html.contains("tok-fn\">main</span>"));
    assert_eq!(doc.meta.stats.code_blocks, 1);
    assert_eq!(doc.meta.stats.code_lines, 1);
}

#[test]
fn fenced_attributes_are_understood() {
    let doc = r("```{.python .numberLines}\nprint('hi')\n```\n");
    assert!(doc.html.contains("data-lang=\"python\""));
    assert!(doc.html.contains("tok-fn\">print</span>"));
}

#[test]
fn unknown_code_is_escaped_and_not_executed() {
    let doc = r("```html\n<script>alert(1)</script>\n```\n");
    assert!(!doc.html.contains("<script>"));
    assert!(doc.html.contains("<span class=\"tok-tag\">&lt;</span>"));
    assert!(doc.html.contains("<span class=\"tok-tag\">script</span>"));
    assert!(doc.html.contains("data-lang=\"html\""));
}

#[test]
fn raw_html_is_shown_as_text_never_executed() {
    let doc =
        r("<div onclick=\"alert(1)\">hello</div>\n\nInline <img src=x onerror=alert(1)> html.\n");
    assert!(!doc.html.contains("<div onclick"));
    assert!(!doc.html.contains("<img src=x"));
    assert!(doc.html.contains("raw-html"));
    assert!(doc
        .html
        .contains("&lt;div onclick=\"alert(1)\"&gt;hello&lt;/div&gt;"));
}

#[test]
fn images_become_figures_with_captions() {
    let doc = r("![A quiet diagram](https://example.com/x.png \"Title\")\n");
    assert!(doc.html.contains("<figure class=\"doc-figure\">"));
    assert!(doc
        .html
        .contains("<figcaption>A quiet diagram</figcaption>"));
    assert!(doc.html.contains("loading=\"lazy\""));
    assert!(doc.html.contains("decoding=\"async\""));
    assert_eq!(doc.meta.stats.images, 1);
}

#[test]
fn an_image_in_a_sentence_stays_inline() {
    let doc = r("Look ![alt](https://example.com/x.png) here.");
    assert!(!doc.html.contains("<figure"));
    assert!(doc.html.contains("<p>Look <img"));
    assert!(doc.html.contains("</p>"));
}

#[test]
fn local_images_get_a_data_attribute_for_the_host() {
    let doc = localized("![diagram](images/diagram.png)\n");
    assert!(doc
        .html
        .contains("data-local-image=\"/Users/ada/notes/images/diagram.png\""));
    assert!(doc.html.contains("class=\"local-image\""));
    assert!(!doc.html.contains("src=\"images/diagram.png\""));
}

#[test]
fn links_are_classified_and_sanitized() {
    let doc = r("[web](https://example.com) [mail](mailto:a@b.c) [jump](#part-one) [bad](javascript:alert(1)) [rel](other.md)\n\n## Part One\n");
    assert!(doc
        .html
        .contains("href=\"https://example.com\" data-external=\"true\""));
    assert!(doc.html.contains("href=\"mailto:a@b.c\""));
    assert!(doc
        .html
        .contains("class=\"anchor-link\" href=\"#part-one\""));
    assert!(doc.html.contains("class=\"link-blocked\""));
    assert!(!doc.html.contains("javascript:"));
    assert!(doc.html.contains("href=\"other.md\""));
    assert_eq!(doc.meta.stats.links, 5);
}

#[test]
fn local_links_resolve_for_the_host() {
    let doc = localized("[next](other.md#next) [pdf](files/a.pdf)\n");
    assert!(doc
        .html
        .contains("data-open-path=\"/Users/ada/notes/other.md\""));
    assert!(doc.html.contains("data-anchor=\"next\""));
    assert!(doc.html.contains("data-link-kind=\"doc\""));
    assert!(doc.html.contains("data-link-kind=\"file\""));
    assert!(!doc.html.contains("href=\"other.md\""));
}

#[test]
fn footnotes_are_collected_at_the_end_and_numbered() {
    let doc =
        r("A claim[^a] and another[^b] and again[^a].\n\n[^a]: First note.\n[^b]: Second note.\n");
    assert!(doc.html.ends_with("</section>\n"));
    assert!(doc
        .html
        .contains("<section class=\"footnotes\" aria-label=\"Footnotes\">"));
    assert!(doc
        .html
        .contains("<a href=\"#fn-1\" id=\"fnref-1\" aria-label=\"Footnote 1\">1</a>"));
    assert!(doc
        .html
        .contains("<a href=\"#fn-1\" id=\"fnref-1-1\" aria-label=\"Footnote 1\">1</a>"));
    assert!(doc.html.contains("<aside class=\"footnote\" id=\"fn-1\">"));
    assert!(doc.html.contains("<aside class=\"footnote\" id=\"fn-2\">"));
    assert!(doc.html.contains("class=\"fn-back\" href=\"#fnref-1\""));
    assert!(doc.html.contains("class=\"fn-back\" href=\"#fnref-2\""));
    assert_eq!(doc.meta.stats.footnotes, 2);
    assert!(!doc.meta.outline.iter().any(|h| h.text == "Notes"));
}

#[test]
fn stats_count_prose_not_code() {
    let doc = r("# Title\n\nOne two three.\n\n```rust\nfn ignored_words_here() {}\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n");
    // Title + prose + table text; code is deliberately excluded.
    assert_eq!(doc.meta.stats.words, 8);
    assert_eq!(doc.meta.stats.minutes, 1);
    assert_eq!(doc.meta.stats.code_blocks, 1);
    assert_eq!(doc.meta.stats.tables, 1);
}

#[test]
fn smart_punctuation_can_be_disabled() {
    let md = "\"Quotes\" -- and...";
    let smart = r(md);
    assert!(smart.html.contains("“Quotes”"));
    assert!(smart.html.contains("–"));
    let plain = render(
        md,
        &RenderOptions {
            smart_punctuation: false,
            ..RenderOptions::default()
        },
    );
    assert!(plain.html.contains("\"Quotes\""));
    assert!(plain.html.contains("--"));
}

#[test]
fn hard_breaks_are_opt_in() {
    let md = "one\ntwo";
    assert!(r(md).html.contains("one\ntwo"));
    let hard = render(
        md,
        &RenderOptions {
            hard_breaks: true,
            ..RenderOptions::default()
        },
    );
    assert!(hard.html.contains("one<br />"));
}

#[test]
fn crlf_is_normalized() {
    let doc = r("# Title\r\n\r\nLine one\r\nLine two\r\n");
    assert!(doc.html.contains("<h1 id=\"title\""));
    assert!(!doc.html.contains('\r'));
}

#[test]
fn unicode_headings_and_slugs_work() {
    let doc = r("# 笔记 — 第二章\n\n## Café notes\n");
    assert!(doc.html.contains("id=\"笔记-第二章\""));
    assert!(doc.html.contains("id=\"café-notes\""));
    assert_eq!(doc.meta.outline[0].id, "笔记-第二章");
}

#[test]
fn documents_without_h1_expose_that_to_the_ui() {
    let doc = r("## Only a section\n\nText.");
    assert!(!doc.meta.has_h1);
    assert!(doc.meta.title.is_none());
    assert_eq!(doc.meta.outline[0].level, 2);
}

#[test]
fn empty_document_is_safe() {
    let doc = r("");
    assert!(doc.html.trim().is_empty());
    assert_eq!(doc.meta.stats.words, 0);
    assert_eq!(doc.meta.stats.minutes, 0);
    assert!(!doc.meta.has_h1);
}

#[test]
fn xss_payloads_never_escape() {
    let payloads = [
        "<script>alert(1)</script>",
        "[x](javascript:alert(1))",
        "![x](javascript:alert(1))",
        "<img src=x onerror=alert(1)>",
        "[x](data:text/html,<script>alert(1)</script>)",
        "```html\n</code></pre><script>alert(1)</script>\n```",
        "| <script> | b |\n|---|---|\n| c | d |",
        "> [!NOTE]\n> <script>alert(1)</script>",
    ];
    for payload in payloads {
        let doc = r(payload);
        assert!(!doc.html.contains("<script>"), "{payload} -> {}", doc.html);
        assert!(
            !doc.html.contains("<img src=x"),
            "{payload} -> {}",
            doc.html
        );
        assert!(
            !doc.html.contains("javascript:"),
            "{payload} -> {}",
            doc.html
        );
    }
}

#[test]
fn html_and_mermaid_fences_keep_their_preview_language() {
    let doc = r("```html
<h1>Preview</h1>
```

```mermaid
flowchart LR
  A --> B
```
");
    assert!(doc.html.contains(r#"data-lang="html""#));
    assert!(doc.html.contains(r#"data-lang="mermaid""#));
    assert!(doc.html.contains("Preview"));
    assert!(doc.html.contains("&lt;"));
}

#[test]
fn large_document_renders_in_one_pass() {
    let mut md = String::new();
    for i in 0..1500 {
        md.push_str(&format!("## Section {i}\n\nParagraph with **bold** and `code` and [a link](https://example.com/{i}).\n\n- one\n- two\n\n```rust\nfn f{i}() -> u32 {{ {i} }}\n```\n\n"));
    }
    let start = std::time::Instant::now();
    let doc = render(&md, &RenderOptions::default());
    let elapsed = start.elapsed();
    assert!(doc.meta.stats.headings >= 1500);
    assert!(doc.html.len() > md.len());
    // Debug builds are slow; this is a smoke test, not the benchmark.
    assert!(elapsed.as_secs() < 15, "took {elapsed:?}");
}
