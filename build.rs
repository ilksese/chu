use std::env;

fn main() {
    println!("cargo:rerun-if-changed=assets/app-icon.png");
    println!("cargo:rerun-if-changed=assets/windows/app.ico");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/windows/app.ico")
            .compile()
            .expect("compile Windows application icon");
    }
    let config = slint_build::CompilerConfiguration::new()
        .with_style("fluent".into())
        .embed_resources(slint_build::EmbedResourcesKind::EmbedFiles)
        .with_debug_info(cfg!(debug_assertions));
    slint_build::compile_with_config("ui/app-window.slint", config)
        .expect("Slint UI compilation failed");
}
