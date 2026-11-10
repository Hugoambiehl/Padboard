// The Lucide icon set, loaded on demand (its own chunk: ~2,300 icons).
import { iconToPaths, type IconNode } from "./paths";
import type { IconConfig } from "../generated/types";

let catalog: Promise<Record<string, IconNode>> | null = null;

export function loadIcons(): Promise<Record<string, IconNode>> {
  catalog ??= import("lucide").then((m) => m.icons as unknown as Record<string, IconNode>);
  return catalog;
}

/** The stored form of a Lucide icon (name + path data), or `null` if unknown. */
export async function iconConfig(name: string): Promise<IconConfig | null> {
  const node = (await loadIcons())[name];
  return node ? { name, paths: iconToPaths(node) } : null;
}
