// @vellum/view — thin TypeScript view layer for the Vellum editor.
export { mountVellum } from "./view.js";
export { groupTokensByKind } from "./highlights.js";

// Input port (ADR-0003) + adapters.
export type {
  InputChange,
  InputListener,
  InputSource,
} from "./input/input-source.js";
export { FakeInput } from "./input/fake-input.js";
export { HiddenTextareaInput } from "./input/hidden-textarea-input.js";
export { EditContextInput } from "./input/edit-context-input.js";
export {
  createInputSource,
  supportsEditContext,
} from "./input/create-input-source.js";
