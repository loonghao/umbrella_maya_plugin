//! Threat cleaning functionality
//! 
//! This module provides threat cleaning capabilities for removing
//! malicious code from Maya files and scripts.

use crate::error::{Result, UmbrellaError};
use std::fs;
use std::path::{Path, PathBuf};

/// Options for configuring threat cleaning
#[derive(Debug, Clone)]
pub struct CleanOptions {
    /// Whether to create backups before cleaning
    pub create_backup: bool,
    /// Directory to store backups
    pub backup_directory: Option<String>,
    /// Whether to remove the original file after cleaning
    pub remove_original: bool,
    /// Whether to clean files in-place or create new cleaned files
    pub in_place: bool,
}

impl Default for CleanOptions {
    fn default() -> Self {
        CleanOptions {
            create_backup: true,
            backup_directory: None, // Use default backup location
            remove_original: false,
            in_place: true,
        }
    }
}

/// Status of a cleaning operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanStatus {
    /// File was successfully cleaned
    Success,
    /// File was already clean (no action needed)
    AlreadyClean,
    /// Cleaning failed
    Failed,
    /// File was quarantined instead of cleaned
    Quarantined,
    /// Backup was created but cleaning failed
    BackupCreated,
}

impl std::fmt::Display for CleanStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CleanStatus::Success => write!(f, "Success"),
            CleanStatus::AlreadyClean => write!(f, "Already Clean"),
            CleanStatus::Failed => write!(f, "Failed"),
            CleanStatus::Quarantined => write!(f, "Quarantined"),
            CleanStatus::BackupCreated => write!(f, "Backup Created"),
        }
    }
}

/// Result of a cleaning operation
#[derive(Debug, Clone)]
pub struct CleanResult {
    /// Path to the file that was cleaned
    pub file_path: String,
    /// Status of the cleaning operation
    pub status: CleanStatus,
    /// Descriptive message about the operation
    pub message: String,
    /// Path to the backup file (if created)
    pub backup_path: Option<String>,
}

impl CleanResult {
    /// Create a successful clean result
    pub fn success(file_path: &str, message: &str, backup_path: Option<String>) -> Self {
        CleanResult {
            file_path: file_path.to_string(),
            status: CleanStatus::Success,
            message: message.to_string(),
            backup_path,
        }
    }
    
    /// Create a failed clean result
    pub fn failed(file_path: &str, message: &str) -> Self {
        CleanResult {
            file_path: file_path.to_string(),
            status: CleanStatus::Failed,
            message: message.to_string(),
            backup_path: None,
        }
    }
    
    /// Create an already clean result
    pub fn already_clean(file_path: &str) -> Self {
        CleanResult {
            file_path: file_path.to_string(),
            status: CleanStatus::AlreadyClean,
            message: "File is already clean".to_string(),
            backup_path: None,
        }
    }
}

/// Trait for implementing threat cleaners
pub trait Cleaner {
    /// Clean threats from the specified file
    fn clean(&self, file_path: &str, options: &CleanOptions) -> Result<CleanResult>;
    
    /// Get the cleaner name
    fn name(&self) -> &str;
    
    /// Check if the cleaner can handle the specified file type
    fn can_clean(&self, file_path: &str) -> bool {
        let path = Path::new(file_path);
        if let Some(extension) = path.extension() {
            let ext = extension.to_string_lossy().to_lowercase();
            matches!(ext.as_str(), "py" | "mel" | "ma" | "mb")
        } else {
            false
        }
    }
}

/// Backup-based cleaner that creates backups before cleaning
pub struct BackupCleaner {
    name: String,
}

impl BackupCleaner {
    /// Create a new backup cleaner
    pub fn new() -> Self {
        BackupCleaner {
            name: "BackupCleaner".to_string(),
        }
    }
    
    /// Create a backup of the specified file
    fn create_backup(&self, file_path: &str, options: &CleanOptions) -> Result<String> {
        let source_path = Path::new(file_path);
        
        if !source_path.exists() {
            return Err(UmbrellaError::Antivirus(format!("Source file does not exist: {}", file_path)));
        }
        
        // Determine backup directory
        let backup_dir = if let Some(ref dir) = options.backup_directory {
            PathBuf::from(dir)
        } else {
            // Use a default backup directory next to the original file
            let mut backup_dir = source_path.parent().unwrap_or(Path::new(".")).to_path_buf();
            backup_dir.push("_virus_backup");
            backup_dir
        };
        
        // Create backup directory if it doesn't exist
        if !backup_dir.exists() {
            fs::create_dir_all(&backup_dir)
                .map_err(|e| UmbrellaError::Antivirus(format!("Failed to create backup directory: {}", e)))?;
        }
        
        // Generate backup filename with timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let file_name = source_path.file_name()
            .ok_or_else(|| UmbrellaError::Antivirus("Invalid file name".to_string()))?;
        
        let backup_filename = format!("{}_{}", timestamp, file_name.to_string_lossy());
        let backup_path = backup_dir.join(backup_filename);
        
        // Copy the file to backup location
        fs::copy(source_path, &backup_path)
            .map_err(|e| UmbrellaError::Antivirus(format!("Failed to create backup: {}", e)))?;
        
        Ok(backup_path.to_string_lossy().to_string())
    }
    
    /// Clean malicious content from a file
    fn clean_file_content(&self, content: &str) -> (String, bool) {
        let mut cleaned_content = String::new();
        let mut was_modified = false;
        
        for line in content.lines() {
            let line_lower = line.to_lowercase();
            
            // Simple cleaning rules (in a real implementation, this would be more sophisticated)
            if line_lower.contains("os.system") ||
               line_lower.contains("subprocess.call") ||
               line_lower.contains("eval(") ||
               line_lower.contains("exec(") {
                // Comment out suspicious lines
                cleaned_content.push_str(&format!("# REMOVED BY UMBRELLA: {}\n", line));
                was_modified = true;
            } else {
                cleaned_content.push_str(line);
                cleaned_content.push('\n');
            }
        }
        
        (cleaned_content, was_modified)
    }
}

impl Default for BackupCleaner {
    fn default() -> Self {
        Self::new()
    }
}

impl Cleaner for BackupCleaner {
    fn clean(&self, file_path: &str, options: &CleanOptions) -> Result<CleanResult> {
        if !self.can_clean(file_path) {
            return Ok(CleanResult::failed(file_path, "File type not supported for cleaning"));
        }
        
        let path = Path::new(file_path);
        if !path.exists() {
            return Ok(CleanResult::failed(file_path, "File does not exist"));
        }
        
        // Read the file content
        let content = fs::read_to_string(path)
            .map_err(|e| UmbrellaError::Antivirus(format!("Failed to read file: {}", e)))?;
        
        // Clean the content
        let (cleaned_content, was_modified) = self.clean_file_content(&content);
        
        if !was_modified {
            return Ok(CleanResult::already_clean(file_path));
        }
        
        // Create backup if requested
        let backup_path = if options.create_backup {
            Some(self.create_backup(file_path, options)?)
        } else {
            None
        };
        
        // Write cleaned content
        if options.in_place {
            fs::write(path, cleaned_content)
                .map_err(|e| UmbrellaError::Antivirus(format!("Failed to write cleaned file: {}", e)))?;
        } else {
            // Create a new file with .cleaned extension
            let mut cleaned_path = path.to_path_buf();
            cleaned_path.set_extension("cleaned");
            fs::write(&cleaned_path, cleaned_content)
                .map_err(|e| UmbrellaError::Antivirus(format!("Failed to write cleaned file: {}", e)))?;
        }
        
        Ok(CleanResult::success(
            file_path,
            "File successfully cleaned",
            backup_path,
        ))
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_options_default() {
        let options = CleanOptions::default();
        assert!(options.create_backup);
        assert!(options.backup_directory.is_none());
        assert!(!options.remove_original);
        assert!(options.in_place);
    }

    #[test]
    fn test_clean_status_display() {
        assert_eq!(CleanStatus::Success.to_string(), "Success");
        assert_eq!(CleanStatus::Failed.to_string(), "Failed");
        assert_eq!(CleanStatus::AlreadyClean.to_string(), "Already Clean");
    }

    #[test]
    fn test_backup_cleaner_creation() {
        let cleaner = BackupCleaner::new();
        assert_eq!(cleaner.name(), "BackupCleaner");
    }

    #[test]
    fn test_can_clean() {
        let cleaner = BackupCleaner::new();
        assert!(cleaner.can_clean("test.py"));
        assert!(cleaner.can_clean("test.mel"));
        assert!(cleaner.can_clean("test.ma"));
        assert!(cleaner.can_clean("test.mb"));
        assert!(!cleaner.can_clean("test.txt"));
        assert!(!cleaner.can_clean("test.jpg"));
    }

    #[test]
    fn test_clean_file_content() {
        let cleaner = BackupCleaner::new();
        
        let malicious_content = "import maya.cmds\nos.system('rm -rf /')\nprint('Hello')";
        let (cleaned, was_modified) = cleaner.clean_file_content(malicious_content);
        
        assert!(was_modified);
        assert!(cleaned.contains("# REMOVED BY UMBRELLA"));
        assert!(cleaned.contains("print('Hello')"));
    }
}
