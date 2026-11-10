// Lucide icons -> SVG path data, the form stored in the configuration and
// sent to the tablet (protocol `ButtonIcon`): every element (circle, rect,
// line…) becomes a `d` string, so the tablet only has to draw paths.

/** A Lucide icon: a list of SVG elements, as exported by the `lucide` package. */
export type IconNode = [tag: string, attrs: Record<string, string | number>][];

const num = (attrs: Record<string, string | number>, key: string): number => {
  const value = Number(attrs[key] ?? 0);
  return Number.isFinite(value) ? value : 0;
};

/** Short decimal form: no trailing zeros, at most 3 decimals. */
const f = (n: number): string => String(Math.round(n * 1000) / 1000);

function points(raw: string | number | undefined): number[][] {
  const values = String(raw ?? "")
    .trim()
    .split(/[\s,]+/)
    .filter(Boolean)
    .map(Number);
  const pairs: number[][] = [];
  for (let i = 0; i + 1 < values.length; i += 2) pairs.push([values[i]!, values[i + 1]!]);
  return pairs;
}

function ellipse(cx: number, cy: number, rx: number, ry: number): string {
  // Two half arcs: an SVG arc cannot draw a full ellipse by itself.
  return `M${f(cx - rx)} ${f(cy)}a${f(rx)} ${f(ry)} 0 1 0 ${f(2 * rx)} 0a${f(rx)} ${f(ry)} 0 1 0 ${f(-2 * rx)} 0z`;
}

function rect(a: Record<string, string | number>): string {
  const x = num(a, "x");
  const y = num(a, "y");
  const w = num(a, "width");
  const h = num(a, "height");
  let rx = a.rx !== undefined ? num(a, "rx") : num(a, "ry");
  let ry = a.ry !== undefined ? num(a, "ry") : rx;
  rx = Math.min(rx, w / 2);
  ry = Math.min(ry, h / 2);
  if (rx <= 0 || ry <= 0) return `M${f(x)} ${f(y)}h${f(w)}v${f(h)}h${f(-w)}z`;
  const arc = `A${f(rx)} ${f(ry)} 0 0 1`;
  return (
    `M${f(x + rx)} ${f(y)}H${f(x + w - rx)}${arc} ${f(x + w)} ${f(y + ry)}` +
    `V${f(y + h - ry)}${arc} ${f(x + w - rx)} ${f(y + h)}H${f(x + rx)}` +
    `${arc} ${f(x)} ${f(y + h - ry)}V${f(y + ry)}${arc} ${f(x + rx)} ${f(y)}z`
  );
}

/** One `d` string per element; unknown elements are skipped. */
export function iconToPaths(node: IconNode): string[] {
  const out: string[] = [];
  for (const [tag, a] of node) {
    switch (tag) {
      case "path":
        out.push(String(a.d ?? ""));
        break;
      case "circle":
        out.push(ellipse(num(a, "cx"), num(a, "cy"), num(a, "r"), num(a, "r")));
        break;
      case "ellipse":
        out.push(ellipse(num(a, "cx"), num(a, "cy"), num(a, "rx"), num(a, "ry")));
        break;
      case "rect":
        out.push(rect(a));
        break;
      case "line":
        out.push(`M${f(num(a, "x1"))} ${f(num(a, "y1"))}L${f(num(a, "x2"))} ${f(num(a, "y2"))}`);
        break;
      case "polyline":
      case "polygon": {
        const p = points(a.points);
        if (p.length < 2) break;
        const d = p.map(([x, y], i) => `${i === 0 ? "M" : "L"}${f(x!)} ${f(y!)}`).join("");
        out.push(tag === "polygon" ? `${d}z` : d);
        break;
      }
    }
  }
  return out.filter((d) => d.length > 0);
}
