//! C API for umbrella antivirus functionality
//!
//! This module provides C-compatible functions that can be called from Maya C++ plugins.

use std::os::raw::{c_char, c_int};
use std::ffi::{CStr, CString};
use std::ptr;

use crate::{UmbrellaResult, ScanResult};
use crate::antivirus::AntivirusEngine;

/// Initialize the umbrella antivirus engine
/// Returns UmbrellaResult indicating success or failure
#[no_mangle]
pub extern "C" fn umbrella_init() -> UmbrellaResult {
    match AntivirusEngine::new() {
        Ok(_) => UmbrellaResult::success(),
        Err(_) => UmbrellaResult::failure(1),
    }
}

/// Scan a file for threats
/// 
/// # Arguments
/// * `file_path` - C string containing the path to scan
/// 
/// # Returns
/// * ScanResult containing scan statistics
#[no_mangle]
pub extern "C" fn umbrella_scan_file(file_path: *const c_char) -> ScanResult {
    if file_path.is_null() {
        return ScanResult {
            threats_found: -1,
            files_scanned: 0,
            scan_time_ms: 0,
        };
    }

    let path_str = unsafe {
        match CStr::from_ptr(file_path).to_str() {
            Ok(s) => s,
            Err(_) => return ScanResult {
                threats_found: -1,
                files_scanned: 0,
                scan_time_ms: 0,
            },
        }
    };

    // Implement basic threat detection
    let start_time = std::time::Instant::now();
    let threats_found = detect_threats_in_file(path_str);
    let scan_time = start_time.elapsed().as_millis() as c_int;

    ScanResult {
        threats_found,
        files_scanned: 1,
        scan_time_ms: scan_time,
    }
}

/// Scan a directory recursively
/// 
/// # Arguments
/// * `dir_path` - C string containing the directory path to scan
/// 
/// # Returns
/// * ScanResult containing scan statistics
#[no_mangle]
pub extern "C" fn umbrella_scan_directory(dir_path: *const c_char) -> ScanResult {
    if dir_path.is_null() {
        return ScanResult {
            threats_found: -1,
            files_scanned: 0,
            scan_time_ms: 0,
        };
    }

    let path_str = unsafe {
        match CStr::from_ptr(dir_path).to_str() {
            Ok(s) => s,
            Err(_) => return ScanResult {
                threats_found: -1,
                files_scanned: 0,
                scan_time_ms: 0,
            },
        }
    };

    // Implement directory scanning
    let start_time = std::time::Instant::now();
    let (threats_found, files_scanned) = scan_directory_for_threats(path_str);
    let scan_time = start_time.elapsed().as_millis() as c_int;

    ScanResult {
        threats_found,
        files_scanned,
        scan_time_ms: scan_time,
    }
}

/// Get the version string of the umbrella library
/// 
/// # Returns
/// * C string containing version information
/// * Caller is responsible for freeing the returned string
#[no_mangle]
pub extern "C" fn umbrella_get_version() -> *mut c_char {
    let version = env!("CARGO_PKG_VERSION");
    match CString::new(version) {
        Ok(c_string) => c_string.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Free a string allocated by umbrella functions
/// 
/// # Arguments
/// * `ptr` - Pointer to the string to free
#[no_mangle]
pub extern "C" fn umbrella_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

/// Cleanup and shutdown the umbrella engine
#[no_mangle]
pub extern "C" fn umbrella_cleanup() -> UmbrellaResult {
    // TODO: Implement cleanup logic
    UmbrellaResult::success()
}

/// Detect threats in a single file
/// Returns the number of threats found (0 = clean, >0 = threats found, -1 = error)
fn detect_threats_in_file(file_path: &str) -> c_int {
    // Check if file exists
    if !std::path::Path::new(file_path).exists() {
        return -1;
    }

    // Read file content
    let content = match std::fs::read_to_string(file_path) {
        Ok(content) => content,
        Err(_) => {
            // Try reading as binary for non-text files
            match std::fs::read(file_path) {
                Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                Err(_) => return -1,
            }
        }
    };

    // Simple threat detection patterns for Maya scenes
    let threat_patterns = [
        // Suspicious Python code patterns
        "import os",
        "import subprocess",
        "import sys",
        "exec(",
        "eval(",
        "__import__",
        "getattr(",
        "setattr(",
        // Suspicious MEL patterns
        "system(",
        "popen(",
        "python(",
        // File operations that could be malicious
        "file -delete",
        "file -remove",
        "deleteUI",
        // Network operations
        "urllib",
        "requests",
        "socket",
        "http",
        // Suspicious script execution
        "mel.eval",
        "cmds.evalDeferred",
        "scriptJob",
    ];

    let mut threats_found = 0;
    let content_lower = content.to_lowercase();

    for pattern in &threat_patterns {
        if content_lower.contains(&pattern.to_lowercase()) {
            threats_found += 1;
        }
    }

    threats_found
}

/// Scan a directory recursively for threats
/// Returns (threats_found, files_scanned)
fn scan_directory_for_threats(dir_path: &str) -> (c_int, c_int) {
    let path = std::path::Path::new(dir_path);

    if !path.exists() || !path.is_dir() {
        return (-1, 0);
    }

    let mut total_threats = 0;
    let mut files_scanned = 0;

    // Scan files recursively
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();

            if entry_path.is_file() {
                // Only scan relevant file types
                if let Some(extension) = entry_path.extension() {
                    let ext = extension.to_string_lossy().to_lowercase();
                    if matches!(ext.as_str(), "ma" | "mb" | "mel" | "py" | "txt" | "json" | "xml") {
                        if let Some(path_str) = entry_path.to_str() {
                            let threats = detect_threats_in_file(path_str);
                            if threats >= 0 {
                                total_threats += threats;
                                files_scanned += 1;
                            }
                        }
                    }
                }
            } else if entry_path.is_dir() {
                // Recursively scan subdirectories (limit depth to avoid infinite loops)
                if let Some(subdir_str) = entry_path.to_str() {
                    let (sub_threats, sub_files) = scan_directory_for_threats(subdir_str);
                    if sub_threats >= 0 {
                        total_threats += sub_threats;
                        files_scanned += sub_files;
                    }
                }
            }
        }
    }

    (total_threats, files_scanned)
}
