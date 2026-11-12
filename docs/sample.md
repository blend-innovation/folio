---
title: The Shape of a Good Document
description: Notes on measure, rhythm, and the quiet craft of making words readable.
author: Mira Chen
date: 2026-02-14
tags: [reading, typography, craft]
---

# The Shape of a Good Document

Most documents are read once, in a hurry, on the way to somewhere else. A
few are read slowly. The difference is rarely the words alone. It is the
measure, the rhythm of the page, the small courtesies that let an idea
arrive without being announced.

This is a note about those courtesies — and about the kind of page a
careful reader deserves.

## The measure

A line of text has a natural length. Too short, and the eye stumbles at every
turn. Too long, and it loses its place on the return. Somewhere between sixty
and seventy-five characters, prose begins to breathe.[^measure] Robert
Bringhurst wrote the classic study of this craft, [*The Elements of
Typographic Style*](https://en.wikipedia.org/wiki/The_Elements_of_Typographic_Style).

> The reader's eye, moving along a line, should never have to wonder where
> the line ends. Typography is the art of removing that wonder.

The best typography does not call attention to itself. It sets a width,
chooses a comfortable distance between lines, and then steps aside. The page
should feel inevitable, as if it could not have been arranged any other way.

### Three small decisions

- **A measure that holds.** Long paragraphs get a column; short ones get room.
- **A rhythm that repeats.** Spacing is a promise. Keep it, and the page feels calm.
- **A hierarchy that whispers.** A heading can be clear without being loud.

## Structure you can feel

Readers navigate a document the way they navigate a room: by noticing where
the walls are. Headings are walls. Lists are doorways. A rule is a pause.

### What a section should do

A good section opens with a sentence that earns the next one. It does not
begin with *In this section, we will discuss*. It begins with the thing
itself.

1. Say what changed.
2. Show why it matters.
3. Leave the reader with somewhere to go.

That is usually enough. Length is not a virtue; resolution is. The rest of
[the details](#the-details) are below, if you want them.

> [!TIP]
> Write the first sentence last. It is the one that knows what the section
> is actually about.

## A little colour, used sparingly

Colour is punctuation. A page with too much of it is like a paragraph with
too many exclamation marks: the emphasis cancels itself out.

> [!NOTE]
> The accent here is sage. It appears in links, list markers, the rail, and
> almost nowhere else. Restraint is what makes it feel intentional.

> [!WARNING]
> If everything is highlighted, nothing is. Use a callout when skipping it
> would cost the reader something.

> [!IMPORTANT]
> The document is the interface. Chrome should get out of its way, then come
> back the moment it is needed.

## Code is part of the prose

A code block is still a paragraph. It needs a comfortable measure, enough
line height to scan, and a way to get out of the way when the reader is done.

```rust
use std::path::Path;

/// Rendered documents are small, self-contained values.
pub struct Document {
    pub title: String,
    pub html: String,
    pub words: usize,
}

impl Document {
    pub fn read(path: &Path) -> std::io::Result<Self> {
        let source = std::fs::read_to_string(path)?;
        Ok(Self::render(&source))
    }
}
```

The same idea in TypeScript, where the shapes are mostly types and promises:

```ts
type Theme = {
  id: string;
  name: string;
  appearance: "light" | "dark";
  colors: Record<string, string>;
};

async function loadTheme(id: string): Promise<Theme> {
  const response = await fetch(`/themes/${id}.json`);
  if (!response.ok) throw new Error(`No theme named ${id}`);
  return response.json();
}
```

And the shell command that ships it:

```bash
#!/bin/sh
set -eu
pnpm build:ui
cargo tauri build --target universal-apple-darwin
echo "Folio.app is ready in src-tauri/target/universal-apple-darwin/release/bundle"
```

A diff deserves its own colour, quieter than a warning:

```diff
- line-height: 1.45;
+ line-height: 1.72;
- margin: 0 0 12px;
+ margin: 0 0 1.05em;
```

Inline code — `document.querySelectorAll`, `--f-measure`, `Theme::validate` —
should sit on the line without breaking it. Press <kbd>⌘</kbd> <kbd>B</kbd> to
bring the contents back, or <kbd>⌘</kbd> <kbd>F</kbd> to find a word you
half-remember.

## Tables, when they earn their place

A table is a promise that the reader will compare things. Keep the columns
few, the numbers aligned, and the borders almost invisible.

| Decision        | What it buys                       | What it costs        |
| :-------------- | :--------------------------------- | -------------------: |
| One column      | A single, uninterrupted reading    | Less room for asides |
| Serif headings  | A human, editorial voice           | A second typeface    |
| Soft rules      | Structure without a cage           | Slightly less contrast |
| Six themes      | A page for every kind of afternoon | A little more upkeep |

None of these are rules. They are the kind of choices that become invisible
when they are right.

## The details

- [x] A measure that holds at any window width
- [x] Headings that stay in view while you read
- [ ] A page that remembers exactly where you were
- [ ] Fewer decisions, more reading

---

The best compliment a reader can pay a document is not *this is beautiful*.
It is *I forgot I was reading*. Everything above is in service of that small
disappearance.

[^measure]: The comfortable range depends on the typeface, but 60–75
    characters is a good place to begin. Serif faces often tolerate a longer
    line; sans faces usually prefer a shorter one.
