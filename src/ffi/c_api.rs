//! C API for umbrella antivirus functionality.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::time::Instant;

use crate::antivirus::AntivirusEngine;
use crate::antivirus::cleaner::CleanStatus;
use crate::antivirus::detector::PatternDetector;
use crate::{CleanFFIResult, ScanResult, UmbrellaResult};

#[unsafe(no_mangle)]
pub extern "C" fn umbrella_init() -> UmbrellaResult {
    match AntivirusEngine::new() {
        Ok(_) => UmbrellaResult::success(),
        Err(_) => UmbrellaResult::failure(1),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn umbrella_scan_file(file_path: *const c_char) -> ScanResult {
    let Some(path) = c_str_to_string(file_path) else {
        return scan_error();
    };

    let start = Instant::now();
    match AntivirusEngine::new().and_then(|engine| engine.scan_file_report(&path)) {
        Ok(report) => ScanResult {
            threats_found: report.threats_found as c_int,
            files_scanned: report.files_scanned as c_int,
            scan_time_ms: start.elapsed().as_millis() as c_int,
        },
        Err(_) => scan_error(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn umbrella_scan_directory(dir_path: *const c_char) -> ScanResult {
    let Some(path) = c_str_to_string(dir_path) else {
        return scan_error();
    };

    let start = Instant::now();
    match AntivirusEngine::new().and_then(|engine| engine.scan_directory_report(&path)) {
        Ok(report) => ScanResult {
            threats_found: report.threats_found as c_int,
            files_scanned: report.files_scanned as c_int,
            scan_time_ms: start.elapsed().as_millis() as c_int,
        },
        Err(_) => scan_error(),
    }
}

/// Scan an in-memory string. Used by the C++ Maya plugin to inspect scriptNode
/// attributes and scriptJob bodies without routing through Python.
#[unsafe(no_mangle)]
pub extern "C" fn umbrella_scan_content(content: *const c_char) -> c_int {
    let Some(content) = c_str_to_string(content) else {
        return -1;
    };

    let detector = PatternDetector::new();
    detector
        .detect_bytes("<maya-content>", content.as_bytes())
        .matches
        .len() as c_int
}

#[unsafe(no_mangle)]
pub extern "C" fn umbrella_clean_file(file_path: *const c_char) -> CleanFFIResult {
    let Some(path) = c_str_to_string(file_path) else {
        return clean_error();
    };

    let start = Instant::now();
    match AntivirusEngine::new().and_then(|engine| engine.clean_file(&path, &Default::default())) {
        Ok(result) => {
            let files_cleaned = matches!(result.status, CleanStatus::Success) as c_int;
            let files_deleted = matches!(result.status, CleanStatus::Deleted) as c_int;
            CleanFFIResult {
                files_cleaned,
                files_deleted,
                files_failed: 0,
                threats_removed: result.threats_removed as c_int,
                scan_time_ms: start.elapsed().as_millis() as c_int,
            }
        }
        Err(_) => clean_error(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn umbrella_clean_directory(dir_path: *const c_char) -> CleanFFIResult {
    let Some(path) = c_str_to_string(dir_path) else {
        return clean_error();
    };

    let start = Instant::now();
    match AntivirusEngine::new().and_then(|engine| engine.clean_path(&path, &Default::default())) {
        Ok(summary) => CleanFFIResult {
            files_cleaned: summary.files_cleaned as c_int,
            files_deleted: summary.files_deleted as c_int,
            files_failed: summary.files_failed as c_int,
            threats_removed: summary.threats_removed as c_int,
            scan_time_ms: start.elapsed().as_millis() as c_int,
        },
        Err(_) => clean_error(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn umbrella_get_version() -> *mut c_char {
    match CString::new(env!("CARGO_PKG_VERSION")) {
        Ok(c_string) => c_string.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn umbrella_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn umbrella_cleanup() -> UmbrellaResult {
    UmbrellaResult::success()
}

fn c_str_to_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }

    unsafe { CStr::from_ptr(ptr).to_str().ok().map(ToOwned::to_owned) }
}

fn scan_error() -> ScanResult {
    ScanResult {
        threats_found: -1,
        files_scanned: 0,
        scan_time_ms: 0,
    }
}

fn clean_error() -> CleanFFIResult {
    CleanFFIResult {
        files_cleaned: 0,
        files_deleted: 0,
        files_failed: 1,
        threats_removed: 0,
        scan_time_ms: 0,
    }
}
