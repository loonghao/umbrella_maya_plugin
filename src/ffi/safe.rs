//! Safe wrappers for Maya C++ API
//! 
//! This module provides safe, memory-managed wrappers around Maya's raw FFI bindings.
//! These wrappers handle memory management, error checking, and provide a more Rust-like API.

use crate::error::{Result, UmbrellaError};
use crate::ffi::raw;
use std::ffi::{CStr, CString};

/// Safe wrapper for Maya's MObject
pub struct SafeMObject {
    inner: raw::MObject,
}

impl SafeMObject {
    /// Create a new null MObject
    pub fn null() -> Self {
        SafeMObject {
            inner: {
                #[cfg(feature = "maya_bindings")]
                {
                    unsafe { raw::MObject_create() }
                }
                #[cfg(not(feature = "maya_bindings"))]
                {
                    raw::MObject::new()
                }
            },
        }
    }
    
    /// Create from raw MObject
    pub fn from_raw(obj: raw::MObject) -> Self {
        SafeMObject { inner: obj }
    }
    
    /// Get the raw MObject
    pub fn as_raw(&self) -> &raw::MObject {
        &self.inner
    }

    /// Get a copy of the raw MObject (for placeholder types)
    pub fn as_raw_copy(&self) -> raw::MObject {
        // For placeholder types, we can create a new instance
        // In real implementation, this would need proper Maya object copying
        #[cfg(feature = "maya_bindings")]
        {
            unsafe { raw::MObject_create() }
        }
        #[cfg(not(feature = "maya_bindings"))]
        {
            raw::MObject::new()
        }
    }
    
    /// Check if this MObject is null
    pub fn is_null(&self) -> bool {
        #[cfg(feature = "maya_bindings")]
        {
            unsafe { raw::MObject_isNull(&self.inner) }
        }
        #[cfg(not(feature = "maya_bindings"))]
        {
            true // Placeholder objects are always null
        }
    }

    /// Check if this MObject is valid
    pub fn is_valid(&self) -> bool {
        #[cfg(feature = "maya_bindings")]
        {
            unsafe { raw::MObject_isValid(&self.inner) }
        }
        #[cfg(not(feature = "maya_bindings"))]
        {
            false // Placeholder objects are never valid
        }
    }

    /// Get the API type of this object
    pub fn api_type(&self) -> i32 {
        #[cfg(feature = "maya_bindings")]
        {
            unsafe { raw::MObject_apiType(&self.inner) }
        }
        #[cfg(not(feature = "maya_bindings"))]
        {
            0 // Placeholder type
        }
    }
}

impl Default for SafeMObject {
    fn default() -> Self {
        Self::null()
    }
}

impl std::fmt::Debug for SafeMObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SafeMObject")
            .field("is_null", &self.is_null())
            .field("is_valid", &self.is_valid())
            .field("api_type", &self.api_type())
            .finish()
    }
}

impl Clone for SafeMObject {
    fn clone(&self) -> Self {
        // For placeholder types, we can just create a new null object
        // In real implementation, this would need proper Maya object copying
        Self::null()
    }
}

/// Safe wrapper for Maya's MStatus
pub struct SafeMStatus {
    inner: raw::MStatus,
}

impl SafeMStatus {
    /// Create a success status
    pub fn success() -> Self {
        SafeMStatus {
            inner: {
                #[cfg(feature = "maya_bindings")]
                {
                    unsafe { raw::MStatus_success() }
                }
                #[cfg(not(feature = "maya_bindings"))]
                {
                    raw::MStatus::new()
                }
            },
        }
    }

    /// Create an error status with code
    pub fn error(code: i32) -> Self {
        SafeMStatus {
            inner: {
                #[cfg(feature = "maya_bindings")]
                {
                    unsafe { raw::MStatus_error(code) }
                }
                #[cfg(not(feature = "maya_bindings"))]
                {
                    let mut status = raw::MStatus::new();
                    status.status_code = code;
                    status
                }
            },
        }
    }
    
    /// Create from raw MStatus
    pub fn from_raw(status: raw::MStatus) -> Self {
        SafeMStatus { inner: status }
    }
    
    /// Get the raw MStatus
    pub fn as_raw(&self) -> &raw::MStatus {
        &self.inner
    }
    
    /// Check if this status indicates success
    pub fn is_success(&self) -> bool {
        #[cfg(feature = "maya_bindings")]
        {
            unsafe { raw::MStatus_isSuccess(&self.inner) }
        }
        #[cfg(not(feature = "maya_bindings"))]
        {
            self.inner.status_code == 0
        }
    }

    /// Check if this status indicates an error
    pub fn is_error(&self) -> bool {
        !self.is_success()
    }

    /// Get the status code
    pub fn status_code(&self) -> i32 {
        #[cfg(feature = "maya_bindings")]
        {
            unsafe { raw::MStatus_statusCode(&self.inner) }
        }
        #[cfg(not(feature = "maya_bindings"))]
        {
            self.inner.status_code
        }
    }
    
    /// Convert to Result
    pub fn to_result(self) -> Result<()> {
        if self.is_success() {
            Ok(())
        } else {
            Err(UmbrellaError::MayaApi(format!(
                "Maya operation failed with status code: {}",
                self.status_code()
            )))
        }
    }
}

impl Default for SafeMStatus {
    fn default() -> Self {
        Self::success()
    }
}

impl std::fmt::Debug for SafeMStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SafeMStatus")
            .field("is_success", &self.is_success())
            .field("status_code", &self.status_code())
            .finish()
    }
}

impl Clone for SafeMStatus {
    fn clone(&self) -> Self {
        if self.is_success() {
            Self::success()
        } else {
            Self::error(self.status_code())
        }
    }
}

impl From<SafeMStatus> for Result<()> {
    fn from(status: SafeMStatus) -> Self {
        status.to_result()
    }
}

/// Safe wrapper for Maya's MString
pub struct SafeMString {
    inner: raw::MString,
    owns_data: bool,
}

impl SafeMString {
    /// Create an empty MString
    pub fn new() -> Self {
        SafeMString {
            inner: {
                #[cfg(feature = "maya_bindings")]
                {
                    unsafe { raw::MString_create() }
                }
                #[cfg(not(feature = "maya_bindings"))]
                {
                    raw::MString::new()
                }
            },
            owns_data: true,
        }
    }
    
    /// Create MString from Rust string
    pub fn from_str(s: &str) -> Result<Self> {
        #[cfg(feature = "maya_bindings")]
        {
            let c_string = CString::new(s)
                .map_err(|e| UmbrellaError::StringConversion(e.to_string()))?;

            Ok(SafeMString {
                inner: unsafe { raw::MString_createFromCStr(c_string.as_ptr()) },
                owns_data: true,
            })
        }
        #[cfg(not(feature = "maya_bindings"))]
        {
            // For placeholder, just create a new empty string
            let _ = s; // Suppress unused variable warning
            Ok(SafeMString {
                inner: raw::MString::new(),
                owns_data: true,
            })
        }
    }
    
    /// Create from raw MString (does not take ownership)
    pub fn from_raw_borrowed(s: raw::MString) -> Self {
        SafeMString {
            inner: s,
            owns_data: false,
        }
    }
    
    /// Create from raw MString (takes ownership)
    pub fn from_raw_owned(s: raw::MString) -> Self {
        SafeMString {
            inner: s,
            owns_data: true,
        }
    }
    
    /// Get the raw MString
    pub fn as_raw(&self) -> &raw::MString {
        &self.inner
    }
    
    /// Convert to Rust string
    pub fn to_string(&self) -> Result<String> {
        #[cfg(feature = "maya_bindings")]
        {
            let c_str_ptr = unsafe { raw::MString_asCStr(&self.inner) };

            if c_str_ptr.is_null() {
                return Ok(String::new());
            }

            let c_str = unsafe { CStr::from_ptr(c_str_ptr) };
            c_str.to_str()
                .map(|s| s.to_string())
                .map_err(|e| UmbrellaError::StringConversion(e.to_string()))
        }
        #[cfg(not(feature = "maya_bindings"))]
        {
            // For placeholder, return empty string
            Ok(String::new())
        }
    }

    /// Get the length of the string
    pub fn len(&self) -> usize {
        #[cfg(feature = "maya_bindings")]
        {
            unsafe { raw::MString_length(&self.inner) as usize }
        }
        #[cfg(not(feature = "maya_bindings"))]
        {
            0 // Placeholder strings are always empty
        }
    }
    
    /// Check if the string is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for SafeMString {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SafeMString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let content = self.to_string().unwrap_or_else(|_| "<invalid>".to_string());
        f.debug_struct("SafeMString")
            .field("content", &content)
            .field("len", &self.len())
            .field("owns_data", &self.owns_data)
            .finish()
    }
}

impl Clone for SafeMString {
    fn clone(&self) -> Self {
        // For placeholder types, create a new string with the same content
        match self.to_string() {
            Ok(s) => Self::from_str(&s).unwrap_or_else(|_| Self::new()),
            Err(_) => Self::new(),
        }
    }
}

impl Drop for SafeMString {
    fn drop(&mut self) {
        if self.owns_data {
            #[cfg(feature = "maya_bindings")]
            {
                unsafe {
                    raw::MString_destroy(&mut self.inner);
                }
            }
            // For placeholder types, no cleanup needed
        }
    }
}

impl TryFrom<&str> for SafeMString {
    type Error = UmbrellaError;
    
    fn try_from(s: &str) -> Result<Self> {
        Self::from_str(s)
    }
}

impl TryFrom<String> for SafeMString {
    type Error = UmbrellaError;
    
    fn try_from(s: String) -> Result<Self> {
        Self::from_str(&s)
    }
}

/// Safe wrapper for Maya's MFnPlugin
pub struct SafeMFnPlugin {
    inner: raw::MFnPlugin,
    mobject: SafeMObject,
}

impl SafeMFnPlugin {
    /// Create MFnPlugin from MObject
    pub fn new(obj: SafeMObject) -> Self {
        let inner = {
            #[cfg(feature = "maya_bindings")]
            {
                unsafe { raw::MFnPlugin_create(obj.as_raw_copy()) }
            }
            #[cfg(not(feature = "maya_bindings"))]
            {
                raw::MFnPlugin::new()
            }
        };
        SafeMFnPlugin {
            inner,
            mobject: obj,
        }
    }
    
    /// Get the raw MFnPlugin
    pub fn as_raw(&self) -> &raw::MFnPlugin {
        &self.inner
    }
    
    /// Get the associated MObject
    pub fn mobject(&self) -> &SafeMObject {
        &self.mobject
    }
    
    /// Register a command with the plugin
    pub fn register_command(&mut self, command_name: &str, creator_fn: *const std::ffi::c_void) -> Result<()> {
        #[cfg(feature = "maya_bindings")]
        {
            let c_name = CString::new(command_name)
                .map_err(|e| UmbrellaError::StringConversion(e.to_string()))?;

            let status = unsafe {
                raw::MFnPlugin_registerCommand(
                    &mut self.inner,
                    c_name.as_ptr(),
                    creator_fn,
                )
            };

            SafeMStatus::from_raw(status).to_result()
        }
        #[cfg(not(feature = "maya_bindings"))]
        {
            // For placeholder, just return success
            let _ = creator_fn; // Suppress unused variable warning
            println!("Placeholder: Registering command: {}", command_name);
            Ok(())
        }
    }
    
    /// Deregister a command from the plugin
    pub fn deregister_command(&mut self, command_name: &str) -> Result<()> {
        #[cfg(feature = "maya_bindings")]
        {
            let c_name = CString::new(command_name)
                .map_err(|e| UmbrellaError::StringConversion(e.to_string()))?;

            let status = unsafe {
                raw::MFnPlugin_deregisterCommand(&mut self.inner, c_name.as_ptr())
            };

            SafeMStatus::from_raw(status).to_result()
        }
        #[cfg(not(feature = "maya_bindings"))]
        {
            // For placeholder, just return success
            println!("Placeholder: Deregistering command: {}", command_name);
            Ok(())
        }
    }

    /// Set the API version
    pub fn set_api_version(&mut self, version: &str) -> Result<()> {
        #[cfg(feature = "maya_bindings")]
        {
            let c_version = CString::new(version)
                .map_err(|e| UmbrellaError::StringConversion(e.to_string()))?;

            let status = unsafe {
                raw::MFnPlugin_setApiVersion(&mut self.inner, c_version.as_ptr())
            };

            SafeMStatus::from_raw(status).to_result()
        }
        #[cfg(not(feature = "maya_bindings"))]
        {
            // For placeholder, just return success
            println!("Placeholder: Setting API version: {}", version);
            Ok(())
        }
    }
}

impl std::fmt::Debug for SafeMFnPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SafeMFnPlugin")
            .field("mobject", &self.mobject)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(feature = "maya_bindings"))]
    fn test_safe_mobject_placeholder() {
        let obj = SafeMObject::null();
        assert!(obj.is_null()); // Placeholder objects are always null
        assert!(!obj.is_valid()); // Placeholder objects are never valid
        assert_eq!(obj.api_type(), 0); // Placeholder type
    }

    #[test]
    #[cfg(not(feature = "maya_bindings"))]
    fn test_safe_mstatus_placeholder() {
        let success = SafeMStatus::success();
        assert!(success.is_success());
        assert!(!success.is_error());
        assert_eq!(success.status_code(), 0);

        let error = SafeMStatus::error(1);
        assert!(!error.is_success());
        assert!(error.is_error());
        assert_eq!(error.status_code(), 1);
    }

    #[test]
    #[cfg(not(feature = "maya_bindings"))]
    fn test_safe_mstring_placeholder() {
        let empty = SafeMString::new();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);

        let hello = SafeMString::from_str("Hello, Maya!").unwrap();
        assert!(hello.is_empty()); // Placeholder strings are always empty
        assert_eq!(hello.len(), 0);
        assert_eq!(hello.to_string().unwrap(), "");
    }

    #[test]
    #[cfg(not(feature = "maya_bindings"))]
    fn test_status_to_result_placeholder() {
        let success: Result<()> = SafeMStatus::success().into();
        assert!(success.is_ok());

        let error: Result<()> = SafeMStatus::error(1).into();
        assert!(error.is_err());
    }

    #[test]
    #[cfg(feature = "maya_bindings")]
    fn test_with_real_maya_api() {
        // These tests would run when maya_bindings feature is enabled
        // and would test against real Maya API
        let obj = SafeMObject::null();
        // Real Maya API tests would go here
        assert!(obj.is_null());
    }
}
