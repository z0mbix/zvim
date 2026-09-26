fn main() {
    println!("cargo:rerun-if-changed=src/platform_macos.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("src/platform_macos.m")
            .compile("zvim_macos");
    }
}
