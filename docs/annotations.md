# Notes and annotations

Folio lets you leave notes on a document without changing the Markdown file.

## Adding a note

1. Select text in the reading view.
2. Choose **Add note** next to the selection.
3. Write the note and save.

The selection is highlighted and listed in the **Notes** panel. Notes can be
edited or deleted from that panel. Clicking a note scrolls to its highlight.

Selecting text across several inline elements or blocks may save the note but
cannot always restore its visual highlight. The note remains in the panel and is
marked **Not found** rather than being silently attached to different text.

## Where notes live

Notes are stored outside the document, under the application data directory:

```text
~/Library/Application Support/com.folio.reader/annotations/
```

Each document has a JSON sidecar keyed by a hash of its canonical path. The
original Markdown, its frontmatter, and its modification time are untouched.

Alongside the JSON, Folio writes a cached annotated Markdown copy. The cache is
safe to regenerate: it is built from the current source plus the saved notes.

## Changes to the original

Every document load includes a SHA-256 hash of its source. When the original
changes, Folio notices before restoring the highlights and shows a warning in
the Notes panel. Notes are re-anchored by their quoted text and surrounding
context where possible; notes whose quotation no longer exists are marked
**Not found**. Folio never maps a stale note onto unrelated text.

## Exporting

The Notes panel has two actions:

- **Export annotated markdown** opens a save dialog and writes a new
  `<name>.annotated.md`. The original path is refused.
- **Open cached copy** opens the automatically generated annotated copy in your
  default Markdown app.

The exported copy starts with the original document exactly as it was when the
notes were saved, followed by:

```markdown
## Notes

### Note 1

> The quoted passage

Your note.
```

The exported document is an ordinary Markdown file. Folio does not rewrite the
source, add hidden markers to it, or keep a second editable copy in the reading
view.

## Privacy

Annotations and the cache stay in Folio's application data directory. They are
not uploaded, indexed, or sent over the network. Removing the annotations
directory removes the notes; the original documents are unaffected.

## Limitations

- Notes are local to this machine and this canonical path. Renaming or moving a
  document starts a separate note set.
- The cached annotated copy is updated when notes are saved; editing the
  original later does not rewrite that cache until the notes are saved again.
- Notes are not part of the Markdown file and do not appear in Git diffs.
