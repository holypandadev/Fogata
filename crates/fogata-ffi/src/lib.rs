pub mod api;
mod bridge;
mod denoise;
mod disk;
// Written by flutter_rust_bridge_codegen; not formatted by hand. The bridge needs unsafe
// code; everywhere else the workspace denies it, but where a function allows it. Its unsafe
// blocks carry no SAFETY comments (the generator does not write them).
#[rustfmt::skip]
#[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
mod frb_generated;
mod idle;
mod logs;
mod sounds;
mod voice;
