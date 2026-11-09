// Runs approved extensions. The core only serves the code of an extension
// approved for this vault and unchanged since; here, it receives the `eloria` object
// (see docs/extensions.md) and everything it adds is removed when unloaded.

import { untrack } from "svelte";
import { api } from "./api";
import { commands } from "./commands";
import { pushLog } from "./devlog.svelte";
import { editors } from "./editor/registry";
import { app } from "./state.svelte";

export const API_VERSION = 1;

export type ExtEvent = "note-open" | "notes-changed";

export interface StatusItem {
  key: number;
  ext: string;
  text: string;
  title: string;
  onClick?: () => void;
}

export interface LoadedExt {
  id: string;
  name: string;
  version: string;
  error: string | null;
}

export const extState = $state({ loaded: [] as LoadedExt[], status: [] as StatusItem[], busy: false });

const cleanups = new Map<string, (() => void)[]>();
const handlers = new Map<ExtEvent, Set<{ ext: string; cb: (...a: unknown[]) => void }>>();
let statusSeq = 0;

function emit(event: ExtEvent, ...args: unknown[]) {
  for (const h of handlers.get(event) ?? []) {
    try {
      const r = h.cb(...args) as unknown;
      if (r instanceof Promise) r.catch((e) => pushLog("error", h.ext, [e]));
    } catch (e) {
      pushLog("error", h.ext, [e]);
    }
  }
}

let watching = false;
function watch() {
  if (watching) return;
  watching = true;
  $effect.root(() => {
    $effect(() => {
      const p = app.activeNotePath;
      untrack(() => emit("note-open", p ?? null));
    });
    $effect(() => {
      void app.notes;
      untrack(() => emit("notes-changed"));
    });
  });
}

/** Value without a reactive proxy, safe to hand to an extension. */
const plain = <T>(v: T): T => JSON.parse(JSON.stringify(v ?? null)) as T;

/** The `eloria` object given to an extension. */
export function makeApi(id: string, name: string, done: (() => void)[]) {
  const log =
    (level: "log" | "warn" | "error") =>
    (...args: unknown[]) =>
      pushLog(level, id, args);
  const view = () => editors.active()?.view;
  return Object.freeze({
    id,
    apiVersion: API_VERSION,
    log: log("log"),
    warn: log("warn"),
    error: log("error"),
    commands: Object.freeze({
      add(c: { id: string; name: string; keys?: string; run: () => unknown }) {
        if (!c?.id || !c.name || typeof c.run !== "function") throw new Error("commands.add : id, name et run sont requis");
        const full = `ext:${id}:${c.id}`;
        const i = commands.findIndex((x) => x.id === full);
        if (i >= 0) commands.splice(i, 1);
        commands.push({
          id: full,
          label: c.name,
          raw: true,
          keys: c.keys,
          group: name,
          run: () => {
            try {
              const r = c.run() as unknown;
              if (r instanceof Promise) r.catch((e) => pushLog("error", id, [e]));
            } catch (e) {
              pushLog("error", id, [e]);
            }
          },
        });
        done.push(() => {
          const j = commands.findIndex((x) => x.id === full);
          if (j >= 0) commands.splice(j, 1);
        });
      },
    }),
    notes: Object.freeze({
      list: () => app.notes.map((n) => ({ path: n.path, title: n.title, tags: [...n.tags], aliases: [...n.aliases], frontmatter: plain(n.frontmatter), words: n.words, mtime: n.mtime })),
      active: () => app.activeNotePath ?? null,
      async read(path: string) {
        await editors.savePath(path);
        return (await api.readNote(path)).content;
      },
      async write(path: string, content: string) {
        await editors.savePath(path);
        await api.saveNote(path, content);
        editors.pushContent(path, content);
        await app.refresh();
      },
      async create(title: string, content = "", folder = "") {
        const p = await api.createNote(folder, title, content);
        await app.refresh();
        return p;
      },
      open: (path: string) => app.openNote(path, { newTab: true }),
      search: async (query: string, limit = 50) => plain(await api.search(query, limit)),
    }),
    editor: Object.freeze({
      selection() {
        const v = view();
        if (!v) return "";
        const s = v.state.selection.main;
        return v.state.sliceDoc(s.from, s.to);
      },
      insert(text: string) {
        const ed = editors.active();
        if (!ed) throw new Error("aucune note ouverte dans l'éditeur");
        ed.insert(String(text));
      },
      replaceSelection(text: string) {
        const v = view();
        if (!v) throw new Error("aucune note ouverte dans l'éditeur");
        v.dispatch(v.state.replaceSelection(String(text)));
      },
    }),
    ui: Object.freeze({
      toast: (text: string, kind: "info" | "success" | "error" = "info") => void app.toast(String(text), kind),
      prompt: (question: string, value = "") => app.ask(String(question), String(value)),
      statusItem(opts: { text?: string; title?: string; onClick?: () => void }) {
        const key = ++statusSeq;
        extState.status.push({ key, ext: id, text: opts?.text ?? "", title: opts?.title ?? name, onClick: opts?.onClick });
        const remove = () => {
          extState.status = extState.status.filter((x) => x.key !== key);
        };
        done.push(remove);
        return Object.freeze({
          set(text: string) {
            const it = extState.status.find((x) => x.key === key);
            if (it) it.text = String(text);
          },
          remove,
        });
      },
    }),
    events: Object.freeze({
      on(event: ExtEvent, cb: (...a: unknown[]) => void) {
        if (event !== "note-open" && event !== "notes-changed") throw new Error(`événement inconnu : ${event}`);
        const h = { ext: id, cb };
        if (!handlers.has(event)) handlers.set(event, new Set());
        handlers.get(event)!.add(h);
        const off = () => handlers.get(event)?.delete(h);
        done.push(off);
        // The current state is given right away.
        if (event === "note-open") queueMicrotask(() => handlers.get(event)?.has(h) && emit1(h, app.activeNotePath ?? null));
        return off;
      },
    }),
    data: Object.freeze({
      load: () => api.extensionDataGet(id),
      save: (value: unknown) => api.extensionDataSet(id, plain(value)),
    }),
    onunload(fn: () => void) {
      if (typeof fn === "function") done.push(fn);
    },
  });
}

function emit1(h: { ext: string; cb: (...a: unknown[]) => void }, ...args: unknown[]) {
  try {
    h.cb(...args);
  } catch (e) {
    pushLog("error", h.ext, [e]);
  }
}

function runCleanups(id: string) {
  for (const f of (cleanups.get(id) ?? []).reverse()) {
    try {
      f();
    } catch (e) {
      pushLog("error", id, [e]);
    }
  }
  cleanups.delete(id);
}

export function unloadExtension(id: string) {
  if (!cleanups.has(id)) return;
  runCleanups(id);
  extState.loaded = extState.loaded.filter((x) => x.id !== id);
  pushLog("info", id, ["déchargée"]);
}

export async function loadExtension(id: string) {
  unloadExtension(id);
  watch();
  const done: (() => void)[] = [];
  cleanups.set(id, done);
  let name = id;
  let version = "";
  try {
    const c = await api.extensionLoad(id);
    name = c.name;
    version = c.version;
    if (c.css) {
      const style = document.createElement("style");
      style.dataset.extension = id;
      style.textContent = c.css;
      document.head.append(style);
      done.push(() => style.remove());
    }
    const run = new Function("eloria", `"use strict";\n${c.code}\n//# sourceURL=eloria-extension://${id}/main.js`) as (e: unknown) => unknown;
    const r = run(makeApi(id, c.name, done));
    if (r instanceof Promise) await r;
    extState.loaded = [...extState.loaded.filter((x) => x.id !== id), { id, name, version, error: null }];
    pushLog("info", id, [`chargée (${name} ${version})`]);
  } catch (e) {
    runCleanups(id);
    const msg = e instanceof Error ? e.message : String(e);
    extState.loaded = [...extState.loaded.filter((x) => x.id !== id), { id, name, version, error: msg }];
    pushLog("error", id, ["échec du chargement :", e]);
  }
}

/** Unloads everything, then loads the approved extensions of the open vault. */
export async function loadExtensions() {
  for (const id of [...cleanups.keys()]) unloadExtension(id);
  if (!app.vault) return;
  extState.busy = true;
  try {
    const list = await api.extensionsList();
    for (const e of list) if (e.enabled) await loadExtension(e.id);
  } catch (e) {
    pushLog("error", "app", ["extensions :", e]);
  } finally {
    extState.busy = false;
  }
}

/** Expression or block evaluated by the developer console, with `eloria` and `app` in scope. */
export async function evaluate(source: string): Promise<unknown> {
  const done: (() => void)[] = [];
  const eloria = makeApi("console", "Console", done);
  const body = /^\s*(const|let|var|if|for|while|await\s+\w+\s*=|return)\b|;\s*\S/.test(source) ? source : `return (${source});`;
  const fn = new Function("eloria", "app", "api", `"use strict";\nreturn (async () => {\n${body}\n})();\n//# sourceURL=eloria-console`) as (...a: unknown[]) => Promise<unknown>;
  return fn(eloria, app, api);
}
