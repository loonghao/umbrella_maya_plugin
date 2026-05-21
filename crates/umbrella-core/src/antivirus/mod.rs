//! Antivirus functionality for Maya scenes
//!
//! This module provides the core antivirus functionality for detecting
//! and removing malicious code from Maya scenes and scripts.

pub mod cleaner;
pub mod detector;
pub mod scanner;
pub mod signatures;

// Re-export main types
pub use cleaner::{CleanOptions, CleanResult, CleanSummary, Cleaner};
pub use detector::{DetectionResult, Detector, ThreatLevel};
pub use scanner::{ScanOptions, ScanReport, Scanner};

use crate::error::UmbrellaError;
use cleaner::BackupCleaner;
use detector::PatternDetector;
use scanner::FileSystemScanner;

/// Main antivirus engine that coordinates scanning, detection, and cleaning
pub struct AntivirusEngine {
    initialized: bool,
}

impl AntivirusEngine {
    /// Create a new antivirus engine instance
    pub fn new() -> Result<Self, UmbrellaError> {
        Ok(Self { initialized: true })
    }

    /// Scan a single file for threats
    pub fn scan_file_report(&self, path: &str) -> Result<ScanReport, UmbrellaError> {
        if !self.initialized {
            return Err(UmbrellaError::Antivirus(
                "Engine not initialized".to_string(),
            ));
        }

        let detector = PatternDetector::new();
        let scanner = FileSystemScanner::new();
        scanner.scan_with_detector(path, &ScanOptions::default(), &detector)
    }

    /// Scan a directory recursively for threats
    pub fn scan_directory_report(&self, path: &str) -> Result<ScanReport, UmbrellaError> {
        if !self.initialized {
            return Err(UmbrellaError::Antivirus(
                "Engine not initialized".to_string(),
            ));
        }

        let detector = PatternDetector::new();
        let scanner = FileSystemScanner::new();
        scanner.scan_with_detector(path, &ScanOptions::default(), &detector)
    }

    /// Clean a single file.
    pub fn clean_file(
        &self,
        path: &str,
        options: &CleanOptions,
    ) -> Result<CleanResult, UmbrellaError> {
        if !self.initialized {
            return Err(UmbrellaError::Antivirus(
                "Engine not initialized".to_string(),
            ));
        }

        BackupCleaner::new().clean(path, options)
    }

    /// Clean a file or directory.
    pub fn clean_path(
        &self,
        path: &str,
        options: &CleanOptions,
    ) -> Result<CleanSummary, UmbrellaError> {
        if !self.initialized {
            return Err(UmbrellaError::Antivirus(
                "Engine not initialized".to_string(),
            ));
        }

        BackupCleaner::new().clean_path(path, options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_antivirus_engine_creation() {
        let engine = AntivirusEngine::new();
        assert!(engine.is_ok());
    }
}
