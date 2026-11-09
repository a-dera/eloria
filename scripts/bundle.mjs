// Builds the installers: compiles the MCP server, then runs `tauri build` with eloria-mcp
// declared as a sidecar (externalBin), so it is installed next to the Eloria executable on
// every platform (Windows install folder, macOS Contents/MacOS, Linux /usr/bin).
//
// Usage: npm run bundle [-- --target <triple>] [other tauri build arguments]
// The build folder follows CARGO_TARGET_DIR (default: target/), so installers can be built
// while Eloria runs from target/release. With --target, the output is in
// <target>/<triple>/release/bundle/.
import { execSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const target = path.resolve(root, process.env.CARGO_TARGET_DIR || "target");
const args = process.argv.slice(2);
const ti = args.indexOf("--target");
const cross = ti >= 0 ? args[ti + 1] : null;
const triple = cross ?? /host: (\S+)/.exec(execSync("rustc -vV", { encoding: "utf8" }))[1];
const ext = triple.includes("windows") ? ".exe" : "";
const out = cross ? path.join(target, cross, "release") : path.join(target, "release");
const env = { ...process.env, CARGO_TARGET_DIR: target };
const run = (cmd) => execSync(cmd, { cwd: root, stdio: "inherit", env });

run(`cargo build --release -p eloria-mcp${cross ? ` --target ${cross}` : ""}`);

// Tauri expects sidecars as src-tauri/binaries/<name>-<target triple><ext>.
const binDir = path.join(root, "src-tauri", "binaries");
fs.mkdirSync(binDir, { recursive: true });
fs.copyFileSync(path.join(out, `eloria-mcp${ext}`), path.join(binDir, `eloria-mcp-${triple}${ext}`));

// Configuration added for this build only: `npm run app` does not need the sidecar.
const extra = path.join(target, "tauri.bundle.conf.json");
fs.writeFileSync(extra, JSON.stringify({ bundle: { externalBin: ["binaries/eloria-mcp"] } }, null, 2));
run(`npx tauri build --config "${extra}" ${args.join(" ")}`);
console.log(`Installers in ${path.join(out, "bundle")}`);
