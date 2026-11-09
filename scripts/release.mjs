// Prepares a release: installers built, gathered in release/<version>/ with their
// SHA-256 checksums and the release notes taken from CHANGELOG.md.
// Usage: npm run release            (builds, then gathers)
//         npm run release -- --skip-build   (gathers the installers already built)
import { execSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const pkg = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
const version = pkg.version;
const bundle = path.join(path.resolve(root, process.env.CARGO_TARGET_DIR || "target"), "release", "bundle");

if (!process.argv.includes("--skip-build")) {
  console.log(`Building Eloria ${version}…`);
  execSync("npm run bundle", { cwd: root, stdio: "inherit" });
}

const found = [];
for (const [dir, ext] of [["nsis", ".exe"], ["msi", ".msi"], ["deb", ".deb"], ["rpm", ".rpm"], ["appimage", ".AppImage"], ["dmg", ".dmg"]]) {
  const d = path.join(bundle, dir);
  if (!fs.existsSync(d)) continue;
  for (const f of fs.readdirSync(d)) if (f.endsWith(ext) && f.includes(version)) found.push(path.join(d, f));
}
if (!found.length) {
  console.error(`No ${version} installer in ${bundle}. Run "npm run bundle" first.`);
  process.exit(1);
}

const out = path.join(root, "release", version);
fs.rmSync(out, { recursive: true, force: true });
fs.mkdirSync(out, { recursive: true });
const sums = [];
for (const f of found) {
  const name = path.basename(f);
  fs.copyFileSync(f, path.join(out, name));
  const hash = createHash("sha256").update(fs.readFileSync(f)).digest("hex");
  sums.push(`${hash}  ${name}`);
  console.log(`${name}  ${(fs.statSync(f).size / 1048576).toFixed(1)} MB`);
}
fs.writeFileSync(path.join(out, "SHA256SUMS.txt"), sums.join("\n") + "\n");

// Release notes: the CHANGELOG.md section with this number.
const log = fs.readFileSync(path.join(root, "CHANGELOG.md"), "utf8");
const start = log.indexOf(`## ${version}`);
if (start < 0) {
  console.error(`CHANGELOG.md has no "## ${version}" section.`);
  process.exit(1);
}
const next = log.indexOf("\n## ", start + 4);
fs.writeFileSync(path.join(out, "NOTES.md"), `# Eloria ${version}\n\n` + log.slice(start, next < 0 ? undefined : next).split("\n").slice(1).join("\n").trim() + "\n");
fs.copyFileSync(path.join(root, "LICENSE"), path.join(out, "LICENSE.txt"));
console.log(`\nVersion ${version} ready in ${out}`);
