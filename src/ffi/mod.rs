//! FFI bindings for Maya C++ API
//!
//! This module contains the Foreign Function Interface (FFI) bindings
//! for the Maya C++ API, providing low-level access to Maya functionality.

pub mod c_api;

// Simple type definitions for Maya compatibility
pub type MObject = *mut std::os::raw::c_void;
pub type MStatus = std::os::raw::c_int;

// Re-export C API functions
pub use c_api::*;

/// Check if Maya bindings are available
pub fn maya_bindings_available() -> bool {
    cfg!(feature = "maya_bindings")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maya_bindings_availability() {
        // This test will pass regardless of whether bindings are available
        let available = maya_bindings_available();
        println!("Maya bindings available: {}", available);
    }
}
