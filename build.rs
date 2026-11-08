fn main() {
    // LLD avoids GNU ld's high peak memory on bounded Linux builders.
    // Select the target OS so native Windows builds retain their linker.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg=-fuse-ld=lld");
    }
    slint_build::compile("ui/workspace.slint").expect("compile dPDF interface");
}
