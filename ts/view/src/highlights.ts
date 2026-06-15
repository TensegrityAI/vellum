/**
 * Pure, DOM-free heart of the view: turn the flat token wire format produced by
 * the WASM `Editor.tokens()` into per-kind offset groups ready for the CSS
 * Custom Highlight API.
 *
 * The wire format is a flat `Uint32Array` of `[start, end, kind, ...]` triples
 * (kinds: 0=Text, 1=Variable, 2=Statement, 3=Comment).
 */
export function groupTokensByKind(
  flat: Uint32Array,
): Record<number, [number, number][]> {
  const groups: Record<number, [number, number][]> = {};
  for (let i = 0; i + 2 < flat.length; i += 3) {
    const start = flat[i]!;
    const end = flat[i + 1]!;
    const kind = flat[i + 2]!;
    (groups[kind] ??= []).push([start, end]);
  }
  return groups;
}
