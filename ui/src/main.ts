import "./styles/index.css";

import {
  hasTauri,
  invoke,
  listen,
  openDocumentPath,
} from "./bridge";
import { FindController } from "./find";
import { OutlineController, scrollToId } from "./outline";
performance.mark("folio:module-start");

import type {
  Bootstrap,
  DefaultAppStatus,
  DocumentPayload,
  MenuEvent,
  RecentFile,
  Theme,
} from "./types";

/* --------------------------------------------------------------------------
   Elements
   -------------------------------------------------------------------------- */

const $ = <T extends HTMLElement>(id: string): T => {
  const element = document.getElementById(id);
  if (!element) throw new Error(`Missing element #${id}`);
  return element as T;
};

const app = $<HTMLDivElement>("app");
const progressBar = $<HTMLElement>("progress-bar");
const railScrim = $<HTMLElement>("rail-scrim");
const railToggle = $<HTMLButtonElement>("rail-toggle");
const railClose = $<HTMLButtonElement>("rail-close");
const outlineList = $<HTMLOListElement>("outline");
const outlineEmpty = $<HTMLElement>("outline-empty");
const themeTrigger = $<HTMLButtonElement>("theme-trigger");
const themePanel = $<HTMLElement>("theme-panel");
const themeClose = $<HTMLButtonElement>("theme-close");
const themeList = $<HTMLElement>("theme-list");
const themeName = $<HTMLElement>("theme-name");
const themeSwatch = $<HTMLElement>("theme-swatch");
const themeAuto = $<HTMLButtonElement>("theme-auto");
const themeNote = $<HTMLElement>("theme-note");
const themeDir = $<HTMLButtonElement>("theme-dir");
const docElement = $<HTMLElement>("doc");
const welcome = $<HTMLElement>("welcome");
const recentsWrap = $<HTMLElement>("recents-wrap");
const recentList = $<HTMLUListElement>("recent-list");
const colophon = $<HTMLElement>("colophon");
const colophonTheme = $<HTMLElement>("colophon-theme");
const colophonMeta = $<HTMLElement>("colophon-meta");
const dropOverlay = $<HTMLElement>("drop-overlay");
const docName = $<HTMLElement>("doc-name");
const docPath = $<HTMLElement>("doc-path");
const docStats = $<HTMLElement>("doc-stats");
const scrollElement = $<HTMLElement>("scroll");
const findButton = $<HTMLButtonElement>("find-btn");
const openButton = $<HTMLButtonElement>("open-btn");
const welcomeOpen = $<HTMLButtonElement>("welcome-open");
const welcomeSample = $<HTMLButtonElement>("welcome-sample");
const toastElement = $<HTMLElement>("toast");
const findBar = $<HTMLElement>("findbar");
const findInput = $<HTMLInputElement>("find-input");
const findCount = $<HTMLElement>("find-count");
const findPrevious = $<HTMLButtonElement>("find-prev");
const findNext = $<HTMLButtonElement>("find-next");
const findClose = $<HTMLButtonElement>("find-close");
const defaultPrompt = $<HTMLElement>("default-prompt");
const defaultYes = $<HTMLButtonElement>("default-yes");
const defaultNo = $<HTMLButtonElement>("default-no");
const defaultNever = $<HTMLButtonElement>("default-never");

/* --------------------------------------------------------------------------
   State
   -------------------------------------------------------------------------- */

interface State {
  boot: Bootstrap | null;
  themes: Theme[];
  themeById: Map<string, Theme>;
  selectedTheme: string;
  autoTheme: boolean;
  textScale: number;
  document: DocumentPayload | null;
  activeTheme: Theme | null;
  pendingOpen: string | null;
  pendingActions: string[];
  toastTimer: number | undefined;
  scrollFrame: number | null;
  renderToken: number;
}

const state: State = {
  boot: null,
  themes: [],
  themeById: new Map(),
  selectedTheme: "blend",
  autoTheme: true,
  textScale: 1,
  document: null,
  activeTheme: null,
  pendingOpen: null,
  pendingActions: [],
  toastTimer: undefined,
  scrollFrame: null,
  renderToken: 0,
};

const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");
const narrowQuery = window.matchMedia("(max-width: 1040px)");

const outline = new OutlineController(
  outlineList,
  outlineEmpty,
  scrollElement,
  (id) => {
    outline.select(id);
    scrollToId(scrollElement, id);
    history.replaceState(null, "", `#${encodeURIComponent(id)}`);
    if (narrowQuery.matches) closeRail();
  },
);

const finder = new FindController({
  root: docElement,
  scroll: scrollElement,
  bar: findBar,
  input: findInput,
  count: findCount,
  previous: findPrevious,
  next: findNext,
  close: findClose,
  onClose: () => {
    scrollElement.focus({ preventScroll: true });
  },
});

/* --------------------------------------------------------------------------
   Boot
   -------------------------------------------------------------------------- */

async function boot(): Promise<void> {
  installEventListeners();

  try {
    const bootData = await invoke<Bootstrap>("bootstrap");
    state.boot = bootData;
    state.themes = bootData.themes.themes;
    state.themeById = new Map(state.themes.map((theme) => [theme.id, theme]));
    injectThemeCss(bootData.themes.css);
    restorePreferences(bootData);
    buildThemeList();
    applyTheme();
    renderRecents(bootData.recents);

    document.documentElement.classList.toggle(
      "is-macos",
      bootData.platform === "macos",
    );
    themeDir.title = bootData.themesDir;

    const params = new URLSearchParams(location.search);
    if (params.get("welcome") === "1") {
      showWelcome();
    } else if (state.pendingOpen) {
      const path = state.pendingOpen;
      state.pendingOpen = null;
      await openPath(path);
    } else if (bootData.initialPath) {
      await openPath(bootData.initialPath);
    } else if (bootData.initialDocument) {
      renderDocument(bootData.initialDocument);
    } else {
      showWelcome();
    }
  } catch (error) {
    showWelcome();
    showToast(errorText(error));
  } finally {
    for (const action of state.pendingActions.splice(0)) runAction(action);
    invoke("show_window").catch(() => {});
    updateFindAvailability();
  }
}

function installEventListeners(): void {
  railToggle.addEventListener("click", () => runAction("toggle-outline"));
  railClose.addEventListener("click", closeRail);
  railScrim.addEventListener("click", closeRail);
  openButton.addEventListener("click", openFile);
  welcomeOpen.addEventListener("click", openFile);
  welcomeSample.addEventListener("click", openSample);
  findButton.addEventListener("click", () => finder.open());

  themeTrigger.addEventListener("click", () => toggleThemePanel());
  themeClose.addEventListener("click", () => setThemePanel(false));
  themeAuto.addEventListener("click", toggleAutoTheme);
  themeDir.addEventListener("click", revealThemes);

  defaultYes.addEventListener("click", () => void answerDefaultPrompt("yes"));
  defaultNo.addEventListener("click", () => void answerDefaultPrompt("no"));
  defaultNever.addEventListener("click", () => void answerDefaultPrompt("never"));
  defaultPrompt.addEventListener("click", (event) => {
    if (event.target === defaultPrompt) void answerDefaultPrompt("no");
  });
  defaultPrompt.addEventListener("keydown", handleDefaultPromptKeys);

  docElement.addEventListener("click", handleDocumentClick);

  scrollElement.addEventListener("scroll", onScroll, { passive: true });
  window.addEventListener("resize", () => {
    window.clearTimeout(resizeTimer);
    resizeTimer = window.setTimeout(() => outline.measure(), 120);
  });

  document.addEventListener("pointerdown", (event) => {
    if (themePanel.hidden) return;
    const target = event.target as Node | null;
    if (!target) return;
    if (themePanel.contains(target) || themeTrigger.contains(target)) return;
    setThemePanel(false);
  });

  document.addEventListener("keydown", handleKeyboard);

  darkQuery.addEventListener("change", () => {
    if (state.autoTheme) applyTheme();
  });

  narrowQuery.addEventListener("change", () => {
    app.classList.remove("rail-open");
    railScrim.hidden = true;
    if (!narrowQuery.matches) app.classList.remove("is-rail-hidden");
  });

  document.fonts?.ready.then(() => outline.measure()).catch(() => {});

  void listen<DocumentPayload>("folio:document-update", (payload) => {
    if (!payload) return;
    if (!state.document || state.document.path === payload.path) {
      renderDocument(payload, true);
    }
  });
  void listen<{ path: string }>("folio:open-document", (payload) => {
    const path = payload?.path;
    if (!path) return;
    if (state.boot) void openPath(path);
    else state.pendingOpen = path;
  });
  void listen<MenuEvent>("folio:menu", (payload) => {
    if (!payload?.action) return;
    if (state.boot) runAction(payload.action);
    else state.pendingActions.push(payload.action);
  });
  void listen<{ message: string }>("folio:document-error", (payload) => {
    showToast(payload?.message ?? "The document could not be reloaded.");
  });
  void listen<{ paths?: string[] }>("tauri://drag-enter", () => {
    dropOverlay.hidden = false;
  });
  void listen("tauri://drag-leave", () => {
    dropOverlay.hidden = true;
  });
  void listen<{ paths?: string[] }>("tauri://drag-drop", (payload) => {
    dropOverlay.hidden = true;
    const file = payload?.paths?.find((path) => {
      const ext = path.split(".").pop()?.toLowerCase() ?? "";
      return ["md", "markdown", "mdown", "mkd", "mkdn", "mdx", "txt", "text"].includes(ext);
    });
    if (file) void openPath(file);
    else showToast("Drop a Markdown or plain text file.");
  });
}

let resizeTimer: number | undefined;
let bootMeasured = false;
let codeObserver: IntersectionObserver | null = null;
let defaultPromptChecked = false;
let defaultPromptShown = false;
let defaultPromptTimer: number | undefined;
let defaultPromptPreviousFocus: HTMLElement | null = null;

/* --------------------------------------------------------------------------
   Theme
   -------------------------------------------------------------------------- */

function restorePreferences(bootData: Bootstrap): void {
  const catalog = bootData.themes;
  let storedTheme = "";
  let storedAuto = "1";
  let storedScale = "1";
  try {
    storedTheme = localStorage.getItem("folio.theme") ?? "";
    storedAuto = localStorage.getItem("folio.auto") ?? "1";
    storedScale = localStorage.getItem("folio.textScale") ?? "1";
  } catch {
    // localStorage can be unavailable in locked-down webviews.
  }
  state.selectedTheme =
    (storedTheme && catalog.themes.some((theme) => theme.id === storedTheme)
      ? storedTheme
      : catalog.defaultId) || "blend";
  state.autoTheme = storedAuto !== "0";
  const scale = Number.parseFloat(storedScale);
  state.textScale = Number.isFinite(scale)
    ? Math.min(1.5, Math.max(0.85, scale))
    : 1;

  const params = new URLSearchParams(location.search);
  const paramTheme = params.get("theme");
  if (paramTheme && catalog.themes.some((theme) => theme.id === paramTheme)) {
    state.selectedTheme = paramTheme;
    state.autoTheme = false;
  }
  const paramScale = Number.parseFloat(params.get("scale") ?? "");
  if (Number.isFinite(paramScale)) {
    state.textScale = Math.min(1.5, Math.max(0.85, paramScale));
  }
}

function injectThemeCss(css: string): void {
  document.getElementById("folio-theme-vars")?.remove();
  const style = document.createElement("style");
  style.id = "folio-theme-vars";
  style.textContent = css;
  document.head.append(style);
}

function resolveTheme(): Theme {
  const fallback =
    state.themeById.get("blend") ?? state.themes[0] ?? null;
  if (!fallback) {
    throw new Error("No themes available");
  }
  if (!state.autoTheme) {
    return state.themeById.get(state.selectedTheme) ?? fallback;
  }
  const target = darkQuery.matches ? "dark" : "light";
  const preferred = state.themeById.get(state.selectedTheme);
  if (preferred?.appearance === target) return preferred;
  if (preferred?.counterpart) {
    const counterpart = state.themeById.get(preferred.counterpart);
    if (counterpart?.appearance === target) return counterpart;
  }
  const named = state.themeById.get(target === "dark" ? "blend-dark" : "blend");
  if (named?.appearance === target) return named;
  return state.themes.find((theme) => theme.appearance === target) ?? preferred ?? fallback;
}

function applyTheme(): void {
  let theme: Theme;
  try {
    theme = resolveTheme();
  } catch {
    return;
  }
  state.activeTheme = theme;
  const root = document.documentElement;
  root.dataset.theme = theme.id;
  root.dataset.appearance = theme.appearance;

  themeName.textContent = theme.name;
  themeTrigger.title = `${theme.name} theme`;
  themeSwatch.style.background = theme.colors.accent;
  themeAuto.setAttribute("aria-checked", String(state.autoTheme));
  colophonTheme.textContent = `Set in ${theme.name}`;

  let themeColor = document.querySelector<HTMLMetaElement>(
    'meta[name="theme-color"]',
  );
  if (!themeColor) {
    themeColor = document.createElement("meta");
    themeColor.name = "theme-color";
    document.head.append(themeColor);
  }
  themeColor.content = theme.colors.canvas;

  for (const row of themeList.querySelectorAll<HTMLElement>(".theme-row")) {
    row.setAttribute("aria-checked", String(row.dataset.themeId === theme.id));
  }

  applyTextScale();
  requestAnimationFrame(() => outline.measure());
}

function applyTextScale(): void {
  const root = document.documentElement;
  const theme = state.activeTheme;
  const scale = state.textScale;
  if (!theme || scale === 1) {
    root.style.removeProperty("--f-body-size");
    root.style.removeProperty("--f-measure");
    root.style.removeProperty("--f-title-size");
    root.style.removeProperty("--f-radius");
    return;
  }
  const { layout } = theme;
  root.style.setProperty(
    "--f-body-size",
    `calc(${layout.bodySize} * ${scale})`,
  );
  root.style.setProperty("--f-measure", `calc(${layout.measure} * ${scale})`);
  root.style.setProperty(
    "--f-title-size",
    `calc(${layout.titleSize} * ${scale})`,
  );
  root.style.setProperty("--f-radius", `calc(${layout.radius} * ${scale})`);
}

function buildThemeList(): void {
  themeList.replaceChildren();
  for (const theme of state.themes) {
    const row = document.createElement("button");
    row.type = "button";
    row.className = "theme-row";
    row.dataset.themeId = theme.id;
    row.setAttribute("role", "radio");
    row.setAttribute("aria-checked", "false");
    row.title = theme.description;

    const dots = document.createElement("span");
    dots.className = "theme-dots";
    dots.setAttribute("aria-hidden", "true");
    for (const color of [
      theme.colors.canvas,
      theme.colors.accent,
      theme.colors.ink,
    ]) {
      const dot = document.createElement("span");
      dot.className = "theme-dot";
      dot.style.background = color;
      dots.append(dot);
    }

    const name = document.createElement("span");
    name.className = "theme-row-name";
    name.textContent = theme.name;

    const mode = document.createElement("span");
    mode.className = "theme-row-mode";
    mode.textContent = theme.appearance;

    const check = document.createElement("span");
    check.className = "theme-row-check";
    check.setAttribute("aria-hidden", "true");
    check.textContent = "✓";

    row.append(dots, name, mode, check);
    row.addEventListener("click", () => {
      state.selectedTheme = theme.id;
      state.autoTheme = false;
      persistPreferences();
      applyTheme();
    });
    themeList.append(row);
  }

  const errors = state.boot?.themes.errors ?? [];
  themeNote.hidden = errors.length === 0;
  themeNote.textContent = errors.length
    ? `${errors.length} theme file${errors.length === 1 ? "" : "s"} skipped`
    : "";
  themeNote.title = errors.join("\n");
}

function toggleAutoTheme(): void {
  state.autoTheme = !state.autoTheme;
  persistPreferences();
  applyTheme();
}

function persistPreferences(): void {
  try {
    localStorage.setItem("folio.theme", state.selectedTheme);
    localStorage.setItem("folio.auto", state.autoTheme ? "1" : "0");
    localStorage.setItem("folio.textScale", String(state.textScale));
  } catch {
    // ignore
  }
}

function setThemePanel(open: boolean): void {
  const wasOpen = !themePanel.hidden;
  themePanel.hidden = !open;
  themeTrigger.setAttribute("aria-expanded", String(open));
  if (!open && wasOpen && themePanel.contains(document.activeElement)) {
    themeTrigger.focus();
  } else if (open) {
    requestAnimationFrame(() => {
      const checked = themeList.querySelector<HTMLElement>(
        '.theme-row[aria-checked="true"]',
      );
      (checked ?? themeAuto).focus();
    });
  }
}

function toggleThemePanel(): void {
  setThemePanel(themePanel.hidden);
}

async function revealThemes(): Promise<void> {
  try {
    await invoke("reveal_themes");
  } catch (error) {
    showToast(errorText(error));
  }
}

/* --------------------------------------------------------------------------
   Files and documents
   -------------------------------------------------------------------------- */

async function openFile(): Promise<void> {
  if (!hasTauri()) {
    showToast("Open Folio's native app to read local files.");
    return;
  }
  try {
    const payload = await invoke<DocumentPayload | null>("open_file_dialog");
    if (payload) renderDocument(payload);
  } catch (error) {
    showToast(errorText(error));
  }
}

async function openSample(): Promise<void> {
  try {
    const payload = await invoke<DocumentPayload>("open_sample");
    if (payload) renderDocument(payload);
  } catch (error) {
    showToast(errorText(error));
  }
}

async function openPath(path: string, anchor?: string | null): Promise<void> {
  try {
    const payload = await openDocumentPath(path, anchor ?? undefined);
    if (payload) renderDocument(payload, false, anchor ?? undefined);
  } catch (error) {
    showToast(errorText(error));
  }
}

function renderDocument(
  payload: DocumentPayload,
  preserveScroll = false,
  anchor?: string,
): void {
  const previous = state.document;
  const isSameDocument =
    preserveScroll && previous !== null && previous.path === payload.path;
  const previousRatio =
    isSameDocument && scrollElement.scrollHeight > scrollElement.clientHeight
      ? scrollElement.scrollTop /
        (scrollElement.scrollHeight - scrollElement.clientHeight)
      : null;

  state.document = payload;
  const token = ++state.renderToken;

  app.dataset.state = "doc";
  welcome.hidden = true;
  docElement.hidden = false;
  performance.clearMarks("folio:html-start");
  performance.clearMarks("folio:html-complete");
  performance.clearMarks("folio:decorate-complete");
  performance.clearMarks("folio:enhance-complete");
  performance.clearMarks("folio:outline-complete");
  performance.mark("folio:html-start");
  docElement.innerHTML = payload.html;
  performance.mark("folio:html-complete");
  decorateDocument(payload);
  performance.mark("folio:decorate-complete");
  enhanceCode();
  performance.mark("folio:enhance-complete");
  updateChrome(payload);
  updateFindAvailability();
  outline.render(payload.meta.outline);
  performance.mark("folio:outline-complete");
  finder.refresh();
  void loadImages(token);

  const targetAnchor = anchor ?? "";

  requestAnimationFrame(() => {
    if (targetAnchor) {
      scrollToId(scrollElement, targetAnchor);
      outline.select(targetAnchor);
    } else if (previousRatio !== null) {
      const max = scrollElement.scrollHeight - scrollElement.clientHeight;
      scrollElement.scrollTop = Math.max(0, previousRatio * max);
    } else {
      scrollElement.scrollTop = 0;
    }
    outline.measure();
    onScroll();
  });

  document.fonts?.ready
    .then(() => {
      if (token !== state.renderToken) return;
      outline.measure();
    })
    .catch(() => {});

  const title = payload.meta.title || prettyName(payload.name) || "Untitled";
  document.title = `${title} · Folio`;
  maybeOfferDefaultApp(payload);
  if (!bootMeasured) {
    bootMeasured = true;
    try {
      performance.mark("folio:first-render");
      performance.measure(
        "folio:boot",
        "folio:module-start",
        "folio:first-render",
      );
    } catch {
      // marks may not exist in exotic webviews
    }
  }
  invoke("set_title", { title: document.title }).catch(() => {});
}

/* --------------------------------------------------------------------------
   Default Markdown app
   -------------------------------------------------------------------------- */

function isMarkdownPath(path: string): boolean {
  const extension = path.split(".").pop()?.toLowerCase() ?? "";
  return extension === "md" || extension === "markdown";
}

function defaultAppPreference(): string | null {
  try {
    return localStorage.getItem("folio.defaultApp");
  } catch {
    return null;
  }
}

function maybeOfferDefaultApp(payload: DocumentPayload): void {
  if (payload.sample || state.boot?.platform !== "macos") return;
  if (!isMarkdownPath(payload.path)) return;
  if (defaultPromptChecked || defaultPromptShown) return;
  if (defaultAppPreference()) return;

  defaultPromptChecked = true;
  window.clearTimeout(defaultPromptTimer);
  defaultPromptTimer = window.setTimeout(async () => {
    if (defaultPromptShown) return;
    try {
      const status = await invoke<DefaultAppStatus>("markdown_default_status");
      if (!status.available || status.isDefault) return;
      if (defaultAppPreference() || defaultPromptShown) return;
      showDefaultPrompt();
    } catch {
      // If LaunchServices cannot be queried, stay quiet rather than nag.
    }
  }, 700);
}

function showDefaultPrompt(): void {
  if (defaultPromptShown || !defaultPrompt.hidden) return;
  defaultPromptShown = true;
  defaultPromptPreviousFocus =
    document.activeElement instanceof HTMLElement ? document.activeElement : null;
  defaultPrompt.hidden = false;
  app.classList.add("prompt-open");
  requestAnimationFrame(() => defaultYes.focus({ preventScroll: true }));
}

function closeDefaultPrompt(): void {
  defaultPrompt.hidden = true;
  app.classList.remove("prompt-open");
  defaultYes.disabled = false;
  defaultNo.disabled = false;
  defaultNever.disabled = false;
  const previous = defaultPromptPreviousFocus;
  defaultPromptPreviousFocus = null;
  if (previous?.isConnected) previous.focus({ preventScroll: true });
  else scrollElement.focus({ preventScroll: true });
}

async function answerDefaultPrompt(choice: "yes" | "no" | "never"): Promise<void> {
  if (defaultPrompt.hidden) return;
  if (choice === "no") {
    closeDefaultPrompt();
    return;
  }
  if (choice === "never") {
    try {
      localStorage.setItem("folio.defaultApp", "never");
    } catch {
      // ignore storage failures
    }
    closeDefaultPrompt();
    return;
  }

  defaultYes.disabled = true;
  defaultNo.disabled = true;
  defaultNever.disabled = true;
  try {
    await invoke("set_markdown_default");
    try {
      localStorage.setItem("folio.defaultApp", "set");
    } catch {
      // ignore storage failures
    }
    closeDefaultPrompt();
    showToast("Folio is now the default app for Markdown.");
  } catch (error) {
    closeDefaultPrompt();
    showToast(errorText(error));
  }
}

function handleDefaultPromptKeys(event: KeyboardEvent): void {
  if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    void answerDefaultPrompt("no");
    return;
  }
  if (event.key !== "Tab") return;
  const focusable = [defaultYes, defaultNo, defaultNever].filter(
    (button) => !button.disabled,
  );
  if (focusable.length === 0) return;
  const first = focusable[0];
  const last = focusable[focusable.length - 1];
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}

function showWelcome(): void {
  state.document = null;
  app.dataset.state = "welcome";
  codeObserver?.disconnect();
  codeObserver = null;
  closeDefaultPrompt();
  docElement.innerHTML = "";
  docElement.hidden = true;
  welcome.hidden = false;
  colophon.hidden = true;
  docName.textContent = "Folio";
  docPath.textContent = "A quiet reader for Markdown";
  docPath.title = "";
  docStats.replaceChildren();
  outline.clear();
  finder.close();
  progressBar.style.transform = "scaleX(0)";
  app.classList.remove("is-scrolled");
  scrollElement.scrollTop = 0;
  document.title = "Folio";
  invoke("set_title", { title: "Folio" }).catch(() => {});
  updateFindAvailability();
  if (state.boot) renderRecents(state.boot.recents);
}

function renderRecents(recents: RecentFile[]): void {
  recentList.replaceChildren();
  const shown = recents.slice(0, 6);
  recentsWrap.hidden = shown.length === 0 || app.dataset.state === "doc";
  for (const recent of shown) {
    const item = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    button.className = "recent";

    const icon = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    icon.setAttribute("class", "icon");
    icon.setAttribute("aria-hidden", "true");
    const use = document.createElementNS("http://www.w3.org/2000/svg", "use");
    use.setAttribute("href", "#i-doc");
    icon.append(use);

    const text = document.createElement("span");
    text.className = "recent-text";
    const name = document.createElement("span");
    name.className = "recent-name";
    name.textContent = recent.name;
    const directory = document.createElement("span");
    directory.className = "recent-dir";
    directory.textContent = shortenHome(recent.directory);
    text.append(name, directory);

    const arrow = document.createElement("span");
    arrow.className = "recent-arrow";
    arrow.setAttribute("aria-hidden", "true");
    arrow.textContent = "→";

    button.append(icon, text, arrow);
    button.addEventListener("click", () => void openPath(recent.path));
    item.append(button);
    recentList.append(item);
  }
}

function updateChrome(payload: DocumentPayload): void {
  docName.textContent = payload.name;
  docPath.textContent = payload.sample
    ? "Built-in sample"
    : shortenHome(payload.path);
  docPath.title = payload.sample ? "A document that ships with Folio" : payload.path;
  colophon.hidden = false;
  colophonTheme.textContent = state.activeTheme
    ? `Set in ${state.activeTheme.name}`
    : "Set in Blend";

  const words = formatNumber(payload.meta.stats.words);
  const minutes = payload.meta.stats.minutes;
  docStats.replaceChildren();
  const wordSpan = document.createElement("span");
  wordSpan.textContent = `${words} ${payload.meta.stats.words === 1 ? "word" : "words"}`;
  docStats.append(wordSpan);
  if (minutes > 0) {
    const dot = document.createElement("span");
    dot.className = "dot";
    const readSpan = document.createElement("span");
    readSpan.textContent = `${minutes} min read`;
    docStats.append(dot, readSpan);
  }
  if (!payload.sample) {
    const live = document.createElement("span");
    live.className = "live-dot";
    live.title = "Watching for changes";
    docStats.append(live);
  }

  const modified = payload.modifiedMs ? formatModified(payload.modifiedMs) : "";
  const metaParts = [
    `${words} ${payload.meta.stats.words === 1 ? "word" : "words"}`,
    minutes > 0 ? `${minutes} min` : "",
    modified ? `Edited ${modified}` : "",
  ].filter(Boolean);
  colophonMeta.textContent = metaParts.join(" · ");
}

function decorateDocument(payload: DocumentPayload): void {
  let title = docElement.querySelector<HTMLElement>(".doc-title");
  if (!title) {
    const firstH1 = docElement.querySelector<HTMLHeadingElement>("h1");
    if (firstH1) {
      title = firstH1;
      title.classList.add("doc-title");
    } else {
      const injected = document.createElement("h1");
      injected.className = "doc-title";
      injected.textContent =
        payload.meta.title || prettyName(payload.name) || "Untitled";
      docElement.prepend(injected);
      const firstParagraph = docElement.querySelector("p");
      firstParagraph?.classList.add("lead");
      title = injected;
    }
  }

  const kicker = document.createElement("p");
  kicker.className = "doc-kicker";
  const parts: string[] = [];
  if (payload.meta.stats.minutes > 0) {
    parts.push(`${payload.meta.stats.minutes} min read`);
  }
  if (payload.meta.date) parts.push(formatDate(payload.meta.date));
  if (payload.meta.author) parts.push(payload.meta.author);
  for (const [index, part] of parts.entries()) {
    if (index > 0) {
      const separator = document.createElement("span");
      separator.className = "sep";
      separator.textContent = "·";
      kicker.append(separator);
    }
    const span = document.createElement("span");
    span.textContent = part;
    kicker.append(span);
  }
  if (parts.length > 0) title.before(kicker);

  if (payload.meta.tags.length > 0) {
    const tags = document.createElement("div");
    tags.className = "doc-tags";
    for (const tag of payload.meta.tags) {
      const chip = document.createElement("span");
      chip.className = "doc-tag";
      chip.textContent = tag;
      tags.append(chip);
    }
    title.after(tags);
  }
}

/* --------------------------------------------------------------------------
   Document interactions
   -------------------------------------------------------------------------- */

function handleDocumentClick(event: MouseEvent): void {
  const target = event.target as HTMLElement | null;
  if (!target) return;

  const copyButton = target.closest<HTMLElement>(".code-copy");
  if (copyButton) {
    const block = copyButton.closest(".code-block");
    const code = block?.querySelector("pre code")?.textContent ?? "";
    void copyCode(copyButton, code);
    return;
  }

  const anchor = target.closest<HTMLAnchorElement>("a");
  if (!anchor) return;

  const openPathAttribute = anchor.dataset.openPath;
  if (openPathAttribute) {
    event.preventDefault();
    if (anchor.dataset.linkKind === "doc") {
      void openPath(openPathAttribute, anchor.dataset.anchor);
    } else if (hasTauri()) {
      invoke("open_path", { path: openPathAttribute }).catch((error: unknown) =>
        showToast(errorText(error)),
      );
    } else {
      showToast("Only the native app can open local files.");
    }
    return;
  }

  if (anchor.dataset.external) {
    event.preventDefault();
    const url = anchor.getAttribute("href") ?? "";
    if (hasTauri()) {
      invoke("open_external", { url }).catch((error: unknown) =>
        showToast(errorText(error)),
      );
    } else {
      showToast("The native app opens this link in your browser.");
    }
    return;
  }

  const href = anchor.getAttribute("href") ?? "";
  if (href.startsWith("#")) {
    event.preventDefault();
    const id = href.slice(1);
    if (id && scrollToId(scrollElement, id)) {
      outline.select(id);
      history.replaceState(null, "", `#${id}`);
    }
    return;
  }

  if (!hasTauri() && !/^[a-z]+:/i.test(href)) {
    event.preventDefault();
    showToast("Local links open in the native app.");
  }
}

async function copyCode(button: HTMLElement, text: string): Promise<void> {
  try {
    await copyText(text);
    button.textContent = "Copied";
    button.classList.add("is-copied");
    window.setTimeout(() => {
      button.textContent = "Copy";
      button.classList.remove("is-copied");
    }, 1300);
  } catch {
    showToast("Could not copy this snippet.");
  }
}

async function copyText(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text);
      return;
    } catch {
      // fall through to the legacy path
    }
  }
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  textarea.style.pointerEvents = "none";
  document.body.append(textarea);
  textarea.select();
  const ok = document.execCommand("copy");
  textarea.remove();
  if (!ok) throw new Error("copy failed");
}

function enhanceCode(): void {
  codeObserver?.disconnect();
  const blocks = docElement.querySelectorAll<HTMLElement>(".code-block");
  if (!('IntersectionObserver' in window)) {
    blocks.forEach(enhanceCodeBlock);
    return;
  }
  // Counting lines means reading every code block's text. For a very large
  // document that is measurable work, so only do it for blocks near the
  // viewport. Copy buttons are already in the server-rendered markup.
  codeObserver = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        codeObserver?.unobserve(entry.target);
        enhanceCodeBlock(entry.target as HTMLElement);
      }
    },
    { root: scrollElement, rootMargin: "700px 0px" },
  );
  blocks.forEach((block) => codeObserver?.observe(block));
}

function enhanceCodeBlock(block: HTMLElement): void {
  if (block.dataset.enhanced === "true") return;
  const code = block.querySelector("pre code");
  if (!code) return;
  block.dataset.enhanced = "true";
  const lines = (code.textContent ?? "").split("\n").length;
  if (lines <= 34) return;
  block.classList.add("is-collapsible");
  const button = document.createElement("button");
  button.type = "button";
  button.className = "code-expand";
  const hidden = lines - 34;
  button.textContent = `Show ${hidden} more lines`;
  button.setAttribute("aria-expanded", "false");
  button.addEventListener("click", () => {
    const expanded = block.classList.toggle("is-expanded");
    button.setAttribute("aria-expanded", String(expanded));
    button.textContent = expanded ? "Collapse code" : `Show ${hidden} more lines`;
    requestAnimationFrame(() => outline.measure());
  });
  block.append(button);
}

async function loadImages(token: number): Promise<void> {
  const images = Array.from(
    docElement.querySelectorAll<HTMLImageElement>("img[data-local-image]"),
  );
  await Promise.all(
    images.map(async (image) => {
      const path = image.dataset.localImage;
      if (!path) return;
      try {
        const source = await invoke<string>("read_image", { path });
        if (token !== state.renderToken) return;
        if (!source) throw new Error("unsupported image");
        image.addEventListener(
          "load",
          () => {
            image.classList.add("is-loaded");
            requestAnimationFrame(() => outline.measure());
          },
          { once: true },
        );
        image.src = source;
        if (image.complete) image.classList.add("is-loaded");
      } catch {
        if (token !== state.renderToken) return;
        const missing = document.createElement("div");
        missing.className = "image-missing";
        missing.textContent = `Image unavailable · ${fileName(path)}`;
        image.replaceWith(missing);
      }
    }),
  );
}

/* --------------------------------------------------------------------------
   Chrome, scrolling, find availability
   -------------------------------------------------------------------------- */

function onScroll(): void {
  if (state.scrollFrame !== null) return;
  state.scrollFrame = requestAnimationFrame(() => {
    state.scrollFrame = null;
    const max = scrollElement.scrollHeight - scrollElement.clientHeight;
    const ratio = max > 0 ? Math.min(1, scrollElement.scrollTop / max) : 0;
    progressBar.style.transform = `scaleX(${ratio})`;
    app.classList.toggle("is-scrolled", scrollElement.scrollTop > 8);
    outline.update();
  });
}

function updateFindAvailability(): void {
  const enabled = state.document !== null;
  findButton.disabled = !enabled;
  findButton.setAttribute("aria-disabled", String(!enabled));
}

/* --------------------------------------------------------------------------
   Rail, theme panel, actions
   -------------------------------------------------------------------------- */

function closeRail(): void {
  const wasOpen = app.classList.contains("rail-open");
  app.classList.remove("rail-open");
  railScrim.hidden = true;
  if (wasOpen) railToggle.focus({ preventScroll: true });
}

function toggleRail(): void {
  if (narrowQuery.matches) {
    const open = !app.classList.contains("rail-open");
    app.classList.toggle("rail-open", open);
    railScrim.hidden = !open;
    if (open) requestAnimationFrame(() => outline.measure());
  } else {
    app.classList.toggle("is-rail-hidden");
    requestAnimationFrame(() => outline.measure());
  }
}

function ensureRailVisible(): void {
  if (narrowQuery.matches) {
    app.classList.add("rail-open");
    railScrim.hidden = false;
  } else {
    app.classList.remove("is-rail-hidden");
  }
  requestAnimationFrame(() => outline.measure());
}

const actionGuard = new Map<string, number>();

function runAction(action: string): void {
  const now = performance.now();
  const last = actionGuard.get(action) ?? -Infinity;
  if (now - last < 90) return;
  actionGuard.set(action, now);
  performAction(action);
}

function performAction(action: string): void {
  switch (action) {
    case "open":
      void openFile();
      break;
    case "open-sample":
      void openSample();
      break;
    case "toggle-outline":
      toggleRail();
      break;
    case "find":
      if (state.document) {
        setThemePanel(false);
        finder.open();
      }
      break;
    case "theme":
      ensureRailVisible();
      setThemePanel(true);
      break;
    case "text-larger":
      changeTextScale(0.1);
      break;
    case "text-smaller":
      changeTextScale(-0.1);
      break;
    case "text-reset":
      setTextScale(1);
      break;
    case "reload":
      if (state.document) {
        if (state.document.sample) void openSample();
        else void openPath(state.document.path);
      }
      break;
    case "welcome":
      showWelcome();
      break;
    case "reveal-themes":
      void revealThemes();
      break;
    default:
      break;
  }
}

function changeTextScale(delta: number): void {
  const next = Math.round((state.textScale + delta) * 100) / 100;
  setTextScale(next);
}

function setTextScale(scale: number): void {
  const clamped = Math.min(1.5, Math.max(0.85, scale));
  state.textScale = clamped;
  persistPreferences();
  applyTextScale();
  requestAnimationFrame(() => outline.measure());
  showToast(`Text size ${Math.round(clamped * 100)}%`);
}

function handleKeyboard(event: KeyboardEvent): void {
  if (!defaultPrompt.hidden) return;
  if (event.key === "Escape") {
    if (!themePanel.hidden) {
      setThemePanel(false);
      return;
    }
    if (finder.opened) {
      finder.close();
      return;
    }
    if (app.classList.contains("rail-open")) {
      closeRail();
      return;
    }
    return;
  }

  const modifier = event.metaKey || event.ctrlKey;
  if (!modifier || event.altKey) return;
  const key = event.key.toLowerCase();

  if (key === "o") {
    event.preventDefault();
    runAction("open");
  } else if (key === "b") {
    event.preventDefault();
    runAction("toggle-outline");
  } else if (key === "f") {
    event.preventDefault();
    runAction("find");
  } else if (key === ",") {
    event.preventDefault();
    runAction("theme");
  } else if (key === "=" || key === "+") {
    event.preventDefault();
    runAction("text-larger");
  } else if (key === "-") {
    event.preventDefault();
    runAction("text-smaller");
  } else if (key === "0") {
    event.preventDefault();
    runAction("text-reset");
  } else if (key === "r") {
    event.preventDefault();
    runAction("reload");
  }
}

/* --------------------------------------------------------------------------
   Toast
   -------------------------------------------------------------------------- */

function showToast(message: string): void {
  if (!message) return;
  toastElement.textContent = message;
  toastElement.hidden = false;
  window.clearTimeout(state.toastTimer);
  state.toastTimer = window.setTimeout(() => {
    toastElement.hidden = true;
  }, 3600);
}

/* --------------------------------------------------------------------------
   Small helpers
   -------------------------------------------------------------------------- */

function errorText(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Something went wrong.";
}

function formatNumber(value: number): string {
  return new Intl.NumberFormat().format(value);
}

function formatDate(value: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(value);
  if (!match) return value;
  const date = new Date(
    Number(match[1]),
    Number(match[2]) - 1,
    Number(match[3]),
  );
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

function formatModified(ms: number): string {
  if (!ms) return "";
  const date = new Date(ms);
  if (Number.isNaN(date.getTime())) return "";
  const today = new Date();
  if (date.toDateString() === today.toDateString()) {
    return date.toLocaleTimeString(undefined, {
      hour: "numeric",
      minute: "2-digit",
    });
  }
  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

function shortenHome(path: string): string {
  return path.replace(/^\/(?:Users|home)\/[^/]+/, "~");
}

function fileName(path: string): string {
  const parts = path.split("/");
  return parts[parts.length - 1] || path;
}

function prettyName(name: string): string {
  const cleaned = name
    .replace(/\.(md|markdown|mdown|mkd|mkdn|txt|text)$/i, "")
    .replace(/[-_]+/g, " ")
    .trim();
  return cleaned.length > 0 ? cleaned : name;
}

/* --------------------------------------------------------------------------
   Test surface (browser only)
   -------------------------------------------------------------------------- */

if (!hasTauri()) {
  (
    window as unknown as {
      __folio?: {
        render: (payload: DocumentPayload) => void;
        welcome: () => void;
        theme: (id: string) => void;
        scale: (value: number) => void;
        find: () => void;
        menu: (action: string) => void;
        defaultPrompt: () => void;
      };
    }
  ).__folio = {
    render: (payload) => renderDocument(payload),
    welcome: showWelcome,
    theme: (id) => {
      if (state.themeById.has(id)) {
        state.selectedTheme = id;
        state.autoTheme = false;
        persistPreferences();
        applyTheme();
      }
    },
    scale: setTextScale,
    find: () => runAction("find"),
    menu: runAction,
    defaultPrompt: showDefaultPrompt,
  };
}

void boot();
