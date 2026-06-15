import { defineConfig } from "vite";

export default defineConfig({
  // The wasm-bindgen `web` loader and the .wasm asset live under the gitignored
  // ts/view/wasm/ dir. Vite resolves and bundles them as static assets.
  build: {
    target: "es2022",
  },
});
