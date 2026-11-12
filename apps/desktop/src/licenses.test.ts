import { describe, expect, it } from "vitest";
import { filterPackages, type LicensedPackage } from "./licenses";

const pkg = (name: string, license: string): LicensedPackage => ({
  name,
  version: "1.0.0",
  ecosystem: "rust",
  license,
  texts: [],
});

describe("filterPackages", () => {
  const packages = [pkg("tokio", "MIT"), pkg("cssparser", "MPL-2.0"), pkg("lucide", "ISC")];

  it("keeps everything for an empty query", () => {
    expect(filterPackages(packages, "  ")).toEqual(packages);
  });

  it("matches the name or the license, ignoring case", () => {
    expect(filterPackages(packages, "TOK").map((p) => p.name)).toEqual(["tokio"]);
    expect(filterPackages(packages, "mpl").map((p) => p.name)).toEqual(["cssparser"]);
  });
});
