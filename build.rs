//! Build script for Maya Plugin

use std::env;
use std::path::PathBuf;

fn main() {
    // Generate C bindings using cbindgen (only if cbindgen is available)
    if let Err(e) = generate_c_bindings() {
        println!("cargo:warning=Failed to generate C bindings: {}", e);
        println!("cargo:warning=This is expected if cbindgen is not properly configured");
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/ffi/mod.rs");
}

fn generate_c_bindings() -> Result<(), Box<dyn std::error::Error>> {
    let crate_dir = env::var("CARGO_MANIFEST_DIR")?;
    let output_dir = PathBuf::from(&crate_dir).join("build").join("include");

    // Create output directory if it doesn't exist
    std::fs::create_dir_all(&output_dir)?;

    let config = cbindgen::Config::from_file("cbindgen.toml")?;

    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()?
        .write_to_file(output_dir.join("umbrella_maya_plugin.h"));

    println!("Generated C bindings at: {:?}", output_dir.join("umbrella_maya_plugin.h"));
    Ok(())
}

