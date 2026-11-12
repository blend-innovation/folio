import type { Heading } from "./types";

const reducedMotion = (): boolean =>
  window.matchMedia("(prefers-reduced-motion: reduce)").matches;

/** Scroll the reading column to a heading, respecting the floating chrome. */
export function scrollToId(scroll: HTMLElement, rawId: string): boolean {
  let id = rawId;
  try {
    id = decodeURIComponent(rawId);
  } catch {
    // keep the raw fragment
  }
  const element =
    document.getElementById(id) ?? document.getElementById(rawId) ?? null;
  if (!element) return false;
  const scrollRect = scroll.getBoundingClientRect();
  const top =
    element.getBoundingClientRect().top -
    scrollRect.top +
    scroll.scrollTop -
    82;
  scroll.scrollTo({
    top: Math.max(0, top),
    behavior: reducedMotion() ? "auto" : "smooth",
  });
  return true;
}

/**
 * The contents rail: builds links, tracks the active section with cached
 * offsets, and keeps the active link visible inside its own scroller.
 */
export class OutlineController {
  private entries: Heading[] = [];
  private links = new Map<string, HTMLAnchorElement>();
  private tops: Array<{ id: string; top: number }> = [];
  private active: string | null = null;

  constructor(
    private readonly list: HTMLElement,
    private readonly empty: HTMLElement,
    private readonly scroll: HTMLElement,
    onNavigate: (id: string) => void,
  ) {
    this.list.addEventListener("click", (event) => {
      const target = event.target as HTMLElement | null;
      const anchor = target?.closest<HTMLAnchorElement>("a[data-id]");
      if (!anchor) return;
      event.preventDefault();
      const id = anchor.dataset.id;
      if (id) onNavigate(id);
    });
  }

  render(entries: Heading[]): void {
    this.entries = entries;
    this.links.clear();
    this.tops = [];
    this.active = null;
    this.list.replaceChildren();

    const sections = entries.filter((entry) => entry.level > 1);
    const shown = sections.length > 0 ? sections : entries;

    this.empty.hidden = shown.length > 0;
    this.list.hidden = shown.length === 0;

    for (const entry of shown) {
      const item = document.createElement("li");
      item.dataset.level = String(entry.level);
      const anchor = document.createElement("a");
      anchor.href = `#${entry.id}`;
      anchor.dataset.id = entry.id;
      anchor.textContent = entry.text;
      anchor.title = entry.text;
      item.append(anchor);
      this.list.append(item);
      this.links.set(entry.id, anchor);
    }

    requestAnimationFrame(() => this.measure());
  }

  clear(): void {
    this.entries = [];
    this.links.clear();
    this.tops = [];
    this.active = null;
    this.list.replaceChildren();
    this.list.hidden = true;
    this.empty.hidden = false;
  }

  measure(): void {
    if (this.entries.length === 0) return;
    const base = this.scroll.getBoundingClientRect().top;
    const scrollTop = this.scroll.scrollTop;
    const tops: Array<{ id: string; top: number }> = [];
    for (const entry of this.entries) {
      const element = document.getElementById(entry.id);
      if (!element) continue;
      const top = element.getBoundingClientRect().top - base + scrollTop;
      tops.push({ id: entry.id, top });
    }
    this.tops = tops;
    this.update();
  }

  /** Called on every rAF-throttled scroll tick. */
  update(): void {
    if (this.tops.length === 0) return;
    const y = this.scroll.scrollTop + 108;
    let low = 0;
    let high = this.tops.length - 1;
    let chosen = -1;
    while (low <= high) {
      const mid = (low + high) >> 1;
      if (this.tops[mid].top <= y) {
        chosen = mid;
        low = mid + 1;
      } else {
        high = mid - 1;
      }
    }
    const id = chosen >= 0 ? this.tops[chosen].id : null;
    if (id === this.active) return;
    this.setActive(id);
  }

  select(id: string): void {
    this.setActive(id);
  }

  private setActive(id: string | null): void {
    if (this.active) {
      const previous = this.links.get(this.active);
      previous?.classList.remove("is-active");
      previous?.removeAttribute("aria-current");
    }
    this.active = id;
    if (!id) return;
    const link = this.links.get(id);
    if (!link) return;
    link.classList.add("is-active");
    link.setAttribute("aria-current", "true");

    const linkRect = link.getBoundingClientRect();
    const listRect = this.list.getBoundingClientRect();
    if (linkRect.top < listRect.top + 8) {
      this.list.scrollTop -= listRect.top + 8 - linkRect.top;
    } else if (linkRect.bottom > listRect.bottom - 8) {
      this.list.scrollTop += linkRect.bottom - (listRect.bottom - 8);
    }
  }
}
