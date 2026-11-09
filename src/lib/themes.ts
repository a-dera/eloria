// Theme gallery. Each theme has a scheme (light or dark), a suggested accent
// colour and a palette overriding the CSS variables of app.css.
// The base themes (light, dark, sepia) are defined directly in app.css.

import { tk } from "./i18n.svelte";

export type Scheme = "light" | "dark";

export interface Theme {
  id: string;
  /** Displayed name (translated if it is a generic name) */
  name: string;
  scheme: Scheme;
  accent: string;
  /** CSS variables (without the -- prefix); empty for base themes. */
  vars: Record<string, string>;
}

/** Full palette from a few colours. */
function palette(p: {
  bg: string;
  soft: string;
  sunk: string;
  raised: string;
  line: string;
  lineStrong: string;
  text: string;
  textSoft: string;
  textFaint: string;
  mark: string;
  danger: string;
  success: string;
  code: string;
  syn: [keyword: string, string: string, number: string, fn: string, type: string, prop: string];
}): Record<string, string> {
  const [keyword, string, number, fn, type, prop] = p.syn;
  return {
    bg: p.bg,
    "bg-soft": p.soft,
    "bg-sunk": p.sunk,
    "bg-raised": p.raised,
    line: p.line,
    "line-strong": p.lineStrong,
    text: p.text,
    "text-soft": p.textSoft,
    "text-faint": p.textFaint,
    mark: p.mark,
    danger: p.danger,
    success: p.success,
    "code-bg": p.code,
    "syn-keyword": keyword,
    "syn-string": string,
    "syn-number": number,
    "syn-fn": fn,
    "syn-type": type,
    "syn-prop": prop,
  };
}

export const THEMES: Theme[] = [
  { id: "light", name: tk("Clair"), scheme: "light", accent: "#7c5cff", vars: {} },
  { id: "dark", name: tk("Sombre"), scheme: "dark", accent: "#7c5cff", vars: {} },
  { id: "sepia", name: tk("Sépia"), scheme: "light", accent: "#9a5b2e", vars: {} },
  {
    id: "one-dark",
    name: tk("One Dark Pro"),
    scheme: "dark",
    accent: "#61afef",
    vars: palette({
      bg: "#282c34", soft: "#21252b", sunk: "#1b1e23", raised: "#2c313a", line: "#3a3f4b", lineStrong: "#4b5263",
      text: "#d7dae0", textSoft: "#abb2bf", textFaint: "#7f848e", mark: "#5a4d1e", danger: "#e06c75", success: "#98c379", code: "#21252b",
      syn: ["#c678dd", "#98c379", "#d19a66", "#61afef", "#e5c07b", "#e06c75"],
    }),
  },
  {
    id: "dracula",
    name: tk("Dracula"),
    scheme: "dark",
    accent: "#bd93f9",
    vars: palette({
      bg: "#282a36", soft: "#21222c", sunk: "#191a21", raised: "#2f3140", line: "#3b3d4f", lineStrong: "#44475a",
      text: "#f8f8f2", textSoft: "#c6c8d6", textFaint: "#6272a4", mark: "#5c5a2a", danger: "#ff5555", success: "#50fa7b", code: "#21222c",
      syn: ["#ff79c6", "#f1fa8c", "#bd93f9", "#50fa7b", "#8be9fd", "#ffb86c"],
    }),
  },
  {
    id: "nord",
    name: tk("Nord"),
    scheme: "dark",
    accent: "#88c0d0",
    vars: palette({
      bg: "#2e3440", soft: "#2a2f3a", sunk: "#242933", raised: "#3b4252", line: "#3b4252", lineStrong: "#4c566a",
      text: "#eceff4", textSoft: "#d8dee9", textFaint: "#7b88a1", mark: "#5c5638", danger: "#bf616a", success: "#a3be8c", code: "#3b4252",
      syn: ["#81a1c1", "#a3be8c", "#b48ead", "#88c0d0", "#8fbcbb", "#d08770"],
    }),
  },
  {
    id: "solarized-light",
    name: tk("Solarized Light"),
    scheme: "light",
    accent: "#268bd2",
    vars: palette({
      bg: "#fdf6e3", soft: "#f5efdc", sunk: "#eee8d5", raised: "#fffbf0", line: "#e6dfc8", lineStrong: "#d6cdb0",
      text: "#3b4a50", textSoft: "#586e75", textFaint: "#93a1a1", mark: "#f5e3a4", danger: "#dc322f", success: "#859900", code: "#eee8d5",
      syn: ["#859900", "#2aa198", "#d33682", "#268bd2", "#b58900", "#cb4b16"],
    }),
  },
  {
    id: "solarized-dark",
    name: tk("Solarized Dark"),
    scheme: "dark",
    accent: "#2aa198",
    vars: palette({
      bg: "#002b36", soft: "#00252f", sunk: "#001f27", raised: "#073642", line: "#0d3a46", lineStrong: "#1d4b56",
      text: "#cfd8d8", textSoft: "#93a1a1", textFaint: "#657b83", mark: "#4a4410", danger: "#dc322f", success: "#859900", code: "#073642",
      syn: ["#859900", "#2aa198", "#d33682", "#268bd2", "#b58900", "#cb4b16"],
    }),
  },
  {
    id: "gruvbox",
    name: tk("Gruvbox"),
    scheme: "dark",
    accent: "#fabd2f",
    vars: palette({
      bg: "#282828", soft: "#232323", sunk: "#1d2021", raised: "#32302f", line: "#3c3836", lineStrong: "#504945",
      text: "#ebdbb2", textSoft: "#d5c4a1", textFaint: "#928374", mark: "#5a4a17", danger: "#fb4934", success: "#b8bb26", code: "#1d2021",
      syn: ["#fb4934", "#b8bb26", "#d3869b", "#8ec07c", "#fabd2f", "#83a598"],
    }),
  },
  {
    id: "catppuccin-latte",
    name: tk("Catppuccin Latte"),
    scheme: "light",
    accent: "#8839ef",
    vars: palette({
      bg: "#eff1f5", soft: "#e6e9ef", sunk: "#dce0e8", raised: "#f6f7fa", line: "#ccd0da", lineStrong: "#bcc0cc",
      text: "#4c4f69", textSoft: "#5c5f77", textFaint: "#8c8fa1", mark: "#f6e3b4", danger: "#d20f39", success: "#40a02b", code: "#e6e9ef",
      syn: ["#8839ef", "#40a02b", "#fe640b", "#1e66f5", "#df8e1d", "#179299"],
    }),
  },
  {
    id: "catppuccin-mocha",
    name: tk("Catppuccin Mocha"),
    scheme: "dark",
    accent: "#cba6f7",
    vars: palette({
      bg: "#1e1e2e", soft: "#181825", sunk: "#11111b", raised: "#262637", line: "#313244", lineStrong: "#45475a",
      text: "#cdd6f4", textSoft: "#bac2de", textFaint: "#7f849c", mark: "#5b5034", danger: "#f38ba8", success: "#a6e3a1", code: "#181825",
      syn: ["#cba6f7", "#a6e3a1", "#fab387", "#89b4fa", "#f9e2af", "#94e2d5"],
    }),
  },
  {
    id: "github-light",
    name: tk("GitHub Light"),
    scheme: "light",
    accent: "#0969da",
    vars: palette({
      bg: "#ffffff", soft: "#f6f8fa", sunk: "#eaeef2", raised: "#ffffff", line: "#d8dee4", lineStrong: "#d0d7de",
      text: "#1f2328", textSoft: "#424a53", textFaint: "#6e7781", mark: "#fff8c5", danger: "#cf222e", success: "#1a7f37", code: "#f6f8fa",
      syn: ["#cf222e", "#0a3069", "#0550ae", "#8250df", "#953800", "#116329"],
    }),
  },
  {
    id: "github-dark",
    name: tk("GitHub Dark"),
    scheme: "dark",
    accent: "#58a6ff",
    vars: palette({
      bg: "#0d1117", soft: "#010409", sunk: "#010409", raised: "#161b22", line: "#21262d", lineStrong: "#30363d",
      text: "#e6edf3", textSoft: "#c9d1d9", textFaint: "#7d8590", mark: "#4f3f0f", danger: "#ff7b72", success: "#3fb950", code: "#161b22",
      syn: ["#ff7b72", "#a5d6ff", "#79c0ff", "#d2a8ff", "#ffa657", "#7ee787"],
    }),
  },
];

/** Colours of the base themes for previews (their variables live in app.css). */
const BASE_PREVIEW: Record<string, Record<string, string>> = {
  light: { bg: "#fbfbfa", "bg-soft": "#f4f3f1", line: "#e2e0db", text: "#201d29", "text-faint": "#908c99", "syn-keyword": "#a43fa9", "syn-string": "#2f7d4a" },
  dark: { bg: "#1c1a24", "bg-soft": "#17151e", line: "#2e2b38", text: "#e8e5ef", "text-faint": "#777284", "syn-keyword": "#e08ce6", "syn-string": "#8fd19e" },
  sepia: { bg: "#f5eee1", "bg-soft": "#ede4d3", line: "#ddd0b8", text: "#3a2f22", "text-faint": "#9a8a72", "syn-keyword": "#a43fa9", "syn-string": "#2f7d4a" },
};

/** Colours to draw a theme's thumbnail preview. */
export function previewColors(th: Theme): Record<string, string> {
  return { ...(BASE_PREVIEW[th.id] ?? {}), ...th.vars };
}

/** Custom theme of the vault: `custom:<file name>`. */
export const CUSTOM_PREFIX = "custom:";

export function findTheme(id: string): Theme | undefined {
  return THEMES.find((t) => t.id === id);
}

/** Variables set by the previous theme, to remove when the theme changes. */
let applied: string[] = [];

/**
 * Applies a theme: scheme, palette and an optional custom stylesheet.
 * `id` is a gallery theme, "system" or "custom:<file>".
 */
export function applyTheme(id: string, custom: { css: string; scheme: Scheme } | null, prefersDark: boolean) {
  const root = document.documentElement;
  for (const k of applied) root.style.removeProperty(`--${k}`);
  applied = [];
  let theme = id === "system" ? findTheme(prefersDark ? "dark" : "light")! : findTheme(id);
  let scheme: Scheme = theme?.scheme ?? (custom?.scheme ?? (prefersDark ? "dark" : "light"));
  if (!theme && !custom) theme = findTheme(prefersDark ? "dark" : "light")!;
  root.dataset.theme = theme?.id ?? id;
  root.dataset.scheme = scheme;
  for (const [k, v] of Object.entries(theme?.vars ?? {})) {
    root.style.setProperty(`--${k}`, v);
    applied.push(k);
  }
  let style = document.getElementById("eloria-custom-theme");
  if (custom) {
    if (!style) {
      style = document.createElement("style");
      style.id = "eloria-custom-theme";
      document.head.appendChild(style);
    }
    // The custom theme's `:root` must win over app.css's `:root[data-scheme="dark"]`:
    // the selector is strengthened, so that users can write simple CSS.
    style.textContent = custom.css.replace(/(^|[\s,}])(:root)(?![\w-])/g, "$1html$2[data-theme]");
  } else style?.remove();
  scheme = root.dataset.scheme as Scheme;
  return scheme;
}
