import "@vellum/view/highlight-styles.css";
import { mountVellum } from "@vellum/view";
import init, { Editor } from "@vellum/view/wasm/vellum.js";

async function main(): Promise<void> {
  // Instantiate the Rust core compiled to WASM. Vite resolves the .wasm asset
  // referenced by the wasm-bindgen `web` loader via import.meta.url.
  await init();

  const host = document.getElementById("app");
  if (host === null) throw new Error("missing #app host element");

  const editor = new Editor(
    "Hello {{ name }}, {# note #} {% if cond %}greeting{% endif %}",
  );

  mountVellum(host, editor);

  // Increment 0 acceptance aid: every edit round-trips through the Rust core.
  host.addEventListener("input", () => {
    // eslint-disable-next-line no-console
    console.log("[vellum] core text:", editor.text());
  });
}

void main();
