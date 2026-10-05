//! Umbrella Maya Plugin
//!
//! A Rust library for Maya antivirus functionality with C FFI bindings.

use std::os::raw::c_int;

pub mod antivirus;
pub mod error;
pub mod ffi;

#[cfg(feature = "python")]
mod python;

/// cbindgen:derive-eq
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UmbrellaResult {
    pub success: bool,
    pub error_code: c_int,
}

impl UmbrellaResult {
    pub fn success() -> Self {
        Self {
            success: true,
            error_code: 0,
        }
    }

    pub fn failure(code: c_int) -> Self {
        Self {
            success: false,
            error_code: code,
        }
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

/// cbindgen:derive-eq
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CleanFFIResult {
    pub files_cleaned: c_int,
    pub files_deleted: c_int,
    pub files_failed: c_int,
    pub threats_removed: c_int,
    pub scan_time_ms: c_int,
}

/// Simple test function to verify DLL loading works
/// This can be called from Maya to test basic functionality
#[unsafe(no_mangle)]
pub extern "C" fn testFunction() -> c_int {
    42 // Return a test value
}

// The plugin entry points deliberately do not live here. The C++ plugin owns them,
// and exporting no-op placeholders from the Rust cdylib is actively harmful: on macOS
// the linker resolves the plugin's -exported_symbol list against whatever it can find,
// so a placeholder named initializePlugin here is re-exported by the bundle instead of
// the C++ implementation, and Maya ends up loading a plugin that registers no commands.
