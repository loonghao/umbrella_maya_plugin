//! Offline cleaning for infected Maya files and scripts.

use crate::antivirus::detector::{Detector, PatternDetector};
use crate::antivirus::scanner::{FileSystemScanner, ScanOptions, Scanner};
use crate::antivirus::signatures::{
    VirusSignature, aggressive_clean_signatures, default_clean_signatures,
};
use crate::error::{Result, UmbrellaError};
use regex::bytes::Regex;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Options for configuring threat cleaning.
#[derive(Debug, Clone)]
pub struct CleanOptions {
    pub create_backup: bool,
    pub backup_directory: Option<String>,
    pub remove_original: bool,
    pub in_place: bool,
    /// Remove job-script signatures too. Default false mirrors upstream
    /// `MayaVirusCleaner.fix_infected_files`, which only strips file signatures.
    pub aggressive: bool,
}

impl Default for CleanOptions {
    fn default() -> Self {
        let ignore_backup =
            std::env::var("MAYA_UMBRELLA_IGNORE_BACKUP").unwrap_or_else(|_| "false".into());

        CleanOptions {
            create_backup: !ignore_backup.eq_ignore_ascii_case("true"),
            backup_directory: None,
            remove_original: true,
            in_place: true,
            aggressive: false,
        }
    }
}

/// Status of a cleaning operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CleanStatus {
    Success,
    AlreadyClean,
    Failed,
    Quarantined,
    BackupCreated,
    Deleted,
}

impl std::fmt::Display for CleanStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CleanStatus::Success => write!(f, "Success"),
            CleanStatus::AlreadyClean => write!(f, "Already Clean"),
            CleanStatus::Failed => write!(f, "Failed"),
            CleanStatus::Quarantined => write!(f, "Quarantined"),
            CleanStatus::BackupCreated => write!(f, "Backup Created"),
            CleanStatus::Deleted => write!(f, "Deleted"),
        }
    }
}

/// Result of a cleaning operation.
#[derive(Debug, Clone, Serialize)]
pub struct CleanResult {
    pub file_path: String,
    pub status: CleanStatus,
    pub message: String,
    pub backup_path: Option<String>,
    pub threats_removed: usize,
}

impl CleanResult {
    pub fn success(
        file_path: &str,
        message: &str,
        backup_path: Option<String>,
        threats_removed: usize,
    ) -> Self {
        CleanResult {
            file_path: file_path.to_string(),
            status: CleanStatus::Success,
            message: message.to_string(),
            backup_path,
            threats_removed,
        }
    }

    pub fn failed(file_path: &str, message: &str) -> Self {
        CleanResult {
            file_path: file_path.to_string(),
            status: CleanStatus::Failed,
            message: message.to_string(),
            backup_path: None,
            threats_removed: 0,
        }
    }

    pub fn already_clean(file_path: &str) -> Self {
        CleanResult {
            file_path: file_path.to_string(),
            status: CleanStatus::AlreadyClean,
            message: "File is already clean".to_string(),
            backup_path: None,
            threats_removed: 0,
        }
    }

    pub fn deleted(file_path: &str, backup_path: Option<String>, threats_removed: usize) -> Self {
        CleanResult {
            file_path: file_path.to_string(),
            status: CleanStatus::Deleted,
            message: "File became empty after removing signatures and was deleted".to_string(),
            backup_path,
            threats_removed,
        }
    }
}

/// Summary for directory/batch cleaning.
#[derive(Debug, Clone, Serialize)]
pub struct CleanSummary {
    pub root: String,
    pub files_scanned: usize,
    pub files_cleaned: usize,
    pub files_deleted: usize,
    pub files_failed: usize,
    pub threats_removed: usize,
    pub duration_ms: u64,
    pub results: Vec<CleanResult>,
}

pub trait Cleaner {
    fn clean(&self, file_path: &str, options: &CleanOptions) -> Result<CleanResult>;
    fn name(&self) -> &str;

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

/// Backup-based cleaner that mirrors upstream file signature removal.
pub struct BackupCleaner {
    name: String,
}

impl BackupCleaner {
    pub fn new() -> Self {
        BackupCleaner {
            name: "BackupCleaner".to_string(),
        }
    }

    pub fn clean_path(&self, path: &str, options: &CleanOptions) -> Result<CleanSummary> {
        let start = Instant::now();
        let scanner = FileSystemScanner::new();
        let scan_options = ScanOptions::default();
        let files = scanner.scan(path, &scan_options)?.files;

        let mut results = Vec::new();
        let mut files_cleaned = 0usize;
        let mut files_deleted = 0usize;
        let mut files_failed = 0usize;
        let mut threats_removed = 0usize;

        for file in &files {
            let result = match self.clean(file, options) {
                Ok(result) => result,
                Err(err) => CleanResult::failed(file, &err.to_string()),
            };

            match result.status {
                CleanStatus::Success => files_cleaned += 1,
                CleanStatus::Deleted => files_deleted += 1,
                CleanStatus::Failed => files_failed += 1,
                _ => {}
            }
            threats_removed += result.threats_removed;
            results.push(result);
        }

        Ok(CleanSummary {
            root: path.to_string(),
            files_scanned: files.len(),
            files_cleaned,
            files_deleted,
            files_failed,
            threats_removed,
            duration_ms: start.elapsed().as_millis() as u64,
            results,
        })
    }

    fn create_backup(&self, file_path: &str, options: &CleanOptions) -> Result<Option<String>> {
        if !options.create_backup {
            return Ok(None);
        }

        let source_path = Path::new(file_path);
        if !source_path.exists() {
            return Err(UmbrellaError::Antivirus(format!(
                "Source file does not exist: {}",
                file_path
            )));
        }

        let backup_path = backup_path(source_path, options.backup_directory.as_deref())?;
        if backup_path == source_path {
            return Ok(None);
        }
        if let Some(parent) = backup_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source_path, &backup_path)?;
        Ok(Some(backup_path.to_string_lossy().to_string()))
    }

    fn clean_file_content(
        &self,
        file_path: &str,
        content: &[u8],
        options: &CleanOptions,
    ) -> Result<(Vec<u8>, usize)> {
        let signatures = if options.aggressive {
            aggressive_clean_signatures()
        } else {
            default_clean_signatures()
        };
        replace_content_by_signatures(content, &signatures).map_err(|err| {
            UmbrellaError::Antivirus(format!("Failed to clean {}: {}", file_path, err))
        })
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
            return Ok(CleanResult::failed(
                file_path,
                "File type not supported for cleaning",
            ));
        }

        let path = Path::new(file_path);
        if !path.exists() {
            return Ok(CleanResult::failed(file_path, "File does not exist"));
        }

        let content = fs::read(path)?;
        let detector = PatternDetector::new();
        let detection = detector.detect(file_path)?;
        if !detection.is_infected() {
            return Ok(CleanResult::already_clean(file_path));
        }

        let (cleaned_content, threats_removed) =
            self.clean_file_content(file_path, &content, options)?;
        if threats_removed == 0 {
            return Ok(CleanResult::failed(
                file_path,
                "Threats were detected but no removable file signatures matched",
            ));
        }

        let backup = self.create_backup(file_path, options)?;
        let trimmed_empty = cleaned_content
            .iter()
            .all(|byte| byte.is_ascii_whitespace());

        if trimmed_empty && options.remove_original {
            fs::remove_file(path)?;
            return Ok(CleanResult::deleted(file_path, backup, threats_removed));
        }

        if options.in_place {
            fs::write(path, cleaned_content)?;
        } else {
            let cleaned_path = cleaned_output_path(path);
            if let Some(parent) = cleaned_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&cleaned_path, cleaned_content)?;
        }

        Ok(CleanResult::success(
            file_path,
            "File successfully cleaned",
            backup,
            threats_removed,
        ))
    }

    fn name(&self) -> &str {
        &self.name
    }
}

fn replace_content_by_signatures(
    content: &[u8],
    signatures: &[VirusSignature],
) -> std::result::Result<(Vec<u8>, usize), regex::Error> {
    let mut current = content.to_vec();
    let mut removals = 0usize;

    for signature in signatures {
        let regex = Regex::new(signature.pattern)?;
        let count = regex.find_iter(&current).count();
        if count > 0 {
            current = regex.replace_all(&current, &b""[..]).into_owned();
            removals += count;
        }
    }

    Ok((current, removals))
}

fn backup_path(source_path: &Path, backup_root: Option<&str>) -> Result<PathBuf> {
    if std::env::var("MAYA_UMBRELLA_IGNORE_BACKUP")
        .unwrap_or_else(|_| "false".to_string())
        .eq_ignore_ascii_case("true")
    {
        return Ok(source_path.to_path_buf());
    }

    let file_name = source_path
        .file_name()
        .ok_or_else(|| UmbrellaError::Antivirus("Invalid file name".to_string()))?;

    if let Some(root) = backup_root {
        let mut relative_parent = source_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        if relative_parent.has_root() {
            relative_parent = relative_parent
                .components()
                .filter_map(|component| match component {
                    std::path::Component::Normal(part) => Some(PathBuf::from(part)),
                    _ => None,
                })
                .collect();
        }
        return Ok(Path::new(root).join(relative_parent).join(file_name));
    }

    let backup_folder_name =
        std::env::var("MAYA_UMBRELLA_BACKUP_FOLDER_NAME").unwrap_or_else(|_| "_virus".into());
    Ok(source_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(backup_folder_name)
        .join(file_name))
}

fn cleaned_output_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "cleaned".to_string());
    path.with_file_name(format!("{}.cleaned", file_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_clean_options_default() {
        let options = CleanOptions::default();
        assert!(options.create_backup);
        assert!(options.backup_directory.is_none());
        assert!(options.remove_original);
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
        let malicious_content = b"import maya.cmds as cmds\nimport vaccine\nprint('Hello')";
        let (cleaned, removals) = cleaner
            .clean_file_content("userSetup.py", malicious_content, &CleanOptions::default())
            .unwrap();

        assert_eq!(removals, 1);
        assert!(!String::from_utf8_lossy(&cleaned).contains("import vaccine"));
        assert!(String::from_utf8_lossy(&cleaned).contains("print('Hello')"));
    }

    #[test]
    fn test_clean_file_creates_upstream_style_backup() {
        let temp = tempdir().unwrap();
        let file = temp.path().join("userSetup.py");
        fs::write(&file, b"import vaccine\nprint('Hello')").unwrap();

        let cleaner = BackupCleaner::new();
        let result = cleaner
            .clean(file.to_str().unwrap(), &CleanOptions::default())
            .unwrap();

        assert_eq!(result.status, CleanStatus::Success);
        assert!(result.backup_path.unwrap().contains("_virus"));
        assert!(
            !fs::read_to_string(&file)
                .unwrap()
                .contains("import vaccine")
        );
    }
}
