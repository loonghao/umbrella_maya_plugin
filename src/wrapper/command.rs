//! Safe wrapper for Maya commands
//! 
//! This module provides a safe, high-level interface for creating and managing Maya commands.

use crate::error::{Result, UmbrellaError};
use crate::ffi::types::{MStatus, MString};
use crate::wrapper::check_status;

/// Trait for implementing Maya commands
pub trait Command {
    /// Get the command name
    fn name(&self) -> &str;
    
    /// Execute the command with the given arguments
    fn execute(&mut self, args: &[String]) -> Result<String>;
    
    /// Check if the command can be undone
    fn is_undoable(&self) -> bool {
        false
    }
    
    /// Undo the command (if supported)
    fn undo(&mut self) -> Result<()> {
        Err(UmbrellaError::CommandExecution("Command does not support undo".to_string()))
    }
    
    /// Get command help text
    fn help(&self) -> String {
        format!("Help for command: {}", self.name())
    }
}

/// Base command implementation
pub struct BaseCommand {
    name: String,
    description: String,
}

impl BaseCommand {
    /// Create a new base command
    pub fn new(name: &str, description: &str) -> Self {
        BaseCommand {
            name: name.to_string(),
            description: description.to_string(),
        }
    }
    
    /// Get the command description
    pub fn description(&self) -> &str {
        &self.description
    }
}

impl Command for BaseCommand {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn execute(&mut self, _args: &[String]) -> Result<String> {
        Ok(format!("Base command '{}' executed", self.name))
    }
    
    fn help(&self) -> String {
        format!("{}: {}", self.name, self.description)
    }
}

/// Command registry for managing registered commands
pub struct CommandRegistry {
    commands: std::collections::HashMap<String, Box<dyn Command>>,
}

impl CommandRegistry {
    /// Create a new command registry
    pub fn new() -> Self {
        CommandRegistry {
            commands: std::collections::HashMap::new(),
        }
    }
    
    /// Register a command
    pub fn register<C: Command + 'static>(&mut self, command: C) -> Result<()> {
        let name = command.name().to_string();
        
        if self.commands.contains_key(&name) {
            return Err(UmbrellaError::CommandExecution(
                format!("Command '{}' is already registered", name)
            ));
        }
        
        self.commands.insert(name.clone(), Box::new(command));
        log::info!("Registered command: {}", name);
        
        Ok(())
    }
    
    /// Deregister a command
    pub fn deregister(&mut self, name: &str) -> Result<()> {
        if self.commands.remove(name).is_some() {
            log::info!("Deregistered command: {}", name);
            Ok(())
        } else {
            Err(UmbrellaError::CommandExecution(
                format!("Command '{}' is not registered", name)
            ))
        }
    }
    
    /// Execute a command by name
    pub fn execute(&mut self, name: &str, args: &[String]) -> Result<String> {
        match self.commands.get_mut(name) {
            Some(command) => {
                log::info!("Executing command: {} with args: {:?}", name, args);
                command.execute(args)
            }
            None => Err(UmbrellaError::CommandExecution(
                format!("Command '{}' is not registered", name)
            ))
        }
    }
    
    /// Get a list of registered command names
    pub fn list_commands(&self) -> Vec<String> {
        self.commands.keys().cloned().collect()
    }
    
    /// Get help for a specific command
    pub fn get_help(&self, name: &str) -> Result<String> {
        match self.commands.get(name) {
            Some(command) => Ok(command.help()),
            None => Err(UmbrellaError::CommandExecution(
                format!("Command '{}' is not registered", name)
            ))
        }
    }
    
    /// Get help for all commands
    pub fn get_all_help(&self) -> String {
        let mut help = String::from("Available commands:\n");
        for command in self.commands.values() {
            help.push_str(&format!("  {}\n", command.help()));
        }
        help
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCommand {
        name: String,
    }

    impl TestCommand {
        fn new(name: &str) -> Self {
            TestCommand {
                name: name.to_string(),
            }
        }
    }

    impl Command for TestCommand {
        fn name(&self) -> &str {
            &self.name
        }
        
        fn execute(&mut self, args: &[String]) -> Result<String> {
            Ok(format!("TestCommand '{}' executed with args: {:?}", self.name, args))
        }
    }

    #[test]
    fn test_base_command() {
        let mut cmd = BaseCommand::new("test", "A test command");
        assert_eq!(cmd.name(), "test");
        assert_eq!(cmd.description(), "A test command");
        
        let result = cmd.execute(&[]).unwrap();
        assert!(result.contains("test"));
    }

    #[test]
    fn test_command_registry() {
        let mut registry = CommandRegistry::new();
        
        // Register a command
        let cmd = TestCommand::new("testcmd");
        assert!(registry.register(cmd).is_ok());
        
        // Check it's in the list
        let commands = registry.list_commands();
        assert!(commands.contains(&"testcmd".to_string()));
        
        // Execute the command
        let result = registry.execute("testcmd", &["arg1".to_string()]).unwrap();
        assert!(result.contains("testcmd"));
        assert!(result.contains("arg1"));
        
        // Deregister the command
        assert!(registry.deregister("testcmd").is_ok());
        
        // Check it's no longer in the list
        let commands = registry.list_commands();
        assert!(!commands.contains(&"testcmd".to_string()));
    }

    #[test]
    fn test_duplicate_registration() {
        let mut registry = CommandRegistry::new();
        
        let cmd1 = TestCommand::new("duplicate");
        let cmd2 = TestCommand::new("duplicate");
        
        assert!(registry.register(cmd1).is_ok());
        assert!(registry.register(cmd2).is_err());
    }
}
