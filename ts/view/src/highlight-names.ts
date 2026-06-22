/**
 * Per-instance CSS Custom Highlight naming (Increment-1 blocker #2). `CSS.highlights`
 * is a process-global registry, so two `mountVellum` surfaces that registered ranges
 * under the same names (`vellum-keyword`, …) would clobber each other. Each instance
 * instead gets a unique id and its own names (`vellum-${id}-keyword`, …), plus the
 * `::highlight()` style rules that paint them — `::highlight()` only accepts a literal
 * custom-ident, so the rules are generated per instance rather than living in static CSS.
 *
 * This module is pure and DOM-free; `view.ts` injects `styleText` into a `<style>` and
 * registers ranges under `nameByKind`, then removes both on dispose.
 */

/**
 * The painted HighlightKind discriminants (ADR-0009; 0=Text is unpainted) paired
 * with their CSS name suffix and color declarations. One record so the suffix and
 * its style can never drift apart. This is the single source of truth for the
 * highlight colors (they no longer live in `highlight-styles.css`).
 */
const PAINTED_KINDS: Record<number, { suffix: string; declarations: string }> = {
  1: { suffix: "variable", declarations: "color: #7c5cff;" },
  2: { suffix: "keyword", declarations: "color: #d08770;" },
  3: { suffix: "comment", declarations: "color: #8a8f98; font-style: italic;" },
};

/** The selection highlight tints the selected text; it is not a token kind. */
const SELECTION_DECLARATIONS = "background-color: #2d4f67;";

export interface InstanceHighlights {
  /** HighlightKind u32 → the instance-scoped `CSS.highlights` registry name. */
  readonly nameByKind: Record<number, string>;
  /** The instance-scoped name for the text-selection highlight. */
  readonly selectionName: string;
  /** `::highlight()` rules registering the colors for those names; inject as a `<style>`. */
  readonly styleText: string;
}

/**
 * Build the instance-scoped highlight names and their `::highlight()` style rules for
 * the editor instance identified by `id` (any value unique within the document).
 */
export function instanceHighlights(id: string): InstanceHighlights {
  const nameByKind: Record<number, string> = {};
  const rules: string[] = [];
  for (const [kind, { suffix, declarations }] of Object.entries(PAINTED_KINDS)) {
    const name = `vellum-${id}-${suffix}`;
    nameByKind[Number(kind)] = name;
    rules.push(`::highlight(${name}) { ${declarations} }`);
  }
  const selectionName = `vellum-${id}-selection`;
  rules.push(`::highlight(${selectionName}) { ${SELECTION_DECLARATIONS} }`);
  return { nameByKind, selectionName, styleText: rules.join("\n") };
}
