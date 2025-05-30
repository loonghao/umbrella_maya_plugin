//! Umbrella Maya Plugin
//!
//! A Rust library for Maya antivirus functionality with C FFI bindings.

use std::os::raw::c_int;

pub mod antivirus;
pub mod ffi;
pub mod error;

// Maya status codes - these match Maya's MStatus values
const MS_SUCCESS: c_int = 0;  // MS::kSuccess
#[allow(dead_code)]
const MS_FAILURE: c_int = 1;  // MS::kFailure

/// Maya MObject representation
/// For maximum compatibility, treat it as an opaque pointer
/// This avoids any potential ABI issues with struct layout
type MObject = *mut std::os::raw::c_void;

/// Maya MStatus representation
/// MStatus in Maya is essentially an integer status code
type MStatus = c_int;

/// cbindgen:derive-eq
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UmbrellaResult {
    pub success: bool,
    pub error_code: c_int,
}

impl UmbrellaResult {
    pub fn success() -> Self {
        Self { success: true, error_code: 0 }
    }

    pub fn failure(code: c_int) -> Self {
        Self { success: false, error_code: code }
    }
}

/// cbindgen:derive-eq
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ScanResult {
    pub threats_found: c_int,
    pub files_scanned: c_int,
    pub scan_time_ms: c_int,
}

/// Simple test function to verify DLL loading works
/// This can be called from Maya to test basic functionality
#[no_mangle]
pub extern "C" fn testFunction() -> c_int {
    42  // Return a test value
}

/// Maya plugin initialization function
/// This function is called when the plugin is loaded by Maya
///
/// Using extern "C" to match Maya's expected calling convention
/// The function signature must exactly match what Maya expects:
/// extern "C" MStatus initializePlugin(MObject obj)
#[no_mangle]
pub extern "C" fn initializePlugin(_obj: MObject) -> MStatus {
    // Just return success - minimal implementation
    MS_SUCCESS
}

/// Maya plugin cleanup function
/// This function is called when the plugin is unloaded by Maya
///
/// Using extern "C" to match Maya's expected calling convention
/// The function signature must exactly match what Maya expects:
/// extern "C" MStatus uninitializePlugin(MObject obj)
#[no_mangle]
pub extern "C" fn uninitializePlugin(_obj: MObject) -> MStatus {
    // Just return success - minimal implementation
    MS_SUCCESS
}
