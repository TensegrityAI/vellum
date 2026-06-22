import "@vellum/view/highlight-styles.css";
import { mountVellum } from "@vellum/view";
import init, { Editor } from "@vellum/view/wasm/vellum.js";

async function main(): Promise<void> {
  // Instantiate the Rust core compiled to WASM. Vite resolves the .wasm asset
  // referenced by the wasm-bindgen `web` loader via import.meta.url.
  await init();

  const host = document.getElementById("app");
  if (host === null) throw new Error("missing #app host element");

  // Multi-line, multibyte seed to exercise virtualization + highlighting + the
  // UTF-16↔byte layout: enough lines to scroll, with emoji/CJK in the mix.
  const seed = [
    "Hello {{ name }} 😀,",
    "{# a note that spans",
    "   two lines #}",
    "{% if cond %}",
    "  日本語 {{ value }}",
    "{% endif %}",
    "",
    "line 8",
    "line 9",
    "line 10 — {{ tail }}",
    "line 11",
    "line 12",
    "line 13",
    "line 14",
    "line 15",
  ].join("\n");
  const editor = new Editor(seed);

  mountVellum(host, editor);

  // Increment 0 acceptance aid: every edit round-trips through the Rust core.
  host.addEventListener("input", () => {
    // eslint-disable-next-line no-console
    console.log("[vellum] core text:", editor.text());
  });
}

void main();
