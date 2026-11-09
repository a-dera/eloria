// Customization: palette commands, saved views, typed properties.
// Definitions live in the settings (core); this module turns them into commands.

import { api } from "./api";
import { commands, type Command } from "./commands";
import { editors } from "./editor/registry";
import { t, tk } from "./i18n.svelte";
import { app } from "./state.svelte";
import type { CustomCommand, PropDef, Settings } from "./types";
import { stem } from "./util";

const DYNAMIC = /^(custom|vue):/;

/** Stable identifier of a custom command (used by shortcuts). */
export const customId = (c: CustomCommand) => "custom:" + c.name.trim().toLowerCase().replace(/\s+/g, "-");

/** Inserted text: {{date}}, {{heure}} and \n are replaced. */
export function expandSnippet(text: string, now = new Date()): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  const date = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
  const time = `${pad(now.getHours())}:${pad(now.getMinutes())}`;
  return text.replace(/\{\{date\}\}/g, date).replace(/\{\{(heure|time)\}\}/g, time).replace(/\\n/g, "\n");
}

export async function runCustom(c: CustomCommand) {
  const arg = c.arg.trim();
  if (c.kind === "modele") {
    const list = await api.templates();
    const tp = list.find((x) => x === arg || stem(x).toLowerCase() === arg.toLowerCase());
    if (!tp) return app.toast(t("Modèle introuvable : {name}", { name: arg }), "error");
    return app.newNoteFromTemplate(tp, c.folder.trim() || undefined);
  }
  if (c.kind === "texte") {
    const ed = editors.active();
    if (!ed) return app.toast(t("Ouvre une note pour insérer ce texte."), "error");
    ed.insert(expandSnippet(c.arg));
    return ed.focus();
  }
  if (c.kind === "vue") return app.openView("table", { key: arg, title: arg });
  if (c.kind === "note") {
    const q = arg.replace(/\.md$/i, "").toLowerCase();
    const n = app.notes.find((x) => x.path.replace(/\.md$/i, "").toLowerCase() === q || x.stem.toLowerCase() === q || x.title.toLowerCase() === q);
    if (!n) return app.toast(t("Note introuvable : {name}", { name: arg }), "error");
    return app.openNote(n.path, { newTab: true });
  }
  if (c.kind === "recherche") {
    app.leftOpen = true;
    app.leftPanel = "search";
    app.searchQuery = arg;
  }
}

/** Replaces the dynamic commands (custom, views) with those of the settings. */
export function syncCustomCommands(s: Settings | null) {
  for (let i = commands.length - 1; i >= 0; i--) if (DYNAMIC.test(commands[i].id)) commands.splice(i, 1);
  if (!s) return;
  const add: Command[] = [];
  for (const c of s.custom_commands ?? []) {
    if (!c.name.trim()) continue;
    add.push({ id: customId(c), label: c.name.trim(), raw: true, group: tk("Personnalisées"), needsNote: c.kind === "texte", run: () => void runCustom(c) });
  }
  for (const v of s.saved_views ?? []) {
    if (!v.name.trim()) continue;
    add.push({ id: "vue:" + v.name, label: t("Vue : {name}", { name: v.name }), raw: true, group: tk("Base de notes"), run: () => app.openView("table", { key: v.name, title: v.name }) });
  }
  commands.push(...add);
}

export const propDef = (name: string): PropDef | undefined => app.settings?.property_defs?.find((d) => d.name === name);

/** Initial value of a newly added typed property. */
export function initialValue(d: PropDef | undefined): unknown {
  if (!d) return "";
  if (d.kind === "case") return false;
  if (d.kind === "liste") return [];
  if (d.kind === "choix") return d.options[0] ?? "";
  if (d.kind === "date") return expandSnippet("{{date}}");
  return "";
}

/** Typed-in value converted according to the type. */
export function typedValue(d: PropDef, raw: string | boolean): unknown {
  if (d.kind === "case") return Boolean(raw);
  const s = String(raw).trim();
  if (d.kind === "nombre") return s === "" ? "" : Number(s.replace(",", "."));
  if (d.kind === "liste") return s.split(",").map((x) => x.trim()).filter(Boolean);
  if (d.kind === "lien") return s && !/^\[\[.*\]\]$/.test(s) ? `[[${s}]]` : s;
  return s;
}

/** Target of a [[X]] or [[X|alias]] link. */
export const linkTarget = (v: unknown) => /^\[\[([^\]|#]+)/.exec(String(v ?? ""))?.[1]?.trim() ?? "";
