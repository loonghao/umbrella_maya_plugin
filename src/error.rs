//! Error handling for the Umbrella Maya Plugin

use thiserror::Error;

/// Result type alias for the plugin
pub type Result<T> = std::result::Result<T, UmbrellaError>;

/// Main error type for the Umbrella Maya Plugin
#[derive(Error, Debug)]
pub enum UmbrellaError {
    /// Maya API related errors
    #[error("Maya API error: {0}")]
    MayaApi(String),

    /// FFI related errors
    #[error("FFI error: {0}")]
    Ffi(String),

    /// Null pointer error
    #[error("Null pointer error: {0}")]
    NullPointer(String),

    /// String conversion error
    #[error("String conversion error: {0}")]
    StringConversion(String),

    /// Plugin initialization error
    #[error("Plugin initialization error: {0}")]
    PluginInit(String),

    /// Command execution error
    #[error("Command execution error: {0}")]
    CommandExecution(String),

    /// Antivirus operation error
    #[error("Antivirus operation error: {0}")]
    Antivirus(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic error
    #[error("Error: {0}")]
    Generic(String),
}

impl UmbrellaError {
    /// Create a new Maya API error
    pub fn maya_api<S: Into<String>>(msg: S) -> Self {
        UmbrellaError::MayaApi(msg.into())
    }

    /// Create a new FFI error
    pub fn ffi<S: Into<String>>(msg: S) -> Self {
        UmbrellaError::Ffi(msg.into())
    }

    /// Create a new plugin initialization error
    pub fn plugin_init<S: Into<String>>(msg: S) -> Self {
        UmbrellaError::PluginInit(msg.into())
    }

    /// Create a new command execution error
    pub fn command_execution<S: Into<String>>(msg: S) -> Self {
        UmbrellaError::CommandExecution(msg.into())
    }
}
