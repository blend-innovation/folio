const reducedMotion = (): boolean =>
  window.matchMedia("(prefers-reduced-motion: reduce)").matches;

const MAX_MATCHES = 1200;

const SKIP = "button, .code-head, .code-expand, .heading-anchor, .task-box, script, style, mark.find-hit";

export interface FindOptions {
  root: HTMLElement;
  scroll: HTMLElement;
  bar: HTMLElement;
  input: HTMLInputElement;
  count: HTMLElement;
  previous: HTMLButtonElement;
  next: HTMLButtonElement;
  close: HTMLButtonElement;
  onClose?: () => void;
}

/**
 * A fast, dependency-free find bar. It wraps text nodes in <mark> elements and
 * unwraps them on close, leaving the rendered document (and the outline
 * observers) untouched.
 */
export class FindController {
  private marks: HTMLElement[] = [];
  private current = -1;
  private query = "";
  private timer: number | undefined;
  private truncated = false;
  private isOpen = false;

  constructor(private readonly options: FindOptions) {
    const { input, previous, next, close, bar } = options;

    input.addEventListener("input", () => {
      window.clearTimeout(this.timer);
      this.timer = window.setTimeout(() => this.search(input.value), 80);
    });

    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        if (event.shiftKey) this.step(-1);
        else this.step(1);
      } else if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        this.close();
      }
    });

    previous.addEventListener("click", () => this.step(-1));
    next.addEventListener("click", () => this.step(1));
    close.addEventListener("click", () => this.close());

    bar.addEventListener("mousedown", (event) => {
      // Keep focus in the field while interacting with controls.
      if ((event.target as HTMLElement).closest("button")) {
        event.preventDefault();
      }
    });
  }

  open(): void {
    if (this.isOpen) {
      this.focusInput();
      return;
    }
    this.isOpen = true;
    this.options.bar.hidden = false;
    this.options.input.value = this.query;
    this.focusInput();
    if (this.query.trim()) this.search(this.query);
  }

  close(): void {
    if (!this.isOpen) return;
    this.isOpen = false;
    this.options.bar.hidden = true;
    this.clearMarks();
    this.options.input.blur();
    this.options.onClose?.();
  }

  get opened(): boolean {
    return this.isOpen;
  }

  /** Re-run the current search after the document is re-rendered. */
  refresh(): void {
    if (!this.isOpen) return;
    if (this.query.trim()) this.search(this.query);
    else this.updateCount();
  }

  private focusInput(): void {
    const input = this.options.input;
    input.focus();
    input.select();
  }

  private search(rawQuery: string): void {
    const query = rawQuery.trim();
    this.query = rawQuery;
    this.clearMarks();
    this.truncated = false;
    if (!query) {
      this.updateCount();
      return;
    }

    const needle = query.toLocaleLowerCase();
    const walker = document.createTreeWalker(
      this.options.root,
      NodeFilter.SHOW_TEXT,
      {
        acceptNode: (node) => {
          const parent = node.parentElement;
          if (!parent || parent.closest(SKIP)) return NodeFilter.FILTER_REJECT;
          const value = node.nodeValue;
          if (!value) return NodeFilter.FILTER_REJECT;
          return value.toLocaleLowerCase().includes(needle)
            ? NodeFilter.FILTER_ACCEPT
            : NodeFilter.FILTER_REJECT;
        },
      },
    );

    // Snapshot before mutating the tree.
    const nodes: Text[] = [];
    while (walker.nextNode()) nodes.push(walker.currentNode as Text);

    let budget = MAX_MATCHES;
    for (const node of nodes) {
      if (budget <= 0) {
        this.truncated = true;
        break;
      }
      budget -= this.highlightNode(node, needle, budget);
    }

    this.marks = Array.from(
      this.options.root.querySelectorAll<HTMLElement>("mark.find-hit"),
    );
    if (this.marks.length > 0) {
      this.current = 0;
      this.applyCurrent(true);
    } else {
      this.current = -1;
    }
    this.updateCount();
  }

  private highlightNode(node: Text, needle: string, budget: number): number {
    const text = node.nodeValue ?? "";
    const lower = text.toLocaleLowerCase();
    let from = 0;
    let index = lower.indexOf(needle, from);
    if (index === -1) return 0;

    const fragment = document.createDocumentFragment();
    let count = 0;
    while (index !== -1 && count < budget) {
      if (index > from) {
        fragment.append(document.createTextNode(text.slice(from, index)));
      }
      const mark = document.createElement("mark");
      mark.className = "find-hit";
      mark.textContent = text.slice(index, index + needle.length);
      fragment.append(mark);
      count += 1;
      from = index + needle.length;
      index = lower.indexOf(needle, from);
    }
    if (from < text.length) fragment.append(document.createTextNode(text.slice(from)));
    node.parentNode?.replaceChild(fragment, node);
    return count;
  }

  private clearMarks(): void {
    const marks = Array.from(
      this.options.root.querySelectorAll<HTMLElement>("mark.find-hit"),
    );
    if (marks.length === 0) {
      this.marks = [];
      this.current = -1;
      return;
    }
    const parents = new Set<Node>();
    for (const mark of marks) {
      const parent = mark.parentNode;
      if (!parent) continue;
      parents.add(parent);
      parent.replaceChild(document.createTextNode(mark.textContent ?? ""), mark);
    }
    for (const parent of parents) {
      (parent as Element).normalize?.();
    }
    this.marks = [];
    this.current = -1;
  }

  private step(direction: 1 | -1): void {
    if (this.marks.length === 0) return;
    this.current =
      (this.current + direction + this.marks.length) % this.marks.length;
    this.applyCurrent(true);
    this.updateCount();
  }

  private applyCurrent(scroll: boolean): void {
    for (const mark of this.marks) mark.classList.remove("is-current");
    const mark = this.marks[this.current];
    if (!mark) return;
    mark.classList.add("is-current");
    if (scroll) {
      mark.scrollIntoView({
        block: "center",
        inline: "nearest",
        behavior: reducedMotion() ? "auto" : "smooth",
      });
    }
  }

  private updateCount(): void {
    const count = this.options.count;
    if (!this.query.trim()) {
      count.textContent = "0 / 0";
      return;
    }
    if (this.marks.length === 0) {
      count.textContent = "No results";
      return;
    }
    const total = this.truncated
      ? `${this.marks.length}+`
      : String(this.marks.length);
    count.textContent = `${this.current + 1} / ${total}`;
  }
}
