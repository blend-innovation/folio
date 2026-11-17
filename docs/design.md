# Design

Folio is a reading room, not a dashboard. The design follows one rule: the
document is the interface; chrome should be present when you look for it and
invisible when you are reading.

## The room: Blend

The default theme comes from Blend's palette and type pairing:

| Token | Value | Role |
| --- | --- | --- |
| canvas | `#FAF9F6` | Warm cream paper |
| ink | `#1C1C1E` | Near-black text |
| sage | `#5C9C7C` | Accent; links use the stronger `#3F7A5D` |
| terracotta | `#C97463` → `#A94F39` | Warnings and diff removals |
| rule | `#E8E5DE` | Hairlines |
| display | Newsreader | Titles, section headings, pull quotes |
| body | Manrope | Prose, interface labels, code labels |
| mono | SF Mono stack | Code, keys, metadata |

The palette is deliberately small. Sage appears in links, list markers, rules,
the rail's active indicator, and almost nowhere else. Warm colour is reserved
for warnings and things that need attention. That scarcity is what makes the
accent feel intentional.

## The page

- **One measure.** The default column is 42 rem. At 17 px body text that is a
  comfortable 65–80 characters. Editorial narrows it to 40 rem; Studio tightens
  it to 38 rem and reduces the body to 15.5 px.
- **A title, not a header bar.** The first H1 is set in Newsreader at up to
  3.35 rem with tight tracking. A small uppercase kicker above it carries the
  reading time, date, and author. Tags sit below on a quiet line.
- **A lead.** The first paragraph after the title is slightly larger and
  muted. It is the doorway into the document.
- **Section rhythm.** H2s are display type with a hairline above them; H3s are
  bold sans; H4+ step down into muted labels. Spacing is expressed in `em` so it
  follows the theme's body size.
- **Code is a card, not a hole.** White surface, hairline, a small language
  label, and a copy button. The header is deliberately quiet; the code is the
  content. Long blocks collapse after 34 lines with a mask and an explicit
  “Show N more lines” control. HTML cards add a quiet **Preview** action and,
  in the app, **Open** for a separate native window; the preview stays inside
  the card so source and result can be compared.
- **Diagrams are still documents.** A Mermaid block becomes a centred,
  theme-coloured figure with a **Source** toggle. The diagram is rendered only
  when it approaches the viewport; it should feel like a figure in the page,
  not an embedded application.
- **Tables are comparisons.** A single outer card, a muted header row, no
  vertical rules, right-aligned numerals.
- **Callouts are margin notes with weight.** A tinted surface and a 3 px accent
  edge. The label is 9 px uppercase with a dot; the body stays in the prose
  typeface.
- **Footnotes leave the body.** Definitions are moved into a small, quiet
  section at the end with return links.
- **Images become figures.** A standalone image gets a rounded card and its alt
  text as a caption. Sentences keep their images inline.

## The rail

A 244 px column on the same cream as the page, separated by a single hairline.
The brand sits below the traffic lights. The contents list is

- filtered to H2 and deeper (the document title is already at the top of the page),
- indented by heading level,
- clamped to two lines so a long heading does not truncate into an ellipsis,
- marked with a sage left edge and a soft gradient for the active section.

The list is its own scroller: a long outline scrolls between the fixed brand and
the fixed theme control, with overscroll contained so the page does not move.

The theme control is pinned to the bottom. It shows a colour swatch and the
theme name; the popover lists all six themes with three-colour swatches and a
“Match system” switch. Automatic mode follows the selected theme's
`counterpart`, so Editorial becomes Editorial Dark rather than jumping to an
unrelated palette.

## The chrome

A 58 px top bar floats over the page with a blurred cream surface. At rest it
has no border; after scrolling, a hairline appears. It carries the filename and
path, word count and reading time, find, and open. A 2 px sage progress line
runs along the very top of the stage.

On the first Markdown open, a small centered card may ask whether to make Folio
the default reader. It is the same room: surface, hairline, display heading,
one confident primary action, a quiet No, and a deliberately low-key “Never
show again” beneath a rule.

The find bar is a small floating card at the bottom. Matches use a warm
highlight; the current match uses the strong accent. It does not replace the
page or cover the reading column for long.

## Themes as rooms

- **Blend** — the default. Cream, sage, serif display.
- **Blend Dark** — warm charcoal (`#161614`), not black. Softer contrast, sage
  that gets lighter instead of darker.
- **Editorial** — ivory paper, Newsreader for *both* body and display, deep
  terracotta, 3 px corners. It is meant for essays, not API references.
- **Editorial Dark** — espresso and copper.
- **Studio** — cool white, compact measure, sans headings, small body, dark
  code cards in the dark variant. For technical notes and long code reviews.
- **Studio Dark** — graphite and mint.

The themes are not recolours of one stylesheet. They change type, measure,
corner radius, weight, and contrast. The token system is broad enough to make a
different room without allowing arbitrary CSS.

## Motion

Motion is short and mostly useful:

- the theme panel rises 4 px and fades in over 200 ms,
- the find bar rises 5 px over 180 ms,
- the outline's active edge and background cross-fade over 160 ms,
- the live-reload dot pulses slowly (2.8 s) and never demands attention,
- page scrolling uses the OS's smooth-scroll behaviour.

`prefers-reduced-motion` disables all of it.

## Accessibility

- Body text is 17 px with a 1.72 line height in the default theme; the text-size
  control scales the measure, body size, title, and radius together.
- All small metadata colours were darkened until they pass 4.5:1 on their
  surfaces in the light themes. Dark themes use lighter muted tokens.
- Links are underlined, not colour-only. Focus rings are a 2 px sage outline
  with a 2 px offset.
- The active outline item has `aria-current`; task boxes have accessible names;
  the contents and theme picker expose landmarks and roles.
- The end-to-end suite runs axe on the welcome page, the document in four
  themes, and the open theme picker, and fails on serious or critical
  violations.

## What was rejected

- **A WYSIWYG editor.** Folio observes files; it does not own them.
- **A chat transcript look.** Bubbles and timestamps flatten long-form prose
  into messages. A document deserves a page.
- **GitHub-flavoured default styling.** Utilities and README chrome are for
  repositories, not reading.
- **A theme API with raw CSS.** Powerful, but unsafe to share and hard to
  validate. Tokens cover what a reading page actually needs.
- **Electron.** A Markdown reader should not ship a second browser.
