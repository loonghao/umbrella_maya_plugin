//! Safe wrappers for Maya C++ API
//! 
//! This module provides high-level, safe wrappers around Maya's C++ API,
//! abstracting away the low-level FFI details and providing a more Rust-like interface.

pub mod plugin;
pub mod command;

// Re-export commonly used wrappers
pub use plugin::Plugin;
pub use command::Command;

use crate::error::{Result, UmbrellaError};
use crate::ffi::types::{MObject, MStatus, MString};

/// Trait for types that can be converted from Maya's native types
pub trait FromMaya<T> {
    /// Convert from a Maya native type
    fn from_maya(value: T) -> Result<Self>
    where
        Self: Sized;
}

/// Trait for types that can be converted to Maya's native types
pub trait ToMaya<T> {
    /// Convert to a Maya native type
    fn to_maya(&self) -> Result<T>;
}

/// Trait for managing Maya object lifecycle
pub trait MayaObject {
    /// Check if the object is valid
    fn is_valid(&self) -> bool;
    
    /// Get the underlying MObject if available
    fn mobject(&self) -> Option<&MObject> {
        None
    }
}

/// Convert MStatus to Result
impl From<MStatus> for Result<()> {
    fn from(status: MStatus) -> Self {
        if status.is_success() {
            Ok(())
        } else {
            Err(UmbrellaError::MayaApi(format!("Maya operation failed with code: {}", status.code())))
        }
    }
}

/// Helper function to check and convert MStatus to Result
pub fn check_status(status: MStatus) -> Result<()> {
    status.into()
}

/// Helper function to safely call Maya API functions that return MStatus
pub fn safe_maya_call<F>(f: F) -> Result<()>
where
    F: FnOnce() -> MStatus,
{
    let status = f();
    check_status(status)
}

/// Helper function to safely call Maya API functions that return a value and MStatus
pub fn safe_maya_call_with_result<T, F>(f: F) -> Result<T>
where
    F: FnOnce() -> (T, MStatus),
{
    let (result, status) = f();
    check_status(status)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_conversion() {
        let success = MStatus::success();
        let result: Result<()> = success.into();
        assert!(result.is_ok());
        
        let error = MStatus::error(1);
        let result: Result<()> = error.into();
        assert!(result.is_err());
    }

    #[test]
    fn test_check_status() {
        assert!(check_status(MStatus::success()).is_ok());
        assert!(check_status(MStatus::error(1)).is_err());
    }

    #[test]
    fn test_safe_maya_call() {
        let result = safe_maya_call(|| MStatus::success());
        assert!(result.is_ok());
        
        let result = safe_maya_call(|| MStatus::error(1));
        assert!(result.is_err());
    }
}
