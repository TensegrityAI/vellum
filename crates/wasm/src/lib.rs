//! Vellum WASM bindings. The only place `unsafe` may appear in the workspace:
//! wasm-bindgen generates glue code that requires it. The pure `vellum-core`
//! crate stays `#![forbid(unsafe_code)]`; this opt-out is scoped to this crate
//! and applies to wasm-bindgen generated glue ONLY.
#![allow(unsafe_code)]
