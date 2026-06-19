import { EditContextInput } from "./edit-context-input.js";
import { HiddenTextareaInput } from "./hidden-textarea-input.js";
import type { InputSource } from "./input-source.js";

/**
 * Whether this environment supports the EditContext API (Chromium 121+). Pure and
 * injectable: pass a global-like object in tests; defaults to `globalThis`. This
 * is the single feature-detection point the factory uses to choose an adapter
 * (ADR-0003) — the view itself never branches on the browser.
 */
export function supportsEditContext(global: object = globalThis): boolean {
  return "EditContext" in global;
}

/**
 * Pick and construct the right [`InputSource`](./input-source.ts) by feature
 * detection: `EditContextInput` (attached to `surface`) on Chromium, else
 * `HiddenTextareaInput` (a textarea overlay appended to `host`). The view holds
 * the returned port and is identical across browsers.
 */
export function createInputSource(
  host: HTMLElement,
  surface: HTMLElement,
  initial: string,
): InputSource {
  if (supportsEditContext()) {
    return new EditContextInput(surface, initial);
  }
  return new HiddenTextareaInput(host, initial);
}
