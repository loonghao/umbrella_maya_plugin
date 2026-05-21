//! File system scanning for Maya virus detection.

use crate::antivirus::detector::{DetectionResult, Detector, PatternDetector};
use crate::error::{Result, UmbrellaError};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Instant;
use walkdir::WalkDir;

/// Options for configuring file scanning.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub recursive: bool,
    pub include_extensions: Vec<String>,
    pub exclude_extensions: Vec<String>,
    pub max_file_size: Option<u64>,
    pub follow_symlinks: bool,
    pub ignore_directories: Vec<String>,
}

impl ScanOptions {
    /// Matches the original portable scanner: recursively inspect Maya scene files.
    pub fn maya_scene_files() -> Self {
        ScanOptions {
            include_extensions: vec!["ma".to_string(), "mb".to_string()],
            ..Self::default()
        }
    }
}

impl Default for ScanOptions {
    fn default() -> Self {
        let backup_folder_name =
            std::env::var("MAYA_UMBRELLA_BACKUP_FOLDER_NAME").unwrap_or_else(|_| "_virus".into());

        ScanOptions {
            recursive: true,
            include_extensions: vec![
                "ma".to_string(),
                "mb".to_string(),
                "mel".to_string(),
                "py".to_string(),
            ],
            exclude_extensions: Vec::new(),
            max_file_size: Some(100 * 1024 * 1024),
            follow_symlinks: false,
            ignore_directories: vec![backup_folder_name, "_virus_backup".to_string()],
        }
    }
}

/// Result of a file discovery operation.
#[derive(Debug, Clone, Serialize)]
pub struct ScanResult {
    pub files: Vec<String>,
    pub directories_scanned: usize,
    pub total_size: u64,
    pub duration_ms: u64,
}

/// High-level scan report with detections.
#[derive(Debug, Clone, Serialize)]
pub struct ScanReport {
    pub root: String,
    pub files_scanned: usize,
    pub threats_found: usize,
    pub infected_files: Vec<DetectionResult>,
    pub duration_ms: u64,
}

impl ScanReport {
    pub fn is_clean(&self) -> bool {
        self.threats_found == 0
    }
}

pub trait Scanner {
    fn scan(&self, path: &str, options: &ScanOptions) -> Result<ScanResult>;

    fn should_include_file(&self, file_path: &Path, options: &ScanOptions) -> bool {
        if let Some(extension) = file_path.extension() {
            let ext_str = extension.to_string_lossy().to_lowercase();

            if !options.include_extensions.is_empty()
                && !options
                    .include_extensions
                    .iter()
                    .any(|extension| extension.eq_ignore_ascii_case(&ext_str))
            {
                return false;
            }

            if options
                .exclude_extensions
                .iter()
                .any(|extension| extension.eq_ignore_ascii_case(&ext_str))
            {
                return false;
            }
        } else if !options.include_extensions.is_empty() {
            return false;
        }

        if let Some(max_size) = options.max_file_size
            && let Ok(metadata) = file_path.metadata()
            && metadata.len() > max_size
        {
            return false;
        }

        true
    }
}

/// File system scanner implementation.
pub struct FileSystemScanner {
    name: String,
}

impl FileSystemScanner {
    pub fn new() -> Self {
        FileSystemScanner {
            name: "FileSystemScanner".to_string(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn scan_with_detector(
        &self,
        path: &str,
        options: &ScanOptions,
        detector: &PatternDetector,
    ) -> Result<ScanReport> {
        let start = Instant::now();
        let discovered = self.scan(path, options)?;
        let mut infected_files = Vec::new();
        let mut threats_found = 0usize;

        for file in &discovered.files {
            let detection = detector.detect(file)?;
            if detection.is_infected() {
                threats_found += detection.matches.len();
                infected_files.push(detection);
            }
        }

        Ok(ScanReport {
            root: path.to_string(),
            files_scanned: discovered.files.len(),
            threats_found,
            infected_files,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}

impl Default for FileSystemScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner for FileSystemScanner {
    fn scan(&self, path: &str, options: &ScanOptions) -> Result<ScanResult> {
        let start_time = Instant::now();
        let scan_path = Path::new(path);

        if !scan_path.exists() {
            return Err(UmbrellaError::Antivirus(format!(
                "Path does not exist: {}",
                path
            )));
        }

        if scan_path.is_file() {
            let mut total_size = 0;
            let files = if self.should_include_file(scan_path, options) {
                if let Ok(metadata) = scan_path.metadata() {
                    total_size = metadata.len();
                }
                vec![scan_path.to_string_lossy().to_string()]
            } else {
                Vec::new()
            };

            return Ok(ScanResult {
                files,
                directories_scanned: 0,
                total_size,
                duration_ms: start_time.elapsed().as_millis() as u64,
            });
        }

        if !scan_path.is_dir() {
            return Err(UmbrellaError::Antivirus(format!(
                "Path is neither a file nor a directory: {}",
                path
            )));
        }

        let max_depth = if options.recursive { usize::MAX } else { 1 };
        let mut files = Vec::new();
        let mut directories_scanned = 0usize;
        let mut total_size = 0u64;

        let walker = WalkDir::new(scan_path)
            .follow_links(options.follow_symlinks)
            .max_depth(max_depth)
            .into_iter()
            .filter_entry(|entry| !should_skip_entry(entry.path(), options));

        for entry in walker {
            let entry = entry.map_err(|err| {
                UmbrellaError::Antivirus(format!("Failed to scan directory: {}", err))
            })?;
            let entry_path: PathBuf = entry.path().to_path_buf();

            if entry_path.is_dir() {
                directories_scanned += 1;
                continue;
            }

            if entry_path.is_file() && self.should_include_file(&entry_path, options) {
                if let Ok(metadata) = entry_path.metadata() {
                    total_size += metadata.len();
                }
                files.push(entry_path.to_string_lossy().to_string());
            }
        }

        files.sort();

        Ok(ScanResult {
            files,
            directories_scanned,
            total_size,
            duration_ms: start_time.elapsed().as_millis() as u64,
        })
    }
}

fn should_skip_entry(path: &Path, options: &ScanOptions) -> bool {
    if !path.is_dir() {
        return false;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    options
        .ignore_directories
        .iter()
        .any(|ignored| ignored.eq_ignore_ascii_case(name))
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
        assert!(options.ignore_directories.contains(&"_virus".to_string()));
    }

    #[test]
    fn test_maya_scene_files_options() {
        let options = ScanOptions::maya_scene_files();
        assert_eq!(
            options.include_extensions,
            vec!["ma".to_string(), "mb".to_string()]
        );
    }

    #[test]
    fn test_should_include_file() {
        let scanner = FileSystemScanner::new();
        let options = ScanOptions::default();

        assert!(scanner.should_include_file(Path::new("test.ma"), &options));
        assert!(scanner.should_include_file(Path::new("test.mb"), &options));
        assert!(scanner.should_include_file(Path::new("test.mel"), &options));
        assert!(scanner.should_include_file(Path::new("test.py"), &options));
        assert!(!scanner.should_include_file(Path::new("test.txt"), &options));
        assert!(!scanner.should_include_file(Path::new("test.jpg"), &options));
    }

    #[test]
    fn test_file_system_scanner_creation() {
        let scanner = FileSystemScanner::new();
        assert_eq!(scanner.name(), "FileSystemScanner");
    }
}
