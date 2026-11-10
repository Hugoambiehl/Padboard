import type { Category } from "../generated/types";

/** Icon color and tinted background of each category (Main.dc.html: 16 %, red 18 %, grey 14 %). */
export const categoryColors: Record<Category, { fg: string; bg: string }> = {
  violet: { fg: "#8B7CFF", bg: "rgba(139,124,255,.16)" },
  blue: { fg: "#5AA9FF", bg: "rgba(90,169,255,.16)" },
  teal: { fg: "#3DD6B5", bg: "rgba(61,214,181,.16)" },
  orange: { fg: "#FF9F5A", bg: "rgba(255,159,90,.16)" },
  pink: { fg: "#FF6FAE", bg: "rgba(255,111,174,.16)" },
  red: { fg: "#FF5C5C", bg: "rgba(255,92,92,.18)" },
  grey: { fg: "#A3ABBD", bg: "rgba(163,171,189,.14)" },
};

export const categories = Object.keys(categoryColors) as Category[];
