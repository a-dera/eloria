// Mermaid rendering, loaded on demand (the library is large).

type MermaidApi = typeof import("mermaid").default;
let mod: MermaidApi | null = null;
let currentTheme = "";
let counter = 0;
const cache = new Map<string, string>();
let queue: Promise<unknown> = Promise.resolve();

async function load(theme: string): Promise<MermaidApi> {
  if (!mod) mod = (await import("mermaid")).default;
  if (theme !== currentTheme) {
    mod.initialize({
      startOnLoad: false,
      theme: theme === "dark" ? "dark" : theme === "sepia" ? "neutral" : "default",
      securityLevel: "strict",
      fontFamily: "var(--font-ui)",
    });
    currentTheme = theme;
  }
  return mod;
}

/** Renders a diagram as SVG. Renders are serialised (Mermaid dislikes parallelism). */
export function renderMermaid(code: string, theme: string): Promise<string> {
  const key = `${theme}\u0000${code}`;
  const hit = cache.get(key);
  if (hit) return Promise.resolve(hit);
  const job = queue.then(async () => {
    const m = await load(theme);
    const id = `eloria-mermaid-${++counter}`;
    try {
      const { svg } = await m.render(id, code);
      if (cache.size > 200) cache.clear();
      cache.set(key, svg);
      return svg;
    } finally {
      document.getElementById(id)?.remove();
      document.getElementById(`d${id}`)?.remove();
    }
  });
  queue = job.catch(() => {});
  return job;
}
