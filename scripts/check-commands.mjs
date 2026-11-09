// Checks the full path of every command (see CONTRIBUTING.md, "Adding a feature"):
// called in api.ts -> defined in commands.rs -> registered in lib.rs -> mocked in mock.ts.
// Fails (exit code 1) on the slightest gap: a command called but missing from the binary
// would give an empty screen in the real application.
import { readFileSync } from "node:fs";

const read = (p) => readFileSync(new URL(`../${p}`, import.meta.url), "utf8");
const api = read("src/lib/api.ts");
const commands = read("src-tauri/src/commands.rs");
const lib = read("src-tauri/src/lib.rs");
const mock = read("src/lib/mock.ts");

const used = new Set([...api.matchAll(/(?:call|invoke)(?:<[^>]*>)?\(\s*"([a-z0-9_]+)"/g)].map((m) => m[1]));
const defined = new Set();
const lines = commands.split("\n");
lines.forEach((l, i) => {
  if (!l.trim().startsWith("#[tauri::command")) return;
  for (let j = i + 1; j < Math.min(i + 6, lines.length); j++) {
    const m = /pub (?:async )?fn ([a-z0-9_]+)/.exec(lines[j]);
    if (m) {
      defined.add(m[1]);
      break;
    }
  }
});
const registered = new Set([...lib.matchAll(/commands::([a-z0-9_]+)/g)].map((m) => m[1]));
const mocked = new Set([...mock.matchAll(/case "([a-z0-9_]+)"/g)].map((m) => m[1]));
// Called outside call() in mock mode (streaming channel): mocked separately.
const mockedElsewhere = new Set(["ai_fiche_generate", "ai_rewrite", "ai_ask", "ai_review_draft", "semantic_index", "build_info"]);

const problems = [];
for (const c of [...used].sort()) {
  if (!defined.has(c)) problems.push(`called but not defined in commands.rs: ${c}`);
  if (!registered.has(c)) problems.push(`called but not registered in lib.rs (generate_handler!): ${c}`);
  if (!mocked.has(c) && !mockedElsewhere.has(c)) problems.push(`called but not mocked in mock.ts: ${c}`);
}
for (const c of [...defined].sort()) if (!registered.has(c)) problems.push(`defined but not registered: ${c}`);

// Palette identifiers (src/lib/commands.ts): a duplicate breaks its display.
const palette = read("src/lib/commands.ts");
const ids = [...palette.matchAll(/\bid: "([a-z0-9-]+)"/g)].map((m) => m[1]);
for (const d of [...new Set(ids.filter((x, i) => ids.indexOf(x) !== i))]) problems.push(`duplicate command identifier in the palette: ${d}`);

if (problems.length) {
  for (const p of problems) console.log(p);
  console.log(`${problems.length} problem(s)`);
  process.exit(1);
}
console.log(`${used.size} commands called, all defined, registered and mocked`);
