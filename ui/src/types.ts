export interface Stats {
  words: number;
  minutes: number;
  headings: number;
  links: number;
  images: number;
  codeBlocks: number;
  codeLines: number;
  tables: number;
  footnotes: number;
}

export interface Heading {
  level: number;
  id: string;
  text: string;
}

export interface DocumentMeta {
  title: string | null;
  description: string | null;
  author: string | null;
  date: string | null;
  tags: string[];
  outline: Heading[];
  stats: Stats;
  hasH1: boolean;
}

export interface DocumentPayload {
  path: string;
  name: string;
  html: string;
  meta: DocumentMeta;
  size: number;
  modifiedMs: number;
  sample: boolean;
}

export type Appearance = "light" | "dark";

export interface ThemeColors {
  canvas: string;
  surface: string;
  surfaceMuted: string;
  ink: string;
  muted: string;
  subtle: string;
  accent: string;
  accentStrong: string;
  accentSoft: string;
  accentLine: string;
  warm: string;
  warmSoft: string;
  rule: string;
  selection: string;
  shadow: string;
}

export interface ThemeFonts {
  body: string;
  display: string;
  mono: string;
}

export interface ThemeLayout {
  measure: string;
  radius: string;
  bodySize: string;
  lineHeight: number;
  titleSize: string;
}

export interface Theme {
  id: string;
  name: string;
  description: string;
  appearance: Appearance;
  counterpart: string | null;
  colors: ThemeColors;
  fonts: ThemeFonts;
  layout: ThemeLayout;
}

export interface ThemeCatalog {
  themes: Theme[];
  css: string;
  defaultId: string;
  errors: string[];
}

export interface RecentFile {
  path: string;
  name: string;
  directory: string;
  openedAt: number;
}

export interface AppConfig {
  theme: string | null;
  matchSystem: boolean | null;
  textSize: number | null;
  showOutline: boolean | null;
}

export interface Bootstrap {
  themes: ThemeCatalog;
  recents: RecentFile[];
  version: string;
  initialPath: string | null;
  initialDocument?: DocumentPayload | null;
  sampleDocument?: DocumentPayload | null;
  themesDir: string;
  config: AppConfig;
  configPath: string;
  configErrors: string[];
  platform: string;
}

export interface MenuEvent {
  action: string;
}

export interface DefaultAppStatus {
  available: boolean;
  isDefault: boolean;
}
