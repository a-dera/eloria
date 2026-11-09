// Consistency check between the interface and the Rust core.
//
// In development, the interface hot-reloads while the Rust core is only
// rebuilt on success: after a failed build, the application can
// run with an old binary. The commands the interface calls (read from the
// api.ts source) are compared with those the binary exposes (written at
// build time by build.rs). A mismatch is shown in red, never ignored.

import apiSource from "./api.ts?raw";

export interface BuildInfo {
  commit: string;
  built: string;
  commands: string[];
}

export interface BuildProblem {
  missing: string[];
  info: BuildInfo | null;
}

/** Rust commands called by the interface. */
export function usedCommands(src: string = apiSource): string[] {
  const names = new Set<string>();
  for (const m of src.matchAll(/(?:call|invoke)(?:<[^>]*>)?\(\s*"([a-z0-9_]+)"/g)) names.add(m[1]);
  return [...names].sort();
}

/** Compares the interface with the binary. `null`: everything matches. */
export async function checkBuild(): Promise<{ info: BuildInfo | null; problem: BuildProblem | null }> {
  const { invoke } = await import("@tauri-apps/api/core");
  let info: BuildInfo | null = null;
  try {
    info = await invoke<BuildInfo>("build_info");
  } catch {
    return { info: null, problem: { missing: ["build_info"], info: null } };
  }
  const have = new Set(info.commands);
  const missing = usedCommands().filter((c) => !have.has(c));
  return { info, problem: missing.length ? { missing, info } : null };
}
