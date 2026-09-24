fn main() {
    println!("cargo:rerun-if-changed=shim");
    println!("cargo:rerun-if-changed=include");
    let mut build = cc::Build::new();
    build.include("include");
    match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos") => {
            build
                .file("shim/ghostty_surface.m")
                .flag("-fblocks")
                .flag("-fno-objc-arc");
        }
        Ok("linux") => {
            build
                .file("shim/ghostty_surface_linux.c")
                .flag("-std=gnu11");
        }
        _ => return,
    }
    build.compile("zvim_ghostty_surface");
}
