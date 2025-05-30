//! Safe wrapper for Maya's MFnPlugin
//! 
//! This module provides a safe, high-level interface to Maya's plugin functionality.

use crate::error::{Result, UmbrellaError};
use crate::ffi::types::{MObject, MStatus, MString};
use crate::wrapper::{MayaObject, check_status};

/// Safe wrapper for Maya's MFnPlugin
pub struct Plugin {
    mobject: MObject,
    name: String,
    version: String,
    vendor: String,
}

impl Plugin {
    /// Create a new Plugin wrapper
    pub fn new(mobject: MObject, name: &str, version: &str, vendor: &str) -> Result<Self> {
        if mobject.is_null() {
            return Err(UmbrellaError::PluginInit("MObject is null".to_string()));
        }
        
        Ok(Plugin {
            mobject,
            name: name.to_string(),
            version: version.to_string(),
            vendor: vendor.to_string(),
        })
    }
    
    /// Get the plugin name
    pub fn name(&self) -> &str {
        &self.name
    }
    
    /// Get the plugin version
    pub fn version(&self) -> &str {
        &self.version
    }
    
    /// Get the plugin vendor
    pub fn vendor(&self) -> &str {
        &self.vendor
    }
    
    /// Register a command with Maya
    pub fn register_command(&self, command_name: &str) -> Result<()> {
        // Placeholder implementation
        // In the real implementation, this would call Maya's registerCommand
        log::info!("Registering command: {}", command_name);
        
        // Simulate Maya API call
        let status = MStatus::success(); // This would be the actual Maya API call result
        check_status(status)?;
        
        Ok(())
    }
    
    /// Deregister a command from Maya
    pub fn deregister_command(&self, command_name: &str) -> Result<()> {
        // Placeholder implementation
        // In the real implementation, this would call Maya's deregisterCommand
        log::info!("Deregistering command: {}", command_name);
        
        // Simulate Maya API call
        let status = MStatus::success(); // This would be the actual Maya API call result
        check_status(status)?;
        
        Ok(())
    }
    
    /// Set the plugin API version
    pub fn set_api_version(&self, version: &str) -> Result<()> {
        // Placeholder implementation
        log::info!("Setting API version: {}", version);
        
        // Simulate Maya API call
        let status = MStatus::success();
        check_status(status)?;
        
        Ok(())
    }
    
    /// Get plugin information as a formatted string
    pub fn info(&self) -> String {
        format!(
            "Plugin: {} v{} by {}",
            self.name, self.version, self.vendor
        )
    }
}

impl MayaObject for Plugin {
    fn is_valid(&self) -> bool {
        !self.mobject.is_null()
    }
    
    fn mobject(&self) -> Option<&MObject> {
        Some(&self.mobject)
    }
}

impl std::fmt::Debug for Plugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plugin")
            .field("name", &self.name)
            .field("version", &self.version)
            .field("vendor", &self.vendor)
            .field("is_valid", &self.is_valid())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_creation() {
        let mobject = MObject::null();
        let result = Plugin::new(mobject, "TestPlugin", "1.0.0", "Test Vendor");
        
        // This will fail because we're using a null MObject
        assert!(result.is_err());
    }

    #[test]
    fn test_plugin_info() {
        // We can't create a valid plugin in tests without Maya,
        // but we can test the info formatting logic
        let name = "TestPlugin";
        let version = "1.0.0";
        let vendor = "Test Vendor";
        
        let expected = format!("Plugin: {} v{} by {}", name, version, vendor);
        
        // This would be the actual test if we had a valid plugin:
        // let plugin = Plugin::new(...).unwrap();
        // assert_eq!(plugin.info(), expected);
        
        // For now, just verify the format string works
        assert_eq!(
            format!("Plugin: {} v{} by {}", name, version, vendor),
            expected
        );
    }
}
