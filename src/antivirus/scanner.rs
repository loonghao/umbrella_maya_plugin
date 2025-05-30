//! File scanning functionality
//! 
//! This module provides file system scanning capabilities for finding
//! Maya files and scripts that need to be analyzed for threats.

use crate::error::{Result, UmbrellaError};
use std::path::Path;

/// Options for configuring file scanning
#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Whether to scan subdirectories recursively
    pub recursive: bool,
    /// File extensions to include in the scan
    pub include_extensions: Vec<String>,
    /// File extensions to exclude from the scan
    pub exclude_extensions: Vec<String>,
    /// Maximum file size to scan (in bytes)
    pub max_file_size: Option<u64>,
    /// Whether to follow symbolic links
    pub follow_symlinks: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        ScanOptions {
            recursive: true,
            include_extensions: vec![
                "ma".to_string(),
                "mb".to_string(),
                "mel".to_string(),
                "py".to_string(),
            ],
            exclude_extensions: vec![],
            max_file_size: Some(100 * 1024 * 1024), // 100MB
            follow_symlinks: false,
        }
    }
}

/// Result of a file scan operation
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// List of files found during the scan
    pub files: Vec<String>,
    /// Number of directories scanned
    pub directories_scanned: usize,
    /// Total size of files found (in bytes)
    pub total_size: u64,
    /// Scan duration in milliseconds
    pub duration_ms: u64,
}

/// Trait for implementing file scanners
pub trait Scanner {
    /// Scan the specified path for files matching the given options
    fn scan(&self, path: &str, options: &ScanOptions) -> Result<ScanResult>;
    
    /// Check if a file should be included based on the scan options
    fn should_include_file(&self, file_path: &Path, options: &ScanOptions) -> bool {
        // Check file extension
        if let Some(extension) = file_path.extension() {
            let ext_str = extension.to_string_lossy().to_lowercase();
            
            // If include_extensions is specified, file must be in the list
            if !options.include_extensions.is_empty() {
                if !options.include_extensions.iter().any(|e| e.to_lowercase() == ext_str) {
                    return false;
                }
            }
            
            // If exclude_extensions is specified, file must not be in the list
            if options.exclude_extensions.iter().any(|e| e.to_lowercase() == ext_str) {
                return false;
            }
        }
        
        // Check file size if specified
        if let Some(max_size) = options.max_file_size {
            if let Ok(metadata) = file_path.metadata() {
                if metadata.len() > max_size {
                    return false;
                }
            }
        }
        
        true
    }
}

/// File system scanner implementation
pub struct FileSystemScanner {
    name: String,
}

impl FileSystemScanner {
    /// Create a new file system scanner
    pub fn new() -> Self {
        FileSystemScanner {
            name: "FileSystemScanner".to_string(),
        }
    }
    
    /// Get the scanner name
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Default for FileSystemScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner for FileSystemScanner {
    fn scan(&self, path: &str, options: &ScanOptions) -> Result<ScanResult> {
        let start_time = std::time::Instant::now();
        let scan_path = Path::new(path);
        
        if !scan_path.exists() {
            return Err(UmbrellaError::Antivirus(format!("Path does not exist: {}", path)));
        }
        
        let mut files = Vec::new();
        let mut directories_scanned = 0;
        let mut total_size = 0;
        
        if scan_path.is_file() {
            // Single file scan
            if self.should_include_file(scan_path, options) {
                files.push(path.to_string());
                if let Ok(metadata) = scan_path.metadata() {
                    total_size += metadata.len();
                }
            }
        } else if scan_path.is_dir() {
            // Directory scan
            self.scan_directory(scan_path, options, &mut files, &mut directories_scanned, &mut total_size)?;
        }
        
        let duration = start_time.elapsed();
        
        Ok(ScanResult {
            files,
            directories_scanned,
            total_size,
            duration_ms: duration.as_millis() as u64,
        })
    }
}

impl FileSystemScanner {
    fn scan_directory(
        &self,
        dir_path: &Path,
        options: &ScanOptions,
        files: &mut Vec<String>,
        directories_scanned: &mut usize,
        total_size: &mut u64,
    ) -> Result<()> {
        *directories_scanned += 1;
        
        let entries = std::fs::read_dir(dir_path)
            .map_err(|e| UmbrellaError::Antivirus(format!("Failed to read directory {}: {}", dir_path.display(), e)))?;
        
        for entry in entries {
            let entry = entry
                .map_err(|e| UmbrellaError::Antivirus(format!("Failed to read directory entry: {}", e)))?;
            
            let entry_path = entry.path();
            
            if entry_path.is_file() {
                if self.should_include_file(&entry_path, options) {
                    files.push(entry_path.to_string_lossy().to_string());
                    if let Ok(metadata) = entry_path.metadata() {
                        *total_size += metadata.len();
                    }
                }
            } else if entry_path.is_dir() && options.recursive {
                // Handle symbolic links
                if entry_path.is_symlink() && !options.follow_symlinks {
                    continue;
                }
                
                self.scan_directory(&entry_path, options, files, directories_scanned, total_size)?;
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_scan_options_default() {
        let options = ScanOptions::default();
        assert!(options.recursive);
        assert!(options.include_extensions.contains(&"ma".to_string()));
        assert!(options.include_extensions.contains(&"mb".to_string()));
        assert!(options.include_extensions.contains(&"mel".to_string()));
        assert!(options.include_extensions.contains(&"py".to_string()));
    }

    #[test]
    fn test_should_include_file() {
        let scanner = FileSystemScanner::new();
        let options = ScanOptions::default();
        
        // Test Maya files
        assert!(scanner.should_include_file(Path::new("test.ma"), &options));
        assert!(scanner.should_include_file(Path::new("test.mb"), &options));
        assert!(scanner.should_include_file(Path::new("test.mel"), &options));
        assert!(scanner.should_include_file(Path::new("test.py"), &options));
        
        // Test excluded files
        assert!(!scanner.should_include_file(Path::new("test.txt"), &options));
        assert!(!scanner.should_include_file(Path::new("test.jpg"), &options));
    }

    #[test]
    fn test_file_system_scanner_creation() {
        let scanner = FileSystemScanner::new();
        assert_eq!(scanner.name(), "FileSystemScanner");
    }
}
