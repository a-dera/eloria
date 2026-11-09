// Developer console log: console messages, uncaught errors,
// extension messages. Entries go through a queue: a console.log called
// while a derived value is being computed must not change state at that moment.

export type LogLevel = "log" | "info" | "warn" | "error";

export interface LogEntry {
  id: number;
  time: number;
  level: LogLevel;
  /** "app" or an extension identifier */
  source: string;
  text: string;
}

const MAX = 1000;

export const devlog = $state({ entries: [] as LogEntry[], errors: 0 });

let seq = 0;
let queue: LogEntry[] = [];
let scheduled = false;

/** Readable text of any value. */
export function fmt(v: unknown): string {
  if (typeof v === "string") return v;
  if (v instanceof Error) return v.stack && !v.stack.startsWith(v.message) ? `${v.message}\n${v.stack}` : (v.stack ?? v.message);
  if (v === undefined) return "undefined";
  if (typeof v === "function") return `ƒ ${v.name || "anonyme"}()`;
  if (typeof v === "object" && v !== null && v instanceof Node) return `<${v.nodeName.toLowerCase()}>`;
  try {
    const seen = new WeakSet();
    return JSON.stringify(
      v,
      (_k, x) => {
        if (typeof x === "object" && x !== null) {
          if (seen.has(x)) return "[circulaire]";
          seen.add(x);
        }
        return typeof x === "bigint" ? `${x}n` : x;
      },
      2,
    );
  } catch {
    return String(v);
  }
}

function flush() {
  scheduled = false;
  const batch = queue;
  queue = [];
  devlog.entries.push(...batch);
  devlog.errors += batch.filter((e) => e.level === "error").length;
  if (devlog.entries.length > MAX) devlog.entries.splice(0, devlog.entries.length - MAX);
}

export function pushLog(level: LogLevel, source: string, args: unknown[]) {
  queue.push({ id: ++seq, time: Date.now(), level, source, text: args.map(fmt).join(" ") });
  if (!scheduled) {
    scheduled = true;
    queueMicrotask(flush);
  }
}

export function clearLog() {
  devlog.entries = [];
  devlog.errors = 0;
}

/** Applies console.log markers (%s, %d, %o…); %c (style) is removed. */
export function formatArgs(args: unknown[]): unknown[] {
  if (typeof args[0] !== "string" || !/%[csdifoO%]/.test(args[0])) return args;
  const rest = args.slice(1);
  const text = args[0].replace(/%([csdifoO%])/g, (m, k: string) => {
    if (k === "%") return "%";
    if (!rest.length) return m;
    const v = rest.shift();
    return k === "c" ? "" : fmt(v);
  });
  return [text, ...rest];
}

let installed = false;

/** Captures the console and the window's errors (once). */
export function installDevlog() {
  if (installed || typeof window === "undefined") return;
  installed = true;
  for (const level of ["log", "info", "warn", "error"] as const) {
    const orig = console[level].bind(console);
    console[level] = (...args: unknown[]) => {
      orig(...args);
      try {
        pushLog(level, "app", formatArgs(args));
      } catch {
        // the log must never break the caller
      }
    };
  }
  window.addEventListener("error", (e) => pushLog("error", "app", [e.error ?? e.message]));
  window.addEventListener("unhandledrejection", (e) => pushLog("error", "app", ["Promesse rejetée :", e.reason]));
}
