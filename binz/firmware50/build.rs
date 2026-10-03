//! Put `memory.x` where cortex-m-rt's `link.x` can find it.
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::copy("memory.x", out.join("memory.x")).expect("copy memory.x");
    println!("cargo:rustc-link-search={}", out.display());
    // The linker script. It used to arrive only through `binz/.cargo/config.toml`
    // (an ancestor config cargo merges in), so a checkout without that file linked
    // an empty image. The crate now carries it itself (scripts/build.ps1 relies on it).
    println!("cargo:rustc-link-arg=-Tlink.x");

    // Deliberately do NOT emit `-Tlink.x` here: cortex-m-rt 0.7's own build
    // script already emits it as a `rustc-link-arg`, and passing it twice
    // makes the linker read `memory.x` twice and fail with
    // "region 'FLASH' already defined". All this crate has to do is put
    // `memory.x` somewhere `link.x` can INCLUDE it, which is the link-search
    // line above.

    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");
}
