/**
 * Third-party licenses ("Licences" dialog). The list is built by
 * scripts/licenses.sh into src/generated/licenses.json (not committed: CI and
 * release builds generate it, development builds may not have it).
 */

export interface LicensedPackage {
  name: string;
  version: string;
  ecosystem: "rust" | "npm";
  /** SPDX expression, e.g. "MIT OR Apache-2.0". */
  license: string;
  url?: string;
  /** Indices into `Licenses.texts`; empty when the package ships no text. */
  texts: number[];
}

export interface Licenses {
  project: { license: string; text: string };
  packages: LicensedPackage[];
  texts: string[];
}

const files = import.meta.glob<Licenses>("./generated/licenses.json", { import: "default" });

/** Loads the generated list (a separate chunk); null when it was not generated. */
export async function loadLicenses(): Promise<Licenses | null> {
  const load = Object.values(files)[0];
  return load ? load() : null;
}

/** Packages whose name or license contains the query (case-insensitive). */
export function filterPackages(packages: LicensedPackage[], query: string): LicensedPackage[] {
  const q = query.trim().toLowerCase();
  if (!q) return packages;
  return packages.filter(
    (p) => p.name.toLowerCase().includes(q) || p.license.toLowerCase().includes(q),
  );
}
