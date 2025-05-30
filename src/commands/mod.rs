//! Maya command implementations
//! 
//! This module contains the implementation of various Maya commands
//! provided by the Umbrella plugin.

use crate::error::Result;
use crate::wrapper::command::{Command, CommandRegistry};

/// Initialize and register all plugin commands
pub fn register_all_commands(registry: &mut CommandRegistry) -> Result<()> {
    log::info!("Registering all Umbrella plugin commands");
    
    // Commands will be registered here as they are implemented
    // For now, this is just a placeholder
    
    log::info!("All commands registered successfully");
    Ok(())
}

/// Deregister all plugin commands
pub fn deregister_all_commands(registry: &mut CommandRegistry) -> Result<()> {
    log::info!("Deregistering all Umbrella plugin commands");
    
    // Get a list of all registered commands and deregister them
    let command_names = registry.list_commands();
    for name in command_names {
        if let Err(e) = registry.deregister(&name) {
            log::warn!("Failed to deregister command '{}': {}", name, e);
        }
    }
    
    log::info!("All commands deregistered");
    Ok(())
}

/// Get information about all available commands
pub fn get_commands_info(registry: &CommandRegistry) -> String {
    let mut info = String::from("Umbrella Maya Plugin Commands:\n");
    info.push_str("=====================================\n\n");
    
    let commands = registry.list_commands();
    if commands.is_empty() {
        info.push_str("No commands are currently registered.\n");
    } else {
        info.push_str(&registry.get_all_help());
    }
    
    info
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_all_commands() {
        let mut registry = CommandRegistry::new();
        let result = register_all_commands(&mut registry);
        assert!(result.is_ok());
    }

    #[test]
    fn test_deregister_all_commands() {
        let mut registry = CommandRegistry::new();
        let result = deregister_all_commands(&mut registry);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_commands_info() {
        let registry = CommandRegistry::new();
        let info = get_commands_info(&registry);
        assert!(info.contains("Umbrella Maya Plugin Commands"));
        assert!(info.contains("No commands are currently registered"));
    }
}
