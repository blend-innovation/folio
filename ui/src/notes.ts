import { invoke } from "./bridge";
import type {
  Annotation,
  AnnotationPayload,
  DocumentPayload,
} from "./types";

interface SelectionAnchor {
  quote: string;
  prefix: string;
  suffix: string;
}

interface NotesOptions {
  root: HTMLElement;
  scroll: HTMLElement;
  onOpen: () => void;
  onError: (error: unknown) => void;
  onToast: (message: string) => void;
}

const TEXT_NODE = 3;

function element<T extends HTMLElement>(id: string): T {
  const node = document.getElementById(id);
  if (!node) throw new Error(`Missing element #${id}`);
  return node as T;
}

function newId(): string {
  const cryptoApi = globalThis.crypto;
  if (cryptoApi?.randomUUID) return cryptoApi.randomUUID();
  return `note-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

export class NotesController {
  private readonly root: HTMLElement;
  private readonly scroll: HTMLElement;
  private readonly options: NotesOptions;

  private readonly button = element<HTMLButtonElement>("notes-btn");
  private readonly panel = element<HTMLElement>("notes-panel");
  private readonly closeButton = element<HTMLButtonElement>("notes-close");
  private readonly count = element<HTMLElement>("notes-count");
  private readonly warning = element<HTMLElement>("notes-warning");
  private readonly list = element<HTMLElement>("notes-list");
  private readonly empty = element<HTMLElement>("notes-empty");
  private readonly composer = element<HTMLElement>("note-composer");
  private readonly quoteEl = element<HTMLElement>("note-quote");
  private readonly textarea = element<HTMLTextAreaElement>("note-text");
  private readonly saveButton = element<HTMLButtonElement>("note-save");
  private readonly cancelButton = element<HTMLButtonElement>("note-cancel");
  private readonly fab = element<HTMLButtonElement>("note-fab");
  private readonly exportButton = element<HTMLButtonElement>("notes-export");
  private readonly cacheButton = element<HTMLButtonElement>("notes-cache");

  private document: DocumentPayload | null = null;
  private annotations: Annotation[] = [];
  private sourceHash = "";
  private sourceChanged = false;
  private cachePath: string | null = null;
  private stale = new Set<string>();
  private pending: SelectionAnchor | null = null;
  private editingId: string | null = null;
  private loadToken = 0;
  private saveToken = 0;
  private opened = false;

  constructor(options: NotesOptions) {
    this.root = options.root;
    this.scroll = options.scroll;
    this.options = options;

    this.button.addEventListener("click", () => this.toggle());
    this.closeButton.addEventListener("click", () => this.close());
    this.fab.addEventListener("mousedown", (event) => event.preventDefault());
    this.fab.addEventListener("click", () => this.startFromSelection());
    this.saveButton.addEventListener("click", () => this.saveComposer());
    this.cancelButton.addEventListener("click", () => this.cancelComposer());
    this.exportButton.addEventListener("click", () => void this.export());
    this.cacheButton.addEventListener("click", () => void this.openCache());
    this.root.addEventListener("mouseup", (event) => this.onMouseUp(event));
    this.root.addEventListener("click", (event) => this.onMarkClick(event));
    this.scroll.addEventListener("scroll", () => this.hideFab(), {
      passive: true,
    });
    window.addEventListener("resize", () => this.hideFab());
    document.addEventListener("selectionchange", () => {
      if (!window.getSelection()?.toString().trim()) this.hideFab();
    });

    this.setEnabled(false);
  }

  /** Load notes for a newly rendered document. */
  setDocument(payload: DocumentPayload): void {
    if (payload.sample || !payload.path) {
      this.clear();
      return;
    }
    this.document = payload;
    this.sourceHash = payload.sourceHash ?? "";
    this.sourceChanged = false;
    this.cachePath = null;
    this.annotations = [];
    this.stale.clear();
    this.pending = null;
    this.editingId = null;
    this.composer.hidden = true;
    this.hideFab();
    this.setEnabled(true);
    this.render();
    this.applyHighlights();

    const token = ++this.loadToken;
    void this.load(payload.path, token);
  }

  clear(): void {
    this.loadToken += 1;
    this.document = null;
    this.annotations = [];
    this.sourceHash = "";
    this.sourceChanged = false;
    this.cachePath = null;
    this.stale.clear();
    this.pending = null;
    this.editingId = null;
    this.composer.hidden = true;
    this.close();
    this.setEnabled(false);
    this.render();
  }

  /** Close notes on Escape. Returns true when the event was consumed. */
  handleEscape(): boolean {
    if (!this.composer.hidden) {
      this.cancelComposer();
      return true;
    }
    if (this.opened) {
      this.close();
      return true;
    }
    this.hideFab();
    return false;
  }

  private setEnabled(enabled: boolean): void {
    this.button.disabled = !enabled;
    if (!enabled) this.button.setAttribute("aria-expanded", "false");
  }

  private toggle(): void {
    if (!this.document) return;
    if (this.opened) {
      this.close();
    } else {
      this.open();
    }
  }

  private open(): void {
    if (!this.document) return;
    this.opened = true;
    this.panel.hidden = false;
    this.button.setAttribute("aria-expanded", "true");
    this.options.onOpen();
    this.render();
    this.closeButton.focus({ preventScroll: true });
  }

  close(): void {
    this.opened = false;
    this.panel.hidden = true;
    this.button.setAttribute("aria-expanded", "false");
    this.hideFab();
  }

  private async load(path: string, token: number): Promise<void> {
    try {
      const payload = await invoke<AnnotationPayload>("load_annotations", {
        path,
      });
      if (token !== this.loadToken || this.document?.path !== path) return;
      this.annotations = payload.annotations ?? [];
      this.sourceHash = payload.sourceHash || this.sourceHash;
      this.sourceChanged = Boolean(payload.sourceChanged);
      this.cachePath = payload.cachePath;
      this.applyHighlights();
      this.render();
    } catch (error) {
      if (token === this.loadToken) this.options.onError(error);
    }
  }

  private async persist(): Promise<boolean> {
    const document = this.document;
    if (!document) return false;
    const token = ++this.saveToken;
    const annotations = this.annotations;
    try {
      const payload = await invoke<AnnotationPayload>("save_annotations", {
        path: document.path,
        annotations,
      });
      if (token !== this.saveToken || this.document?.path !== document.path) {
        return false;
      }
      this.sourceHash = payload.sourceHash || this.sourceHash;
      this.sourceChanged = false;
      this.cachePath = payload.cachePath;
      this.render();
      return true;
    } catch (error) {
      if (token === this.saveToken) this.options.onError(error);
      return false;
    }
  }

  private render(): void {
    const count = this.annotations.length;
    this.count.textContent =
      count === 0 ? "No notes yet" : `${count} note${count === 1 ? "" : "s"}`;
    this.button.title = count > 0 ? `Notes (${count})` : "Notes";
    this.button.setAttribute(
      "aria-label",
      count > 0 ? `Notes (${count})` : "Notes",
    );

    const missing = this.stale.size;
    if (count === 0) {
      this.warning.hidden = true;
    } else if (this.sourceChanged) {
      this.warning.hidden = false;
      this.warning.textContent =
        "The original changed since these notes were saved. Notes remain where their quote can still be found.";
    } else if (missing > 0) {
      this.warning.hidden = false;
      this.warning.textContent = `${missing} note${missing === 1 ? "" : "s"} could not be re-anchored in the current document.`;
    } else {
      this.warning.hidden = true;
    }

    this.list.replaceChildren();
    for (const annotation of this.annotations) {
      const item = document.createElement("article");
      item.className = "note-item";

      const quote = document.createElement("button");
      quote.type = "button";
      quote.className = "note-item-quote";
      quote.textContent = annotation.quote;
      quote.title = "Go to highlighted text";
      quote.addEventListener("click", () => {
        this.focusAnnotation(annotation.id);
      });

      const text = document.createElement("p");
      text.className = "note-item-text";
      text.textContent = annotation.note;

      const meta = document.createElement("div");
      meta.className = "note-item-meta";

      if (this.stale.has(annotation.id)) {
        const stale = document.createElement("span");
        stale.className = "note-stale";
        stale.textContent = "Not found";
        meta.append(stale);
      }

      const edit = document.createElement("button");
      edit.type = "button";
      edit.textContent = "Edit";
      edit.addEventListener("click", () => {
        this.openComposer(annotation);
      });

      const remove = document.createElement("button");
      remove.type = "button";
      remove.textContent = "Delete";
      remove.addEventListener("click", () => {
        if (!window.confirm("Delete this note?")) return;
        this.annotations = this.annotations.filter(
          (item) => item.id !== annotation.id,
        );
        this.applyHighlights();
        this.render();
        void this.persist();
      });

      meta.append(edit, remove);
      item.append(quote, text, meta);
      this.list.append(item);
    }

    this.empty.hidden = count > 0;
    this.exportButton.disabled = count === 0;
    this.cacheButton.disabled = count === 0 || !this.cachePath;
  }

  private applyHighlights(): void {
    this.unwrapHighlights();
    this.stale.clear();
    for (const annotation of this.annotations) {
      const range = this.findRange(annotation);
      if (!range) {
        this.stale.add(annotation.id);
        continue;
      }
      let selected = range.node;
      if (range.start > 0) selected = selected.splitText(range.start);
      if (range.end - range.start < selected.data.length) {
        selected.splitText(range.end - range.start);
      }
      const mark = document.createElement("mark");
      mark.className = "note-mark";
      mark.dataset.noteId = annotation.id;
      mark.title = annotation.note;
      selected.parentNode?.insertBefore(mark, selected);
      mark.append(selected);
    }
    this.render();
  }

  private unwrapHighlights(): void {
    for (const mark of Array.from(
      this.root.querySelectorAll<HTMLElement>("mark.note-mark"),
    )) {
      const parent = mark.parentNode;
      if (!parent) continue;
      parent.replaceChild(document.createTextNode(mark.textContent ?? ""), mark);
      parent.normalize();
    }
  }

  private findRange(annotation: Annotation): { node: Text; start: number; end: number } | null {
    const quote = annotation.quote.trim();
    if (!quote) return null;
    const walker = document.createTreeWalker(this.root, NodeFilter.SHOW_TEXT);
    const candidates: { node: Text; start: number; end: number }[] = [];
    let node = walker.nextNode();
    while (node) {
      const text = node as Text;
      const parent = text.parentElement;
      if (
        parent &&
        !parent.closest("mark.note-mark") &&
        !parent.closest("script, style, textarea")
      ) {
        let start = text.data.indexOf(quote);
        while (start !== -1) {
          candidates.push({ node: text, start, end: start + quote.length });
          start = text.data.indexOf(quote, start + 1);
        }
      }
      node = walker.nextNode();
    }
    if (candidates.length === 0) return null;

    const withContext = candidates.find((candidate) => {
      const before = candidate.node.data.slice(0, candidate.start);
      const after = candidate.node.data.slice(candidate.end);
      return (
        (!annotation.prefix || before.endsWith(annotation.prefix)) &&
        (!annotation.suffix || after.startsWith(annotation.suffix))
      );
    });
    return withContext ?? candidates[0];
  }

  private focusAnnotation(id: string): void {
    const mark = Array.from(
      this.root.querySelectorAll<HTMLElement>("mark.note-mark"),
    ).find((node) => node.dataset.noteId === id);
    if (!mark) return;
    mark.scrollIntoView({ block: "center", behavior: "smooth" });
  }

  private onMarkClick(event: MouseEvent): void {
    const target = event.target as Element | null;
    const mark = target?.closest<HTMLElement>("mark.note-mark");
    if (!mark) return;
    const note = this.annotations.find(
      (annotation) => annotation.id === mark.dataset.noteId,
    );
    if (!note) return;
    event.preventDefault();
    event.stopPropagation();
    this.open();
    this.openComposer(note);
  }

  private onMouseUp(event: MouseEvent): void {
    if (!this.document) return;
    const target = event.target as Element | null;
    if (target?.closest(".note-mark")) return;
    const selection = window.getSelection();
    if (!selection || selection.rangeCount === 0 || selection.isCollapsed) {
      this.hideFab();
      return;
    }
    const range = selection.getRangeAt(0);
    if (!this.root.contains(range.commonAncestorContainer)) {
      this.hideFab();
      return;
    }
    const quote = range.toString().trim();
    if (!quote || quote.length > 8_000) {
      this.hideFab();
      return;
    }

    const start = range.startContainer;
    const end = range.endContainer;
    const prefix =
      start.nodeType === TEXT_NODE
        ? start.textContent?.slice(0, range.startOffset).slice(-80) ?? ""
        : "";
    const suffix =
      end.nodeType === TEXT_NODE
        ? end.textContent?.slice(range.endOffset).slice(0, 80) ?? ""
        : "";

    this.pending = { quote, prefix, suffix };
    const rect = range.getBoundingClientRect();
    const left = Math.max(70, Math.min(window.innerWidth - 70, rect.left + rect.width / 2));
    const top =
      rect.bottom + 10 < window.innerHeight - 40
        ? rect.bottom + 10
        : Math.max(8, rect.top - 38);
    this.fab.style.left = `${left}px`;
    this.fab.style.top = `${top}px`;
    this.fab.hidden = false;
  }

  private hideFab(): void {
    this.fab.hidden = true;
  }

  private startFromSelection(): void {
    const selection = this.pending;
    if (!selection) return;
    this.hideFab();
    this.open();
    this.openComposer(undefined, selection);
  }

  private openComposer(annotation?: Annotation, selection?: SelectionAnchor): void {
    const quote = annotation?.quote ?? selection?.quote ?? "";
    if (!quote) return;
    this.pending = annotation ? null : selection ?? null;
    this.editingId = annotation?.id ?? null;
    this.quoteEl.textContent =
      quote.length > 420 ? `${quote.slice(0, 420)}…` : quote;
    this.textarea.value = annotation?.note ?? "";
    this.composer.hidden = false;
    this.render();
    this.textarea.focus();
  }

  private saveComposer(): void {
    const text = this.textarea.value.trim();
    if (!text) {
      this.textarea.focus();
      this.options.onToast("Write a note first.");
      return;
    }

    const now = Date.now();
    const editing = this.editingId
      ? this.annotations.find((annotation) => annotation.id === this.editingId)
      : null;

    if (editing) {
      editing.note = text;
      editing.updatedAt = now;
    } else {
      const pending = this.pending;
      if (!pending) return;
      this.annotations.push({
        id: newId(),
        quote: pending.quote,
        prefix: pending.prefix,
        suffix: pending.suffix,
        note: text,
        createdAt: now,
        updatedAt: now,
      });
    }

    this.cancelComposer();
    this.annotations = [...this.annotations];
    this.applyHighlights();
    this.render();
    void this.persist();
  }

  private cancelComposer(): void {
    this.composer.hidden = true;
    this.textarea.value = "";
    this.editingId = null;
    this.pending = null;
    this.render();
  }

  private async export(): Promise<void> {
    if (!this.document || this.annotations.length === 0) return;
    if (!(await this.persist())) return;
    try {
      const output = await invoke<string | null>("export_annotated", {
        path: this.document.path,
      });
      if (output) this.options.onToast("Annotated Markdown exported.");
    } catch (error) {
      this.options.onError(error);
    }
  }

  private async openCache(): Promise<void> {
    if (!this.document) return;
    try {
      await invoke("open_annotated_cache", { path: this.document.path });
    } catch (error) {
      this.options.onError(error);
    }
  }
}
