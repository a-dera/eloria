// Registry of open editors (one per note tab).

import type { EditorView } from "@codemirror/view";

export interface EditorHandle {
  tabId: string;
  path: string;
  view: EditorView;
  dirty: boolean;
  saveNow(): Promise<void>;
  /** Content changed outside the editor (another app, Zotero sync, rename). */
  externalUpdate(content: string, mtime: number, force?: boolean): void;
  setPath(path: string): void;
  markDeleted(): void;
  reconfigure(): void;
  reveal(line?: number, heading?: string): void;
  insert(text: string): void;
  focus(): void;
}

class Registry {
  private map = new Map<string, EditorHandle>();
  activeId: string | null = null;

  register(h: EditorHandle) {
    this.map.set(h.tabId, h);
  }

  get(tabId: string) {
    return this.map.get(tabId);
  }

  byPath(path: string) {
    for (const h of this.map.values()) if (h.path === path) return h;
    return undefined;
  }

  /** Every editor of a file (a note can be open in both panes). */
  allByPath(path: string) {
    return [...this.map.values()].filter((h) => h.path === path);
  }

  /** Saves the pending keystrokes of every copy of a note. */
  async savePath(path: string) {
    for (const h of this.allByPath(path)) await h.saveNow();
  }

  /** Replaces the text of every copy of a note (content rewritten by the core). */
  pushContent(path: string, content: string) {
    for (const h of this.allByPath(path)) h.externalUpdate(content, Date.now(), true);
  }

  all() {
    return [...this.map.values()];
  }

  active(): EditorHandle | undefined {
    return this.activeId ? this.map.get(this.activeId) : undefined;
  }

  /** CodeMirror view of the active editor (for assistants that rewrite a passage). */
  activeView() {
    return this.active()?.view;
  }

  /** Last active editor, even if the focus is elsewhere (Zotero panel). */
  dispose(tabId: string) {
    const h = this.map.get(tabId);
    if (!h) return;
    this.map.delete(tabId);
    if (this.activeId === tabId) this.activeId = null;
    try {
      h.view.destroy();
    } catch {}
  }
}

export const editors = new Registry();
