// Exports every Lucide icon as converted by src/icons/paths.ts, to check them
// all against the desktop's validator after upgrading `lucide`:
//   node scripts/export-icons.mts /tmp/lucide.json
//   PADBOARD_LUCIDE_JSON=/tmp/lucide.json cargo test -p padboard-desktop -- --ignored lucide
import { writeFileSync } from "node:fs";
import { icons } from "lucide";
import { iconToPaths, type IconNode } from "../src/icons/paths.ts";

const out = process.argv[2];
if (!out) throw new Error("usage: node scripts/export-icons.mts <output.json>");
const all = Object.fromEntries(
  Object.entries(icons).map(([name, node]) => [name, iconToPaths(node as IconNode)]),
);
writeFileSync(out, JSON.stringify(all));
console.log(`${Object.keys(all).length} icons written to ${out}`);
