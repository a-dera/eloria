// Checks that every text passed to t(), tk() or tn() has its English translation.
// Usage: node scripts/check-i18n.mjs [--unused]  (exit code 1 if any is missing)
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const en = JSON.parse(readFileSync("src/lib/locales/en.json", "utf8"));
const files = [];
(function walk(d) {
  for (const f of readdirSync(d)) {
    const p = join(d, f);
    if (statSync(p).isDirectory()) walk(p);
    else if (/\.(ts|svelte)$/.test(f) && !p.includes("locales")) files.push(p);
  }
})("src");

// String in double or single quotes, with escapes.
const q = (n) => `(["'])((?:\\\\.|(?!\\${n})[^\\\\])*?)\\${n}`;
const reT = new RegExp(`\\btk?\\(\\s*${q(1)}`, "g");
// Any first argument (even with parentheses), then both forms.
const reTn = new RegExp(`\\btn\\(.*?${q(1)}\\s*,\\s*${q(3)}\\s*[,)]`, "g");
const unescape = (s) => s.replace(/\\n/g, "\n").replace(/\\(["'\\])/g, "$1");

const used = new Map();
for (const f of files) {
  const src = readFileSync(f, "utf8");
  for (const m of src.matchAll(reT)) used.set(unescape(m[2]), f);
  for (const m of src.matchAll(reTn)) {
    used.set(unescape(m[2]), f);
    used.set(unescape(m[4]), f);
  }
}
const missing = [...used].filter(([k]) => !(k in en));
const unused = Object.keys(en).filter((k) => !used.has(k));
for (const [k, f] of missing) console.log(`missing  ${f}  ${JSON.stringify(k)}`);
if (process.argv.includes("--unused")) for (const k of unused) console.log(`unused   ${JSON.stringify(k)}`);
console.log(`${used.size} texts, ${missing.length} without translation, ${unused.length} unused translations`);
process.exit(missing.length ? 1 : 0);
