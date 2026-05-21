#![allow(clippy::useless_conversion)]

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use crate::antivirus::cleaner::{BackupCleaner, CleanOptions, Cleaner};
use crate::antivirus::detector::PatternDetector;
use crate::antivirus::scanner::{FileSystemScanner, ScanOptions};

#[pyfunction]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[pyfunction]
fn scan_file(path: &str) -> PyResult<String> {
    scan_path_inner(path, false)
}

#[pyfunction]
#[pyo3(signature = (path, scene_only=None))]
fn scan_path(path: &str, scene_only: Option<bool>) -> PyResult<String> {
    scan_path_inner(path, scene_only.unwrap_or(false))
}

#[pyfunction]
#[pyo3(signature = (path, backup_root=None, aggressive=None))]
fn clean_file(
    path: &str,
    backup_root: Option<String>,
    aggressive: Option<bool>,
) -> PyResult<String> {
    let cleaner = BackupCleaner::new();
    let options = CleanOptions {
        backup_directory: backup_root,
        aggressive: aggressive.unwrap_or(false),
        ..Default::default()
    };

    let result = cleaner
        .clean(path, &options)
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
    serde_json::to_string(&result).map_err(|err| PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
#[pyo3(signature = (path, backup_root=None, aggressive=None))]
fn clean_path(
    path: &str,
    backup_root: Option<String>,
    aggressive: Option<bool>,
) -> PyResult<String> {
    let cleaner = BackupCleaner::new();
    let options = CleanOptions {
        backup_directory: backup_root,
        aggressive: aggressive.unwrap_or(false),
        ..Default::default()
    };

    let summary = cleaner
        .clean_path(path, &options)
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
    serde_json::to_string(&summary).map_err(|err| PyRuntimeError::new_err(err.to_string()))
}

/// Compatibility facade for scripts that used maya_umbrella.MayaVirusScanner.
#[pyclass]
struct MayaVirusScanner {
    output_path: Option<String>,
}

#[pymethods]
impl MayaVirusScanner {
    #[new]
    #[pyo3(signature = (output_path=None))]
    fn new(output_path: Option<String>) -> Self {
        Self { output_path }
    }

    fn scan_files_from_pattern(&self, pattern: &str) -> PyResult<Vec<String>> {
        let mut fixed = Vec::new();
        for entry in glob::glob(pattern).map_err(|err| PyRuntimeError::new_err(err.to_string()))? {
            let path = entry.map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
            if self.fix_one(&path.to_string_lossy())? {
                fixed.push(path.to_string_lossy().to_string());
            }
        }
        Ok(fixed)
    }

    fn scan_files_from_list(&self, files: Vec<String>) -> PyResult<Vec<String>> {
        let mut fixed = Vec::new();
        for file in files {
            if self.fix_one(&file)? {
                fixed.push(file);
            }
        }
        Ok(fixed)
    }

    fn scan_files_from_file(&self, text_file: &str) -> PyResult<Vec<String>> {
        let content = std::fs::read_to_string(text_file)
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
        self.scan_files_from_list(
            content
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
        )
    }
}

impl MayaVirusScanner {
    fn fix_one(&self, path: &str) -> PyResult<bool> {
        let cleaner = BackupCleaner::new();
        let options = CleanOptions {
            backup_directory: self.output_path.clone(),
            ..Default::default()
        };
        let result = cleaner
            .clean(path, &options)
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
        Ok(matches!(
            result.status,
            crate::antivirus::cleaner::CleanStatus::Success
                | crate::antivirus::cleaner::CleanStatus::Deleted
        ))
    }
}

/// Compatibility facade for maya_umbrella.MayaVirusDefender in non-Maya batch scripts.
#[pyclass]
struct MayaVirusDefender {}

#[pymethods]
impl MayaVirusDefender {
    #[new]
    fn new() -> Self {
        Self {}
    }

    fn get_unfixed_references(&self) -> Vec<String> {
        Vec::new()
    }

    fn collect(&self) {}
    fn fix(&self) {}
    fn report(&self) {}
    fn setup(&self) {}
    fn stop(&self) {}
    fn start(&self) {}
}

fn scan_path_inner(path: &str, scene_only: bool) -> PyResult<String> {
    let scanner = FileSystemScanner::new();
    let detector = PatternDetector::new();
    let options = if scene_only {
        ScanOptions::maya_scene_files()
    } else {
        ScanOptions::default()
    };
    let report = scanner
        .scan_with_detector(path, &options, &detector)
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
    serde_json::to_string(&report).map_err(|err| PyRuntimeError::new_err(err.to_string()))
}

#[pymodule]
fn umbrella_maya(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(version, module)?)?;
    module.add_function(wrap_pyfunction!(scan_file, module)?)?;
    module.add_function(wrap_pyfunction!(scan_path, module)?)?;
    module.add_function(wrap_pyfunction!(clean_file, module)?)?;
    module.add_function(wrap_pyfunction!(clean_path, module)?)?;
    module.add_class::<MayaVirusScanner>()?;
    module.add_class::<MayaVirusDefender>()?;
    Ok(())
}
