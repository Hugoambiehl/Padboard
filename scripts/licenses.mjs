#!/usr/bin/env node
// Builds the third-party notices shown by the desktop app ("Licences").
//
//   node scripts/licenses.mjs <cargo-about.json> <output.json>
//
// Inputs: the Rust crates (`cargo about generate --format json`, licenses
// already checked against about.toml) and the npm packages the frontend
// ships (production entries of apps/desktop/package-lock.json, including the
// bundled fonts). Each license text is stored once. Padboard's own crates and
// package are left out; its own license (LICENSE) comes first.
//
// Run through scripts/licenses.sh, never by hand.
import { readFileSync, readdirSync, writeFileSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const [aboutPath, outPath] = process.argv.slice(2);
if (!aboutPath || !outPath) {
  console.error("usage: node scripts/licenses.mjs <cargo-about.json> <output.json>");
  process.exit(2);
}

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const desktop = join(root, "apps/desktop");

/** npm licenses accepted in the frontend bundle (GPL-3.0-compatible). */
const NPM_ALLOWED = new Set([
  "MIT",
  "ISC",
  "0BSD",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "Apache-2.0",
  "OFL-1.1",
]);

const texts = [];
const textIndex = new Map();
/** Stores a license text once (whitespace differences ignored). */
function addText(text) {
  const key = text.replace(/\s+/g, " ").trim();
  let index = textIndex.get(key);
  if (index === undefined) {
    index = texts.length;
    texts.push(text.trim());
    textIndex.set(key, index);
  }
  return index;
}

const packages = new Map();
function addPackage(ecosystem, name, version, license, textIds, url) {
  const id = `${ecosystem}:${name}@${version}`;
  let entry = packages.get(id);
  if (!entry) {
    entry = { name, version, ecosystem, license, url: url || undefined, texts: [] };
    packages.set(id, entry);
  }
  for (const t of textIds) if (!entry.texts.includes(t)) entry.texts.push(t);
}

// Rust crates: cargo-about lists each license text with the crates using it.
const about = JSON.parse(readFileSync(aboutPath, "utf8"));
for (const license of about.licenses) {
  for (const { crate } of license.used_by) {
    if (crate.name.startsWith("padboard-")) continue;
    addPackage("rust", crate.name, crate.version, crate.license, [addText(license.text)], crate.repository);
  }
}

// npm packages shipped in the frontend (production dependencies).
const lock = JSON.parse(readFileSync(join(desktop, "package-lock.json"), "utf8"));
const errors = [];
for (const [path, entry] of Object.entries(lock.packages)) {
  if (!path || entry.dev || entry.devOptional) continue;
  const dir = join(desktop, path);
  if (!existsSync(join(dir, "package.json"))) {
    if (entry.optional) continue;
    errors.push(`${path}: not installed (run npm ci in apps/desktop)`);
    continue;
  }
  const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
  const license = pkg.license ?? entry.license;
  const ids = String(license).replace(/[()]/g, "").split(/\s+OR\s+/);
  if (!ids.some((id) => NPM_ALLOWED.has(id))) {
    errors.push(`${pkg.name}: license "${license}" is not in NPM_ALLOWED`);
    continue;
  }
  const files = readdirSync(dir)
    .filter((f) => /^(licen[cs]e|copying|notice)/i.test(f) && !f.endsWith(".spdx"))
    .sort();
  const textIds = files.map((f) => addText(readFileSync(join(dir, f), "utf8")));
  const url = typeof pkg.repository === "string" ? pkg.repository : pkg.repository?.url;
  addPackage("npm", pkg.name, pkg.version, license, textIds, url?.replace(/^git\+/, "").replace(/\.git$/, ""));
}
if (errors.length) {
  console.error(errors.join("\n"));
  process.exit(1);
}

const sorted = [...packages.values()].sort(
  (a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version, undefined, { numeric: true }),
);
const project = {
  license: "GPL-3.0-or-later",
  text: readFileSync(join(root, "LICENSE"), "utf8").trim(),
};
writeFileSync(outPath, JSON.stringify({ project, packages: sorted, texts }) + "\n");
console.log(`${sorted.length} packages, ${texts.length} license texts -> ${outPath}`);
