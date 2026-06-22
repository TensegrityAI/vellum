/**
 * The measurement port (ADR-0004): the ONLY thing in the view that touches the
 * browser for sizing. It measures the editor's monospace font **once** and caches
 * the result, so all layout is pure arithmetic in the core (line windowing, caret
 * position) with zero forced reflow. The impure measurement (Canvas `measureText`
 * + one computed-style read) is isolated in `canvasMeasure`; the caching and the
 * line-height resolution are pure and unit-tested.
 */

/** Pixel metrics of the editor's monospace font. */
export interface FontMetrics {
  /** Width of one monospace cell. */
  readonly advance: number;
  /** Line box height — the vertical stride between lines. */
  readonly lineHeight: number;
}

/** A one-shot, impure measurement of the surface font; the boundary to the DOM. */
export type MeasureFn = () => FontMetrics;

/** The view's read-side of the port: the font metrics, measured once. */
export interface MeasurePort {
  metrics(): FontMetrics;
}

/**
 * Caches a single measurement so the impure `MeasureFn` runs at most once
 * (ADR-0004's "touch the slow thing once, then arithmetic"). Pure given its
 * `MeasureFn` — inject a fake in tests.
 */
export class CachingMeasurePort implements MeasurePort {
  readonly #measure: MeasureFn;
  #cached: FontMetrics | null = null;

  constructor(measure: MeasureFn) {
    this.#measure = measure;
  }

  metrics(): FontMetrics {
    return (this.#cached ??= this.#measure());
  }
}

/**
 * Resolve a computed `line-height` to pixels. A px computed value is used as-is;
 * `"normal"` (or any non-px value) is approximated as `font-size × 1.2`, CSS's
 * rough `normal` ratio. Pure. Font size defaults to 16px when unparseable.
 */
export function resolveLineHeight(computed: {
  lineHeight: string;
  fontSize: string;
}): number {
  if (computed.lineHeight.endsWith("px")) {
    const px = parseFloat(computed.lineHeight);
    if (!Number.isNaN(px)) return px;
  }
  const fontSize = parseFloat(computed.fontSize);
  return (Number.isNaN(fontSize) ? 16 : fontSize) * 1.2;
}

/**
 * Build the impure `MeasureFn` that measures `surface`'s font: one monospace cell
 * advance via an offscreen Canvas `measureText` (no reflow) and the line height
 * from the computed style. Browser-bound — verified in the demo, not unit tests.
 */
export function canvasMeasure(surface: HTMLElement): MeasureFn {
  return () => {
    const cs = getComputedStyle(surface);
    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d");
    if (ctx === null) throw new Error("2D canvas context unavailable for measuring");
    ctx.font = `${cs.fontSize} ${cs.fontFamily}`;
    return {
      advance: ctx.measureText("0").width,
      lineHeight: resolveLineHeight(cs),
    };
  };
}
