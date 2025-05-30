//! Type definitions and conversions for Maya C++ types
//!
//! This module provides type definitions and conversion utilities for Maya's C++ types.
//! For safe wrappers, use the `safe` module instead.

use crate::error::{Result, UmbrellaError};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

// Re-export safe types for convenience
pub use crate::ffi::safe::{SafeMObject, SafeMStatus, SafeMString, SafeMFnPlugin};

/// Safe wrapper for Maya's MStatus
#[derive(Debug, Clone)]
pub struct MStatus {
    pub(crate) code: c_int,
}

impl MStatus {
    /// Create a new MStatus with success code
    pub fn success() -> Self {
        MStatus { code: 0 }
    }
    
    /// Create a new MStatus with error code
    pub fn error(code: c_int) -> Self {
        MStatus { code }
    }
    
    /// Check if the status represents success
    pub fn is_success(&self) -> bool {
        self.code == 0
    }
    
    /// Check if the status represents an error
    pub fn is_error(&self) -> bool {
        self.code != 0
    }
    
    /// Get the status code
    pub fn code(&self) -> c_int {
        self.code
    }
}

impl Default for MStatus {
    fn default() -> Self {
        MStatus::success()
    }
}

/// Safe wrapper for Maya's MString
#[derive(Debug, Clone)]
pub struct MString {
    data: String,
}

impl MString {
    /// Create a new MString from a Rust string
    pub fn new<S: Into<String>>(s: S) -> Self {
        MString { data: s.into() }
    }
    
    /// Create an empty MString
    pub fn empty() -> Self {
        MString { data: String::new() }
    }
    
    /// Get the string data as a &str
    pub fn as_str(&self) -> &str {
        &self.data
    }
    
    /// Get the string data as a String
    pub fn to_string(&self) -> String {
        self.data.clone()
    }
    
    /// Get the length of the string
    pub fn len(&self) -> usize {
        self.data.len()
    }
    
    /// Check if the string is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    
    /// Convert to a C string for FFI
    pub fn to_c_string(&self) -> Result<CString> {
        CString::new(self.data.clone())
            .map_err(|e| UmbrellaError::StringConversion(e.to_string()))
    }
    
    /// Create from a C string pointer
    pub unsafe fn from_c_str(ptr: *const c_char) -> Result<Self> {
        if ptr.is_null() {
            return Err(UmbrellaError::NullPointer("C string pointer is null".to_string()));
        }
        
        let c_str = CStr::from_ptr(ptr);
        let rust_str = c_str.to_str()
            .map_err(|e| UmbrellaError::StringConversion(e.to_string()))?;
        
        Ok(MString::new(rust_str))
    }
}

impl Default for MString {
    fn default() -> Self {
        MString::empty()
    }
}

impl From<String> for MString {
    fn from(s: String) -> Self {
        MString::new(s)
    }
}

impl From<&str> for MString {
    fn from(s: &str) -> Self {
        MString::new(s)
    }
}

/// Safe wrapper for Maya's MObject
#[derive(Debug)]
pub struct MObject {
    // This will be populated when we have actual Maya bindings
    _placeholder: (),
}

impl MObject {
    /// Create a new null MObject
    pub fn null() -> Self {
        MObject { _placeholder: () }
    }
    
    /// Check if this MObject is null
    pub fn is_null(&self) -> bool {
        // Placeholder implementation
        true
    }
}

impl Default for MObject {
    fn default() -> Self {
        MObject::null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mstatus() {
        let success = MStatus::success();
        assert!(success.is_success());
        assert!(!success.is_error());
        
        let error = MStatus::error(1);
        assert!(!error.is_success());
        assert!(error.is_error());
        assert_eq!(error.code(), 1);
    }

    #[test]
    fn test_mstring() {
        let empty = MString::empty();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
        
        let hello = MString::new("Hello, Maya!");
        assert!(!hello.is_empty());
        assert_eq!(hello.as_str(), "Hello, Maya!");
        assert_eq!(hello.len(), 12);
        
        let from_str: MString = "Test".into();
        assert_eq!(from_str.as_str(), "Test");
    }

    #[test]
    fn test_mobject() {
        let obj = MObject::null();
        assert!(obj.is_null());
    }
}
