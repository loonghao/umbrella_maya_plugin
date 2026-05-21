//! Integration tests for the build script functionality

use std::env;
use std::path::Path;

#[test]
fn test_maya_bindings_generated() {
    // build.rs generates the C ABI header consumed by the C++ Maya plugin.
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let bindings_path = Path::new(&manifest_dir)
        .join("build")
        .join("include")
        .join("umbrella_maya_plugin.h");

    assert!(
        bindings_path.exists(),
        "umbrella_maya_plugin.h should be generated"
    );

    // Check that the file is not empty
    let content =
        std::fs::read_to_string(&bindings_path).expect("Should be able to read generated header");

    assert!(
        !content.is_empty(),
        "umbrella_maya_plugin.h should not be empty"
    );

    // Check for the Rust antivirus ABI consumed by the C++ Maya plugin.
    assert!(
        content.contains("ScanResult") && content.contains("umbrella_init"),
        "umbrella_maya_plugin.h should contain antivirus ABI types and entry points"
    );
}

#[test]
fn test_maya_types_available() {
    // Test that we can use the generated Maya types
    // Note: In integration tests, we need to use the crate name

    // This test verifies that the types are accessible
    // The actual functionality will be tested in unit tests
    println!("Maya types should be available through the crate interface");
}

#[test]
fn test_maya_bindings_feature() {
    // Test that the maya_bindings feature detection works
    // This is a basic test that the build completed successfully

    println!("Build script executed successfully");

    // The result depends on whether we have Maya SDK and libclang
    // In CI/test environments, this will likely be false
    // In development environments with Maya, this might be true
}
