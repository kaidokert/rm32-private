use std::{env, fs, path::Path};

fn main() {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH");

    if target_arch == "arm" {
        let out_dir = env::var("OUT_DIR").unwrap();
        let out = Path::new(&out_dir);
        fs::copy("memory.x", out.join("memory.x")).expect("copy memory.x to OUT_DIR");
        println!("cargo:rustc-link-search={}", out.display());
        println!("cargo:rustc-link-arg=-Tlink.x");
        println!("cargo:rerun-if-changed=memory.x");
    }

    println!("cargo:rerun-if-changed=build.rs");
}
