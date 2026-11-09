// Interface language. The French text is the key: `t("Ouvrir une note")`.
// The English translation comes from `locales/en.json`; a missing key stays in French.
// `npm run check:i18n` lists the texts still without a translation.

import enJson from "./locales/en.json";

const en = enJson as Record<string, string>;

export type Lang = "fr" | "en";

export const LANGS: { id: Lang; label: string }[] = [
  { id: "fr", label: "Français" },
  { id: "en", label: "English" },
];

class I18n {
  lang = $state<Lang>("fr");
}

export const i18n = new I18n();

/** Texts requested in English without a translation (visible in development). */
const missing = new Set<string>();
if (import.meta.env.DEV && typeof window !== "undefined") {
  (window as unknown as { __eloriaMissing: Set<string> }).__eloriaMissing = missing;
}

/** Translates an interface text. `{name}` is replaced by `vars.name`. */
export function t(fr: string, vars?: Record<string, string | number>): string {
  let s = fr;
  if (i18n.lang === "en") {
    const tr = en[fr];
    if (tr === undefined) {
      if (import.meta.env.DEV) missing.add(fr);
    } else s = tr;
  }
  if (vars) s = s.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m));
  return s;
}

/**
 * Marks a text to translate later, at display time (constants, lists):
 * `const label = tk("Fichiers")`, then `t(label)` in the template.
 */
export const tk = (fr: string): string => fr;

/** Singular or plural form: `tn(n, "{n} note", "{n} notes")`. */
export function tn(n: number, one: string, many: string, vars: Record<string, string | number> = {}): string {
  return t(Math.abs(n) <= 1 && i18n.lang === "fr" ? one : n === 1 ? one : many, { n, ...vars });
}

/** Locale for dates and numbers. */
export function locale(): string {
  return i18n.lang === "en" ? "en-GB" : "fr-FR";
}

/**
 * Error messages from the Rust core (always in French): the prefix is translated,
 * the detail (path, technical message) stays as is.
 */
const ERROR_PREFIXES: [string, string][] = [
  ["Erreur d'entrée/sortie : ", "I/O error: "],
  ["Erreur de base de données : ", "Database error: "],
  ["Erreur JSON : ", "JSON error: "],
  ["Erreur YAML : ", "YAML error: "],
  ["Erreur de modèle : ", "Template error: "],
  ["Chemin invalide ou hors du coffre : ", "Invalid path or outside the vault: "],
  ["Introuvable : ", "Not found: "],
  ["Existe déjà : ", "Already exists: "],
  ["Aucun coffre ouvert", "No vault open"],
  ["Zotero : ", "Zotero: "],
  ["Export : ", "Export: "],
  ["IA : ", "AI: "],
  ["Réseau : ", "Network: "],
  ["Extension : ", "Extension: "],
];

export function translateError(e: unknown): string {
  const msg = String(e).replace(/^Error: /, "");
  if (i18n.lang !== "en") return msg;
  for (const [fr, enP] of ERROR_PREFIXES) if (msg.startsWith(fr)) return enP + msg.slice(fr.length);
  return msg;
}
