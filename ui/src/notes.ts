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
  onCopy: (text: string) => Promise<void>;
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
  private readonly toolbar = element<HTMLElement>("selection-toolbar");
  private readonly copyButton = element<HTMLButtonElement>("selection-copy");
  private readonly noteButton = element<HTMLButtonElement>("selection-note");
  private readonly badge = element<HTMLElement>("notes-badge");
  private readonly loading = element<HTMLElement>("notes-loading");
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
  private isLoading = false;
  private activeId: string | null = null;

  constructor(options: NotesOptions) {
    this.root = options.root;
    this.scroll = options.scroll;
    this.options = options;

    this.button.addEventListener("click", () => this.toggle());
    this.closeButton.addEventListener("click", () => this.close());
    this.toolbar.addEventListener("mousedown", (event) => event.preventDefault());
    this.copyButton.addEventListener("click", () => this.copySelection());
    this.noteButton.addEventListener("click", () => this.startFromSelection());
    this.saveButton.addEventListener("click", () => this.saveComposer());
    this.cancelButton.addEventListener("click", () => this.cancelComposer());
    this.exportButton.addEventListener("click", () => void this.export());
    this.cacheButton.addEventListener("click", () => void this.openCache());
    this.root.addEventListener("mouseup", (event) => this.onMouseUp(event));
    this.root.addEventListener("click", (event) => this.onMarkClick(event));
    this.scroll.addEventListener("scroll", () => this.hideToolbar(), {
      passive: true,
    });
    window.addEventListener("resize", () => this.hideToolbar());
    document.addEventListener("selectionchange", () => {
      if (!window.getSelection()?.toString().trim()) this.hideToolbar();
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
    this.activeId = null;
    this.composer.hidden = true;
    this.hideToolbar();
    this.setEnabled(true);
    this.isLoading = true;
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
    this.activeId = null;
    this.isLoading = false;
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
    this.hideToolbar();
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
    this.panel.focus({ preventScroll: true });
  }

  close(): void {
    const wasOpen = this.opened;
    this.opened = false;
    this.panel.hidden = true;
    this.button.setAttribute("aria-expanded", "false");
    this.hideToolbar();
    if (wasOpen && this.document) this.button.focus({ preventScroll: true });
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
      this.isLoading = false;
      this.applyHighlights();
      this.render();
    } catch (error) {
      if (token === this.loadToken) {
        this.isLoading = false;
        this.render();
        this.options.onError(error);
      }
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
    this.badge.hidden = count === 0;
    this.button.title = count > 0 ? `Notes (${count})` : "Notes";
    this.button.setAttribute(
      "aria-label",
      count > 0 ? `Notes (${count})` : "Notes",
    );
    this.exportButton.title = count
      ? "Export the original plus your notes"
      : "Add a note first";
    this.cacheButton.title =
      count === 0
        ? "Add a note first"
        : this.cachePath
          ? "Open the cached annotated copy"
          : "A cached copy is written when notes are saved";

    if (this.isLoading) {
      this.loading.hidden = false;
      this.list.hidden = true;
      this.empty.hidden = true;
      this.warning.hidden = true;
      this.exportButton.disabled = true;
      this.cacheButton.disabled = true;
      return;
    }

    this.loading.hidden = true;
    this.list.hidden = false;
    const missing = this.stale.size;
    if (count === 0) {
      this.warning.hidden = true;
    } else if (this.sourceChanged) {
      this.warning.hidden = false;
      this.warning.textContent = missing
        ? `Source changed since these notes were saved. ${missing} note${missing === 1 ? "" : "s"} kept without a highlight.`
        : "Source changed since these notes were saved.";
    } else if (missing > 0) {
      this.warning.hidden = false;
      this.warning.textContent = `${missing} quoted passage${missing === 1 ? " has" : "s have"} changed. The note${missing === 1 ? " is" : "s are"} kept, but highlighted text could not be restored.`;
    } else {
      this.warning.hidden = true;
    }

    this.list.replaceChildren();
    for (const annotation of this.annotations) {
      const item = document.createElement("article");
      item.className = "note-item";
      item.dataset.noteId = annotation.id;

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
        stale.textContent = "Quote changed";
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
        if (this.activeId === annotation.id) this.activeId = null;
        this.applyHighlights();
        this.render();
        void this.persist();
      });

      meta.append(edit, remove);
      item.append(quote, text, meta);
      this.list.append(item);
    }

    this.empty.hidden = count > 0 || !this.composer.hidden;
    this.exportButton.disabled = count === 0;
    this.cacheButton.disabled = count === 0 || !this.cachePath;
    this.updateActive();
  }

  private updateActive(): void {
    for (const item of this.list.querySelectorAll<HTMLElement>(".note-item")) {
      item.classList.toggle("is-active", item.dataset.noteId === this.activeId);
    }
    for (const mark of this.root.querySelectorAll<HTMLElement>("mark.note-mark")) {
      mark.classList.toggle("is-active", mark.dataset.noteId === this.activeId);
    }
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
    this.activeId = id;
    this.updateActive();
    const mark = Array.from(
      this.root.querySelectorAll<HTMLElement>("mark.note-mark"),
    ).find((node) => node.dataset.noteId === id);
    mark?.scrollIntoView({ block: "center", behavior: "smooth" });
    const item = Array.from(
      this.list.querySelectorAll<HTMLElement>(".note-item"),
    ).find((node) => node.dataset.noteId === id);
    item?.scrollIntoView({ block: "nearest", behavior: "smooth" });
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
    this.focusAnnotation(note.id);
  }

  private onMouseUp(event: MouseEvent): void {
    if (!this.document) return;
    const target = event.target as Element | null;
    if (target?.closest("mark.note-mark")) return;
    const selection = window.getSelection();
    if (!selection || selection.rangeCount === 0 || selection.isCollapsed) {
      this.hideToolbar();
      return;
    }
    const range = selection.getRangeAt(0);
    if (!this.root.contains(range.commonAncestorContainer)) {
      this.hideToolbar();
      return;
    }
    const quote = range.toString().trim();
    if (!quote || quote.length > 8_000) {
      this.hideToolbar();
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
    const left = Math.max(
      82,
      Math.min(window.innerWidth - 82, rect.left + rect.width / 2),
    );
    const top =
      rect.bottom + 10 < window.innerHeight - 44
        ? rect.bottom + 10
        : Math.max(8, rect.top - 40);
    this.toolbar.style.left = `${left}px`;
    this.toolbar.style.top = `${top}px`;
    this.toolbar.hidden = false;
  }

  private hideToolbar(): void {
    this.toolbar.hidden = true;
  }

  private copySelection(): void {
    const selection = this.pending;
    if (!selection) return;
    this.hideToolbar();
    void this.options.onCopy(selection.quote);
  }

  private startFromSelection(): void {
    const selection = this.pending;
    if (!selection) return;
    this.hideToolbar();
    this.open();
    this.openComposer(undefined, selection);
  }

  private openComposer(annotation?: Annotation, selection?: SelectionAnchor): void {
    const quote = annotation?.quote ?? selection?.quote ?? "";
    if (!quote) return;
    this.pending = annotation ? null : selection ?? null;
    this.editingId = annotation?.id ?? null;
    if (annotation) this.activeId = annotation.id;
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
      this.activeId = editing.id;
    } else {
      const pending = this.pending;
      if (!pending) return;
      const annotation: Annotation = {
        id: newId(),
        quote: pending.quote,
        prefix: pending.prefix,
        suffix: pending.suffix,
        note: text,
        createdAt: now,
        updatedAt: now,
      };
      this.annotations.push(annotation);
      this.activeId = annotation.id;
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
      if (output) this.options.onToast("Notes exported.");
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
